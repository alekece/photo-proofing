//! Limits failed sign-in attempts per peer.
//!
//! [`FailedAttempts::limit`] refuses a peer already over the limit with `429` before the route
//! runs, then records the peer when the route answers with a client error. `429` is the
//! limiter's own refusal and is never counted, and neither are server faults, so an outage cannot
//! lock out honest peers. Wrap it with [`from_fn`](actix_web::middleware::from_fn).

use actix_web::{
    Error,
    body::{BoxBody, MessageBody},
    dev::{ServiceRequest, ServiceResponse},
    http::StatusCode,
    middleware::Next,
    web::Data,
};

use crate::{api::ApiState, request::RequestExt, routes::auth::AuthError};

/// Namespace of the failed-attempt middleware.
pub(crate) struct FailedAttempts;

impl FailedAttempts {
    /// The middleware function; actix only accepts a function here, so this is the one
    /// function that cannot be a trait or extractor.
    pub async fn limit<B: MessageBody + 'static>(
        request: ServiceRequest,
        next: Next<B>,
    ) -> Result<ServiceResponse<BoxBody>, Error> {
        let Some(state) = request.app_data::<Data<ApiState>>().cloned() else {
            return Ok(request.error_response(AuthError::Unavailable));
        };
        let peer = request.request().peer_ip();

        if !state.allow_attempt(peer) {
            return Ok(request.error_response(AuthError::Limited));
        }

        let response = next.call(request).await?;
        let status = response.status();

        if status.is_client_error() && status != StatusCode::TOO_MANY_REQUESTS {
            state.record_failure(peer);
        }

        Ok(response.map_into_boxed_body())
    }
}

#[cfg(test)]
mod tests {
    use std::{net::Ipv4Addr, num::NonZeroU32, num::NonZeroUsize};

    use actix_web::{
        App, HttpResponse,
        middleware::from_fn,
        test,
        web::{self, Data},
    };
    use pp_database::Database;
    use sqlx::postgres::PgPoolOptions;

    use super::*;
    use crate::{
        api::ApiState,
        config::{AuthConfig, LimiterConfig, SessionConfig},
        tests,
    };

    // The pool is lazy and these tests never query, so no database is needed.
    async fn state() -> (Data<ApiState>, tempfile::TempDir) {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://localhost/unused")
            .unwrap();
        let session = SessionConfig {
            idle_timeout: "1h".parse().unwrap(),
            secure_cookie: Some(false),
            cookie_name: "pp_session".parse().unwrap(),
        };
        let auth = AuthConfig {
            app_origin: "http://localhost:5173".parse().unwrap(),
            hash_concurrency: NonZeroUsize::new(2).unwrap(),
        };
        let limiter = LimiterConfig {
            max_failures: NonZeroU32::new(2).unwrap(),
            window: "1m".parse().unwrap(),
            max_peers: NonZeroUsize::new(8).unwrap(),
        };

        let (storage, upload_config, directory) = tests::upload_resources().await;

        (
            Data::new(
                ApiState::new(
                    Database::new(pool),
                    session,
                    auth,
                    limiter,
                    storage,
                    upload_config,
                    tests::queue_config(5),
                )
                .unwrap(),
            ),
            directory,
        )
    }

    async fn respond(status: web::Path<u16>) -> HttpResponse {
        HttpResponse::new(StatusCode::from_u16(*status).unwrap())
    }

    #[actix_web::test]
    async fn client_errors_except_too_many_requests_count() {
        let (state, _directory) = state().await;
        let app = test::init_service(
            App::new().app_data(state.clone()).service(
                web::resource("/{status}")
                    .wrap(from_fn(FailedAttempts::limit))
                    .to(respond),
            ),
        )
        .await;

        for (peer, (status, counts)) in (1..).zip([
            (400, true),
            (401, true),
            (413, true),
            (415, true),
            (422, true),
            (200, false),
            (429, false),
            (503, false),
        ]) {
            let peer = Ipv4Addr::new(192, 0, 2, peer);
            let requests = if counts { 2 } else { 5 };
            let status = StatusCode::from_u16(status).unwrap();

            for _ in 0..requests {
                let request = test::TestRequest::get()
                    .uri(&format!("/{}", status.as_u16()))
                    .peer_addr((peer, 1000).into())
                    .to_request();

                assert_eq!(test::call_service(&app, request).await.status(), status);
            }

            assert_eq!(state.allow_attempt(peer.into()), !counts, "{status}");
        }
    }

    #[actix_web::test]
    async fn a_peer_over_the_limit_is_refused_before_the_route() {
        let (state, _directory) = state().await;
        let app = test::init_service(
            App::new().app_data(state).service(
                web::resource("/{status}")
                    .wrap(from_fn(FailedAttempts::limit))
                    .to(respond),
            ),
        )
        .await;
        let mut seen = Vec::new();

        for status in [401, 401, 200] {
            let request = test::TestRequest::get()
                .uri(&format!("/{status}"))
                .to_request();

            seen.push(test::call_service(&app, request).await.status());
        }

        assert_eq!(
            seen,
            [
                StatusCode::UNAUTHORIZED,
                StatusCode::UNAUTHORIZED,
                StatusCode::TOO_MANY_REQUESTS
            ]
        );
    }
}
