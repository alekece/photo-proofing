//! Gallery HTTP API: route wiring, process-wide authentication state and the
//! [`api::Api`] runner.

pub mod access_code;
pub mod api;
pub mod config;
pub mod credentials;
pub mod extractors;
pub mod routes;

mod comment_limiter;
mod error;
mod events;
mod limiter;
mod middlewares;
mod request;
mod response;
#[cfg(test)]
mod tests;
mod upload;
