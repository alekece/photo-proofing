//! Gallery HTTP API: route wiring, process-wide authentication state and the
//! [`api::Api`] runner.

pub mod api;
pub mod config;
pub mod credentials;
pub mod extractors;
pub mod routes;

mod error;
mod limiter;
mod middlewares;
mod request;
mod response;
