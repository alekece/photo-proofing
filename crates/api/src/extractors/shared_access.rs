use std::{future::Future, pin::Pin};

use actix_web::{FromRequest, HttpRequest, dev::Payload, web::Data};
use pp_database::gallery::{ShareToken, SharedGallery};

use crate::{api::ApiState, error::ErrorResponse};

/// The gallery that the private link in the request path grants access to.
///
/// This is the single place client routes authorize: a route that takes it is reachable only
/// with a live link, and later access rules (archive, deletion, delivery state) are added
/// here. A malformed link, an unknown one and an expired one all fail with the same
/// [`ErrorResponse::not_found`], and a malformed one never reaches the database.
///
/// The link is a bearer credential. No request logging exists today; any access log added
/// later must redact `/shared/` paths.
pub(crate) struct SharedAccess(pub SharedGallery);

impl FromRequest for SharedAccess {
    type Error = ErrorResponse;
    type Future = Pin<Box<dyn Future<Output = Result<Self, ErrorResponse>>>>;

    fn from_request(request: &HttpRequest, _: &mut Payload) -> Self::Future {
        let token = request
            .match_info()
            .get("token")
            .and_then(|token| token.parse::<ShareToken>().ok());
        let state = request.app_data::<Data<ApiState>>().cloned();

        Box::pin(async move {
            let state = state.ok_or_else(ErrorResponse::unavailable)?;
            let token = token.ok_or_else(ErrorResponse::not_found)?;

            state
                .galleries()
                .find_shared(&token)
                .await
                .map_err(|_| ErrorResponse::unavailable())?
                .map(Self)
                .ok_or_else(ErrorResponse::not_found)
        })
    }
}
