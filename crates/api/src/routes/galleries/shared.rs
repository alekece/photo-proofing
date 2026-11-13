//! Public client routes under `/api/v1/galleries/shared/{token}`.
//!
//! - `GET /` returns the title, layout, cover, typography and grid settings, shoot date and
//!   expiry of the gallery behind a private link, or only the minimal gate metadata (`locked: true`,
//!   `title`, `shoot_date`, `has_cover`, and studio signature) while a code-protected gallery is
//!   locked.
//! - `POST /unlock` verifies a 6-digit access code behind [`FailedAttempts::limit`] and issues
//!   the `HttpOnly` `pp_gallery_unlock` cookie.
//! - `POST /open` records the client's open as the gallery's last-opened time, at most once per
//!   throttle delay; other calls are no-ops (204).
//! - `GET /photos` lists the photos the client may see, with their displayed size: the ready
//!   proofs, or the ready finals once the gallery is delivered.
//! - `GET /photos/{photo_id}/thumbnail|preview` serves one of those photos.
//! - `GET /favorites` lists the ids the client selected; `PUT|DELETE /favorites/{photo_id}`
//!   selects or unselects one, idempotently, until the gallery is delivered (409).
//! - Photo comment counts, threads and writes live in [`super::comments`], proofs only.
//! - `GET /photos/{photo_id}/download` sends one ready final as an attachment, once delivered.
//! - `POST /download` makes sure the ZIP of every ready final exists: 200 `{"status":"ready"}`,
//!   202 `{"status":"building"}`, or 503 `{"status":"failed"}` once after a failed build.
//!   `GET /download` then sends that ZIP. A download is recorded when it is first sent.
//!
//! Only `GET /`, `/cover`, `/logo`, and `POST /unlock` accept [`SharedLinkAccess`]; every other
//! client route requires [`ClientAccess`], which refuses locked galleries with `401`. Malformed,
//! unknown, unshared, archived, and expired links always fail with `404` on every route. No route
//! serves an original. The link is a bearer credential: any access log added later must redact
//! `/galleries/shared/` paths.

use actix_web::{
    HttpRequest, HttpResponse, delete, get,
    http::StatusCode,
    middleware::from_fn,
    post, put,
    web::{Data, Json, Path, ServiceConfig},
};
use pp_database::export::{DeliveryZip, ExportRepository};
use pp_database::photo::PhotoId;
use pp_database::{
    gallery::{GalleryAccessCode, GalleryRepository, SharedGallery, SharedGalleryGate},
    photo::{PhotoKind, PhotoRepository, Watermarked},
    settings::{CoverSignature, SettingsRepository, SizeStep, WorkspaceSettings},
};
use pp_storage::{ArchiveKey, PhotoKey, Variant};
use serde::{Deserialize, Serialize};

use super::DerivativePhoto;
use crate::{
    access_code::UNLOCK_COOKIE_NAME,
    api::ApiState,
    error::ErrorResponse,
    extractors::client_access::{ClientAccess, ClientPhotoSet, SharedLinkAccess},
    middlewares::failed_attempts::FailedAttempts,
    response::{Attachment, ImageFile, ResponseBuilderExt},
};

/// The name the browser saves the client's ZIP under.
const DOWNLOAD_NAME: &str = "photos.zip";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnlockInput {
    code: GalleryAccessCode,
}

#[derive(Serialize)]
struct StudioBranding {
    studio_name: String,
    signature: CoverSignature,
    signature_size: SizeStep,
    has_logo: bool,
}

impl From<WorkspaceSettings> for StudioBranding {
    fn from(settings: WorkspaceSettings) -> Self {
        let studio_name = if matches!(
            settings.signature,
            CoverSignature::Name | CoverSignature::Both
        ) {
            settings.studio_name
        } else {
            String::new()
        };
        let has_logo = settings.logo.is_some()
            && matches!(
                settings.signature,
                CoverSignature::Logo | CoverSignature::Both
            );

        Self {
            studio_name,
            signature: settings.signature,
            signature_size: settings.signature_size,
            has_logo,
        }
    }
}

#[derive(Serialize)]
struct SharedGalleryView {
    locked: bool,
    #[serde(flatten)]
    gallery: SharedGallery,
    #[serde(flatten)]
    branding: StudioBranding,
}

#[derive(Serialize)]
struct LockedSharedGalleryView {
    locked: bool,
    #[serde(flatten)]
    gate: SharedGalleryGate,
    #[serde(flatten)]
    branding: StudioBranding,
}

#[derive(Serialize)]
#[serde(untagged)]
enum SharedGalleryResponse {
    Unlocked(SharedGalleryView),
    Locked(LockedSharedGalleryView),
}

#[derive(Serialize)]
struct ZipStatusResponse {
    status: &'static str,
}

/// Registers the client routes inside the `/api/v1/galleries` scope.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(show_shared_gallery)
        .service(unlock_shared_gallery)
        .service(record_open)
        .service(list_shared_photos)
        .service(list_favorites)
        .service(add_favorite)
        .service(remove_favorite)
        .service(serve_shared_logo)
        .service(serve_shared_cover)
        .service(serve_shared_photo)
        .service(download_shared_photo)
        .service(prepare_shared_zip)
        .service(download_shared_zip);
}

#[get("/shared/{token}")]
async fn show_shared_gallery(
    state: Data<ApiState>,
    link: SharedLinkAccess,
) -> Result<HttpResponse, ErrorResponse> {
    let settings = state
        .database()
        .repository::<SettingsRepository>()
        .settings(link.access.location.workspace_id)
        .await?;
    let branding = StudioBranding::from(settings);
    let body = if link.unlocked {
        SharedGalleryResponse::Unlocked(SharedGalleryView {
            locked: false,
            gallery: link.access.gallery,
            branding,
        })
    } else {
        SharedGalleryResponse::Locked(LockedSharedGalleryView {
            locked: true,
            gate: SharedGalleryGate::from(link.access.gallery),
            branding,
        })
    };

    Ok(HttpResponse::Ok().no_store().no_referrer().json(body))
}

#[post("/shared/{token}/unlock", wrap = "from_fn(FailedAttempts::limit)")]
async fn unlock_shared_gallery(
    request: HttpRequest,
    state: Data<ApiState>,
    link: SharedLinkAccess,
    input: Json<UnlockInput>,
) -> Result<HttpResponse, ErrorResponse> {
    let Some(ciphertext) = &link.access.access_code_ciphertext else {
        return Ok(HttpResponse::NoContent().no_store().no_referrer().finish());
    };
    let cipher = state
        .access_code_cipher()
        .ok_or_else(ErrorResponse::access_code_unavailable)?;
    let matches = cipher
        .verify_code(link.access.location.gallery_id, ciphertext, &input.code)
        .map_err(|_| ErrorResponse::access_code_unavailable())?;

    if !matches {
        return Err(ErrorResponse::invalid_access_code());
    }

    let existing = request
        .cookie(UNLOCK_COOKIE_NAME)
        .map(|cookie| cookie.value().to_owned());
    let cookie_value = cipher.issue_unlock_cookie_value(
        existing.as_deref(),
        link.access.location.gallery_id,
        ciphertext,
    );

    Ok(HttpResponse::NoContent()
        .no_store()
        .no_referrer()
        .cookie(state.unlock_cookie(cookie_value))
        .finish())
}

