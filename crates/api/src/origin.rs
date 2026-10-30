//! The Origin check that defends cookie-authenticated, state-changing routes against CSRF.

use actix_web::{
    Error,
    body::{EitherBody, MessageBody},
    dev::{ServiceRequest, ServiceResponse},
    http::header,
    middleware::Next,
    web,
};

use crate::{api::ApiState, routes::auth::AuthError};

/// Rejects state-changing requests whose `Origin` is not the configured one, before their
/// payload is read.
pub(crate) async fn require_same_origin<B: MessageBody + 'static>(
    request: ServiceRequest,
    next: Next<B>,
) -> Result<ServiceResponse<EitherBody<B>>, Error> {
    let allowed = request.method().is_safe()
        || request
            .app_data::<web::Data<ApiState>>()
            .is_some_and(|state| {
                let origin = request
                    .headers()
                    .get(header::ORIGIN)
                    .and_then(|value| value.to_str().ok());

                state.allows_origin(origin)
            });

    if !allowed {
        return Ok(request
            .error_response(AuthError::ForbiddenOrigin)
            .map_into_right_body());
    }

    Ok(next.call(request).await?.map_into_left_body())
}
