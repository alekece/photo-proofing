//! Gallery contact association endpoints under `/api/v1/galleries/{id}/contacts`.
//!
//! - `GET /{id}/contacts` lists workspace contacts linked to the gallery, omitting raw tokens.
//! - `PUT /{id}/contacts/{contact_id}` idempotently links a workspace contact to the gallery.
//! - `DELETE /{id}/contacts/{contact_id}` unlinks the contact without deleting it (204).

use actix_web::{
    HttpResponse, delete, get, put,
    web::{Data, Path},
};
use pp_database::{
    contact::{ContactId, ContactRepository},
    gallery::GalleryId,
};
use serde::Deserialize;

use crate::{
    api::ApiState, error::ErrorResponse, extractors::identity::Authenticated,
    response::ResponseBuilderExt,
};

#[derive(Deserialize)]
pub(crate) struct GalleryContactPath {
    id: GalleryId,
    contact_id: ContactId,
}

#[get("/{id}/contacts")]
pub(crate) async fn list_contacts(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: Path<GalleryId>,
) -> Result<HttpResponse, ErrorResponse> {
    let contacts = state
        .database()
        .repository::<ContactRepository>()
        .list_for_gallery(identity.workspace_id, id.into_inner())
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok().no_store().json(contacts))
}

#[put("/{id}/contacts/{contact_id}")]
pub(crate) async fn link_contact(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    path: Path<GalleryContactPath>,
) -> Result<HttpResponse, ErrorResponse> {
    let GalleryContactPath { id, contact_id } = path.into_inner();
    let linked = state
        .database()
        .repository::<ContactRepository>()
        .link_gallery(identity.workspace_id, id, contact_id)
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok().no_store().json(linked.contact))
}

#[delete("/{id}/contacts/{contact_id}")]
pub(crate) async fn unlink_contact(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    path: Path<GalleryContactPath>,
) -> Result<HttpResponse, ErrorResponse> {
    let GalleryContactPath { id, contact_id } = path.into_inner();
    let removed = state
        .database()
        .repository::<ContactRepository>()
        .unlink_gallery(identity.workspace_id, id, contact_id)
        .await?;

    if !removed {
        return Err(ErrorResponse::not_found());
    }

    Ok(HttpResponse::NoContent().no_store().finish())
}

#[cfg(test)]
mod tests {
    use actix_web::{App, cookie::Cookie, http::StatusCode, test, web::Data};
    use pp_database::{
        Database,
        auth::AuthRepository,
        contact::{ContactId, ContactRepository, NewContact},
        gallery::{GalleryId, GalleryRepository},
    };
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

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

    #[sqlx::test(migrations = "../database/migrations")]
    async fn linking_listing_and_unlinking_gallery_contacts_enforce_workspace_and_shared_access(
        pool: PgPool,
    ) {
        let foreign_ws = other_workspace(&pool).await;
        let database = Database::new(pool.clone());
        let foreign_gallery = database
            .repository::<GalleryRepository>()
            .create(foreign_ws)
            .await
            .unwrap();
        let foreign_contact = database
            .repository::<ContactRepository>()
            .create(
                foreign_ws,
                &NewContact {
                    name: "Foreign".parse().unwrap(),
                    email: "foreign@example.test".parse().unwrap(),
                    phone: None,
                    notes: None,
                },
            )
            .await
            .unwrap();

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
            .set_json(json!({"title": "Summer Wedding"})),
        )
        .await;

        let (_, contact) = call(
            &app,
            authed(test::TestRequest::post(), "/api/v1/contacts", &cookie).set_json(json!({
                "name": "Camille & Hugo",
                "email": "camille@example.test"
            })),
        )
        .await;
        let contact_id = contact["id"].as_str().unwrap().to_owned();

