//! The photographer's notifications about client activity, under `/api/v1/notifications`.
//!
//! - `GET /` lists the workspace's newest notifications (unread first) with the unread total.
//! - `POST /{id}/read` marks one read; another workspace's or an unknown id is a 404.
//! - `POST /galleries/{gallery_id}/read` marks the gallery's notification read, as opening the
//!   gallery does; it answers 204 even when there was nothing to read.
//! - `POST /read` marks every unread notification of the workspace read.
//! - `DELETE /{id}` deletes one notification, read or not; another workspace's or an unknown id
//!   is a 404. Later activity on its gallery opens a new unread notification.
//! - `DELETE /` deletes every notification of the workspace. The activity log is not touched.
//!
//! Read state belongs to the workspace, so every signed-in browser sees it. Changes arrive on the
//! event stream as `notification` events. Like every state-changing route, the `POST`s and
//! `DELETE`s must carry
//! the configured `Origin`.

use actix_web::{
    HttpResponse, delete, get, post,
    web::{self, Data, Path, ServiceConfig},
};
use pp_database::{
    gallery::GalleryId,
    notifications::{NotificationId, NotificationRepository},
};

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
            .service(read_gallery)
            .service(read_one)
            .service(delete_all)
            .service(delete_one),
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

#[post("/galleries/{gallery_id}/read")]
async fn read_gallery(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    gallery_id: Path<GalleryId>,
) -> Result<HttpResponse, ErrorResponse> {
    state
        .database()
        .repository::<NotificationRepository>()
        .mark_gallery_read(identity.workspace_id, gallery_id.into_inner())
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

#[delete("")]
async fn delete_all(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
) -> Result<HttpResponse, ErrorResponse> {
    state
        .database()
        .repository::<NotificationRepository>()
        .delete_all(identity.workspace_id)
        .await?;

    Ok(HttpResponse::NoContent().finish())
}

#[delete("/{id}")]
async fn delete_one(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: Path<NotificationId>,
) -> Result<HttpResponse, ErrorResponse> {
    let found = state
        .database()
        .repository::<NotificationRepository>()
        .delete(identity.workspace_id, id.into_inner())
        .await?;

    if !found {
        return Err(ErrorResponse::not_found());
    }

    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU16;

    use actix_web::{App, http::StatusCode, test, web::Data};
    use pp_database::{
        Database,
        activity::{ActivityOptions, Client},
        auth::WorkspaceId,
        comments::{CommentAuthor, CommentBody, CommentChange, CommentRepository},
        gallery::GalleryId,
        photo::{PhotoId, PhotoKind, PhotoLocation, PhotoRepository, Watermarked},
    };
    use serde_json::Value;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::{
        api::configure_routes,
        tests::{ORIGIN, app_state, call, other_workspace, sign_up},
    };

    /// The client behind the general link, used to record activity the bell derives from.
    const CLIENT: Option<Client> = Some(Client { contact_id: None });

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

    /// The gallery a `POST /api/v1/galleries` response named, resolved to its location.
    async fn location_of(pool: &PgPool, gallery: &Value) -> PhotoLocation {
        let gallery_id = GalleryId(gallery["id"].as_str().unwrap().parse().unwrap());
        let workspace_id: WorkspaceId =
            sqlx::query_scalar("SELECT workspace_id FROM galleries WHERE id = $1")
                .bind(gallery_id)
                .fetch_one(pool)
                .await
                .unwrap();

        PhotoLocation {
            workspace_id,
            gallery_id,
        }
    }

    /// Inserts a ready proof photo at `location` and favorites it as a client, recording the
    /// activity the bell's `selection` notification derives from.
    async fn favorite_a_ready_photo(pool: &PgPool, location: PhotoLocation) {
        let photo_id = PhotoId::from(Uuid::new_v4());

        sqlx::query(
            "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width,
                                  height, status, kind, clean_ready)
             VALUES ($1, $2, $3, 'a.jpg', 1, 1, 1, 'ready', 'proof', true)",
        )
        .bind(photo_id)
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .execute(pool)
        .await
        .unwrap();

        Database::new(pool.clone())
            .repository_with::<PhotoRepository>(ActivityOptions::default().client(CLIENT))
            .set_favorite(
                location,
                photo_id,
                true,
                PhotoKind::Proof,
                Watermarked(false),
            )
            .await
            .unwrap();
    }

    /// Inserts a ready proof photo at `location` and comments on it as a client, recording the
    /// activity the bell's `comment` notification derives from.
    async fn comment_on_a_ready_photo(pool: &PgPool, location: PhotoLocation) {
        let photo_id = PhotoId::from(Uuid::new_v4());

        sqlx::query(
            "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width,
                                  height, status, kind, clean_ready)
             VALUES ($1, $2, $3, 'a.jpg', 1, 1, 1, 'ready', 'proof', true)",
        )
        .bind(photo_id)
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .execute(pool)
        .await
        .unwrap();

        let comment = Database::new(pool.clone())
            .repository_with::<CommentRepository>(ActivityOptions::default().client(CLIENT))
            .add(
                location,
                photo_id,
                CommentAuthor::Client,
                &CommentBody::try_from("Nice".to_owned()).unwrap(),
                NonZeroU16::new(100).unwrap(),
            )
            .await
            .unwrap();

        assert!(matches!(comment, CommentChange::Applied(_)));
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

        let (_, gallery) = call(
            &app,
            request(test::TestRequest::post(), "/api/v1/galleries", &cookie),
        )
        .await;
        let location = location_of(&pool, &gallery).await;

        comment_on_a_ready_photo(&pool, location).await;

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
        let location = location_of(&pool, &gallery).await;

        favorite_a_ready_photo(&pool, location).await;

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

    #[sqlx::test(migrations = "../database/migrations")]
    async fn notifications_are_deleted_one_or_all(pool: PgPool) {
        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let (_, gallery) = call(
            &app,
            request(test::TestRequest::post(), "/api/v1/galleries", &cookie),
        )
        .await;
        let location = location_of(&pool, &gallery).await;

        favorite_a_ready_photo(&pool, location).await;

        let (_, list) = call(
            &app,
            request(test::TestRequest::get(), "/api/v1/notifications", &cookie),
        )
        .await;
        let id = list["notifications"][0]["id"].as_str().unwrap().to_owned();
        let uri = format!("/api/v1/notifications/{id}");

        let (status, _) = call(
            &app,
            test::TestRequest::delete().uri(&uri).cookie(cookie.clone()),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);

        let (status, _) = call(&app, request(test::TestRequest::delete(), &uri, &cookie)).await;

        assert_eq!(status, StatusCode::NO_CONTENT);

        let (_, list) = call(
            &app,
            request(test::TestRequest::get(), "/api/v1/notifications", &cookie),
        )
        .await;

        assert_eq!(list["unread"], 0);
        assert!(list["notifications"].as_array().unwrap().is_empty());

        let (status, _) = call(&app, request(test::TestRequest::delete(), &uri, &cookie)).await;

        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _) = call(
            &app,
            request(
                test::TestRequest::delete(),
                "/api/v1/notifications",
                &cookie,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn reading_a_gallery_marks_only_its_own_notification(pool: PgPool) {
        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(actix_web::web::Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let mut galleries = Vec::new();
        let mut locations = Vec::new();

        for _ in 0..2 {
            let (_, gallery) = call(
                &app,
                request(test::TestRequest::post(), "/api/v1/galleries", &cookie),
            )
            .await;

            locations.push(location_of(&pool, &gallery).await);
            galleries.push(gallery["id"].as_str().unwrap().to_owned());
        }

        let foreign_workspace = other_workspace(&pool).await;
        let foreign: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO galleries (id, workspace_id) VALUES (gen_random_uuid(), $1) RETURNING id",
        )
        .bind(foreign_workspace)
        .fetch_one(&pool)
        .await
        .unwrap();

        for location in &locations {
            favorite_a_ready_photo(&pool, *location).await;
        }

        favorite_a_ready_photo(
            &pool,
            PhotoLocation {
                workspace_id: foreign_workspace,
                gallery_id: GalleryId(foreign),
            },
        )
        .await;

        let read_gallery = |gallery: String| {
            request(
                test::TestRequest::post(),
                &format!("/api/v1/notifications/galleries/{gallery}/read"),
                &cookie,
            )
        };

        let (status, _) = call(&app, read_gallery(galleries[0].clone())).await;

        assert_eq!(status, StatusCode::NO_CONTENT);

        let (_, list) = call(
            &app,
            request(test::TestRequest::get(), "/api/v1/notifications", &cookie),
        )
        .await;
        let notifications = list["notifications"].as_array().unwrap();
        let read_of = |gallery: &str| {
            notifications
                .iter()
                .find(|notification| notification["gallery_id"] == gallery)
                .unwrap()["read_at"]
                .is_string()
        };

        assert_eq!(list["unread"], 1);
        assert!(read_of(&galleries[0]));
        assert!(!read_of(&galleries[1]));

        let (status, _) = call(&app, read_gallery(galleries[0].clone())).await;

        assert_eq!(status, StatusCode::NO_CONTENT);

        let (status, _) = call(&app, read_gallery(foreign.to_string())).await;

        assert_eq!(status, StatusCode::NO_CONTENT);

        let foreign_unread: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM notifications WHERE gallery_id = $1 AND read_at IS NULL",
        )
        .bind(foreign)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(foreign_unread, 1);

        let without_origin = test::TestRequest::post()
            .uri(&format!(
                "/api/v1/notifications/galleries/{}/read",
                galleries[1]
            ))
            .cookie(cookie.clone());

        assert_eq!(call(&app, without_origin).await.0, StatusCode::FORBIDDEN);

        let anonymous = test::TestRequest::post()
            .uri(&format!(
                "/api/v1/notifications/galleries/{}/read",
                galleries[1]
            ))
            .insert_header(("Origin", ORIGIN));

        assert_eq!(call(&app, anonymous).await.0, StatusCode::UNAUTHORIZED);
    }
}
