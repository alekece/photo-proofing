//! Authenticated exports of a selection across workspace galleries.

use actix_web::{
    HttpResponse, get, post,
    web::{self, Data, Json, JsonConfig, Path as WebPath, ServiceConfig},
};
use pp_database::export::{ExportId, ExportOptions, ExportRepository};
use pp_storage::ArchiveKey;

use super::{DOWNLOAD_NAME, ExportResponse, ExportSelection};
use crate::{
    api::ApiState, error::ErrorResponse, extractors::identity::Authenticated, response::Attachment,
};

/// Registers the signed-in photographer's multi-gallery ZIP under the Photos scope.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(
        web::scope("/export")
            .app_data(
                JsonConfig::default()
                    .limit(ExportSelection::JSON_LIMIT)
                    .error_handler(|error, _| ErrorResponse::from(error).into()),
            )
            .service(request_workspace)
            .service(show_workspace)
            .service(download_workspace),
    );
}

#[post("")]
async fn request_workspace(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    selection: Json<ExportSelection>,
) -> Result<HttpResponse, ErrorResponse> {
    let outcome = state
        .database()
        .repository_with::<ExportRepository>(ExportOptions {
            job: state.job_options(),
            ..Default::default()
        })
        .request_workspace(
            identity.workspace_id,
            &selection.photo_ids,
            state.export_max_size(),
            state.export_retention(),
        )
        .await?;

    ExportResponse::requested(outcome)
}

#[get("/{export_id}")]
async fn show_workspace(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: WebPath<ExportId>,
) -> Result<HttpResponse, ErrorResponse> {
    let export = state
        .database()
        .repository::<ExportRepository>()
        .workspace_export(
            identity.workspace_id,
            id.into_inner(),
            state.export_retention(),
        )
        .await?;

    Ok(ExportResponse::latest(export))
}

#[get("/{export_id}/download")]
async fn download_workspace(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: WebPath<ExportId>,
) -> Result<Attachment, ErrorResponse> {
    let export = id.into_inner();
    let gallery = state
        .database()
        .repository::<ExportRepository>()
        .workspace_archive(identity.workspace_id, export, state.export_retention())
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    let key = ArchiveKey {
        workspace: identity.workspace_id,
        gallery,
        export,
    };

    Attachment::read_zip(state.storage(), &key, DOWNLOAD_NAME).await
}
