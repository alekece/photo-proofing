use std::{future::Future, pin::Pin, time::SystemTime};

use actix_web::{FromRequest, HttpRequest, dev::Payload, web::Data};
use pp_database::auth::{AuthRepository, Identity};

use crate::{api::ApiState, extractors::session_token::SessionToken, routes::auth::AuthError};

/// The identity behind the request's session cookie.
///
/// Read-only: it never extends the session, as with `GET /auth/status`. Fails with
/// [`AuthError::Unauthorized`] when the cookie is missing, expired or revoked.
pub(crate) struct Authenticated(pub Identity);

impl FromRequest for Authenticated {
    type Error = AuthError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, AuthError>>>>;

    fn from_request(request: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let token = SessionToken::from_request(request, payload);
        let state = request.app_data::<Data<ApiState>>().cloned();

        Box::pin(async move {
            let state = state.ok_or(AuthError::Unavailable)?;
            let token = token.await?;

            state
                .database()
                .repository::<AuthRepository>()
                .authenticate(token.digest().as_bytes(), SystemTime::now())
                .await
                .map_err(|_| AuthError::Unavailable)?
                .map(Self)
                .ok_or(AuthError::Unauthorized)
        })
    }
}
