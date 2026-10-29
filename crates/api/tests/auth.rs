use std::iter;

use actix_web::{App, http::StatusCode, test, web};
use pp_api::{configure_routes, state::AppState};
use pp_core::config::{AuthConfig, SessionConfig};
use pp_database::auth::AuthStore;
use sqlx::PgPool;

#[expect(clippy::unwrap_used, reason = "fixture values are constants")]
fn app_state(pool: PgPool) -> AppState {
    let session = SessionConfig {
        idle_timeout: "1h".parse().unwrap(),
    };
    let auth = AuthConfig {
        app_origin: "http://localhost:5173".parse().unwrap(),
    };

    AppState::new(AuthStore::new(pool), session, auth).unwrap()
}

#[sqlx::test(migrations = "../database/migrations")]
async fn login_cookie_logout_and_copied_token_denial(pool: PgPool) {
    let state = app_state(pool);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(configure_routes),
    )
    .await;

    let request = test::TestRequest::post()
        .uri("/api/v1/auth/setup")
        .insert_header(("Origin", "http://localhost:5173"))
        .set_json(
            serde_json::json!({"email":" Person@Example.test ","password":"a long test password"}),
        )
        .to_request();
    let response = test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::OK);

    let cookie = response
        .response()
        .cookies()
        .find(|c| c.name() == "pp_session")
        .unwrap()
        .into_owned();

    assert_eq!(cookie.http_only(), Some(true));
    assert_eq!(cookie.path(), Some("/"));
    assert!(cookie.max_age().unwrap().whole_seconds() > 0);

    let request = test::TestRequest::post()
        .uri("/api/v1/auth/renew")
        .insert_header(("Origin", "http://localhost:5173"))
        .cookie(cookie.clone())
        .to_request();

    assert_eq!(
        test::call_service(&app, request).await.status(),
        StatusCode::NO_CONTENT
    );

    let request = test::TestRequest::post()
        .uri("/api/v1/auth/logout")
        .insert_header(("Origin", "http://localhost:5173"))
        .cookie(cookie.clone())
        .to_request();

    assert_eq!(
        test::call_service(&app, request).await.status(),
        StatusCode::NO_CONTENT
    );

    let request = test::TestRequest::post()
        .uri("/api/v1/auth/renew")
        .insert_header(("Origin", "http://localhost:5173"))
        .cookie(cookie)
        .to_request();

    assert_eq!(
        test::call_service(&app, request).await.status(),
        StatusCode::UNAUTHORIZED
    );

    let request = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .insert_header(("Origin", "http://localhost:5173"))
        .set_json(
            serde_json::json!({"email":"person@example.test","password":"a long test password"}),
        )
        .to_request();

    assert_eq!(
        test::call_service(&app, request).await.status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../database/migrations")]
async fn origin_and_missing_session_fail_closed(pool: PgPool) {
    let state = app_state(pool);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(configure_routes),
    )
    .await;

    for origin in [None, Some("null"), Some("http://evil.test")] {
        let mut req = test::TestRequest::post().uri("/api/v1/auth/logout");

        if let Some(value) = origin {
            req = req.insert_header(("Origin", value));
        }

        assert_eq!(
            test::call_service(&app, req.to_request()).await.status(),
            StatusCode::FORBIDDEN
        );
    }

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/renew")
        .insert_header(("Origin", "http://localhost:5173"))
        .to_request();

    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../database/migrations")]
async fn passive_status_does_not_renew_and_expired_sessions_fail(pool: PgPool) {
    let state = app_state(pool.clone());
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(configure_routes),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/setup")
        .insert_header(("Origin", "http://localhost:5173"))
        .set_json(serde_json::json!({"email":"a@example.test","password":"a long test password"}))
        .to_request();
    let res = test::call_service(&app, req).await;
    let cookie = res.response().cookies().next().unwrap().into_owned();
    let before: time::OffsetDateTime = sqlx::query_scalar("SELECT expires_at FROM sessions")
        .fetch_one(&pool)
        .await
        .unwrap();

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/status")
        .cookie(cookie.clone())
        .to_request();
    let res = test::call_service(&app, req).await;

    assert_eq!(res.headers().get("Cache-Control").unwrap(), "no-store");
    assert!(res.response().cookies().next().is_none());

    let json: serde_json::Value = test::read_body_json(res).await;

    assert!(json["identity"].is_object());
    assert_eq!(json["idle_timeout_ms"], 3_600_000);
    assert!(!json.to_string().contains(cookie.value()));

    let after: time::OffsetDateTime = sqlx::query_scalar("SELECT expires_at FROM sessions")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(before, after);

    sqlx::query("UPDATE sessions SET expires_at=now()-interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/renew")
        .insert_header(("Origin", "http://localhost:5173"))
        .cookie(cookie)
        .to_request();

    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNAUTHORIZED
    );

    pool.close().await;

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/status")
        .to_request();

    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[sqlx::test(migrations = "../database/migrations")]
async fn requests_have_bounded_bodies_and_attempts(pool: PgPool) {
    let state = app_state(pool);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(configure_routes),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .insert_header(("Origin", "http://localhost:5173"))
        .insert_header(("Content-Type", "text/plain"))
        .set_payload("secret")
        .to_request();

    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .insert_header(("Origin", "http://localhost:5173"))
        .set_json(serde_json::json!({"email":"a@example.test","password":"x".repeat(5000)}))
        .to_request();

    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );

    for attempt in 0..11 {
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .insert_header(("Origin", "http://localhost:5173"))
            .set_json(serde_json::json!({"email":"invalid","password":"short"}))
            .to_request();
        let expected = if attempt < 10 {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::TOO_MANY_REQUESTS
        };

        assert_eq!(test::call_service(&app, req).await.status(), expected);
    }
}

#[sqlx::test(migrations = "../database/migrations")]
async fn successful_logins_do_not_use_the_attempt_limit(pool: PgPool) {
    let state = app_state(pool);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(configure_routes),
    )
    .await;
    let credentials =
        serde_json::json!({"email":"a@example.test","password":"correct horse battery"});

    for action in iter::once("setup").chain(iter::repeat_n("login", 11)) {
        let req = test::TestRequest::post()
            .uri(&format!("/api/v1/auth/{action}"))
            .insert_header(("Origin", "http://localhost:5173"))
            .set_json(&credentials)
            .to_request();

        assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
    }
}
