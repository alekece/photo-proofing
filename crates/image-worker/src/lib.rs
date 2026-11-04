//! The image worker: a durable PostgreSQL job runner.
//!
//! Job handlers are registered by kind in a [`HandlerRegistry`]; photo processing is served by
//! [`process_photo`](process_photo::process_photo).

pub mod config;
pub mod handlers;
pub mod process_photo;
pub mod render;
pub mod runtime;
