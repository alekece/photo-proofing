//! The image worker: a durable PostgreSQL job runner.
//!
//! Job handlers are registered by kind in a [`HandlerRegistry`]; image processing registers
//! its handler in a later slice.

pub mod config;
pub mod handlers;
pub mod render;
pub mod runtime;
