//! Workspace settings under `/api/v1/settings`: the studio name, logo, watermark and cover
//! signature.
//!
//! - `GET /` returns the saved settings, or the defaults. The logo is not inlined: `has_logo`
//!   says whether `GET /logo` has one.
//! - `PUT /` saves every setting at once and returns them. `logo` is `{"action": "keep"}` (the
//!   default when absent), `{"action": "remove"}` or `{"action": "replace", "image_base64": …}`
//!   (a PNG or JPEG, stored as a normalised PNG). A change to what the watermark draws regenerates the proofs of watermarked galleries.
//! - `GET /logo` returns the saved logo as a PNG, or a 404.

use actix_web::{
    HttpResponse, get,
    http::header::{self, ContentType},
    put,
    web::{self, Data, Json, JsonConfig, ServiceConfig},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use pp_database::watermark::{
    CoverSignature, GridCell, LogoChange, SettingsUpdate, SizeStep, WatermarkMark,
    WatermarkPattern, WatermarkRepository, WorkspaceSettings,
};
use serde::{Deserialize, Serialize};

use crate::{
    api::ApiState, error::ErrorResponse, extractors::identity::Authenticated,
    response::ResponseBuilderExt, upload::logo::LogoNormaliser,
};

/// Room for a base64 logo at its size limit and the other settings.
const BODY_LIMIT: usize = LogoNormaliser::MAX_BYTES * 4 / 3 + 4096;

#[derive(Serialize)]
struct SettingsView {
    studio_name: String,
    mark: WatermarkMark,
    pattern: WatermarkPattern,
    position: GridCell,
    size: SizeStep,
    signature: CoverSignature,
    has_logo: bool,
}

impl From<WorkspaceSettings> for SettingsView {
    fn from(settings: WorkspaceSettings) -> Self {
        Self {
            studio_name: settings.studio_name,
            mark: settings.mark,
            pattern: settings.pattern,
            position: settings.position,
            size: settings.size,
            signature: settings.signature,
            has_logo: settings.logo.is_some(),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum LogoRequest {
    Keep,
    Remove,
    Replace { image_base64: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsRequest {
    studio_name: String,
    mark: WatermarkMark,
    pattern: WatermarkPattern,
    position: GridCell,
    size: SizeStep,
    signature: CoverSignature,
    logo: Option<LogoRequest>,
}

impl SettingsRequest {
    async fn into_update(self) -> Result<SettingsUpdate, ErrorResponse> {
        let logo = match self.logo.unwrap_or(LogoRequest::Keep) {
            LogoRequest::Keep => LogoChange::Keep,
            LogoRequest::Remove => LogoChange::Remove,
            LogoRequest::Replace { image_base64 } => {
                let upload = STANDARD
                    .decode(image_base64)
                    .map_err(|_| ErrorResponse::invalid_logo())?;

                LogoChange::Replace(LogoNormaliser::normalise(upload).await?)
            }
        };

        Ok(SettingsUpdate {
            studio_name: self.studio_name,
            mark: self.mark,
            pattern: self.pattern,
            position: self.position,
            size: self.size,
            signature: self.signature,
            logo,
        })
    }
}

/// Registers the settings routes.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(
        web::scope("/settings")
            .app_data(
                JsonConfig::default()
                    .limit(BODY_LIMIT)
                    .error_handler(|error, _| ErrorResponse::from(error).into()),
            )
            .service(show_settings)
            .service(save_settings)
            .service(show_logo),
    );
}

#[get("")]
async fn show_settings(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
) -> Result<HttpResponse, ErrorResponse> {
    let settings = state
        .database()
        .repository::<WatermarkRepository>()
        .settings(identity.workspace_id)
        .await?;

    Ok(HttpResponse::Ok()
        .no_store()
        .json(SettingsView::from(settings)))
}

#[put("")]
async fn save_settings(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    request: Json<SettingsRequest>,
) -> Result<HttpResponse, ErrorResponse> {
    let update = request.into_inner().into_update().await?;
    let settings = state
        .database()
        .repository_with::<WatermarkRepository>(state.job_options())
        .save(identity.workspace_id, update)
        .await?;

    Ok(HttpResponse::Ok()
        .no_store()
        .json(SettingsView::from(settings)))
}

#[get("/logo")]
async fn show_logo(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
) -> Result<HttpResponse, ErrorResponse> {
    let logo = state
        .database()
        .repository::<WatermarkRepository>()
        .settings(identity.workspace_id)
        .await?
        .logo
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok()
        .no_store()
        .insert_header(ContentType::png())
        .insert_header((header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
        .body(logo))
}

#[cfg(test)]
mod tests {
    use actix_web::{App, cookie::Cookie, http::StatusCode, test};
    use libvips::ops::{black, pngsave_buffer};
    use serde_json::{Value, json};
    use sqlx::PgPool;

    use crate::{
        api::configure_routes,
        tests::{ORIGIN, app_state, call, sign_up},
        upload::jpeg,
    };

    fn request(method: test::TestRequest, cookie: &Cookie<'static>) -> test::TestRequest {
        method
            .uri("/api/v1/settings")
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone())
    }

    fn body(extra: Value) -> Value {
        let mut body = json!({
            "studio_name": "Studio",
            "mark": "name",
            "pattern": "repeat",
            "position": 8,
            "size": 2,
            "signature": "name",
        });

        body.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());

        body
    }

    fn logo_base64() -> String {
        use base64::{Engine as _, engine::general_purpose::STANDARD};

        jpeg::install().unwrap();

        STANDARD.encode(pngsave_buffer(&black(60, 30).unwrap()).unwrap())
    }

    macro_rules! signed_in_app {
        ($pool:expr, $app:ident, $cookie:ident) => {
            let (state, _directory) = app_state($pool).await;
            let $app = test::init_service(
                App::new()
                    .app_data(actix_web::web::Data::new(state))
                    .configure(configure_routes),
            )
            .await;
            let $cookie = sign_up(&$app).await;
        };
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn settings_default_then_save_and_read_back(pool: PgPool) {
        signed_in_app!(pool, app, cookie);

        let (status, defaults) = call(&app, request(test::TestRequest::get(), &cookie)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(defaults["studio_name"], "");
        assert_eq!(defaults["position"], 8);
        assert_eq!(defaults["has_logo"], false);

        let (status, saved) = call(
            &app,
            request(test::TestRequest::put(), &cookie).set_json(body(json!({
                "studio_name": "  Studio  ",
                "mark": "both",
                "pattern": "once",
                "position": 4,
                "size": 3,
                "signature": "logo",
                "logo": {"action": "replace", "image_base64": logo_base64()},
            }))),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(saved["studio_name"], "Studio");
        assert_eq!(saved["mark"], "both");
        assert_eq!(saved["pattern"], "once");
        assert_eq!(saved["position"], 4);
        assert_eq!(saved["has_logo"], true);

        let (_, read) = call(&app, request(test::TestRequest::get(), &cookie)).await;

        assert_eq!(read, saved);

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/settings/logo")
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("content-type").unwrap(), "image/png");
        assert_eq!(
            response.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");

        let (_, kept) = call(
            &app,
            request(test::TestRequest::put(), &cookie).set_json(body(json!({}))),
        )
        .await;

        assert_eq!(kept["has_logo"], true);

        let (_, removed) = call(
            &app,
            request(test::TestRequest::put(), &cookie).set_json(body(json!({
                "mark": "name",
                "signature": "name",
                "logo": {"action": "remove"},
            }))),
        )
        .await;

        assert_eq!(removed["has_logo"], false);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn invalid_settings_are_refused_with_their_field(pool: PgPool) {
        signed_in_app!(pool, app, cookie);

        for (extra, field) in [
            (json!({"studio_name": " "}), "studio_name"),
            (json!({"studio_name": "x".repeat(101)}), "studio_name"),
            (json!({"mark": "logo"}), "logo"),
            (json!({"signature": "both"}), "logo"),
        ] {
            let (status, error) = call(
                &app,
                request(test::TestRequest::put(), &cookie).set_json(body(extra)),
            )
            .await;

            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(error["context"][0], field);
        }

        for extra in [
            json!({"position": 9}),
            json!({"size": 5}),
            json!({"mark": "other"}),
            json!({"unknown": 1}),
        ] {
            let (status, _) = call(
                &app,
                request(test::TestRequest::put(), &cookie).set_json(body(extra)),
            )
            .await;

            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        }

        let (status, error) = call(
            &app,
            request(test::TestRequest::put(), &cookie).set_json(body(json!({
                "logo": {"action": "replace", "image_base64": "bm90IGEgcG5n"},
            }))),
        )
        .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(error["error"], "invalid logo image");
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn settings_need_a_session_and_the_logo_may_be_missing(pool: PgPool) {
        signed_in_app!(pool, app, cookie);

        let (status, _) = call(&app, test::TestRequest::get().uri("/api/v1/settings")).await;
        let (missing, _) = call(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/settings/logo")
                .cookie(cookie.clone()),
        )
        .await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(missing, StatusCode::NOT_FOUND);
    }
}
