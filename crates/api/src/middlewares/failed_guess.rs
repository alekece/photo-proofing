//! Counts failed credential guesses for the failed-attempt limiter.
//!
//! The extractor that parses a credential body rejects it before any handler runs, so neither
//! side alone sees every guess. Instead, an error that stands for a guess marks its response
//! with [`FailedGuess`] and this middleware, wrapped around the sign-in routes, records the
//! peer once the response is known. Other 4xx responses (size, content type, limits) carry no
//! marker and never count.

use actix_web::{
    HttpResponse,
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    middleware::Next,
    web::Data,
};

use crate::{api::ApiState, request::RequestExt};

/// Response extension marking an error as a failed credential guess.
pub(crate) struct FailedGuess;

impl FailedGuess {
    /// Marks `response` as the answer to a failed guess.
    pub fn mark(mut response: HttpResponse) -> HttpResponse {
        response.extensions_mut().insert(Self);

        response
    }
}

/// Records the peer of every request whose response is marked [`FailedGuess`].
pub(crate) async fn record_failed_guesses(
    request: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let state = request.app_data::<Data<ApiState>>().cloned();
    let peer = request.request().peer_ip();
    let response = next.call(request).await?;

    if let Some(state) = state
        && response
            .response()
            .extensions()
            .get::<FailedGuess>()
            .is_some()
    {
        state.record_failure(peer);
    }

    Ok(response)
}