#[get("/shared/{token}/logo")]
async fn serve_shared_logo(
    state: Data<ApiState>,
    link: SharedLinkAccess,
) -> Result<HttpResponse, ErrorResponse> {
    let settings = state
        .database()
        .repository::<SettingsRepository>()
        .settings(link.access.location.workspace_id)
        .await?;

    if !matches!(
        settings.signature,
        CoverSignature::Logo | CoverSignature::Both
    ) {
        return Err(ErrorResponse::not_found());
    }

    let logo = settings.logo.ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok()
        .no_store()
        .no_referrer()
        .content_type("image/png")
        .body(logo))
}

#[post("/shared/{token}/open")]
async fn record_open(
    state: Data<ApiState>,
    access: ClientAccess,
) -> Result<HttpResponse, ErrorResponse> {
    state
        .database()
        .repository::<GalleryRepository>()
        .record_opened(access.0.location)
        .await?;

    Ok(HttpResponse::NoContent().no_store().no_referrer().finish())
}

#[get("/shared/{token}/photos")]
async fn list_shared_photos(
    state: Data<ApiState>,
    access: ClientAccess,
) -> Result<HttpResponse, ErrorResponse> {
    let photos = state
        .database()
        .repository::<PhotoRepository>()
        .list_ready(
            access.0.location,
            access.photo_set().kind(),
            access.watermarked(),
        )
        .await?;

    Ok(HttpResponse::Ok().no_store().no_referrer().json(photos))
}

#[get("/shared/{token}/favorites")]
async fn list_favorites(
    state: Data<ApiState>,
    access: ClientAccess,
) -> Result<HttpResponse, ErrorResponse> {
    let favorites = state
        .database()
        .repository::<PhotoRepository>()
        .favorites(access.0.location, access.photo_set().kind())
        .await?;

    Ok(HttpResponse::Ok().no_store().no_referrer().json(favorites))
}

#[put("/shared/{token}/favorites/{photo_id}")]
async fn add_favorite(
    state: Data<ApiState>,
    access: ClientAccess,
    path: Path<(String, PhotoId)>,
) -> Result<HttpResponse, ErrorResponse> {
    let change = state
        .database()
        .repository::<PhotoRepository>()
        .set_favorite(
            access.0.location,
            path.into_inner().1,
            true,
            access.photo_set().kind(),
            access.watermarked(),
        )
        .await?;

    ErrorResponse::ensure_favorite_applied(change)?;

    Ok(HttpResponse::NoContent().no_store().no_referrer().finish())
}

#[delete("/shared/{token}/favorites/{photo_id}")]
async fn remove_favorite(
    state: Data<ApiState>,
    access: ClientAccess,
    path: Path<(String, PhotoId)>,
) -> Result<HttpResponse, ErrorResponse> {
    let change = state
        .database()
        .repository::<PhotoRepository>()
        .set_favorite(
            access.0.location,
            path.into_inner().1,
            false,
            access.photo_set().kind(),
            access.watermarked(),
        )
        .await?;

    ErrorResponse::ensure_favorite_applied(change)?;

    Ok(HttpResponse::NoContent().no_store().no_referrer().finish())
}

#[get("/shared/{token}/photos/{photo_id}/{derivative:thumbnail|preview}")]
async fn serve_shared_photo(
    state: Data<ApiState>,
    access: ClientAccess,
    path: Path<(String, PhotoId, DerivativePhoto)>,
) -> Result<ImageFile, ErrorResponse> {
    let (_, photo_id, derivative) = path.into_inner();
    let location = access.0.location;
    let watermarked = access.watermarked();

    if !state
        .database()
        .repository::<PhotoRepository>()
        .is_ready(location, photo_id, access.photo_set().kind(), watermarked)
        .await?
    {
        return Err(ErrorResponse::not_found());
    }

    ImageFile::read(
        state.storage(),
        &PhotoKey::with_location(location, photo_id, derivative.variant(watermarked)),
    )
    .await
    .map(ImageFile::proof)
}

/// The delivered finals, as the client keeps them: the stored bytes, saved under their own name.
#[get("/shared/{token}/photos/{photo_id}/download")]
async fn download_shared_photo(
    state: Data<ApiState>,
    access: ClientAccess,
    path: Path<(String, PhotoId)>,
) -> Result<Attachment, ErrorResponse> {
    let (_, photo_id) = path.into_inner();
    let location = access.0.location;
    let photos = state.database().repository::<PhotoRepository>();

    if access.photo_set() != ClientPhotoSet::Finals
        || !photos
            .is_ready(location, photo_id, PhotoKind::Final, Watermarked(false))
            .await?
    {
        return Err(ErrorResponse::not_found());
    }

    let photo = photos
        .get(
            location.workspace_id,
            location.gallery_id,
            photo_id,
            PhotoKind::Final,
        )
        .await?
        .ok_or_else(ErrorResponse::not_found)?;
    let file = state
        .storage()
        .read(&PhotoKey::with_location(
            location,
            photo_id,
            Variant::Original,
        ))
        .await?;
    let attachment =
        Attachment::new("application/octet-stream", photo.download_name(), file).await?;

    photos.mark_downloaded(location, photo_id).await?;

    Ok(attachment)
}

#[post("/shared/{token}/download")]
async fn prepare_shared_zip(
    state: Data<ApiState>,
    access: ClientAccess,
) -> Result<HttpResponse, ErrorResponse> {
    if access.photo_set() != ClientPhotoSet::Finals {
        return Err(ErrorResponse::not_found());
    }

    let (status, body) = match state.ensure_delivery(access.0.location).await? {
        DeliveryZip::Ready(_) => (StatusCode::OK, "ready"),
        DeliveryZip::Building => (StatusCode::ACCEPTED, "building"),
        DeliveryZip::Failed => (StatusCode::SERVICE_UNAVAILABLE, "failed"),
        DeliveryZip::Empty | DeliveryZip::Unavailable => return Err(ErrorResponse::not_found()),
    };

    Ok(HttpResponse::build(status)
        .no_store()
        .no_referrer()
        .json(ZipStatusResponse { status: body }))
}

