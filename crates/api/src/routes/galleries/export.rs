//! The photographer's ZIP of a gallery's favorites or, with `?finals=true`, private finals.
//!
//! `POST /api/v1/galleries/{id}/export` stores the selection as it is now and queues
//! the build; `GET` on the same path reports the newest export and `GET
//! .../{export_id}/download` streams a finished ZIP as an attachment. All three need the
//! photographer's session and the gallery's workspace: another workspace, an unknown export or an
//! export past its retention is a plain 404. Progress arrives on the event stream as `export`
//! events.

use actix_web::{
    HttpResponse, get, post,
    web::{Bytes, Data, Path as WebPath, Query, ServiceConfig},
};
use pp_core::types::timestamp::Timestamp;
use pp_database::{
    export::{Export, ExportId, ExportOptions, ExportRepository, ExportRequest},
    gallery::{GalleryId, GalleryRepository},
    photo::{PhotoId, PhotoKind},
};
use pp_storage::ArchiveKey;
use serde::{Deserialize, Deserializer, Serialize, de::Error};

use super::photos::PhotoSet;
use crate::{
    api::ApiState,
    error::{ErrorResponse, ExportStatusResponse},
    extractors::identity::Authenticated,
    response::{Attachment, ResponseBuilderExt},
};

pub(crate) mod workspace;

/// The name the browser saves the ZIP under, the same for every gallery.
const DOWNLOAD_NAME: &str = "selection.zip";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportSelection {
    #[serde(deserialize_with = "ExportSelection::deserialize_photo_ids")]
    photo_ids: Vec<PhotoId>,
}

impl ExportSelection {
    const JSON_LIMIT: usize = 64 * 1024;

    fn deserialize_photo_ids<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<PhotoId>, D::Error> {
        let photo_ids = Vec::deserialize(deserializer)?;

        if !(1..=1000).contains(&photo_ids.len()) {
            return Err(D::Error::custom("expected between 1 and 1000 photo IDs"));
        }

        Ok(photo_ids)
    }
}

/// An export as the photographer's UI receives it.
#[derive(Debug, Serialize)]
struct ExportResponse {
    id: ExportId,
    status: ExportStatusResponse,
    photo_count: u32,
    created_at: Timestamp,
}

