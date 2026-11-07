//! The photographer's ZIP of a gallery's selection.
//!
//! `POST /api/v1/galleries/{id}/export` stores the selection as it is now and queues
//! the build; `GET` on the same path reports the newest export and `GET
//! .../{export_id}/download` streams a finished ZIP as an attachment. All three need the
//! photographer's session and the gallery's workspace: another workspace, an unknown export or an
//! export past its retention is a plain 404. Progress arrives on the event stream as `export`
//! events.

use actix_web::{
    HttpResponse,
    body::SizedStream,
    get,
    http::header::{self, ContentDisposition, DispositionParam, DispositionType},
    post,
    web::{Data, Path as WebPath, ServiceConfig},
};
use pp_core::types::timestamp::Timestamp;
use pp_database::{
    export::{Export, ExportId, ExportRepository, ExportRequest, ExportStatus},
    gallery::GalleryId,
};
use pp_storage::{ArchiveKey, StorageError};
use serde::Serialize;
use tokio_util::io::ReaderStream;

use crate::{
    api::ApiState, error::ErrorResponse, extractors::identity::Authenticated,
    response::ResponseBuilderExt,
};

/// The name the browser saves the ZIP under, the same for every gallery.
const DOWNLOAD_NAME: &str = "selection.zip";

/// An export as the photographer's UI receives it.
#[derive(Debug, Serialize)]
struct ExportResponse {
    id: ExportId,
    status: ExportStatus,
    photo_count: u32,
    created_at: Timestamp,
}

impl From<Export> for ExportResponse {
    fn from(export: Export) -> Self {
        Self {
            id: export.id,
            status: export.status,
            photo_count: export.photo_count,
            created_at: export.created_at,
        }
    }
}

/// Registers the export routes inside the `/api/v1/galleries` scope.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(request_export)
        .service(show_export)
        .service(download_export);
}

#[post("/{id}/export")]
async fn request_export(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: WebPath<GalleryId>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery_id = id.into_inner();
    let outcome = state
        .database()
        .repository_with::<ExportRepository>(state.job_options())
        .request(identity.workspace_id, gallery_id, state.export_max_size())
        .await?;

    match outcome {
        ExportRequest::Created { export, replaced } => {
            for previous in replaced {
                let key = ArchiveKey {
                    workspace: identity.workspace_id,
                    gallery: gallery_id,
                    export: previous,
                };

                if let Err(error) = state.storage().remove_archive(&key).await {
                    tracing::warn!(%error, "replaced selection ZIP could not be removed");
                }
            }

            Ok(HttpResponse::Accepted()
                .no_store()
                .json(ExportResponse::from(export)))
        }
        ExportRequest::InProgress(export) => Ok(HttpResponse::Ok()
            .no_store()
            .json(ExportResponse::from(export))),
        ExportRequest::EmptySelection => Err(ErrorResponse::empty_selection()),
        ExportRequest::TooLarge => Err(ErrorResponse::selection_too_large()),
        ExportRequest::MissingGallery => Err(ErrorResponse::not_found()),
    }
}

/// The newest export, or `204` when there is none or it expired.
#[get("/{id}/export")]
async fn show_export(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: WebPath<GalleryId>,
) -> Result<HttpResponse, ErrorResponse> {
    let export = state
        .database()
        .repository::<ExportRepository>()
        .latest(
            identity.workspace_id,
            id.into_inner(),
            state.export_retention(),
        )
        .await?;

    Ok(match export {
        Some(export) => HttpResponse::Ok()
            .no_store()
            .json(ExportResponse::from(export)),
        None => HttpResponse::NoContent().no_store().finish(),
    })
}

