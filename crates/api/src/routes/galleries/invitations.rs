//! Gallery email invitation endpoint under `/api/v1/galleries/{id}/invitations`.

use std::collections::HashSet;

use actix_web::{
    FromRequest, HttpRequest, HttpResponse,
    dev::Payload,
    post,
    web::{Data, JsonBody, Path, ServiceConfig},
};
use futures_util::future::LocalBoxFuture;
use pp_core::email::{Email, EmailMessage, EmailSubject};
use pp_database::{
    contact::ContactName,
    gallery::{GalleryId, GalleryRepository, InvitationBatch, InvitationContact},
};
use serde::{Deserialize, Serialize};

use super::GalleryView;
use crate::{
    api::ApiState, error::ErrorResponse, extractors::identity::Authenticated,
    response::ResponseBuilderExt,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvitationContactInput {
    email: Email,
    #[serde(default)]
    name: Option<ContactName>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SendInvitationsRequest {
    #[serde(default)]
    to: Vec<InvitationContactInput>,
    #[serde(default)]
    bcc: Vec<Email>,
    subject: EmailSubject,
    message: EmailMessage,
}

impl SendInvitationsRequest {
    /// Fits up to 500 named contacts plus a 5,000-character UTF-8 message and JSON framing.
    const JSON_LIMIT: usize = 256 * 1024;
}

/// Reads the invitation batch with its own size limit, larger than the shared 4 KiB JSON limit.
impl FromRequest for SendInvitationsRequest {
    type Error = ErrorResponse;
    type Future = LocalBoxFuture<'static, Result<Self, ErrorResponse>>;

    fn from_request(request: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let body = JsonBody::<Self>::new(request, payload, None, true).limit(Self::JSON_LIMIT);

        Box::pin(async move { Ok(body.await?) })
    }
}

#[derive(Serialize)]
struct SendInvitationsResponse {
    gallery: GalleryView,
    queued_count: usize,
}

/// Registers the gallery invitations route inside `/api/v1/galleries`.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(send_invitations);
}

#[post("/{id}/invitations")]
async fn send_invitations(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: Path<GalleryId>,
    request: SendInvitationsRequest,
) -> Result<HttpResponse, ErrorResponse> {
    let smtp = state.smtp().ok_or_else(ErrorResponse::email_unavailable)?;

    if request.to.is_empty() && request.bcc.is_empty() {
        return Err(ErrorResponse::bad_request().with_context("at least one recipient is required"));
    }

    let unique_recipients: HashSet<&Email> = request
        .to
        .iter()
        .map(|contact| &contact.email)
        .chain(request.bcc.iter())
        .collect();

    if unique_recipients.len() > smtp.max_recipients_per_send.get() {
        return Err(ErrorResponse::too_many_recipients());
    }

    let gallery_id = id.into_inner();
    let galleries = state.database().repository::<GalleryRepository>();

    if state.access_code_cipher().is_none()
        && let Some(existing) = galleries.get(identity.workspace_id, gallery_id).await?
        && existing.access_code_ciphertext.is_some()
    {
        return Err(ErrorResponse::access_code_unavailable());
    }

    let batch = InvitationBatch {
        to: request
            .to
            .into_iter()
            .map(|contact| InvitationContact {
                email: contact.email,
                name: contact.name,
            })
            .collect(),
        bcc: request.bcc,
        subject: request.subject,
        message: request.message,
    };
    let outcome = galleries
        .queue_invitations(
            identity.workspace_id,
            gallery_id,
            &batch,
            state.job_options(),
        )
        .await?
        .ok_or_else(ErrorResponse::not_found)?;
    let view = GalleryView::try_new(outcome.gallery, state.access_code_cipher())?;

    Ok(HttpResponse::Ok().no_store().json(SendInvitationsResponse {
        gallery: view,
        queued_count: outcome.queued_count,
    }))
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroUsize, time::Duration};

    use actix_web::{App, cookie::Cookie, http::StatusCode, test, web::Data};
    use pp_core::config::{GalleryPasswordConfig, SmtpSettings, SmtpTlsMode};
    use pp_database::{
        Database,
        gallery::GalleryRepository,
        jobs::{JobKind, JobRepository},
    };
    use serde_json::json;
    use sqlx::PgPool;

    use crate::{
        api::configure_routes,
        tests::{ORIGIN, app_state, call, other_workspace, sign_up},
    };

    fn authed(method: test::TestRequest, uri: &str, cookie: &Cookie<'static>) -> test::TestRequest {
        method
            .uri(uri)
            .cookie(cookie.clone())
            .insert_header(("Origin", ORIGIN))
    }

    fn smtp_settings(max_recipients: usize) -> SmtpSettings {
        SmtpSettings {
            host: "127.0.0.1".to_owned(),
            port: 1025,
            tls: SmtpTlsMode::None,
            credentials: None,
            from_email: "noreply@example.test".parse().unwrap(),
            from_name: None,
            max_recipients_per_send: NonZeroUsize::new(max_recipients).unwrap(),
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn invitations_require_smtp_and_reject_unauthenticated_or_missing_origin(pool: PgPool) {
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;

        let (_, gallery) = call(
            &app,
            authed(test::TestRequest::post(), "/api/v1/galleries", &cookie),
        )
        .await;
        let gallery_id = gallery["id"].as_str().unwrap();
        let uri = format!("/api/v1/galleries/{gallery_id}/invitations");
        let valid_body = json!({
            "to": [{"email": "camille@example.test"}],
            "subject": "Your gallery is ready",
            "message": "Hello!"
        });

        let (unauth, _) = call(
            &app,
            test::TestRequest::post()
                .uri(&uri)
                .insert_header(("Origin", ORIGIN))
                .set_json(&valid_body),
        )
        .await;
        assert_eq!(unauth, StatusCode::UNAUTHORIZED);

        let (no_origin, _) = call(
            &app,
            test::TestRequest::post()
                .uri(&uri)
                .cookie(cookie.clone())
                .set_json(&valid_body),
        )
        .await;
        assert_eq!(no_origin, StatusCode::FORBIDDEN);

        let (status, error) = call(
            &app,
            authed(test::TestRequest::post(), &uri, &cookie).set_json(&valid_body),
        )
        .await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error["code"], "email_unavailable");
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn invitations_share_draft_link_contacts_deduplicate_and_enforce_caps_and_states(
        pool: PgPool,
    ) {
        let foreign_ws = other_workspace(&pool).await;
        let database = Database::new(pool.clone());
        let foreign_gallery = database
            .repository::<GalleryRepository>()
            .create(foreign_ws)
            .await
            .unwrap();

        let (state, _directory) = app_state(pool.clone()).await;
        let state = state
            .with_smtp(Some(smtp_settings(3)))
            .with_gallery_password(&GalleryPasswordConfig {
                encryption_key: Some(
                    "0101010101010101010101010101010101010101010101010101010101010101"
                        .parse()
                        .unwrap(),
                ),
            });
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;

        // Pre-create one contact to ensure its existing name is preserved.
        call(
            &app,
            authed(test::TestRequest::post(), "/api/v1/contacts", &cookie).set_json(json!({
                "name": "Camille & Hugo",
                "email": "camille@example.test"
            })),
        )
        .await;

        let (_, gallery) = call(
            &app,
            authed(test::TestRequest::post(), "/api/v1/galleries", &cookie),
        )
        .await;
        let gallery_id = gallery["id"].as_str().unwrap().to_owned();

        call(
            &app,
            authed(
                test::TestRequest::patch(),
                &format!("/api/v1/galleries/{gallery_id}"),
                &cookie,
            )
            .set_json(json!({
                "title": "Camille & Hugo",
                "access_code": "482910"
            })),
        )
        .await;

        let uri = format!("/api/v1/galleries/{gallery_id}/invitations");

        // Foreign gallery -> 404.
        let (foreign_status, _) = call(
            &app,
            authed(
                test::TestRequest::post(),
                &format!("/api/v1/galleries/{}/invitations", foreign_gallery.id.0),
                &cookie,
            )
            .set_json(json!({
                "to": [{"email": "camille@example.test"}],
                "subject": "Ready",
                "message": "Hi"
            })),
        )
        .await;
        assert_eq!(foreign_status, StatusCode::NOT_FOUND);

        // Empty recipients -> 400 bad_request.
        let (empty_status, empty_err) = call(
            &app,
            authed(test::TestRequest::post(), &uri, &cookie).set_json(json!({
                "to": [],
                "bcc": [],
                "subject": "Ready",
                "message": "Hi"
            })),
        )
        .await;
        assert_eq!(empty_status, StatusCode::BAD_REQUEST);
        assert_eq!(empty_err["code"], "bad_request");

        // Exceeds max_recipients_per_send (3) -> 400 too_many_recipients.
        let (cap_status, cap_err) = call(
            &app,
            authed(test::TestRequest::post(), &uri, &cookie).set_json(json!({
                "to": [
                    {"email": "a@example.test"},
                    {"email": "b@example.test"}
                ],
                "bcc": ["c@example.test", "d@example.test"],
                "subject": "Ready",
                "message": "Hi"
            })),
        )
        .await;
        assert_eq!(cap_status, StatusCode::BAD_REQUEST);
        assert_eq!(cap_err["code"], "too_many_recipients");

        // Multiline subject -> 422 invalid_request_body.
        let (subj_status, _) = call(
            &app,
            authed(test::TestRequest::post(), &uri, &cookie).set_json(json!({
                "to": [{"email": "a@example.test"}],
                "subject": "Header\r\nBcc: evil@example.test",
                "message": "Hi"
            })),
        )
        .await;
        assert_eq!(subj_status, StatusCode::UNPROCESSABLE_ENTITY);

        // Oversized JSON payload (> 256 KiB) -> 413 payload_too_large.
        let (oversized_status, oversized_err) = call(
            &app,
            authed(test::TestRequest::post(), &uri, &cookie).set_json(json!({
                "to": [{"email": "a@example.test"}],
                "subject": "Ready",
                "message": "A".repeat(300 * 1024)
            })),
        )
        .await;
        assert_eq!(oversized_status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(oversized_err["code"], "payload_too_large");

        // Valid send (with a near-max 4,800-char personal message exceeding the default 4 KiB JSON limit):
        // shares Draft gallery, links To contacts, deduplicates mixed-case BCC, returns decrypted access_code.
        let (status, response) = call(
            &app,
            authed(test::TestRequest::post(), &uri, &cookie).set_json(json!({
                "to": [
                    {"email": "Camille@Example.test"},
                    {"email": "hugo@example.test", "name": "Hugo"}
                ],
                "bcc": ["camille@example.test", "press@example.test"],
                "subject": "Camille & Hugo — Your gallery is ready",
                "message": "A".repeat(4800)
            })),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(response["queued_count"], 3);
        assert!(response["gallery"]["share_token"].as_str().is_some());
        assert_eq!(response["gallery"]["access_code"], "482910");
        assert!(
            response["gallery"]["states"]
                .as_array()
                .unwrap()
                .contains(&json!("shared"))
        );

        let (_, linked_contacts) = call(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v1/galleries/{gallery_id}/contacts"))
                .cookie(cookie.clone()),
        )
        .await;
        let linked = linked_contacts.as_array().unwrap();
        assert_eq!(linked.len(), 2);
        assert!(linked.iter().any(|c| c["name"] == "Camille & Hugo"));
        assert!(linked.iter().any(|c| c["name"] == "Hugo"));

        // Drain the 3 queued SendEmail jobs.
        let jobs = database.repository::<JobRepository>();
        let mut count = 0;

        while let Some(job) = jobs
            .claim(&[JobKind::SendEmail], Duration::from_secs(30))
            .await
            .unwrap()
        {
            count += 1;
            let _ = jobs.complete(&job).await.unwrap();
        }
        assert_eq!(count, 3);

        // Expired gallery -> 409 gallery_expired.
        sqlx::query(
            "UPDATE galleries SET expires_at = now() - interval '5 minutes' WHERE id = $1::uuid",
        )
        .bind(&gallery_id)
        .execute(&pool)
        .await
        .unwrap();

        let (exp_status, exp_err) = call(
            &app,
            authed(test::TestRequest::post(), &uri, &cookie).set_json(json!({
                "to": [{"email": "camille@example.test"}],
                "subject": "Ready",
                "message": "Hi"
            })),
        )
        .await;
        assert_eq!(exp_status, StatusCode::CONFLICT);
        assert_eq!(exp_err["code"], "gallery_expired");

        // Archived gallery -> 409 gallery_archived.
        call(
            &app,
            authed(
                test::TestRequest::post(),
                &format!("/api/v1/galleries/{gallery_id}/archive"),
                &cookie,
            ),
        )
        .await;

        let (arch_status, arch_err) = call(
            &app,
            authed(test::TestRequest::post(), &uri, &cookie).set_json(json!({
                "to": [{"email": "camille@example.test"}],
                "subject": "Ready",
                "message": "Hi"
            })),
        )
        .await;
        assert_eq!(arch_status, StatusCode::CONFLICT);
        assert_eq!(arch_err["code"], "gallery_archived");
    }
}