impl From<Export> for ExportResponse {
    fn from(export: Export) -> Self {
        Self {
            id: export.id,
            status: export.status.into(),
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
    set: Query<PhotoSet>,
    body: Bytes,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery_id = id.into_inner();
    let selection = if body.is_empty() {
        None
    } else if body.len() > ExportSelection::JSON_LIMIT {
        return Err(ErrorResponse::bad_request());
    } else {
        Some(
            serde_json::from_slice::<ExportSelection>(&body)
                .map_err(|_| ErrorResponse::bad_request())?,
        )
    };
    let outcome = state
        .database()
        .repository_with::<ExportRepository>(ExportOptions {
            job: state.job_options(),
            ..Default::default()
        })
        .request_owner(
            identity.workspace_id,
            gallery_id,
            state.export_max_size(),
            state.export_retention(),
            PhotoKind::from(*set),
            selection
                .as_ref()
                .map(|selection| selection.photo_ids.as_slice()),
        )
        .await?;

    if let ExportRequest::Created { replaced, .. } = &outcome {
        for previous_export in replaced {
            let key = ArchiveKey {
                workspace: identity.workspace_id,
                gallery: gallery_id,
                export: *previous_export,
            };

            if let Err(error) = state.storage().remove_archive(&key).await {
                tracing::warn!(%error, "replaced selection ZIP could not be removed");
            }
        }
    }

    ExportResponse::requested(outcome)
}

impl ExportResponse {
    fn requested(outcome: ExportRequest) -> Result<HttpResponse, ErrorResponse> {
        match outcome {
            ExportRequest::Created { export, .. } => {
                Ok(HttpResponse::Accepted().no_store().json(Self::from(export)))
            }
            ExportRequest::Reused(export) => Ok(HttpResponse::Ok()
                .no_store()
                .json(ExportResponse::from(export))),
            ExportRequest::InProgress(_) => Err(ErrorResponse::zip_in_progress()),
            ExportRequest::InvalidSelection => Err(ErrorResponse::bad_request()),
            ExportRequest::EmptySelection => Err(ErrorResponse::empty_selection()),
            ExportRequest::TooLarge => Err(ErrorResponse::selection_too_large()),
            ExportRequest::MissingGallery => Err(ErrorResponse::not_found()),
        }
    }

    fn latest(export: Option<Export>) -> HttpResponse {
        match export {
            Some(export) => HttpResponse::Ok().no_store().json(Self::from(export)),
            None => HttpResponse::NoContent().no_store().finish(),
        }
    }
}

/// The newest export, or `204` when there is none or it expired.
#[get("/{id}/export")]
async fn show_export(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: WebPath<GalleryId>,
    set: Query<PhotoSet>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery_id = id.into_inner();
    let kind = PhotoKind::from(*set);
    let export = state
        .database()
        .repository::<ExportRepository>()
        .latest_owner(
            identity.workspace_id,
            gallery_id,
            state.export_retention(),
            kind,
        )
        .await?;

    if export.is_none()
        && kind == PhotoKind::Final
        && state
            .database()
            .repository::<GalleryRepository>()
            .get(identity.workspace_id, gallery_id)
            .await?
            .is_none()
    {
        return Err(ErrorResponse::not_found());
    }

    Ok(ExportResponse::latest(export))
}

#[get("/{id}/export/{export_id}/download")]
async fn download_export(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    path: WebPath<(GalleryId, ExportId)>,
    set: Query<PhotoSet>,
) -> Result<Attachment, ErrorResponse> {
    let (gallery_id, export_id) = path.into_inner();
    let ready = state
        .database()
        .repository::<ExportRepository>()
        .ready_owner(
            identity.workspace_id,
            gallery_id,
            export_id,
            state.export_retention(),
            PhotoKind::from(*set),
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
    Attachment::read_zip(state.storage(), &key, DOWNLOAD_NAME).await
}

#[cfg(test)]
mod tests {
    use actix_web::{
        http::{Method, StatusCode},
        test,
    };
    use pp_core::{config::PhotoDir, types::generate::Generate};
    use pp_database::Database;
    use pp_database::photo::PhotoId;
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
                                 status, favorited_at)
             SELECT $1, workspace_id, id, $3, 5, 2, 2, 'ready', now() FROM galleries WHERE id = $2",
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

    /// Inserts a ready final photo directly, skipping upload/processing.
    async fn ready_final(pool: &PgPool, gallery_id: &str, name: &str) -> PhotoId {
        let id = PhotoId::generate();

        sqlx::query(
            "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height,
                                 status, kind)
             SELECT $1, workspace_id, id, $3, 5, 2, 2, 'ready', 'final' FROM galleries WHERE id = $2",
        )
        .bind(id)
        .bind(Uuid::parse_str(gallery_id).unwrap())
        .bind(name)
        .execute(pool)
        .await
        .unwrap();

        id
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn owner_final_exports_snapshot_all_ready_or_exact_subsets_before_delivery(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let proof = favorite(&pool, &gallery_id, "proof.jpg").await;
        let first = ready_final(&pool, &gallery_id, "first.jpg").await;
        let second = ready_final(&pool, &gallery_id, "second.jpg").await;
        let pending = ready_final(&pool, &gallery_id, "pending.jpg").await;
        sqlx::query("UPDATE photos SET status = 'processing' WHERE id = $1")
            .bind(pending)
            .execute(&pool)
            .await
            .unwrap();
        let uri = format!("/api/v1/galleries/{gallery_id}/export?finals=true");
        let request = || post(&gallery_id, &cookie).uri(&uri);
        let (status, all) = call(&app, request()).await;

        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(all["photo_count"], 2);

        let database = Database::new(pool.clone());
        let exports = database.repository::<ExportRepository>();
        let all_id: ExportId = serde_json::from_value(all["id"].clone()).unwrap();
        let all_plan = exports.plan(all_id).await.unwrap().unwrap();
        assert_eq!(
            all_plan
                .photos
                .iter()
                .map(|photo| photo.id)
                .collect::<Vec<_>>(),
            vec![first, second]
        );
        sqlx::query("UPDATE exports SET status = '{\"kind\": \"ready\"}', finished_at = now()")
            .execute(&pool)
            .await
            .unwrap();

        let (status, subset) = call(
            &app,
            request().set_json(serde_json::json!({"photo_ids": [second]})),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(subset["photo_count"], 1);
        let id: ExportId = serde_json::from_value(subset["id"].clone()).unwrap();
        let plan = exports.plan(id).await.unwrap().unwrap();
        assert_eq!(
            plan.photos.iter().map(|photo| photo.id).collect::<Vec<_>>(),
            vec![second]
        );

        let storage = PhotoStorage::open(&PhotoDir::try_from(directory.path().to_owned()).unwrap())
            .await
            .unwrap();
        let location = PhotoLocation {
            workspace_id: plan.workspace_id,
            gallery_id: plan.gallery_id,
        };
        for (photo, bytes) in [
            (proof, b"proof original".as_slice()),
            (first, b"first original"),
            (second, b"second original"),
        ] {
            let mut staged = storage.stage().await.unwrap();

            staged.write(bytes).await.unwrap();
            staged
                .commit(&PhotoKey::with_location(location, photo, Variant::Original))
                .await
                .unwrap();
        }

        storage
            .build_archive(
                &ArchiveKey {
                    workspace: plan.workspace_id,
                    gallery: plan.gallery_id,
                    export: id,
                },
                plan.photos
                    .into_iter()
                    .map(|photo| ArchiveEntry {
                        source: PhotoKey::with_location(location, photo.id, Variant::Original),
                        original_name: photo.original_name,
                    })
                    .collect(),
            )
            .await
            .unwrap();
        sqlx::query("UPDATE exports SET status = '{\"kind\": \"ready\"}', finished_at = now()")
            .execute(&pool)
            .await
            .unwrap();

        let show = test::TestRequest::get().uri(&uri).cookie(cookie.clone());
        let (status, latest) = call(&app, show).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(latest["id"], subset["id"]);
        assert_eq!(latest["status"]["kind"], "ready");

        let download = format!("/api/v1/galleries/{gallery_id}/export/{}/download", id.0);
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("{download}?finals=true"))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("cache-control").unwrap(),
            "private, no-store"
        );
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/zip"
        );
        assert!(response.headers().contains_key("content-disposition"));
        let archive = test::read_body(response).await;
        assert!(archive.starts_with(b"PK"));
        assert!(
            archive
                .windows(15)
                .any(|window| window == b"second original")
        );
        for absent in [b"proof original".as_slice(), b"first original"] {
            assert!(!archive.windows(absent.len()).any(|window| window == absent));
        }
        assert_eq!(
            call(
                &app,
                test::TestRequest::get()
                    .uri(&download)
                    .cookie(cookie.clone())
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(
                &app,
                test::TestRequest::get()
                    .uri(&format!("/api/v1/galleries/{gallery_id}/export"))
                    .cookie(cookie)
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
        let (photo_downloads, archive_downloads, activity): (i64, i64, i64) = sqlx::query_as(
            "SELECT (SELECT count(*) FROM photos WHERE downloaded_at IS NOT NULL),
                    (SELECT count(*) FROM exports WHERE downloaded_at IS NOT NULL),
                    (SELECT count(*) FROM activity_events)",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!((photo_downloads, archive_downloads, activity), (0, 0, 0));
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn owner_final_exports_reject_wrong_set_and_unauthorized_access(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let other_gallery = create_gallery(&app, &cookie).await;
        let foreign = ready_final(&pool, &other_gallery, "other.jpg").await;
        let proof = favorite(&pool, &gallery_id, "proof.jpg").await;
        let ready = ready_final(&pool, &gallery_id, "ready.jpg").await;
        let waiting = ready_final(&pool, &gallery_id, "waiting.jpg").await;
        sqlx::query("UPDATE photos SET status = 'processing' WHERE id = $1")
            .bind(waiting)
            .execute(&pool)
            .await
            .unwrap();
        let uri = format!("/api/v1/galleries/{gallery_id}/export?finals=true");

        for ids in [
            vec![],
            vec![ready, ready],
            vec![proof],
            vec![ready, waiting],
            vec![foreign],
        ] {
            let request = post(&gallery_id, &cookie)
                .uri(&uri)
                .set_json(serde_json::json!({"photo_ids": ids}));

            assert_eq!(call(&app, request).await.0, StatusCode::BAD_REQUEST);
        }

        assert_eq!(
            call(&app, test::TestRequest::get().uri(&uri)).await.0,
            StatusCode::UNAUTHORIZED
        );
        let workspace = other_workspace(&pool).await;
        let foreign_gallery = GalleryId::generate();
        sqlx::query("INSERT INTO galleries (id, workspace_id) VALUES ($1, $2)")
            .bind(foreign_gallery)
            .bind(workspace)
            .execute(&pool)
            .await
            .unwrap();
        let foreign_gallery = foreign_gallery.0.to_string();
        let uri = format!("/api/v1/galleries/{foreign_gallery}/export?finals=true");
        assert_eq!(
            call(&app, post(&foreign_gallery, &cookie).uri(&uri))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(&app, test::TestRequest::get().uri(&uri).cookie(cookie))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn individual_original_is_an_owner_only_unchanged_attachment(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = favorite(&pool, &gallery_id, "proof.jpg").await;
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
        let mut staged = storage.stage().await.unwrap();
        staged.write(b"unchanged original").await.unwrap();
        staged
            .commit(&PhotoKey::with_location(location, photo, Variant::Original))
            .await
            .unwrap();
        let uri = format!("/api/v1/galleries/{gallery_id}/photos/{photo}/download");
        let checked = test::call_service(
            &app,
            test::TestRequest::default()
                .method(Method::HEAD)
                .uri(&uri)
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(checked.status(), StatusCode::OK);
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&uri)
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-disposition").unwrap(),
            "attachment; filename=\"proof.jpg\""
        );
        assert_eq!(
            test::read_body(response).await.as_ref(),
            b"unchanged original"
        );
        assert_eq!(
            call(&app, test::TestRequest::get().uri(&uri)).await.0,
            StatusCode::UNAUTHORIZED
        );
        let wrong_gallery = format!(
            "/api/v1/galleries/{}/photos/{photo}/download",
            GalleryId::generate().0
        );
        assert_eq!(
            call(
                &app,
                test::TestRequest::get()
                    .uri(&wrong_gallery)
                    .cookie(cookie.clone())
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        let finals = test::TestRequest::get()
            .uri(&format!("{uri}?finals=true"))
            .cookie(cookie);
        assert_eq!(call(&app, finals).await.0, StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn individual_final_original_is_downloadable_with_the_finals_query(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = ready_final(&pool, &gallery_id, "final.jpg").await;
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
        let mut staged = storage.stage().await.unwrap();
        staged.write(b"unchanged final").await.unwrap();
        staged
            .commit(&PhotoKey::with_location(location, photo, Variant::Original))
            .await
            .unwrap();
        let uri = format!("/api/v1/galleries/{gallery_id}/photos/{photo}/download?finals=true");

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&uri)
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-disposition").unwrap(),
            "attachment; filename=\"final.jpg\""
        );
        assert_eq!(test::read_body(response).await.as_ref(), b"unchanged final");

        // The same id without `?finals=true` is a proof lookup, which this id never satisfies.
        let proof_uri = format!("/api/v1/galleries/{gallery_id}/photos/{photo}/download");
        assert_eq!(
            call(
                &app,
                test::TestRequest::get().uri(&proof_uri).cookie(cookie)
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn subset_export_snapshots_only_requested_favorites_and_rejects_competing_requests(
        pool: PgPool,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let first = favorite(&pool, &gallery_id, "a.jpg").await;
        favorite(&pool, &gallery_id, "b.jpg").await;
        let request =
            post(&gallery_id, &cookie).set_json(serde_json::json!({"photo_ids": [first]}));
        let (status, export) = call(&app, request).await;

        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(export["photo_count"], 1);
        let snapshot: Vec<PhotoId> = sqlx::query_scalar("SELECT photo_id FROM export_photos")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(snapshot, vec![first]);
        assert_eq!(
            call(&app, post(&gallery_id, &cookie)).await.0,
            StatusCode::CONFLICT
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn subset_export_rejects_empty_duplicate_and_unknown_ids(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let first = favorite(&pool, &gallery_id, "a.jpg").await;

        for ids in [vec![], vec![first, first], vec![PhotoId::generate()]] {
            let (status, _) = call(
                &app,
                post(&gallery_id, &cookie).set_json(serde_json::json!({"photo_ids": ids})),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn subset_export_rejects_ineligible_photos_and_enforces_selection_size(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let other_gallery = create_gallery(&app, &cookie).await;
        let foreign = favorite(&pool, &other_gallery, "foreign.jpg").await;
        let nonfavorite = favorite(&pool, &gallery_id, "not-selected.jpg").await;
        let final_photo = favorite(&pool, &gallery_id, "final.jpg").await;

        sqlx::query("UPDATE photos SET favorited_at = NULL WHERE id = $1")
            .bind(nonfavorite)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE photos SET kind = 'final' WHERE id = $1")
            .bind(final_photo)
            .execute(&pool)
            .await
            .unwrap();

        for ids in [vec![foreign], vec![nonfavorite], vec![final_photo]] {
            assert_eq!(
                call(
                    &app,
                    post(&gallery_id, &cookie).set_json(serde_json::json!({"photo_ids": ids}))
                )
                .await
                .0,
                StatusCode::BAD_REQUEST
            );
        }

        let ids: Vec<PhotoId> = sqlx::query_scalar(
            "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height,
                                 status, favorited_at)
             SELECT gen_random_uuid(), workspace_id, id, 'proof-' || n || '.jpg', 5, 2, 2, 'ready', now()
             FROM galleries CROSS JOIN generate_series(1, 1001) AS n WHERE id = $1
             RETURNING id",
        )
        .bind(Uuid::parse_str(&gallery_id).unwrap())
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(ids.len(), 1001);

        assert_eq!(
            call(
                &app,
                post(&gallery_id, &cookie).set_json(serde_json::json!({"photo_ids": ids})),
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );

        let snapshots: i64 = sqlx::query_scalar("SELECT count(*) FROM exports")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(snapshots, 0);

        assert_eq!(
            call(
                &app,
                post(&gallery_id, &cookie).set_json(serde_json::json!({"photo_ids": &ids[..1000]})),
            )
            .await
            .0,
            StatusCode::ACCEPTED
        );
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
        assert_eq!(again, StatusCode::CONFLICT);
        assert_eq!(second["code"], "zip_in_progress");
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
    async fn an_unchanged_selection_answers_with_its_ready_export(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;

        favorite(&pool, &gallery_id, "a.jpg").await;

        let (status, export) = call(&app, post(&gallery_id, &cookie)).await;

        assert_eq!(status, StatusCode::ACCEPTED);

        sqlx::query("UPDATE exports SET status = '{\"kind\": \"ready\"}', finished_at = now()")
            .execute(&pool)
            .await
            .unwrap();

        let (status, again) = call(&app, post(&gallery_id, &cookie)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(again["id"], export["id"]);
        assert_eq!(again["status"]["kind"], "ready");
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

        // The photographer's own ZIP download is never client activity.
        let activity: i64 =
            sqlx::query_scalar("SELECT count(*) FROM activity_events WHERE gallery_id = $1")
                .bind(Uuid::parse_str(&gallery_id).unwrap())
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(activity, 0);
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
