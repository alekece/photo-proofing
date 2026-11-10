//! The same thread operations under a share link or an authenticated gallery. The extractor
//! chooses the author from credentials, never from input, and scopes every repository call.

use actix_web::{
    HttpResponse, routes,
    web::{Data, Json, Path, ServiceConfig},
};
use pp_database::{
    comments::{CommentBody, CommentId, CommentRepository},
    photo::PhotoId,
};
use serde::Deserialize;

use crate::{
    api::ApiState, error::ErrorResponse, extractors::comment_access::CommentAccess,
    response::ResponseBuilderExt,
};

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(counts)
        .service(thread)
        .service(add)
        .service(edit)
        .service(remove);
}

#[derive(Deserialize)]
struct PhotoPath {
    photo_id: PhotoId,
}

#[derive(Deserialize)]
struct CommentPath {
    comment_id: CommentId,
}

#[derive(Deserialize)]
struct CommentInput {
    body: CommentBody,
}

#[routes]
#[get("/shared/{token}/comments")]
#[get("/{id}/comments")]
async fn counts(
    state: Data<ApiState>,
    access: CommentAccess,
) -> Result<HttpResponse, ErrorResponse> {
    let counts = state
        .database()
        .repository::<CommentRepository>()
        .counts(access.location)
        .await?;

    Ok(HttpResponse::Ok().no_store().no_referrer().json(counts))
}

#[routes]
#[get("/shared/{token}/photos/{photo_id}/comments")]
#[get("/{id}/photos/{photo_id}/comments")]
async fn thread(
    state: Data<ApiState>,
    access: CommentAccess,
    path: Path<PhotoPath>,
) -> Result<HttpResponse, ErrorResponse> {
    let comments = state
        .database()
        .repository::<CommentRepository>()
        .thread(access.location, path.photo_id, access.author)
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok().no_store().no_referrer().json(comments))
}

#[routes]
#[post("/shared/{token}/photos/{photo_id}/comments")]
#[post("/{id}/photos/{photo_id}/comments")]
async fn add(
    state: Data<ApiState>,
    access: CommentAccess,
    path: Path<PhotoPath>,
    input: Json<CommentInput>,
) -> Result<HttpResponse, ErrorResponse> {
    let comment = state
        .database()
        .repository::<CommentRepository>()
        .add(
            access.location,
            path.photo_id,
            access.author,
            &input.body,
            state.comment_limit(),
        )
        .await?;
    let comment = ErrorResponse::applied_comment(comment)?;

    Ok(HttpResponse::Created()
        .no_store()
        .no_referrer()
        .json(comment))
}

#[routes]
#[patch("/shared/{token}/comments/{comment_id}")]
#[patch("/{id}/comments/{comment_id}")]
async fn edit(
    state: Data<ApiState>,
    access: CommentAccess,
    path: Path<CommentPath>,
    input: Json<CommentInput>,
) -> Result<HttpResponse, ErrorResponse> {
    let comment = state
        .database()
        .repository::<CommentRepository>()
        .edit(access.location, path.comment_id, access.author, &input.body)
        .await?;
    let comment = ErrorResponse::applied_comment(comment)?;

    Ok(HttpResponse::Ok().no_store().no_referrer().json(comment))
}

