use actix_web::{HttpResponse, Responder, get};

/// Unauthenticated liveness probe; reveals nothing about configuration or dependencies.
#[get("/health")]
pub async fn report_health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}

#[cfg(test)]
mod tests {
    use actix_web::{App, http::StatusCode, test};

    use crate::configure_routes;

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

        assert_eq!(body, serde_json::json!({"status": "ok"}));
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
