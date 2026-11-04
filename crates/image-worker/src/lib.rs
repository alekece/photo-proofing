//! The image worker: a durable PostgreSQL job runner.
//!
//! Job handlers are registered by kind in a [`HandlerRegistry`]; photo processing is served by
//! [`ProcessPhotoRoutine`](routines::process_photo::ProcessPhotoRoutine).

pub mod config;
pub mod handlers;
pub mod render;
pub mod routines;
pub mod runtime;
