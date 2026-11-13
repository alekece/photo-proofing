//! Workspace contact directory endpoints under `/api/v1/contacts`.
//!
//! - `GET /` lists workspace contacts alphabetically with their linked galleries.
//! - `POST /` creates a contact; duplicate normalized email in the workspace is a 409.
//! - `PATCH /{id}` applies a [`ContactChanges`] changeset; duplicate email is a 409.
//! - `DELETE /{id}` removes the contact and cascades its gallery associations (204).
//!
//! Every query is scoped to the session's workspace, so foreign ids answer 404.

use actix_web::{
    HttpResponse, delete, get, patch, post,
    web::{Data, Json, Path},
};
use pp_database::contact::{ContactChanges, ContactId, ContactRepository, NewContact};

use crate::{
    api::ApiState, error::ErrorResponse, extractors::identity::Authenticated,
    response::ResponseBuilderExt,
};

#[get("")]
pub(crate) async fn list_contacts(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
) -> Result<HttpResponse, ErrorResponse> {
    let contacts = state
        .database()
        .repository::<ContactRepository>()
        .list(identity.workspace_id)
        .await?;

    Ok(HttpResponse::Ok().no_store().json(contacts))
}

#[post("")]
pub(crate) async fn create_contact(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    input: Json<NewContact>,
) -> Result<HttpResponse, ErrorResponse> {
    let contact = state
        .database()
        .repository::<ContactRepository>()
        .create(identity.workspace_id, &input)
        .await?;

    Ok(HttpResponse::Created().no_store().json(contact))
}

#[patch("/{id}")]
pub(crate) async fn update_contact(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: Path<ContactId>,
    changes: Json<ContactChanges>,
) -> Result<HttpResponse, ErrorResponse> {
    let contact = state
        .database()
        .repository::<ContactRepository>()
        .update(identity.workspace_id, id.into_inner(), &changes)
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok().no_store().json(contact))
}

#[delete("/{id}")]
pub(crate) async fn delete_contact(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: Path<ContactId>,
) -> Result<HttpResponse, ErrorResponse> {
    let deleted = state
        .database()
        .repository::<ContactRepository>()
        .delete(identity.workspace_id, id.into_inner())
        .await?;

    if !deleted {
        return Err(ErrorResponse::not_found());
    }

    Ok(HttpResponse::NoContent().no_store().finish())
}

#[cfg(test)]
mod tests {
    use actix_web::{App, cookie::Cookie, http::StatusCode, test, web::Data};
    use pp_database::{Database, contact::ContactRepository};
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use uuid::Uuid;

