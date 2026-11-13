use std::{
    io::Error as IoError,
    net::IpAddr,
    num::{NonZeroU16, NonZeroU64},
    sync::Arc,
    time::{Duration, SystemTime},
};

use actix_web::{
    App, HttpServer,
    cookie::{Cookie, SameSite, time::Duration as CookieMaxAge, time::error::ConversionRange},
    web::{self, Data, JsonConfig, PathConfig, QueryConfig, ServiceConfig},
};
use derive_more::From;
use pp_core::{
    config::{DEFAULT_EXPORT_MAX_SIZE, DEFAULT_EXPORT_RETENTION, ExportConfig},
    url::Origin,
};
use pp_database::{
    Database,
    auth::AuthRepository,
    error::DatabaseError,
    export::{DeliveryZip, ExportRepository},
    gallery::GalleryId,
    jobs::JobOptions,
    photo::PhotoLocation,
};
use pp_storage::{ArchiveKey, PhotoStorage, StorageError};
use thiserror::Error;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::{
    comment_limiter::CommentLimiter,
    config::{
        ApiConfig, AuthConfig, CommentsConfig, CookieDuration, CookieName,
        DEFAULT_COMMENT_INTERVAL, DEFAULT_COMMENT_LIMIT, HttpConfig, LimiterConfig, QueueConfig,
        SessionConfig, UploadConfig,
    },
    credentials::{HashError, PasswordHash},
    error::ErrorResponse,
    events::EventHub,
    extractors::session_token::SessionToken,
    limiter::AttemptLimiter,
    middlewares::origin::SameOrigin,
    routes,
    upload::{
        jpeg::{self, InitializationError},
        limiter::UploadLimiter,
    },
};

/// Largest accepted JSON body: credentials and gallery changesets are a handful of short
/// fields, so this is well above any valid request.
pub(crate) const JSON_LIMIT: usize = 4096;