#[routes]
#[delete("/shared/{token}/comments/{comment_id}")]
#[delete("/{id}/comments/{comment_id}")]
async fn remove(
    state: Data<ApiState>,
    access: CommentAccess,
    path: Path<CommentPath>,
) -> Result<HttpResponse, ErrorResponse> {
    let change = state
        .database()
        .repository::<CommentRepository>()
        .delete(access.location, path.comment_id, access.author)
        .await?;

    ErrorResponse::applied_comment(change)?;

    Ok(HttpResponse::NoContent().no_store().no_referrer().finish())
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU16, time::Duration};

    use actix_web::{App, cookie::Cookie, http::StatusCode, test, web::Data};
    use pp_database::{
        Database,
        auth::WorkspaceId,
        gallery::{GalleryId, GalleryRepository},
        photo::PhotoId,
    };
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::{
        api::configure_routes,
        config::CommentsConfig,
        tests::{ORIGIN, app_state, call, sign_up},
    };

    async fn gallery(pool: &PgPool) -> (String, String, String) {
        let (workspace, id): (WorkspaceId, GalleryId) =
            sqlx::query_as("SELECT workspace_id,id FROM galleries LIMIT 1")
                .fetch_one(pool)
                .await
                .unwrap();
        let token = Database::new(pool.clone())
            .repository::<GalleryRepository>()
            .share(workspace, id)
            .await
            .unwrap()
            .unwrap()
            .share_token
            .unwrap()
            .as_str()
            .to_owned();
        let photo = PhotoId::from(Uuid::new_v4());

        sqlx::query(
            "INSERT INTO photos
                 (id, workspace_id, gallery_id, original_name, size, width, height, status, clean_ready)
             VALUES ($1, $2, $3, 'a.jpg', 1, 1, 1, 'ready', true)",
        )
        .bind(photo)
        .bind(workspace)
        .bind(id)
        .execute(pool)
        .await
        .unwrap();

        (id.0.to_string(), token, photo.to_string())
    }

    fn request(method: &str, path: &str, cookie: Option<&Cookie<'static>>) -> test::TestRequest {
        let mut req = test::TestRequest::default()
            .method(method.parse().unwrap())
            .uri(path)
            .insert_header(("Origin", ORIGIN));

        if let Some(cookie) = cookie {
            req = req.cookie(cookie.clone());
        }

        req
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_client_comments_and_reads_the_thread_and_bell(pool: PgPool) {
        let (state, _dir) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;

        call(&app, request("POST", "/api/v1/galleries", Some(&cookie))).await;
        let (_, token, photo) = gallery(&pool).await;
        let base = format!("/api/v1/galleries/shared/{token}");
        let path = format!("{base}/photos/{photo}/comments");
        let (status, comment) = call(
            &app,
            request("POST", &path, None).set_json(json!({"body":"Nice"})),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(comment["author"], "client");
        let response = test::call_service(&app, request("GET", &path, None).to_request()).await;

        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        assert_eq!(
            response.headers().get("referrer-policy").unwrap(),
            "no-referrer"
        );
        let thread: Value = test::read_body_json(response).await;

        assert_eq!(thread[0], comment);
        let (_, counts) = call(&app, request("GET", &format!("{base}/comments"), None)).await;

        assert_eq!(counts[0]["count"], 1);
        let (_, bell) = call(&app, request("GET", "/api/v1/notifications", Some(&cookie))).await;

        assert_eq!(bell["notifications"][0]["kind"], "comment");
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_deleted_comment_never_leaks_its_text_to_the_client(pool: PgPool) {
        let (state, _dir) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;

        call(&app, request("POST", "/api/v1/galleries", Some(&cookie))).await;
        let (id, token, photo) = gallery(&pool).await;

        sqlx::query(
            "INSERT INTO photo_comments (workspace_id, gallery_id, photo_id, author, body)
             SELECT workspace_id, id, $1, 'client', 'Secret' FROM galleries",
        )
        .bind(Uuid::parse_str(&photo).unwrap())
        .execute(&pool)
        .await
        .unwrap();
        let cid: Uuid = sqlx::query_scalar("SELECT id FROM photo_comments")
            .fetch_one(&pool)
            .await
            .unwrap();
        let base = format!("/api/v1/galleries/shared/{token}");

        assert_eq!(
            call(
                &app,
                request("DELETE", &format!("{base}/comments/{cid}"), None)
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
        let (_, client) = call(
            &app,
            request("GET", &format!("{base}/photos/{photo}/comments"), None),
        )
        .await;
        assert!(client[0]["body"].is_null());
        assert!(client[0]["deleted_at"].is_string());
        let (_, owner) = call(
            &app,
            request(
                "GET",
                &format!("/api/v1/galleries/{id}/photos/{photo}/comments"),
                Some(&cookie),
            ),
        )
        .await;
        assert_eq!(owner[0]["body"], "Secret");
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn delivered_galleries_close_every_client_comment_route(pool: PgPool) {
        let (state, _dir) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;

        call(&app, request("POST", "/api/v1/galleries", Some(&cookie))).await;
        let (id, token, photo) = gallery(&pool).await;

        sqlx::query("UPDATE galleries SET delivered_at=now()")
            .execute(&pool)
            .await
            .unwrap();
        let base = format!("/api/v1/galleries/shared/{token}");

        for (method, path) in [
            ("GET", format!("{base}/comments")),
            ("GET", format!("{base}/photos/{photo}/comments")),
            ("POST", format!("{base}/photos/{photo}/comments")),
            ("PATCH", format!("{base}/comments/{}", Uuid::nil())),
            ("DELETE", format!("{base}/comments/{}", Uuid::nil())),
        ] {
            let (status, body) = call(
                &app,
                request(method, &path, None).set_json(json!({"body":"Hi"})),
            )
            .await;
            assert_eq!(status, StatusCode::CONFLICT, "{path}");
            assert_eq!(body["code"], "gallery_delivered");
        }

        assert_eq!(
            call(
                &app,
                request(
                    "POST",
                    &format!("/api/v1/galleries/{id}/photos/{photo}/comments"),
                    Some(&cookie)
                )
                .set_json(json!({"body":"Reply"}))
            )
            .await
            .0,
            StatusCode::CREATED
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn clients_are_paced_and_a_full_thread_refuses_only_client_posts(pool: PgPool) {
        let (state, _dir) = app_state(pool.clone()).await;
        let config = CommentsConfig {
            min_interval: "1s".parse().unwrap(),
            max_per_photo: NonZeroU16::new(1).unwrap(),
        };
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state.with_comments(&config)))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;

        call(&app, request("POST", "/api/v1/galleries", Some(&cookie))).await;
        let (id, token, photo) = gallery(&pool).await;
        let path = format!("/api/v1/galleries/shared/{token}/photos/{photo}/comments");

        assert_eq!(
            call(
                &app,
                request("POST", &path, None).set_json(json!({"body":"Hi"}))
            )
            .await
            .0,
            StatusCode::CREATED
        );
        let response = test::call_service(
            &app,
            request("POST", &path, None)
                .set_json(json!({"body":"Hi"}))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(response.headers().contains_key("retry-after"));
        tokio::time::sleep(Duration::from_secs(1)).await;
        let (status, body) = call(
            &app,
            request("POST", &path, None).set_json(json!({"body":"Hi"})),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "thread_full");

        for _ in 0..2 {
            assert_eq!(
                call(
                    &app,
                    request(
                        "POST",
                        &format!("/api/v1/galleries/{id}/photos/{photo}/comments"),
                        Some(&cookie)
                    )
                    .set_json(json!({"body":"Reply"}))
                )
                .await
                .0,
                StatusCode::CREATED
            );
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn comments_refuse_bad_bodies_finals_unavailable_links_and_wrong_authors(pool: PgPool) {
        let (state, _dir) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;

        call(&app, request("POST", "/api/v1/galleries", Some(&cookie))).await;
        let (id, token, photo) = gallery(&pool).await;
        let owner = format!("/api/v1/galleries/{id}");
        let client = format!("/api/v1/galleries/shared/{token}");
        let path = format!("{owner}/photos/{photo}/comments");

        for body in [
            "".to_owned(),
            " ".to_owned(),
            "x".repeat(1001),
            "a\0b".to_owned(),
        ] {
            assert_eq!(
                call(
                    &app,
                    request("POST", &path, Some(&cookie)).set_json(json!({"body":body}))
                )
                .await
                .0,
                StatusCode::UNPROCESSABLE_ENTITY
            );
        }

        let (_, reply) = call(
            &app,
            request("POST", &path, Some(&cookie)).set_json(json!({"body":"Reply"})),
        )
        .await;
        let cid = reply["id"].as_str().unwrap();

        assert_eq!(
            call(
                &app,
                request("PATCH", &format!("{client}/comments/{cid}"), None)
                    .set_json(json!({"body":"Wrong"}))
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert_eq!(
            call(
                &app,
                request("DELETE", &format!("{client}/comments/{cid}"), None)
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(&app, request("GET", &path, None)).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            call(
                &app,
                request(
                    "GET",
                    &format!("/api/v1/galleries/{}/comments", Uuid::new_v4()),
                    Some(&cookie)
                )
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(
                &app,
                request("GET", "/api/v1/galleries/shared/invalid/comments", None)
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );

        sqlx::query("UPDATE photos SET kind='final'")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            call(&app, request("GET", &path, Some(&cookie))).await.0,
            StatusCode::NOT_FOUND
        );
        sqlx::query("UPDATE galleries SET archived_at=now()")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            call(&app, request("GET", &format!("{client}/comments"), None))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
}