#[get("/{id}/export/{export_id}/download")]
async fn download_export(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    path: WebPath<(GalleryId, ExportId)>,
) -> Result<HttpResponse, ErrorResponse> {
    let (gallery_id, export_id) = path.into_inner();
    let ready = state
        .database()
        .repository::<ExportRepository>()
        .ready(
            identity.workspace_id,
            gallery_id,
            export_id,
            state.export_retention(),
        )
        .await?;

    if ready.is_none() {
        return Err(ErrorResponse::not_found());
    }

    let key = ArchiveKey {
        workspace: identity.workspace_id,
        gallery: gallery_id,
        export: export_id,
    };
    let file = match state.storage().read_archive(&key).await {
        Ok(file) => file,
        Err(StorageError::NotFound) => return Err(ErrorResponse::not_found()),
        Err(error) => return Err(error.into()),
    };
    let length = file
        .metadata()
        .await
        .map_err(|error| ErrorResponse::from(StorageError::from(error)))?
        .len();

    Ok(HttpResponse::Ok()
        .content_type("application/zip")
        .insert_header(ContentDisposition {
            disposition: DispositionType::Attachment,
            parameters: vec![DispositionParam::Filename(DOWNLOAD_NAME.to_owned())],
        })
        .insert_header((header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
        .insert_header((header::CACHE_CONTROL, "private, no-store"))
        .no_referrer()
        .body(SizedStream::new(length, ReaderStream::new(file))))
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test};
    use pp_core::{config::PhotoDir, notifications::PhotoId, types::generate::Generate};
    use pp_database::photo::PhotoLocation;
    use pp_database::{auth::WorkspaceId, gallery::GalleryId};
    use pp_storage::{ArchiveEntry, PhotoKey, PhotoStorage, Variant};
    use sqlx::PgPool;
    use uuid::Uuid;

    use super::*;
    use crate::{
        routes::galleries::photos::tests::create_gallery,
        tests::{ORIGIN, app, call, other_workspace, sign_up, upload_config},
    };

    async fn favorite(pool: &PgPool, gallery_id: &str, name: &str) -> PhotoId {
        let id = PhotoId::generate();

        sqlx::query(
            "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height,
                                 status, favorite)
             SELECT $1, workspace_id, id, $3, 5, 2, 2, 'ready', true FROM galleries WHERE id = $2",
        )
        .bind(id)
        .bind(Uuid::parse_str(gallery_id).unwrap())
        .bind(name)
        .execute(pool)
        .await
        .unwrap();

        id
    }

    fn post(gallery_id: &str, cookie: &actix_web::cookie::Cookie<'static>) -> test::TestRequest {
        test::TestRequest::post()
            .uri(&format!("/api/v1/galleries/{gallery_id}/export"))
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone())
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn requests_need_the_photographer_session_and_a_selection(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let anonymous = test::TestRequest::post()
            .uri(&format!("/api/v1/galleries/{gallery_id}/export"))
            .insert_header(("Origin", ORIGIN));
        let foreign = format!("/api/v1/galleries/{}/export", GalleryId::generate().0);

        assert_eq!(call(&app, anonymous).await.0, StatusCode::UNAUTHORIZED);
        assert_eq!(
            call(&app, post(&gallery_id, &cookie)).await.0,
            StatusCode::CONFLICT
        );

        let unknown = test::TestRequest::post()
            .uri(&foreign)
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone());

        assert_eq!(call(&app, unknown).await.0, StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_request_is_accepted_once_and_then_reported_as_in_progress(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;

        favorite(&pool, &gallery_id, "a.jpg").await;

        let (status, first) = call(&app, post(&gallery_id, &cookie)).await;
        let (again, second) = call(&app, post(&gallery_id, &cookie)).await;
        let shown = test::TestRequest::get()
            .uri(&format!("/api/v1/galleries/{gallery_id}/export"))
            .cookie(cookie.clone());
        let (shown_status, latest) = call(&app, shown).await;

        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(first["status"]["kind"], "preparing");
        assert_eq!(first["photo_count"], 1);
        assert_eq!(again, StatusCode::OK);
        assert_eq!(second["id"], first["id"]);
        assert_eq!(shown_status, StatusCode::OK);
        assert_eq!(latest["id"], first["id"]);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn with_no_export_the_status_is_no_content(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool, directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let shown = test::TestRequest::get()
            .uri(&format!("/api/v1/galleries/{gallery_id}/export"))
            .cookie(cookie);

        assert_eq!(call(&app, shown).await.0, StatusCode::NO_CONTENT);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_ready_zip_downloads_as_a_private_attachment(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = favorite(&pool, &gallery_id, "../a.jpg").await;
        let (_, export) = call(&app, post(&gallery_id, &cookie)).await;
        let export_id = export["id"].as_str().unwrap();
        let location = PhotoLocation {
            workspace_id: sqlx::query_scalar("SELECT workspace_id FROM galleries WHERE id = $1")
                .bind(Uuid::parse_str(&gallery_id).unwrap())
                .fetch_one(&pool)
                .await
                .unwrap(),
            gallery_id: GalleryId(Uuid::parse_str(&gallery_id).unwrap()),
        };
        let storage = PhotoStorage::open(&PhotoDir::try_from(directory.path().to_owned()).unwrap())
            .await
            .unwrap();
        let source = PhotoKey::with_location(location, photo, Variant::Original);
        let mut staged = storage.stage().await.unwrap();

        staged.write(b"original").await.unwrap();
        staged.commit(&source).await.unwrap();

        let key = ArchiveKey {
            workspace: location.workspace_id,
            gallery: location.gallery_id,
            export: ExportId(Uuid::parse_str(export_id).unwrap()),
        };

        storage
            .build_archive(
                &key,
                vec![ArchiveEntry {
                    source,
                    original_name: "../a.jpg".to_owned(),
                }],
            )
            .await
            .unwrap();

        let uri = format!("/api/v1/galleries/{gallery_id}/export/{export_id}/download");
        let preparing = test::TestRequest::get().uri(&uri).cookie(cookie.clone());

        assert_eq!(call(&app, preparing).await.0, StatusCode::NOT_FOUND);

        sqlx::query("UPDATE exports SET status = '{\"kind\": \"ready\"}', finished_at = now()")
            .execute(&pool)
            .await
            .unwrap();

        let anonymous = test::TestRequest::get().uri(&uri);

        assert_eq!(call(&app, anonymous).await.0, StatusCode::UNAUTHORIZED);

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&uri)
                .cookie(cookie)
                .to_request(),
        )
        .await;
        let headers = response.headers().clone();
        let body = test::read_body(response).await;

        assert_eq!(headers.get("content-type").unwrap(), "application/zip");
        assert_eq!(
            headers.get("content-disposition").unwrap(),
            "attachment; filename=\"selection.zip\""
        );
        assert_eq!(headers.get("cache-control").unwrap(), "private, no-store");
        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        assert!(body.starts_with(b"PK"));
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn another_workspaces_export_is_not_found(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let other: WorkspaceId = other_workspace(&pool).await;
        let foreign_gallery = GalleryId::generate();
        let foreign_export = ExportId::generate();

        sqlx::query("INSERT INTO galleries (id, workspace_id) VALUES ($1, $2)")
            .bind(foreign_gallery)
            .bind(other)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO exports (id, workspace_id, gallery_id, status, photo_count,
                                            size, finished_at)
             VALUES ($1, $2, $3, '{\"kind\": \"ready\"}', 1, 1, now())",
        )
        .bind(foreign_export)
        .bind(other)
        .bind(foreign_gallery)
        .execute(&pool)
        .await
        .unwrap();

        for gallery in [gallery_id, foreign_gallery.0.to_string()] {
            let uri = format!(
                "/api/v1/galleries/{gallery}/export/{}/download",
                foreign_export.0
            );
            let request = test::TestRequest::get().uri(&uri).cookie(cookie.clone());

            assert_eq!(call(&app, request).await.0, StatusCode::NOT_FOUND);
        }
    }
}
