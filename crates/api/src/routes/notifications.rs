//! The photographer's notifications about client activity, under `/api/v1/notifications`.
//!
//! - `GET /` lists the workspace's newest notifications (unread first) with the unread total.
//! - `POST /{id}/read` marks one read; another workspace's or an unknown id is a 404.
//! - `POST /read` marks every unread notification of the workspace read.
//!
//! Read state belongs to the workspace, so every signed-in browser sees it. Changes arrive on the
//! event stream as `notification` events. Like every state-changing route, the `POST`s must carry
//! the configured `Origin`.

use actix_web::{
    HttpResponse, get, post,
    web::{self, Data, Path, ServiceConfig},
};
use pp_database::notifications::{NotificationId, NotificationRepository};

use crate::{
    api::ApiState, error::ErrorResponse, extractors::identity::Authenticated,
    response::ResponseBuilderExt,
};

/// Registers the notification routes.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(
        web::scope("/notifications")
            .service(list_notifications)
            .service(read_all)
            .service(read_one),
    );
}

#[get("")]
async fn list_notifications(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
) -> Result<HttpResponse, ErrorResponse> {
    let list = state
        .database()
        .repository::<NotificationRepository>()
        .list(identity.workspace_id)
        .await?;

    Ok(HttpResponse::Ok().no_store().json(list))
}

#[post("/read")]
async fn read_all(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
) -> Result<HttpResponse, ErrorResponse> {
    state
        .database()
        .repository::<NotificationRepository>()
        .mark_all_read(identity.workspace_id)
        .await?;

    Ok(HttpResponse::NoContent().finish())
}

#[post("/{id}/read")]
async fn read_one(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: Path<NotificationId>,
) -> Result<HttpResponse, ErrorResponse> {
    let found = state
        .database()
        .repository::<NotificationRepository>()
        .mark_read(identity.workspace_id, id.into_inner())
        .await?;

    if !found {
        return Err(ErrorResponse::not_found());
    }

    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use actix_web::{App, http::StatusCode, test, web::Data};
    use sqlx::PgPool;

    use crate::{
        api::configure_routes,
        tests::{ORIGIN, app_state, call, sign_up},
    };

    fn request(
        method: test::TestRequest,
        uri: &str,
        cookie: &actix_web::cookie::Cookie<'static>,
    ) -> test::TestRequest {
        method
            .uri(uri)
            .cookie(cookie.clone())
            .insert_header(("Origin", ORIGIN))
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn client_comments_are_listed_with_their_count(pool: PgPool) {
        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        call(
            &app,
            request(test::TestRequest::post(), "/api/v1/galleries", &cookie),
        )
        .await;
        sqlx::query(
            "WITH photo AS (
            INSERT INTO photos (id,workspace_id,gallery_id,original_name,size,width,height)
            SELECT gen_random_uuid(),workspace_id,id,'a.jpg',1,1,1 FROM galleries RETURNING *
        ) INSERT INTO photo_comments (workspace_id,gallery_id,photo_id,author,body)
          SELECT workspace_id,gallery_id,id,'client','Nice' FROM photo",
        )
        .execute(&pool)
        .await
        .unwrap();

        let (status, list) = call(
            &app,
            request(test::TestRequest::get(), "/api/v1/notifications", &cookie),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(list["notifications"][0]["kind"], "comment");
        assert_eq!(list["notifications"][0]["count"], 1);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn activity_is_listed_and_read_for_the_workspace(pool: PgPool) {
        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;

        let (status, _) = call(&app, test::TestRequest::get().uri("/api/v1/notifications")).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (_, gallery) = call(
            &app,
            request(test::TestRequest::post(), "/api/v1/galleries", &cookie),
        )
        .await;

        sqlx::query(
            "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height,
                                 status, favorited_at)
             SELECT gen_random_uuid(), workspace_id, id, 'a.jpg', 1, 1, 1, 'ready', NULL
             FROM galleries",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE photos SET favorited_at = now()")
            .execute(&pool)
            .await
            .unwrap();

        let (status, list) = call(
            &app,
            request(test::TestRequest::get(), "/api/v1/notifications", &cookie),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(list["unread"], 1);
        assert_eq!(list["notifications"][0]["gallery_id"], gallery["id"]);
        assert_eq!(list["notifications"][0]["kind"], "selection");

        let id = list["notifications"][0]["id"].as_str().unwrap().to_owned();
        let (status, _) = call(
            &app,
            request(
                test::TestRequest::post(),
                &format!("/api/v1/notifications/{id}/read"),
                &cookie,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::NO_CONTENT);

        let (_, list) = call(
            &app,
            request(test::TestRequest::get(), "/api/v1/notifications", &cookie),
        )
        .await;

        assert_eq!(list["unread"], 0);
        assert!(list["notifications"][0]["read_at"].is_string());

        let (status, _) = call(
            &app,
            request(
                test::TestRequest::post(),
                "/api/v1/notifications/00000000-0000-0000-0000-000000000000/read",
                &cookie,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _) = call(
            &app,
            request(
                test::TestRequest::post(),
                "/api/v1/notifications/read",
                &cookie,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::NO_CONTENT);
    }
}