#[get("/shared/{token}/download")]
async fn download_shared_zip(
    state: Data<ApiState>,
    access: ClientAccess,
) -> Result<Attachment, ErrorResponse> {
    let location = access.0.location;

    if access.photo_set() != ClientPhotoSet::Finals {
        return Err(ErrorResponse::not_found());
    }

    let export = match state.ensure_delivery(location).await? {
        DeliveryZip::Ready(export) => export,
        DeliveryZip::Building | DeliveryZip::Failed => {
            return Err(ErrorResponse::zip_not_ready());
        }
        DeliveryZip::Empty | DeliveryZip::Unavailable => return Err(ErrorResponse::not_found()),
    };
    let file = state
        .storage()
        .read_archive(&ArchiveKey {
            workspace: location.workspace_id,
            gallery: location.gallery_id,
            export: export.id,
        })
        .await?;
    let attachment = Attachment::zip(DOWNLOAD_NAME, file).await?;

    state
        .database()
        .repository::<ExportRepository>()
        .mark_downloaded(export.id)
        .await?;

    Ok(attachment)
}

#[get("/shared/{token}/cover")]
async fn serve_shared_cover(
    state: Data<ApiState>,
    link: SharedLinkAccess,
) -> Result<ImageFile, ErrorResponse> {
    let id = link
        .access
        .gallery
        .hero_id
        .ok_or_else(ErrorResponse::not_found)?;

    if !state
        .database()
        .repository::<PhotoRepository>()
        .is_ready(
            link.access.location,
            id,
            PhotoKind::Proof,
            Watermarked(false),
        )
        .await?
    {
        return Err(ErrorResponse::not_found());
    }

    ImageFile::read(
        state.storage(),
        &PhotoKey::with_location(link.access.location, id, Variant::Preview),
    )
    .await
    .map(ImageFile::proof)
}

#[cfg(test)]
mod tests {
    use actix_web::{
        App,
        cookie::Cookie,
        dev::{Service, ServiceResponse},
        http::StatusCode,
        test,
    };
    use pp_database::{
        Database,
        gallery::{GalleryAccessCode, GalleryChanges, GalleryId, GalleryRepository, ShareToken},
        tests::TestRepository,
    };
    use serde_json::{Value, json};
    use sqlx::PgPool;

    use super::*;
    use crate::{
        api::configure_routes,
        routes::galleries::photos::tests::{
            Progress, create_gallery, photo_in, store_derivative, waiting_photo,
        },
        tests::{
            ORIGIN, app, app_state, call, call_request, other_workspace, sign_up, upload_config,
        },
    };

    async fn shared_gallery(pool: &PgPool, changes: Value) -> String {
        let workspace = other_workspace(pool).await;

        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let id = store.create(workspace).await.unwrap().id;
        let changes = serde_json::from_value::<GalleryChanges<GalleryAccessCode>>(changes)
            .unwrap()
            .try_map_access_code(|_| Err(()))
            .unwrap();

        store.update(workspace, id, &changes).await.unwrap();

        store
            .share(workspace, id)
            .await
            .unwrap()
            .unwrap()
            .share_token
            .unwrap()
            .as_str()
            .to_owned()
    }

