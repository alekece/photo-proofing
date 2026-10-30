//! Helpers for the successful responses shared by every route.

use actix_web::{CustomizeResponder, Responder, http::header};

/// Adds the response headers that every route shares to any [`Responder`].
pub(crate) trait ResponderExt: Responder + Sized {
    /// Forbids caching: every response here is specific to the session.
    fn no_store(self) -> CustomizeResponder<Self> {
        self.customize()
            .insert_header((header::CACHE_CONTROL, "no-store"))
    }
}

impl<T: Responder> ResponderExt for T {}