        // Auth & Origin checks on gallery contact routes.
        let (status, _) = call(
            &app,
            test::TestRequest::get().uri(&format!("/api/v1/galleries/{gallery_id}/contacts")),
        )
        .await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, _) = call(
            &app,
            test::TestRequest::put()
                .uri(&format!(
                    "/api/v1/galleries/{gallery_id}/contacts/{contact_id}"
                ))
                .cookie(cookie.clone()),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);

        // Cross-workspace gallery or contact returns 404.
        let (status, _) = call(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/api/v1/galleries/{}/contacts",
                    foreign_gallery.id.0
                ))
                .cookie(cookie.clone()),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _) = call(
            &app,
            authed(
                test::TestRequest::put(),
                &format!(
                    "/api/v1/galleries/{gallery_id}/contacts/{}",
                    foreign_contact.id.0
                ),
                &cookie,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);

        // Link contact to Draft gallery via PUT (idempotent, never leaks share_token).
        let (status, linked_json) = call(
            &app,
            authed(
                test::TestRequest::put(),
                &format!("/api/v1/galleries/{gallery_id}/contacts/{contact_id}"),
                &cookie,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(linked_json["id"], contact_id);
        assert_eq!(
            linked_json["galleries"],
            json!([{"id": gallery_id, "title": "Summer Wedding"}])
        );
        assert!(linked_json.get("share_token").is_none());

        let (status, linked_again) = call(
            &app,
            authed(
                test::TestRequest::put(),
                &format!("/api/v1/galleries/{gallery_id}/contacts/{contact_id}"),
                &cookie,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(linked_again, linked_json);

        // GET /galleries/{id}/contacts lists linked contacts without share_token.
        let (status, listed) = call(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v1/galleries/{gallery_id}/contacts"))
                .cookie(cookie.clone()),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(listed, json!([linked_json]));
        assert!(
            !serde_json::to_string(&listed)
                .unwrap()
                .contains("share_token")
        );

        // Fetch the contact's generated token through ContactRepository (idempotent).
        let account = database
            .repository::<AuthRepository>()
            .account_by_email(&"person@example.test".parse().unwrap())
            .await
            .unwrap()
            .unwrap();
        let link_record = database
            .repository::<ContactRepository>()
            .link_gallery(
                account.workspace_id,
                GalleryId(Uuid::parse_str(&gallery_id).unwrap()),
                ContactId(Uuid::parse_str(&contact_id).unwrap()),
            )
            .await
            .unwrap()
            .unwrap();
        let contact_token = link_record.share_token.as_str().to_owned();

        // Draft gallery MUST NOT be accessible via per-contact token!
        let (status, _) = call(
            &app,
            test::TestRequest::get().uri(&format!("/api/v1/galleries/shared/{contact_token}")),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);

        // Share gallery -> both general token and per-contact token work, and neither leaks contact details!
        let (status, shared_gallery) = call(
            &app,
            authed(
                test::TestRequest::post(),
                &format!("/api/v1/galleries/{gallery_id}/share"),
                &cookie,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);

        let general_token = shared_gallery["share_token"].as_str().unwrap().to_owned();
        let (status, client_view) = call(
            &app,
            test::TestRequest::get().uri(&format!("/api/v1/galleries/shared/{contact_token}")),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(client_view["title"], "Summer Wedding");

        let serialized_client_view = serde_json::to_string(&client_view).unwrap();

        assert!(!serialized_client_view.contains("contact_id"));
        assert!(!serialized_client_view.contains(&contact_id));
        assert!(!serialized_client_view.contains("camille@example.test"));
        assert!(!serialized_client_view.contains("Camille & Hugo"));

        let (status, _) = call(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/v1/galleries/shared/{contact_token}/open"))
                .insert_header(("Origin", ORIGIN)),
        )
        .await;

        assert_eq!(status, StatusCode::NO_CONTENT);

        // Unlink contact -> contact_token immediately 404s, general_token stays 200, contact stays in directory!
        let (status, _) = call(
            &app,
            authed(
                test::TestRequest::delete(),
                &format!("/api/v1/galleries/{gallery_id}/contacts/{contact_id}"),
                &cookie,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::NO_CONTENT);

        let (status, _) = call(
            &app,
            test::TestRequest::get().uri(&format!("/api/v1/galleries/shared/{contact_token}")),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _) = call(
            &app,
            test::TestRequest::get().uri(&format!("/api/v1/galleries/shared/{general_token}")),
        )
        .await;

        assert_eq!(status, StatusCode::OK);

        let (status, directory) = call(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/contacts")
                .cookie(cookie),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(directory.as_array().unwrap().len(), 1);
        assert_eq!(directory[0]["id"], contact_id);
        assert_eq!(directory[0]["galleries"], json!([]));
    }
}
