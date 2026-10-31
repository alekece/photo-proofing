//! Records failed sign-in attempts for the failed-attempt limiter.
//!
//! The middleware is generic: after the wrapped route answers, it records the peer when a
//! predicate accepts the response status. Which statuses count is policy, chosen where the
//! routes are defined. The default counts every client error except `429`, which is the
//! limiter's own refusal, and never counts server faults, so an outage cannot lock out honest
//! peers. [`Attempt`](crate::extractors::attempt::Attempt) is the matching gate that refuses
//! peers already over the limit.

use std::{
    future::{Future, Ready, ready},
    pin::Pin,
    rc::Rc,
};

use actix_web::{
    Error,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::StatusCode,
    web::Data,
};

use crate::{api::ApiState, request::RequestExt};

fn default_counts(status: StatusCode) -> bool {
    status.is_client_error() && status != StatusCode::TOO_MANY_REQUESTS
}

/// Middleware that records the peer of every request whose response status is counted.
pub(crate) struct FailedAttempts<F>
where
    F: Fn(StatusCode) -> bool,
{
    // Shared with each service because `new_transform` only borrows `self` and the response
    // future outlives `call`'s borrow; `Rc` suffices as actix workers are single-threaded.
    predicate: Rc<F>,
}

impl FailedAttempts<fn(StatusCode) -> bool> {
    /// Counts every `4xx` response except `429`.
    pub fn new() -> Self {
        Self::counting(default_counts)
    }
}

impl Default for FailedAttempts<fn(StatusCode) -> bool> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F> FailedAttempts<F>
where
    F: Fn(StatusCode) -> bool + 'static,
{
    /// Counts the responses whose status satisfies `predicate`.
    pub fn counting(predicate: F) -> Self {
        Self {
            predicate: Rc::new(predicate),
        }
    }
}

impl<S, B, F> Transform<S, ServiceRequest> for FailedAttempts<F>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
    F: Fn(StatusCode) -> bool + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = FailedAttemptsService<S, F>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(FailedAttemptsService {
            service,
            predicate: Rc::clone(&self.predicate),
        }))
    }
}

pub(crate) struct FailedAttemptsService<S, F> {
    service: S,
    predicate: Rc<F>,
}

impl<S, B, F> Service<ServiceRequest> for FailedAttemptsService<S, F>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
    F: Fn(StatusCode) -> bool + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, request: ServiceRequest) -> Self::Future {
        let state = request.app_data::<Data<ApiState>>().cloned();
        let peer = request.request().peer_ip();
        let predicate = Rc::clone(&self.predicate);
        let response = self.service.call(request);

        Box::pin(async move {
            let response = response.await?;

            if let Some(state) = state
                && predicate(response.status())
            {
                state.record_failure(peer);
            }

            Ok(response)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{net::IpAddr, num::NonZeroU32, num::NonZeroUsize};

    use actix_web::{
        App, HttpResponse,
        http::StatusCode,
        test,
        web::{self, Data},
    };
    use pp_database::Database;
    use sqlx::postgres::PgPoolOptions;

    use super::*;
    use crate::{
        api::ApiState,
        config::{AuthConfig, LimiterConfig, SessionConfig},
    };

    const PEER: IpAddr = IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);

    // The pool is lazy and these tests never query, so no database is needed.
    fn state() -> Data<ApiState> {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://localhost/unused")
            .unwrap();
        let session = SessionConfig {
            idle_timeout: "1h".parse().unwrap(),
            cookie_name: "pp_session".parse().unwrap(),
        };
        let auth = AuthConfig {
            app_origin: "http://localhost:5173".parse().unwrap(),
        };
        let limiter = LimiterConfig {
            max_failures: NonZeroU32::new(2).unwrap(),
            window: "1m".parse().unwrap(),
            max_peers: NonZeroUsize::new(8).unwrap(),
        };

        Data::new(ApiState::new(Database::new(pool), session, auth, limiter).unwrap())
    }

    async fn respond(status: web::Path<u16>) -> HttpResponse {
        HttpResponse::new(StatusCode::from_u16(*status).unwrap())
    }

    /// Calls a route that answers with each of `statuses` in turn and reports whether the peer
    /// may still attempt afterwards.
    async fn allowed_after<F>(middleware: FailedAttempts<F>, statuses: &[u16]) -> bool
    where
        F: Fn(StatusCode) -> bool + 'static,
    {
        let state = state();
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .service(web::resource("/{status}").wrap(middleware).to(respond)),
        )
        .await;

        for status in statuses {
            let request = test::TestRequest::get()
                .uri(&format!("/{status}"))
                .to_request();

            test::call_service(&app, request).await;
        }

        state.allow_attempt(PEER)
    }

    #[actix_web::test]
    async fn default_counts_client_errors_except_too_many_requests() {
        for status in [400, 401, 413, 415, 422] {
            assert!(
                !allowed_after(FailedAttempts::new(), &[status, status]).await,
                "{status} should count"
            );
        }

        for status in [200, 429, 503] {
            assert!(
                allowed_after(FailedAttempts::new(), &[status; 5]).await,
                "{status} should not count"
            );
        }
    }

    #[actix_web::test]
    async fn custom_predicate_replaces_the_default() {
        let conflicts = || FailedAttempts::counting(|status| status == StatusCode::CONFLICT);

        assert!(!allowed_after(conflicts(), &[409, 409]).await);
        assert!(allowed_after(conflicts(), &[401, 401, 401]).await);
    }

    #[actix_web::test]
    async fn closure_may_capture_non_clone_state() {
        struct Counted(StatusCode);

        let only = Counted(StatusCode::UNAUTHORIZED);
        let middleware = FailedAttempts::counting(move |status| status == only.0);

        assert!(!allowed_after(middleware, &[401, 401]).await);
    }
}
