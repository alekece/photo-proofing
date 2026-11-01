//! HTTP handlers, mounted under `/api/v1` by [`crate::api::configure_routes`].

pub mod auth;
pub mod galleries;
pub mod health;
pub mod photos;