    fn get(token: &str) -> test::TestRequest {
        test::TestRequest::get().uri(&format!("/api/v1/galleries/shared/{token}"))
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn shared_page_returns_only_client_fields(pool: PgPool) {
        let token = shared_gallery(&pool, json!({"title": "Wedding", "layout": "masonry"})).await;
        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;

        let response = test::call_service(&app, get(&token).to_request()).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        assert_eq!(
            response.headers().get("referrer-policy").unwrap(),
            "no-referrer"
        );

        let body: Value = test::read_body_json(response).await;

        assert_eq!(
            body,
            json!({
                "locked": false,
                "title": "Wedding",
                "layout": "masonry",
                "cover_layout": "full_bleed",
                "focal_point": {"x": 50, "y": 35},
                "heading_font": "editorial",
                "grid_density": "compact",
                "grid_spacing": "compact",
                "shoot_date": null,
                "expires_at": null,
                "included_photos": null,
                "delivered": false,
                "hero_id": null,
                "studio_name": "",
                "signature": "name",
                "signature_size": 4,
                "has_logo": false
            })
        );

        let custom_token = shared_gallery(
            &pool,
            json!({
                "cover_layout": "banner",
                "focal_point": {"x": 15, "y": 80},
                "heading_font": "modern",
                "grid_density": "large",
                "grid_spacing": "generous",
                "shoot_date": "2026-06-14"
            }),
        )
        .await;
        let custom_response = test::call_service(&app, get(&custom_token).to_request()).await;
        let custom_body: Value = test::read_body_json(custom_response).await;

        assert_eq!(custom_body["cover_layout"], "banner");
        assert_eq!(custom_body["focal_point"], json!({"x": 15, "y": 80}));
        assert_eq!(custom_body["heading_font"], "modern");
        assert_eq!(custom_body["grid_density"], "large");
        assert_eq!(custom_body["grid_spacing"], "generous");
        assert_eq!(custom_body["shoot_date"], "2026-06-14");
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn shared_signature_logo_requires_an_available_link(pool: PgPool) {
        let token = shared_gallery(&pool, json!({})).await;
        let expired = shared_gallery(&pool, json!({"expires_at": "2000-01-01T00:00:00Z"})).await;
        let workspace: uuid::Uuid =
            sqlx::query_scalar("SELECT workspace_id FROM galleries WHERE share_token = $1")
                .bind(&token)
                .fetch_one(&pool)
                .await
                .unwrap();
        sqlx::query("INSERT INTO workspace_settings(workspace_id, studio_name, signature, logo) VALUES ($1, 'Studio', 'both', $2)")
            .bind(workspace).bind(b"normalized PNG".as_slice()).execute(&pool).await.unwrap();
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;

        let response = test::call_service(&app, get(&token).to_request()).await;
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["studio_name"], "Studio");
        assert_eq!(body["signature"], "both");
        assert_eq!(body["signature_size"], 4);
        assert_eq!(body["has_logo"], true);
        for (link, expected) in [
            (&token, StatusCode::OK),
            (&expired, StatusCode::NOT_FOUND),
            (&"A".repeat(22), StatusCode::NOT_FOUND),
        ] {
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&format!("/api/v1/galleries/shared/{link}/logo"))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), expected);
            if expected == StatusCode::OK {
                assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
                assert_eq!(test::read_body(response).await.as_ref(), b"normalized PNG");
            }
        }
    }

    fn open(token: &str) -> test::TestRequest {
        test::TestRequest::post()
            .uri(&format!("/api/v1/galleries/shared/{token}/open"))
            .insert_header(("Origin", ORIGIN))
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn opening_records_the_last_open_only_for_available_links(pool: PgPool) {
        let live = shared_gallery(&pool, json!({})).await;
        let expired = shared_gallery(&pool, json!({"expires_at": "2000-01-01T00:00:00Z"})).await;
        let last_opened_at = |token: String| {
            let pool = pool.clone();

            async move {
                sqlx::query_scalar::<_, Option<String>>(
                    "SELECT last_opened_at::text FROM galleries WHERE share_token = $1",
                )
                .bind(token)
                .fetch_one(&pool)
                .await
                .unwrap()
            }
        };
        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;

        assert_eq!(last_opened_at(live.clone()).await, None);

        let response = test::call_service(&app, get(&live).to_request()).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(last_opened_at(live.clone()).await, None);

        let response = test::call_service(&app, open(&live).to_request()).await;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");

        let first = last_opened_at(live.clone()).await;

        assert!(first.is_some());

        let response = test::call_service(&app, open(&live).to_request()).await;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(last_opened_at(live).await, first);

        for token in [expired.as_str(), &"A".repeat(22), "short"] {
            let response = test::call_service(&app, open(token).to_request()).await;

            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        assert_eq!(last_opened_at(expired).await, None);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn photographer_reads_the_last_open(pool: PgPool) {
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let (_, created) = call(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/galleries")
                .insert_header(("Origin", ORIGIN))
                .cookie(cookie.clone()),
        )
        .await;
        let id = created["id"].as_str().unwrap();
        let (_, shared) = call(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/v1/galleries/{id}/share"))
                .insert_header(("Origin", ORIGIN))
                .cookie(cookie.clone()),
        )
        .await;
        let token = shared["share_token"].as_str().unwrap();
        let read = || {
            test::TestRequest::get()
                .uri(&format!("/api/v1/galleries/{id}"))
                .cookie(cookie.clone())
        };

        assert_eq!(created["last_opened_at"], Value::Null);
        assert_eq!(call(&app, read()).await.1["last_opened_at"], Value::Null);

        call(&app, open(token)).await;

        let (_, gallery) = call(&app, read()).await;

        assert!(gallery["last_opened_at"].is_string());

        let (_, notifications) = call(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/notifications")
                .cookie(cookie.clone()),
        )
        .await;

        assert_eq!(notifications["unread"], 1);
        assert_eq!(notifications["notifications"][0]["kind"], "open");
        assert_eq!(notifications["notifications"][0]["count"], 1);

        let (_, client) = call(&app, get(token)).await;

        assert_eq!(client.get("last_opened_at"), None);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn unknown_malformed_and_expired_links_are_indistinguishable(pool: PgPool) {
        let expired = shared_gallery(&pool, json!({"expires_at": "2000-01-01T00:00:00Z"})).await;
        let live = shared_gallery(&pool, json!({})).await;
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let swapped_case: String = live
            .chars()
            .map(|c| {
                if c.is_ascii_lowercase() {
                    c.to_ascii_uppercase()
                } else {
                    c.to_ascii_lowercase()
                }
            })
            .collect();
        let unknown = "A".repeat(22);
        let non_canonical = format!("{}B", "A".repeat(21));
        let mut outcomes = Vec::new();

        for token in [
            expired.as_str(),
            unknown.as_str(),
            non_canonical.as_str(),
            "short",
            swapped_case.as_str(),
        ] {
            let response = test::call_service(&app, get(token).to_request()).await;
            let status = response.status();
            let mut headers: Vec<_> = response
                .headers()
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_str().unwrap().to_owned()))
                .collect();

            headers.sort();

            outcomes.push((status, headers, test::read_body(response).await));
        }

        assert_eq!(outcomes[0].0, StatusCode::NOT_FOUND);
        assert!(outcomes.windows(2).all(|pair| pair[0] == pair[1]));
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn extending_the_expiry_restores_access_at_the_same_link(pool: PgPool) {
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let (_, created) = call(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/galleries")
                .insert_header(("Origin", ORIGIN))
                .cookie(cookie.clone()),
        )
        .await;
        let id = created["id"].as_str().unwrap();
        let patch = |value: Value| {
            test::TestRequest::patch()
                .uri(&format!("/api/v1/galleries/{id}"))
                .insert_header(("Origin", ORIGIN))
                .cookie(cookie.clone())
                .set_json(json!({"expires_at": value}))
        };
        let (_, shared) = call(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/v1/galleries/{id}/share"))
                .insert_header(("Origin", ORIGIN))
                .cookie(cookie.clone()),
        )
        .await;
        let token = shared["share_token"].as_str().unwrap();

        call(&app, patch(json!("2000-01-01T00:00:00Z"))).await;

        assert_eq!(call(&app, get(token)).await.0, StatusCode::NOT_FOUND);

        call(&app, patch(json!("2999-01-01T00:00:00Z"))).await;

        let (status, body) = call(&app, get(token)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["expires_at"], "2999-01-01T00:00:00Z");
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn shared_page_keeps_the_clean_hero_during_marked_processing(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let hero = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;
        let token = share(&app, &cookie, &gallery_id).await;
        let request = test::TestRequest::patch()
            .uri(&format!("/api/v1/galleries/{gallery_id}"))
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone())
            .set_json(json!({"hero_id": hero}));

        assert_eq!(call(&app, request).await.0, StatusCode::OK);

        let (status, body) = call(&app, get(&token)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["hero_id"], hero.as_str());

        sqlx::query("UPDATE photos SET status = 'processing' WHERE id = $1::uuid")
            .bind(&hero)
            .execute(&pool)
            .await
            .unwrap();

        let (_, body) = call(&app, get(&token)).await;

        assert_eq!(body["hero_id"], hero.as_str());
    }

    /// Shares the gallery through the photographer route and returns its token.
    async fn share<S>(app: &S, cookie: &Cookie<'static>, gallery_id: &str) -> String
    where
        S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
    {
        let request = test::TestRequest::post()
            .uri(&format!("/api/v1/galleries/{gallery_id}/share"))
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone());
        let (status, gallery) = call(app, request).await;

        assert_eq!(status, StatusCode::OK);

        gallery["share_token"].as_str().unwrap().to_owned()
    }

    async fn expire<S>(app: &S, cookie: &Cookie<'static>, gallery_id: &str)
    where
        S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
    {
        let request = test::TestRequest::patch()
            .uri(&format!("/api/v1/galleries/{gallery_id}"))
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone())
            .set_json(json!({"expires_at": "2000-01-01T00:00:00Z"}));

        assert_eq!(call(app, request).await.0, StatusCode::OK);
    }

    fn client_request(token: &str, path: &str) -> actix_http::Request {
        test::TestRequest::get()
            .uri(&format!("/api/v1/galleries/shared/{token}{path}"))
            .to_request()
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_valid_link_has_no_event_route(pool: PgPool) {
        let token = shared_gallery(&pool, json!({})).await;
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool, directory.path(), upload_config(1024, 100)).await;

        let response = test::call_service(&app, client_request(&token, "/photos")).await;

        assert_eq!(response.status(), StatusCode::OK);

        let response = test::call_service(&app, client_request(&token, "/events")).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn client_lists_only_ready_photos_with_dimensions(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let first = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;

        photo_in(&app, &pool, &cookie, &gallery_id, Progress::Processing).await;
        photo_in(&app, &pool, &cookie, &gallery_id, Progress::Failed).await;

        let second = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;

        waiting_photo(&app, &cookie, &gallery_id).await;

        let token = share(&app, &cookie, &gallery_id).await;
        let response = test::call_service(&app, client_request(&token, "/photos")).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        assert_eq!(
            response.headers().get("referrer-policy").unwrap(),
            "no-referrer"
        );

        let body: Value = test::read_body_json(response).await;

        assert_eq!(
            body,
            json!([
                {"id": first, "width": 2, "height": 1, "revision": 1},
                {"id": second, "width": 2, "height": 1, "revision": 1},
            ])
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn marked_client_images_and_clean_cover_are_separate(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;
        let clean = store_derivative(&pool, directory.path(), &photo, "preview").await;
        let marked = store_derivative(&pool, directory.path(), &photo, "watermarked-preview").await;
        let token = share(&app, &cookie, &gallery_id).await;

        sqlx::query(
            "UPDATE galleries SET watermark_enabled = true, hero_id = $2::uuid WHERE id = $1::uuid",
        )
        .bind(&gallery_id)
        .bind(&photo)
        .execute(&pool)
        .await
        .unwrap();

        let response = test::call_service(
            &app,
            client_request(&token, &format!("/photos/{photo}/preview")),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(test::read_body(response).await.as_ref(), marked);

        let response = test::call_service(
            &app,
            client_request(&token, &format!("/photos/{photo}/preview?watermark=false")),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(test::read_body(response).await.as_ref(), marked);

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/api/v1/galleries/{gallery_id}/photos/{photo}/preview?watermark=true"
                ))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(test::read_body(response).await.as_ref(), marked);

        sqlx::query("UPDATE photos SET status = 'processing', watermark_revision = NULL WHERE id = $1::uuid")
            .bind(&photo).execute(&pool).await.unwrap();

        let response = test::call_service(&app, client_request(&token, "/cover")).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(test::read_body(response).await.as_ref(), clean);

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/api/v1/galleries/{gallery_id}/photos/{photo}/preview"
                ))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(test::read_body(response).await.as_ref(), clean);

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/api/v1/galleries/{gallery_id}/photos?watermark=true"
                ))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        let photos: Value = test::read_body_json(response).await;

        assert_eq!(photos, json!([]));

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/api/v1/galleries/{gallery_id}/photos?watermark=true"
                ))
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = test::call_service(
            &app,
            client_request(&token, &format!("/photos/{photo}/preview?clean=true")),
        )
        .await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn client_serves_thumbnail_and_preview_of_a_ready_photo(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;
        let token = share(&app, &cookie, &gallery_id).await;

        for (image, variant) in [("thumbnail", "thumbnail"), ("preview", "preview")] {
            let stored = store_derivative(&pool, directory.path(), &photo, variant).await;
            let response = test::call_service(
                &app,
                client_request(&token, &format!("/photos/{photo}/{image}")),
            )
            .await;
            let headers = response.headers();

            assert_eq!(response.status(), StatusCode::OK, "{image}");
            assert_eq!(headers.get("content-type").unwrap(), "image/jpeg");
            assert_eq!(headers.get("cache-control").unwrap(), "no-store");
            assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
            assert_eq!(headers.get("referrer-policy").unwrap(), "no-referrer");
            assert_eq!(test::read_body(response).await, stored);
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn client_media_is_404_for_unready_foreign_unknown_and_missing_files(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let other_gallery = create_gallery(&app, &cookie).await;
        let without_file = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;
        let failed = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Failed).await;
        let foreign = photo_in(&app, &pool, &cookie, &other_gallery, Progress::Ready).await;
        let waiting = waiting_photo(&app, &cookie, &gallery_id).await;

        for photo in [&failed, &foreign, &waiting] {
            store_derivative(&pool, directory.path(), photo, "thumbnail").await;
            store_derivative(&pool, directory.path(), photo, "preview").await;
        }

        let token = share(&app, &cookie, &gallery_id).await;
        let unknown = uuid::Uuid::new_v4().to_string();

        for photo in [&without_file, &failed, &foreign, &waiting, &unknown] {
            for image in ["thumbnail", "preview"] {
                let (status, _) = call_request(
                    &app,
                    client_request(&token, &format!("/photos/{photo}/{image}")),
                )
                .await;

                assert_eq!(status, StatusCode::NOT_FOUND, "{photo} {image}");
            }
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn expired_and_unshared_links_deny_list_and_media(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;

        store_derivative(&pool, directory.path(), &photo, "thumbnail").await;
        store_derivative(&pool, directory.path(), &photo, "preview").await;

        let token = share(&app, &cookie, &gallery_id).await;
        let paths = [
            "/photos".to_owned(),
            format!("/photos/{photo}/thumbnail"),
            format!("/photos/{photo}/preview"),
        ];

        for path in &paths {
            let (status, _) = call_request(&app, client_request(&token, path)).await;

            assert_eq!(status, StatusCode::OK, "{path}");
        }

        expire(&app, &cookie, &gallery_id).await;

        let unshared = ShareToken::generate().as_str().to_owned();

        for token in [token.as_str(), unshared.as_str(), "short"] {
            for path in &paths {
                let (status, _) = call_request(&app, client_request(token, path)).await;

                assert_eq!(status, StatusCode::NOT_FOUND, "{token}{path}");
            }
        }
    }

    fn favorite_request(method: &str, token: &str, photo: &str) -> actix_http::Request {
        let request = match method {
            "PUT" => test::TestRequest::put(),
            _ => test::TestRequest::delete(),
        };

        request
            .uri(&format!(
                "/api/v1/galleries/shared/{token}/favorites/{photo}"
            ))
            .insert_header(("Origin", ORIGIN))
            .to_request()
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_client_selects_and_unselects_photos_idempotently(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let first = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;
        let second = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;
        let token = share(&app, &cookie, &gallery_id).await;

        for photo in [&second, &second, &first] {
            let response = test::call_service(&app, favorite_request("PUT", &token, photo)).await;

            assert_eq!(response.status(), StatusCode::NO_CONTENT);
        }

        let (_, favorites) = call_request(&app, client_request(&token, "/favorites")).await;

        assert_eq!(favorites, json!([first, second]));

        for _ in 0..2 {
            let (status, _) = call_request(&app, favorite_request("DELETE", &token, &first)).await;

            assert_eq!(status, StatusCode::NO_CONTENT);
        }

        let (_, favorites) = call_request(&app, client_request(&token, "/favorites")).await;

        assert_eq!(favorites, json!([second]));

        let (_, shared) = call_request(&app, client_request(&token, "")).await;

        assert_eq!(shared["delivered"], false);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn favorites_reject_foreign_unready_unknown_and_unavailable_links(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let other_gallery = create_gallery(&app, &cookie).await;
        let ready = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;
        let failed = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Failed).await;
        let foreign = photo_in(&app, &pool, &cookie, &other_gallery, Progress::Ready).await;
        let waiting = waiting_photo(&app, &cookie, &gallery_id).await;
        let unknown = uuid::Uuid::new_v4().to_string();
        let token = share(&app, &cookie, &gallery_id).await;

        for photo in [&failed, &foreign, &waiting, &unknown] {
            for method in ["PUT", "DELETE"] {
                let (status, _) = call_request(&app, favorite_request(method, &token, photo)).await;

                assert_eq!(status, StatusCode::NOT_FOUND, "{method} {photo}");
            }
        }

        expire(&app, &cookie, &gallery_id).await;

        let unshared = ShareToken::generate().as_str().to_owned();

        for token in [token.as_str(), unshared.as_str(), "short"] {
            for method in ["PUT", "DELETE"] {
                let (status, _) = call_request(&app, favorite_request(method, token, &ready)).await;

                assert_eq!(status, StatusCode::NOT_FOUND, "{method} {token}");
            }

            let (status, _) = call_request(&app, client_request(token, "/favorites")).await;

            assert_eq!(status, StatusCode::NOT_FOUND);
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_delivered_gallery_keeps_its_selection(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;
        let token = share(&app, &cookie, &gallery_id).await;

        call_request(&app, favorite_request("PUT", &token, &photo)).await;
        sqlx::query("UPDATE galleries SET delivered_at = now()")
            .execute(&pool)
            .await
            .unwrap();

        for method in ["PUT", "DELETE"] {
            let (status, _) = call_request(&app, favorite_request(method, &token, &photo)).await;

            assert_eq!(status, StatusCode::NOT_FOUND, "{method}");
        }

        let (_, favorites) = call_request(&app, client_request(&token, "/favorites")).await;
        let (_, shared) = call_request(&app, client_request(&token, "")).await;

        let kept: i64 =
            sqlx::query_scalar("SELECT count(*) FROM photos WHERE favorited_at IS NOT NULL")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(favorites, json!([]));
        assert_eq!(kept, 1);
        assert_eq!(shared["delivered"], true);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn no_client_route_serves_the_original(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;

        store_derivative(&pool, directory.path(), &photo, "preview").await;

        let token = share(&app, &cookie, &gallery_id).await;

        for path in ["original", "Original", "image", "download", ""] {
            let (status, _) = call_request(
                &app,
                client_request(&token, &format!("/photos/{photo}/{path}")),
            )
            .await;

            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        }
    }

    fn lifecycle(
        method: &str,
        gallery_id: &str,
        action: &str,
        cookie: &Cookie<'static>,
    ) -> test::TestRequest {
        let request = match method {
            "DELETE" => test::TestRequest::delete(),
            _ => test::TestRequest::post(),
        };

        request
            .uri(&format!("/api/v1/galleries/{gallery_id}{action}"))
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone())
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn archiving_hides_the_link_and_restoring_brings_it_back(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;

        store_derivative(&pool, directory.path(), &photo, "thumbnail").await;

        let token = share(&app, &cookie, &gallery_id).await;
        let paths = [
            "".to_owned(),
            "/photos".to_owned(),
            "/favorites".to_owned(),
            format!("/photos/{photo}/thumbnail"),
        ];

        let (status, archived) =
            call(&app, lifecycle("POST", &gallery_id, "/archive", &cookie)).await;

        assert_eq!(status, StatusCode::OK);
        assert!(
            archived["states"]
                .as_array()
                .unwrap()
                .contains(&json!("archived"))
        );

        for path in &paths {
            let (status, _) = call_request(&app, client_request(&token, path)).await;

            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        }

        let response = test::call_service(&app, favorite_request("PUT", &token, &photo)).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let (status, restored) =
            call(&app, lifecycle("POST", &gallery_id, "/restore", &cookie)).await;

        assert_eq!(status, StatusCode::OK);
        assert!(
            !restored["states"]
                .as_array()
                .unwrap()
                .contains(&json!("archived"))
        );

        for path in &paths {
            let (status, _) = call_request(&app, client_request(&token, path)).await;

            assert_eq!(status, StatusCode::OK, "{path}");
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn deleting_a_gallery_ends_its_link_and_queues_one_cleanup(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let token = share(&app, &cookie, &gallery_id).await;

        photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;
        waiting_photo(&app, &cookie, &gallery_id).await;

        let response = test::call_service(
            &app,
            lifecycle("DELETE", &gallery_id, "", &cookie).to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        assert_eq!(
            call_request(&app, client_request(&token, "")).await.0,
            StatusCode::NOT_FOUND
        );

        let tests = Database::new(pool.clone());
        let tests = tests.repository::<TestRepository>();

        assert_eq!(
            tests.jobs_of_kind("cleanup_gallery").await.unwrap().len(),
            1
        );
        assert_eq!(tests.count_jobs(None).await.unwrap(), 1);

        let (status, _) = call(&app, lifecycle("DELETE", &gallery_id, "", &cookie)).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            tests.jobs_of_kind("cleanup_gallery").await.unwrap().len(),
            1
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn lifecycle_routes_need_a_session_and_the_owning_workspace(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let foreign = Database::new(pool.clone())
            .repository::<GalleryRepository>()
            .create(other_workspace(&pool).await)
            .await
            .unwrap()
            .id
            .0
            .to_string();

        for (method, action) in [("POST", "/archive"), ("POST", "/restore"), ("DELETE", "")] {
            let anonymous = match method {
                "DELETE" => test::TestRequest::delete(),
                _ => test::TestRequest::post(),
            }
            .uri(&format!("/api/v1/galleries/{gallery_id}{action}"))
            .insert_header(("Origin", ORIGIN));

            assert_eq!(call(&app, anonymous).await.0, StatusCode::UNAUTHORIZED);

            let (status, _) = call(&app, lifecycle(method, &foreign, action, &cookie)).await;

            assert_eq!(status, StatusCode::NOT_FOUND, "{method} {action}");
        }

        assert_eq!(
            call(&app, lifecycle("POST", &gallery_id, "/archive", &cookie))
                .await
                .0,
            StatusCode::OK
        );
        assert!(
            Database::new(pool)
                .repository::<GalleryRepository>()
                .exists(GalleryId(foreign.parse().unwrap()))
                .await
                .unwrap()
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn photographers_can_still_upload_to_an_archived_gallery(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;

        call(&app, lifecycle("POST", &gallery_id, "/archive", &cookie)).await;

        waiting_photo(&app, &cookie, &gallery_id).await;
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn locked_gallery_serves_only_gate_cover_and_logo_until_unlocked(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let photo_dir: pp_core::config::PhotoDir =
            directory.path().to_path_buf().try_into().unwrap();
        let storage = pp_storage::PhotoStorage::open(&photo_dir).await.unwrap();
        crate::upload::jpeg::install().unwrap();
        let cfg = upload_config(1024, 100);
        let key = "0123456789abcdef"
            .repeat(4)
            .parse::<crate::access_code::EncryptionKey>()
            .unwrap();
        let state = crate::tests::state_with_upload(pool.clone(), storage, cfg.clone())
            .with_gallery_password(&crate::config::GalleryPasswordConfig {
                encryption_key: Some(key),
            });
        let app = test::init_service(
            App::new()
                .app_data(Data::new(crate::upload::limiter::UploadLimiter::from(&cfg)))
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let owner_cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &owner_cookie).await;
        let photo = photo_in(&app, &pool, &owner_cookie, &gallery_id, Progress::Ready).await;
        let clean_cover = store_derivative(&pool, directory.path(), &photo, "preview").await;
        store_derivative(&pool, directory.path(), &photo, "thumbnail").await;

        let workspace: uuid::Uuid =
            sqlx::query_scalar("SELECT workspace_id FROM galleries WHERE id = $1::uuid")
                .bind(&gallery_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        sqlx::query(
            "INSERT INTO workspace_settings(workspace_id, studio_name, signature, logo)
             VALUES ($1, 'Studio Lumiere', 'both', $2)",
        )
        .bind(workspace)
        .bind(b"png-logo".as_slice())
        .execute(&pool)
        .await
        .unwrap();

        let (status, _) = call(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/v1/galleries/{gallery_id}"))
                .insert_header(("Origin", ORIGIN))
                .cookie(owner_cookie.clone())
                .set_json(json!({
                    "title": "Secret Shoot",
                    "shoot_date": "2026-07-04",
                    "included_photos": 20,
                    "expires_at": "2999-01-01T00:00:00Z",
                    "hero_id": photo,
                    "access_code": "042819",
                })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // Link a contact BEFORE sharing; while still Draft, contact link must return 404 on every endpoint.
        let (_, created_contact) = call(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/contacts")
                .insert_header(("Origin", ORIGIN))
                .cookie(owner_cookie.clone())
                .set_json(json!({"name": "Claire", "email": "claire@example.test"})),
        )
        .await;
        let contact_id = created_contact["id"].as_str().unwrap();
        call(
            &app,
            test::TestRequest::put()
                .uri(&format!(
                    "/api/v1/galleries/{gallery_id}/contacts/{contact_id}"
                ))
                .insert_header(("Origin", ORIGIN))
                .cookie(owner_cookie.clone()),
        )
        .await;
        let contact_token: String = sqlx::query_scalar(
            "SELECT share_token FROM gallery_contacts WHERE gallery_id = $1::uuid",
        )
        .bind(&gallery_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        for draft_probe in [
            test::TestRequest::get().uri(&format!("/api/v1/galleries/shared/{contact_token}")),
            test::TestRequest::get()
                .uri(&format!("/api/v1/galleries/shared/{contact_token}/cover")),
            test::TestRequest::post()
                .uri(&format!("/api/v1/galleries/shared/{contact_token}/unlock"))
                .insert_header(("Origin", ORIGIN))
                .set_json(json!({"code": "042819"})),
        ] {
            assert_eq!(call(&app, draft_probe).await.0, StatusCode::NOT_FOUND);
        }

        let token = share(&app, &owner_cookie, &gallery_id).await;

        // Second protected gallery to verify multi-gallery cookie retention.
        let second_id = create_gallery(&app, &owner_cookie).await;
        call(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/v1/galleries/{second_id}"))
                .insert_header(("Origin", ORIGIN))
                .cookie(owner_cookie.clone())
                .set_json(json!({"title": "Second Shoot", "access_code": "111222"})),
        )
        .await;
        let second_token = share(&app, &owner_cookie, &second_id).await;

        // Gate metadata omits non-gate fields before unlock.
        let (status, gate) = call(&app, get(&token)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            gate,
            json!({
                "locked": true,
                "title": "Secret Shoot",
                "shoot_date": "2026-07-04",
                "has_cover": true,
                "studio_name": "Studio Lumiere",
                "signature": "both",
                "signature_size": 4,
                "has_logo": true,
            })
        );

        // Cover and logo remain reachable so the darkened cover gate can render.
        let cover_res = test::call_service(&app, client_request(&token, "/cover")).await;
        assert_eq!(cover_res.status(), StatusCode::OK);
        assert_eq!(test::read_body(cover_res).await.as_ref(), clean_cover);

        let logo_res = test::call_service(&app, client_request(&token, "/logo")).await;
        assert_eq!(logo_res.status(), StatusCode::OK);
        assert_eq!(test::read_body(logo_res).await.as_ref(), b"png-logo");

        // All content, selection, comment, download, and open routes fail 401 gallery_locked.
        for blocked in [
            test::TestRequest::get().uri(&format!("/api/v1/galleries/shared/{token}/photos")),
            test::TestRequest::get().uri(&format!(
                "/api/v1/galleries/shared/{token}/photos/{photo}/thumbnail"
            )),
            test::TestRequest::get().uri(&format!(
                "/api/v1/galleries/shared/{token}/photos/{photo}/preview"
            )),
            test::TestRequest::get().uri(&format!(
                "/api/v1/galleries/shared/{token}/photos/{photo}/download"
            )),
            test::TestRequest::get().uri(&format!("/api/v1/galleries/shared/{token}/download")),
            test::TestRequest::post()
                .uri(&format!("/api/v1/galleries/shared/{token}/download"))
                .insert_header(("Origin", ORIGIN)),
            test::TestRequest::get().uri(&format!("/api/v1/galleries/shared/{token}/favorites")),
            test::TestRequest::put()
                .uri(&format!(
                    "/api/v1/galleries/shared/{token}/favorites/{photo}"
                ))
                .insert_header(("Origin", ORIGIN)),
            test::TestRequest::delete()
                .uri(&format!(
                    "/api/v1/galleries/shared/{token}/favorites/{photo}"
                ))
                .insert_header(("Origin", ORIGIN)),
            test::TestRequest::get().uri(&format!("/api/v1/galleries/shared/{token}/comments")),
            test::TestRequest::get().uri(&format!(
                "/api/v1/galleries/shared/{token}/photos/{photo}/comments"
            )),
            test::TestRequest::post()
                .uri(&format!(
                    "/api/v1/galleries/shared/{token}/photos/{photo}/comments"
                ))
                .insert_header(("Origin", ORIGIN))
                .set_json(json!({"body": "Hello"})),
            open(&token),
        ] {
            let (status, err) = call(&app, blocked).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
            assert_eq!(err["code"], "gallery_locked");
        }

        // Non-POST requests miss the #[post] guard without consuming the peer's attempt budget.
        for _ in 0..11 {
            let get_unlock = test::call_service(&app, client_request(&token, "/unlock")).await;
            assert_ne!(get_unlock.status(), StatusCode::TOO_MANY_REQUESTS);
        }

        // Wrong code returns 401 invalid_access_code without issuing a cookie.
        let wrong_res = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/v1/galleries/shared/{token}/unlock"))
                .insert_header(("Origin", ORIGIN))
                .set_json(json!({"code": "000000"}))
                .to_request(),
        )
        .await;
        assert_eq!(wrong_res.status(), StatusCode::UNAUTHORIZED);
        assert!(wrong_res.response().cookies().next().is_none());
        let wrong_body: Value = test::read_body_json(wrong_res).await;
        assert_eq!(wrong_body["code"], "invalid_access_code");

        // Correct code sets HttpOnly SameSite=Lax cookie and unlocks both general and contact tokens.
        let unlock_res = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/v1/galleries/shared/{token}/unlock"))
                .insert_header(("Origin", ORIGIN))
                .set_json(json!({"code": "042819"}))
                .to_request(),
        )
        .await;
        assert_eq!(unlock_res.status(), StatusCode::NO_CONTENT);
        let unlock_cookie = unlock_res
            .response()
            .cookies()
            .find(|c| c.name() == crate::access_code::UNLOCK_COOKIE_NAME)
            .unwrap()
            .into_owned();
        assert_eq!(unlock_cookie.http_only(), Some(true));
        assert_eq!(
            unlock_cookie.same_site(),
            Some(actix_web::cookie::SameSite::Lax)
        );
        assert_eq!(unlock_cookie.path(), Some("/"));

        for link in [&token, &contact_token] {
            let (status, unlocked_body) = call(
                &app,
                test::TestRequest::get()
                    .uri(&format!("/api/v1/galleries/shared/{link}"))
                    .cookie(unlock_cookie.clone()),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(unlocked_body["locked"], false);
            assert_eq!(unlocked_body["included_photos"], 20);

            let (status, photos) = call(
                &app,
                test::TestRequest::get()
                    .uri(&format!("/api/v1/galleries/shared/{link}/photos"))
                    .cookie(unlock_cookie.clone()),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(photos.as_array().unwrap().len(), 1);
        }

        // Unlocking second gallery keeps the first gallery unlocked in the same cookie.
        let second_unlock_res = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/v1/galleries/shared/{second_token}/unlock"))
                .insert_header(("Origin", ORIGIN))
                .cookie(unlock_cookie)
                .set_json(json!({"code": "111222"}))
                .to_request(),
        )
        .await;
        assert_eq!(second_unlock_res.status(), StatusCode::NO_CONTENT);
        let both_cookie = second_unlock_res
            .response()
            .cookies()
            .find(|c| c.name() == crate::access_code::UNLOCK_COOKIE_NAME)
            .unwrap()
            .into_owned();

        for link in [&token, &second_token] {
            let (status, body) = call(
                &app,
                test::TestRequest::get()
                    .uri(&format!("/api/v1/galleries/shared/{link}"))
                    .cookie(both_cookie.clone()),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body["locked"], false);
        }

        // Archiving or expiring a protected gallery returns 404 everywhere, even with a valid unlock cookie.
        call(
            &app,
            lifecycle("POST", &gallery_id, "/archive", &owner_cookie),
        )
        .await;
        for archived_probe in [
            test::TestRequest::get()
                .uri(&format!("/api/v1/galleries/shared/{token}"))
                .cookie(both_cookie.clone()),
            test::TestRequest::get()
                .uri(&format!("/api/v1/galleries/shared/{token}/cover"))
                .cookie(both_cookie.clone()),
            test::TestRequest::post()
                .uri(&format!("/api/v1/galleries/shared/{token}/unlock"))
                .insert_header(("Origin", ORIGIN))
                .cookie(both_cookie.clone())
                .set_json(json!({"code": "042819"})),
        ] {
            assert_eq!(call(&app, archived_probe).await.0, StatusCode::NOT_FOUND);
        }

        call(
            &app,
            lifecycle("POST", &gallery_id, "/restore", &owner_cookie),
        )
        .await;
        expire(&app, &owner_cookie, &gallery_id).await;
        for expired_probe in [
            test::TestRequest::get()
                .uri(&format!("/api/v1/galleries/shared/{token}"))
                .cookie(both_cookie.clone()),
            test::TestRequest::post()
                .uri(&format!("/api/v1/galleries/shared/{token}/unlock"))
                .insert_header(("Origin", ORIGIN))
                .cookie(both_cookie)
                .set_json(json!({"code": "042819"})),
        ] {
            assert_eq!(call(&app, expired_probe).await.0, StatusCode::NOT_FOUND);
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn wrong_unlock_attempts_are_rate_limited_per_peer(pool: PgPool) {
        let (state, _directory) = app_state(pool).await;
        let key = "0123456789abcdef"
            .repeat(4)
            .parse::<crate::access_code::EncryptionKey>()
            .unwrap();
        let state = state.with_gallery_password(&crate::config::GalleryPasswordConfig {
            encryption_key: Some(key),
        });
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let owner_cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &owner_cookie).await;

        call(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/v1/galleries/{gallery_id}"))
                .insert_header(("Origin", ORIGIN))
                .cookie(owner_cookie.clone())
                .set_json(json!({"access_code": "123456"})),
        )
        .await;
        let token = share(&app, &owner_cookie, &gallery_id).await;

        for attempt in 0..10 {
            let (status, body) = call(
                &app,
                test::TestRequest::post()
                    .uri(&format!("/api/v1/galleries/shared/{token}/unlock"))
                    .insert_header(("Origin", ORIGIN))
                    .set_json(json!({"code": "000000"})),
            )
            .await;

            assert_eq!(status, StatusCode::UNAUTHORIZED, "attempt {attempt}");
            assert_eq!(body["code"], "invalid_access_code");
        }

        let (status, body) = call(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/v1/galleries/shared/{token}/unlock"))
                .insert_header(("Origin", ORIGIN))
                .set_json(json!({"code": "123456"})),
        )
        .await;

        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "too_many_attempts");
    }
}
