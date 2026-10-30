use std::future::{self, Ready};

use actix_web::{FromRequest, HttpRequest, dev::Payload, web};

use crate::{api::ApiState, request::RequestExt, routes::auth::AuthError};

/// Extractor for a sign-in request from a peer that is still under the failed-attempt limit.
///
/// It only gates the request; failed guesses are recorded afterwards by
/// [`record_failed_guesses`](crate::middlewares::failed_guess::record_failed_guesses), so
/// successful sign-ins never count.
pub(crate) struct Attempt;

impl FromRequest for Attempt {
    type Error = AuthError;
    type Future = Ready<Result<Self, AuthError>>;

    fn from_request(request: &HttpRequest, _: &mut Payload) -> Self::Future {
        let Some(state) = request.app_data::<web::Data<ApiState>>() else {
            return future::ready(Err(AuthError::Unavailable));
        };

        if !state.allow_attempt(request.peer_ip()) {
            return future::ready(Err(AuthError::Limited));
        }

        future::ready(Ok(Self))
    }
}
