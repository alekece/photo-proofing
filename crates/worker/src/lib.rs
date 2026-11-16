//! The background worker: a durable PostgreSQL job runner.
//!
//! Job handlers are registered by kind in a [`HandlerRegistry`]; photo processing, exports,
//! cleanup, and outbound email delivery run here.

pub mod config;
pub mod handlers;
pub mod maintenance;
pub mod render;
pub mod runtime;
mod watermark;
