//! `GET /language`: the installation's language, public so client pages and the unavailable page
//! can follow it. `null` until setup created the workspace.

use actix_web::{HttpResponse, get, web::Data};
use pp_database::settings::SettingsRepository;
use serde::{Deserialize, Serialize};

use crate::{api::ApiState, error::ErrorResponse, response::ResponseBuilderExt};

/// `{"language": …}`, read by `GET /language` and sent to and returned by
/// `PUT /settings/language`.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LanguageBody<L> {
    pub language: L,
}

#[get("/language")]
pub async fn show_language(state: Data<ApiState>) -> Result<HttpResponse, ErrorResponse> {
    let language = state
        .database()
        .repository::<SettingsRepository>()
        .installation_language()
        .await?;

    Ok(HttpResponse::Ok()
        .no_store()
        .json(LanguageBody { language }))
}

#[cfg(test)]
mod tests {
    use actix_web::{App, http::StatusCode, test, web::Data};
    use serde_json::json;
    use sqlx::PgPool;

    use crate::{
        api::configure_routes,
        tests::{ORIGIN, app_state, call, sign_up},
    };

    #[sqlx::test(migrations = "../database/migrations")]
    async fn language_is_public_and_null_before_setup(pool: PgPool) {
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let (status, body) = call(&app, test::TestRequest::get().uri("/api/v1/language")).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "language": null }));
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn language_follows_the_saved_setting(pool: PgPool) {
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let read = || test::TestRequest::get().uri("/api/v1/language");

        assert_eq!(call(&app, read()).await.1, json!({ "language": "en" }));

        let (status, saved) = call(
            &app,
            test::TestRequest::put()
                .uri("/api/v1/settings/language")
                .insert_header(("Origin", ORIGIN))
                .cookie(cookie)
                .set_json(json!({ "language": "fr" })),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(saved, json!({ "language": "fr" }));
        assert_eq!(call(&app, read()).await.1, json!({ "language": "fr" }));
    }
}
