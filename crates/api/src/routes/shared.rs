//! Client-facing gallery endpoint under `/api/v1/shared`, authorized by the private link alone.
//!
//! - `GET /{token}` returns the title, layout and expiry of the gallery behind the link.
//!
//! No session is involved. Malformed, unknown and expired links are the same 404 (a malformed
//! token fails to deserialize into [`ShareToken`], so it never reaches the database). The link is a bearer credential: any access log
//! added later must redact `/shared/` paths.

use actix_web::{
    HttpResponse, get,
    web::{self, Data, Path, ServiceConfig},
};
use pp_database::gallery::ShareToken;

use crate::{api::ApiState, error::ErrorResponse, response::ResponseBuilderExt};

/// Registers the shared-gallery routes.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(web::scope("/shared").service(show_shared_gallery));
}

#[get("/{token}")]
async fn show_shared_gallery(
    state: Data<ApiState>,
    token: Path<ShareToken>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery = state
        .galleries()
        .find_shared(&token.into_inner())
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok().no_store().no_referrer().json(gallery))
}

#[cfg(test)]
mod tests {
    use actix_web::{App, http::StatusCode, test, web::Data};
    use pp_database::{
        Database,
        gallery::{GalleryChanges, GalleryRepository},
    };
    use serde_json::{Value, json};
    use sqlx::PgPool;

    use crate::{
        api::configure_routes,
        tests::{ORIGIN, app_state, call, other_workspace, sign_up},
    };

    async fn shared_gallery(pool: &PgPool, changes: Value) -> String {
        let workspace = other_workspace(pool).await;

        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let id = store.create(workspace).await.unwrap().id;
        let changes: GalleryChanges = serde_json::from_value(changes).unwrap();

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
        test::TestRequest::get().uri(&format!("/api/v1/shared/{token}"))
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn shared_page_returns_only_client_fields(pool: PgPool) {
        let token = shared_gallery(&pool, json!({"title": "Wedding", "layout": "masonry"})).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(app_state(pool)))
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
            json!({"title": "Wedding", "layout": "masonry", "expires_at": null})
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn unknown_malformed_and_expired_links_are_indistinguishable(pool: PgPool) {
        let expired = shared_gallery(&pool, json!({"expires_at": "2000-01-01T00:00:00Z"})).await;
        let live = shared_gallery(&pool, json!({})).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(app_state(pool)))
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
        let app = test::init_service(
            App::new()
                .app_data(Data::new(app_state(pool)))
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
}
