//! The Origin check that defends cookie-authenticated, state-changing routes against CSRF.

use std::{
    future::{Future, Ready, ready},
    pin::Pin,
};

use actix_web::{
    Error,
    body::{EitherBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header,
    web::Data,
};

use crate::{api::ApiState, error::ErrorResponse, routes::auth::AuthError};

/// Rejects state-changing requests whose `Origin` is not the configured one, before their
/// payload is read.
pub(crate) struct SameOrigin;

impl<S, B> Transform<S, ServiceRequest> for SameOrigin
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = SameOriginService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SameOriginService { service }))
    }
}

pub(crate) struct SameOriginService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for SameOriginService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, request: ServiceRequest) -> Self::Future {
        let allowed = request.method().is_safe()
            || request.app_data::<Data<ApiState>>().is_some_and(|state| {
                let origin = request
                    .headers()
                    .get(header::ORIGIN)
                    .and_then(|value| value.to_str().ok());

                state.allows_origin(origin)
            });

        if !allowed {
            let response = request
                .error_response(ErrorResponse::from(AuthError::ForbiddenOrigin))
                .map_into_right_body();

            return Box::pin(ready(Ok(response)));
        }

        let response = self.service.call(request);

        Box::pin(async move { Ok(response.await?.map_into_left_body()) })
    }
}
