//! The image worker: a durable PostgreSQL job runner.
//!
//! Job handlers are registered by kind in a [`HandlerRegistry`]; photo processing is served by
//! [`ProcessPhotoHandler`](handlers::process_photo::ProcessPhotoHandler).

pub mod config;
pub mod handlers;
pub mod render;
pub mod runtime;
mod watermark;
