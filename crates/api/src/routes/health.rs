use actix_web::{HttpResponse, Responder, get};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum HealthStatus {
    Ok,
}

#[derive(Serialize)]
struct Health {
    status: HealthStatus,
    /// Commit the image was built from; `null` for local builds.
    commit: Option<String>,
}

/// Unauthenticated liveness probe; besides the public build commit, it reveals nothing about
/// configuration or dependencies.
#[get("/health")]
pub async fn report_health() -> impl Responder {
    HttpResponse::Ok().json(Health {
        status: HealthStatus::Ok,
        commit: std::env::var("PP_BUILD_SHA")
            .ok()
            .filter(|sha| !sha.is_empty()),
    })
}

#[cfg(test)]
mod tests {
    use actix_web::{App, http::StatusCode, test};

    use crate::api::configure_routes;

    #[actix_web::test]
    async fn health_returns_ok_json() {
        let app = test::init_service(App::new().configure(configure_routes)).await;
        let res = test::call_service(
            &app,
            test::TestRequest::get().uri("/api/v1/health").to_request(),
        )
        .await;

        assert_eq!(res.status(), StatusCode::OK);

        let body: serde_json::Value = test::read_body_json(res).await;

        assert_eq!(body["status"], "ok");
        assert!(body.get("commit").is_some());
    }

    #[actix_web::test]
    async fn unknown_path_returns_404() {
        let app = test::init_service(App::new().configure(configure_routes)).await;
        let res = test::call_service(
            &app,
            test::TestRequest::get().uri("/api/v1/nope").to_request(),
        )
        .await;

        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