    use super::*;
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
    async fn contacts_require_authentication_and_same_origin_for_mutations(pool: PgPool) {
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let fake_id = Uuid::new_v4();

        for req in [
            test::TestRequest::get().uri("/api/v1/contacts"),
            test::TestRequest::post()
                .uri("/api/v1/contacts")
                .insert_header(("Origin", ORIGIN))
                .set_json(json!({"name": "Alice", "email": "alice@example.test"})),
            test::TestRequest::patch()
                .uri(&format!("/api/v1/contacts/{fake_id}"))
                .insert_header(("Origin", ORIGIN))
                .set_json(json!({"name": "Alice"})),
            test::TestRequest::delete()
                .uri(&format!("/api/v1/contacts/{fake_id}"))
                .insert_header(("Origin", ORIGIN)),
        ] {
            let (status, body) = call(&app, req).await;

            assert_eq!(status, StatusCode::UNAUTHORIZED);
            assert_eq!(body["code"], "unauthorized");
        }

        for req in [
            test::TestRequest::post()
                .uri("/api/v1/contacts")
                .cookie(cookie.clone())
                .set_json(json!({"name": "Alice", "email": "alice@example.test"})),
            test::TestRequest::patch()
                .uri(&format!("/api/v1/contacts/{fake_id}"))
                .cookie(cookie.clone())
                .set_json(json!({"name": "Alice"})),
            test::TestRequest::delete()
                .uri(&format!("/api/v1/contacts/{fake_id}"))
                .cookie(cookie.clone()),
        ] {
            let (status, body) = call(&app, req).await;

            assert_eq!(status, StatusCode::FORBIDDEN);
            assert_eq!(body["code"], "forbidden_origin");
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn creating_listing_updating_and_deleting_contacts_round_trip_and_isolate_workspaces(
        pool: PgPool,
    ) {
        let foreign_ws = other_workspace(&pool).await;
        let foreign_contact = Database::new(pool.clone())
            .repository::<ContactRepository>()
            .create(
                foreign_ws,
                &NewContact {
                    name: "Foreign".parse().unwrap(),
                    email: "camille@example.test".parse().unwrap(),
                    phone: None,
                    notes: None,
                },
            )
            .await
            .unwrap();

        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;

        // Same email as foreign workspace is allowed in our workspace.
        let (status, zoe) = call(
            &app,
            authed(test::TestRequest::post(), "/api/v1/contacts", &cookie).set_json(json!({
                "name": " Zoé Laurent ",
                "email": " Zoe@Example.test ",
                "phone": " +33 6 12 34 56 78 ",
                "notes": " Bride's sister "
            })),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(zoe["name"], "Zoé Laurent");
        assert_eq!(zoe["email"], "zoe@example.test");
        assert_eq!(zoe["phone"], "+33 6 12 34 56 78");
        assert_eq!(zoe["notes"], "Bride's sister");
        assert_eq!(zoe["galleries"], json!([]));

        let (status, camille) = call(
            &app,
            authed(test::TestRequest::post(), "/api/v1/contacts", &cookie).set_json(json!({
                "name": "Camille & Hugo",
                "email": "camille@example.test"
            })),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(camille["phone"], Value::Null);
        assert_eq!(camille["notes"], Value::Null);

        // Duplicate email in same workspace -> 409 contact_email_taken.
        let (status, conflict) = call(
            &app,
            authed(test::TestRequest::post(), "/api/v1/contacts", &cookie).set_json(json!({
                "name": "Another Zoé",
                "email": "ZOE@example.test"
            })),
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(conflict["code"], "contact_email_taken");

        // Validation errors -> 422 invalid_request_body.
        for bad_body in [
            json!({"name": "   ", "email": "ok@example.test"}),
            json!({"name": "Ok", "email": "not-an-email"}),
            json!({"name": "Ok", "email": "ok@example.test", "phone": ""}),
            json!({"name": "Ok", "email": "ok@example.test", "extra": true}),
        ] {
            let (status, err) = call(
                &app,
                authed(test::TestRequest::post(), "/api/v1/contacts", &cookie).set_json(bad_body),
            )
            .await;

            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(err["code"], "invalid_request_body");
        }

        // Listing returns our contacts alphabetically without foreign contacts.
        let (status, list) = call(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/contacts")
                .cookie(cookie.clone()),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(list.as_array().unwrap().len(), 2);
        assert_eq!(list[0]["id"], camille["id"]);
        assert_eq!(list[1]["id"], zoe["id"]);

        let zoe_id = zoe["id"].as_str().unwrap();
        let foreign_id = foreign_contact.id.0;

        // Patching email to another contact's email in same workspace -> 409.
        let (status, patch_conflict) = call(
            &app,
            authed(
                test::TestRequest::patch(),
                &format!("/api/v1/contacts/{zoe_id}"),
                &cookie,
            )
            .set_json(json!({"email": "CAMILLE@example.test"})),
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(patch_conflict["code"], "contact_email_taken");

        // Patching updates supplied fields and clears nulled optional fields.
        let (status, updated) = call(
            &app,
            authed(
                test::TestRequest::patch(),
                &format!("/api/v1/contacts/{zoe_id}"),
                &cookie,
            )
            .set_json(json!({
                "name": "Zoé L.",
                "phone": null,
                "notes": null
            })),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["name"], "Zoé L.");
        assert_eq!(updated["email"], "zoe@example.test");
        assert_eq!(updated["phone"], Value::Null);
        assert_eq!(updated["notes"], Value::Null);

        // Foreign contact returns 404 on PATCH and DELETE.
        let (status, _) = call(
            &app,
            authed(
                test::TestRequest::patch(),
                &format!("/api/v1/contacts/{foreign_id}"),
                &cookie,
            )
            .set_json(json!({"name": "Stolen"})),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _) = call(
            &app,
            authed(
                test::TestRequest::delete(),
                &format!("/api/v1/contacts/{foreign_id}"),
                &cookie,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);

        // Deleting our contact returns 204 once, then 404.
        let (status, _) = call(
            &app,
            authed(
                test::TestRequest::delete(),
                &format!("/api/v1/contacts/{zoe_id}"),
                &cookie,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::NO_CONTENT);

        let (status, _) = call(
            &app,
            authed(
                test::TestRequest::delete(),
                &format!("/api/v1/contacts/{zoe_id}"),
                &cookie,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