/// Failure to start or run the [`Api`].
///
/// Database causes are carried as sources because [`DatabaseError`] is sanitized: it never
/// contains the database URL, which holds credentials.
#[derive(Debug, Error, From)]
pub enum ApiError {
    #[error("database failed")]
    Database(#[source] DatabaseError),
    #[error("photo storage unavailable")]
    Storage(#[source] StorageError),
    #[error("photo upload initialization failed")]
    PhotoUpload(#[source] InitializationError),
    #[error("session idle timeout is out of range")]
    SessionLifetime(#[source] ConversionRange),
    #[error("password hashing unavailable")]
    Hashing(#[source] HashError),
    #[error("server failed")]
    Serve(#[source] IoError),
}

/// Process-wide state shared by every request handler.
///
/// Built once at startup from validated configuration, so request handlers never re-check
/// configuration and cannot fail for configuration reasons. It holds the authentication
/// policy, abuse controls, and bounded upload resources.
pub(crate) struct ApiState {
    database: Database,
    storage: PhotoStorage,
    upload_config: UploadConfig,
    origin: Origin,
    idle_timeout: CookieDuration,
    cookie_name: CookieName,
    secure_cookie: bool,
    // The same span as `idle_timeout` in the cookie crate's type, converted once at startup.
    cookie_max_age: CookieMaxAge,
    hash_slots: Arc<Semaphore>,
    job_options: JobOptions,
    export_max_size: NonZeroU64,
    export_retention: Duration,
    failed_attempts: AttemptLimiter,
    dummy_hash: PasswordHash,
    comment_limiter: CommentLimiter,
    comment_limit: NonZeroU16,
}

impl ApiState {
    pub fn new(
        database: Database,
        session: SessionConfig,
        auth: AuthConfig,
        limiter: LimiterConfig,
        storage: PhotoStorage,
        upload_config: UploadConfig,
        queue: QueueConfig,
    ) -> Result<Self, ApiError> {
        let job_options = JobOptions {
            max_attempts: queue.max_attempts,
            cleanup_batch: queue.cleanup_batch,
        };

        Ok(Self {
            database,
            storage,
            upload_config,
            origin: auth.app_origin,
            idle_timeout: session.idle_timeout,
            cookie_name: session.cookie_name,
            secure_cookie: session.secure_cookie.unwrap_or_default(),
            // `CookieDuration` bounds the timeout far below the cookie and timestamp limits,
            // so adding it to the current time cannot overflow for centuries.
            cookie_max_age: CookieMaxAge::try_from(session.idle_timeout.get())?,
            hash_slots: Arc::new(Semaphore::new(auth.hash_concurrency.get())),
            job_options,
            export_max_size: DEFAULT_EXPORT_MAX_SIZE,
            export_retention: DEFAULT_EXPORT_RETENTION,
            failed_attempts: AttemptLimiter::new(limiter),
            dummy_hash: PasswordHash::generate()?,
            comment_limiter: CommentLimiter::new(DEFAULT_COMMENT_INTERVAL),
            comment_limit: DEFAULT_COMMENT_LIMIT,
        })
    }

    pub fn database(&self) -> &Database {
        &self.database
    }

    #[must_use]
    pub fn with_comments(mut self, config: &CommentsConfig) -> Self {
        self.comment_limiter = CommentLimiter::new(config.min_interval.get());
        self.comment_limit = config.max_per_photo;

        self
    }

    pub fn pace_comments(&self, gallery: GalleryId) -> Result<(), ErrorResponse> {
        self.comment_limiter
            .check(gallery)
            .map_err(ErrorResponse::busy)
    }

    pub fn comment_limit(&self) -> NonZeroU16 {
        self.comment_limit
    }

    /// Applies the selection ZIP limits; without them the defaults of [`ExportConfig`] hold.
    #[must_use]
    pub fn with_export(mut self, config: &ExportConfig) -> Self {
        self.export_max_size = config.size_limit;
        self.export_retention = config.retention.get();

        self
    }

    /// Most bytes of originals one selection ZIP may hold.
    pub const fn export_max_size(&self) -> NonZeroU64 {
        self.export_max_size
    }

    /// How long a finished selection ZIP stays available.
    pub const fn export_retention(&self) -> Duration {
        self.export_retention
    }

    /// How the jobs the API enqueues are configured.
    pub fn job_options(&self) -> JobOptions {
        self.job_options
    }

    /// The state of the gallery's delivery ZIP, starting a build when none matches the finals.
    /// The files of replaced ZIPs are removed; one that cannot be is left to the sweep.
    pub async fn ensure_delivery(
        &self,
        location: PhotoLocation,
    ) -> Result<DeliveryZip, DatabaseError> {
        let delivery = self
            .database
            .repository_with::<ExportRepository>(self.job_options)
            .ensure_delivery(location.workspace_id, location.gallery_id)
            .await?;

        for export in delivery.replaced {
            let key = ArchiveKey {
                workspace: location.workspace_id,
                gallery: location.gallery_id,
                export,
            };

            match self.storage.remove_archive(&key).await {
                Ok(()) | Err(StorageError::NotFound) => {}
                Err(error) => tracing::warn!(%error, "replaced delivery ZIP could not be removed"),
            }
        }

        Ok(delivery.zip)
    }

    pub fn storage(&self) -> &PhotoStorage {
        &self.storage
    }

    pub fn upload_config(&self) -> &UploadConfig {
        &self.upload_config
    }

    /// True only for the exact configured origin; a missing header is not allowed.
    pub fn allows_origin(&self, header: Option<&str>) -> bool {
        header == Some(self.origin.as_str())
    }

    pub fn idle_timeout(&self) -> Duration {
        self.idle_timeout.get()
    }

    /// Name of the cookie carrying the session token.
    pub fn cookie_name(&self) -> &str {
        self.cookie_name.as_str()
    }

    /// Expiry of a session created or renewed at `now`; `None` only if it overflows time.
    pub fn session_expiry(&self, now: SystemTime) -> Option<SystemTime> {
        now.checked_add(self.idle_timeout.get())
    }

    /// The session cookie for `token`, living as long as the idle timeout.
    ///
    /// The `Secure` attribute follows the session configuration.
    pub fn session_cookie(&self, token: &SessionToken) -> Cookie<'static> {
        self.cookie(token.as_str())
    }

    /// A cookie that makes the browser drop the session cookie.
    pub fn removal_cookie(&self) -> Cookie<'static> {
        let mut cookie = self.cookie("");

        cookie.make_removal();

        cookie
    }

    /// Best-effort removal of a small batch of expired sessions. They already fail
    /// authentication, so a failure here is logged and must never fail the caller's request.
    pub async fn delete_expired_sessions(&self, now: SystemTime) {
        if let Err(error) = self
            .database()
            .repository::<AuthRepository>()
            .cleanup_expired(now, 100)
            .await
        {
            tracing::warn!(%error, "expired session cleanup failed");
        }
    }

    /// Hash verified for unknown emails so they cost as much as known ones.
    pub fn dummy_hash(&self) -> &PasswordHash {
        &self.dummy_hash
    }

    /// Reserves one of the bounded password-hashing slots until the permit is dropped.
    pub fn try_acquire_hash_slot(&self) -> Option<OwnedSemaphorePermit> {
        Arc::clone(&self.hash_slots).try_acquire_owned().ok()
    }

    /// False once the peer has failed too many times recently; see [`AttemptLimiter`].
    pub fn allow_attempt(&self, peer: IpAddr) -> bool {
        self.failed_attempts.allows(peer)
    }

    /// Only failed credential attempts count, so successful sign-ins never lock anyone out.
    pub fn record_failure(&self, peer: IpAddr) {
        self.failed_attempts.record(peer);
    }

    fn cookie(&self, value: &str) -> Cookie<'static> {
        Cookie::build(self.cookie_name.as_str().to_owned(), value.to_owned())
            .path("/")
            .http_only(true)
            .same_site(SameSite::Lax)
            .secure(self.secure_cookie)
            .max_age(self.cookie_max_age)
            .finish()
    }
}

/// The API process: [`init`](Self::init) connects and prepares everything the server needs
/// (database, migrations, authentication state), so [`run`](Self::run) only serves requests.
/// A database or configuration problem therefore stops startup instead of opening setup on an
/// unknown state.
pub struct Api {
    http: HttpConfig,
    state: Data<ApiState>,
    upload_limiter: Data<UploadLimiter>,
    events: Data<EventHub>,
}

impl Api {
    pub async fn init(config: ApiConfig) -> Result<Self, ApiError> {
        let storage = PhotoStorage::open(&config.photo.dir).await?;

        jpeg::install()?;

        // Ten connections for requests, plus the one the photo event listener holds.
        let database = Database::connect_lazy(&config.database.url, 11)?;

        database.ensure_reachable().await?;
        database.migrate().await?;

        let upload_limiter = Data::new(UploadLimiter::from(&config.photo_upload));
        let events = Data::new(EventHub::new(&config.events));
        let state = ApiState::new(
            database,
            config.session,
            config.auth,
            config.limiter,
            storage,
            config.photo_upload,
            config.queue,
        )?
        .with_export(&config.export)
        .with_comments(&config.comments);

        Ok(Self {
            http: config.http,
            state: Data::new(state),
            upload_limiter,
            events,
        })
    }

    /// Serves until `shutdown` is cancelled, then closes the event streams and lets in-flight
    /// requests finish.
    pub async fn run(self, shutdown: CancellationToken) -> Result<(), ApiError> {
        tracing::info!(host = %self.http.host, port = self.http.port, "api starting");

        let state = self.state;
        let upload_limiter = self.upload_limiter;
        let events = self.events;
        let relay = tokio::spawn({
            let database = state.database().clone();
            let events = events.clone();

            async move { events.relay(database).await }
        });
        let server = HttpServer::new({
            let events = events.clone();

            move || {
                App::new()
                    .app_data(state.clone())
                    .app_data(upload_limiter.clone())
                    .app_data(events.clone())
                    .configure(configure_routes)
            }
        })
        .disable_signals()
        .bind((self.http.host.as_str(), self.http.port));
        let server = match server {
            Ok(server) => server.run(),
            Err(error) => {
                relay.abort();

                return Err(error.into());
            }
        };
        let handle = server.handle();
        // Event streams never finish on their own, so they are closed before the graceful stop.
        let stopper = tokio::spawn(async move {
            shutdown.cancelled().await;
            events.close();
            handle.stop(true).await;
        });
        let result = server.await;

        stopper.abort();
        relay.abort();

        Ok(result?)
    }
}

/// Mounts every HTTP route under `/api/v1`, behind the same-origin check, with the shared
/// request-parsing error responses.
pub fn configure_routes(cfg: &mut ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .wrap(SameOrigin)
            .app_data(
                JsonConfig::default()
                    .limit(JSON_LIMIT)
                    .error_handler(|error, _| ErrorResponse::from(error).into()),
            )
            .app_data(
                QueryConfig::default().error_handler(|_, _| ErrorResponse::bad_request().into()),
            )
            .app_data(PathConfig::default().error_handler(|_, _| ErrorResponse::not_found().into()))
            .service(routes::health::report_health)
            .service(routes::language::show_language)
            .service(routes::events::stream_events)
            .service(routes::upload::upload_limits)
            .configure(routes::auth::configure)
            .service(
                web::scope("/contacts")
                    .service(routes::contacts::list_contacts)
                    .service(routes::contacts::create_contact)
                    .service(routes::contacts::update_contact)
                    .service(routes::contacts::delete_contact),
            )
            .configure(routes::settings::configure)
            .configure(routes::notifications::configure)
            .configure(routes::galleries::configure),
    );
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use pp_database::error::DatabaseError;
    use sqlx::Error as SqlxError;

    use super::*;

    #[test]
    fn database_errors_expose_a_sanitized_cause() {
        let error = ApiError::from(DatabaseError::from(SqlxError::Configuration(
            "unknown value \"s3cretpw\" for `ssl_mode`".into(),
        )));
        let cause = error.source().expect("the database cause is carried");

        assert!(!error.to_string().contains("s3cretpw"));
        assert!(!cause.to_string().contains("s3cretpw"));
        assert!(cause.source().is_none());
    }

    #[test]
    fn serve_error_exposes_its_cause_once() {
        let error = ApiError::from(IoError::other("bind failed"));

        assert_eq!(error.to_string(), "server failed");
        assert_eq!(error.source().unwrap().to_string(), "bind failed");
    }
}
