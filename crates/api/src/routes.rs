//! HTTP handlers, mounted under `/api/v1` by [`crate::api::configure_routes`].

pub mod auth;
pub mod contacts;
pub mod events;
pub mod galleries;
pub mod health;
pub mod language;
pub mod notifications;
pub mod settings;
pub mod upload;
