//! Helpers for building the HTTP responses shared by every route.

use actix_web::{HttpResponseBuilder, http::header};

/// Adds the headers that every route shares to a response under construction.
pub(crate) trait ResponseBuilderExt {
    /// Forbids caching: every response here is specific to the session.
    fn no_store(&mut self) -> &mut Self;
}

impl ResponseBuilderExt for HttpResponseBuilder {
    fn no_store(&mut self) -> &mut Self {
        self.insert_header((header::CACHE_CONTROL, "no-store"))
    }
}
