//! Helpers for building the HTTP responses shared by every route.

use actix_web::{
    HttpRequest, HttpResponse, HttpResponseBuilder, Responder,
    body::BoxBody,
    http::header::{self, ContentType},
};
use pp_storage::{PhotoKey, PhotoStorage, StorageError};
use tokio::io::AsyncReadExt;

use crate::error::ErrorResponse;

/// Adds the headers that every route shares to a response under construction.
pub(crate) trait ResponseBuilderExt {
    /// Forbids caching: every response here is specific to the session.
    fn no_store(&mut self) -> &mut Self;

    /// Lets the browser, but no shared cache, keep a private image for an hour.
    fn private_cache(&mut self) -> &mut Self;

    /// Keeps the page URL out of `Referer` headers sent onward, which matters when the URL
    /// carries a private link token.
    fn no_referrer(&mut self) -> &mut Self;
}

impl ResponseBuilderExt for HttpResponseBuilder {
    fn no_store(&mut self) -> &mut Self {
        self.insert_header((header::CACHE_CONTROL, "no-store"))
    }

    fn private_cache(&mut self) -> &mut Self {
        self.insert_header((header::CACHE_CONTROL, "private, max-age=3600"))
    }

    fn no_referrer(&mut self) -> &mut Self {
        self.insert_header((header::REFERRER_POLICY, "no-referrer"))
    }
}

/// A private image: kept by the browser only, never sniffed as another type, and never
/// named in a `Referer`. Callers include the published render revision in image URLs so a
/// regenerated derivative does not reuse the previous image's private cache entry.
pub(crate) struct ImageFile {
    content_type: ContentType,
    bytes: Vec<u8>,
}

impl ImageFile {
    pub(crate) fn jpeg(bytes: Vec<u8>) -> Self {
        Self {
            content_type: ContentType::jpeg(),
            bytes,
        }
    }

    /// Reads a stored JPEG; a file removed meanwhile is not found.
    pub(crate) async fn read(
        storage: &PhotoStorage,
        key: &PhotoKey,
    ) -> Result<Self, ErrorResponse> {
        let mut file = match storage.read(key).await {
            Ok(file) => file,
            Err(StorageError::NotFound) => return Err(ErrorResponse::not_found()),
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();

        file.read_to_end(&mut bytes)
            .await
            .map_err(|error| ErrorResponse::from(StorageError::from(error)))?;

        Ok(Self::jpeg(bytes))
    }
}

impl Responder for ImageFile {
    type Body = BoxBody;

    fn respond_to(self, _request: &HttpRequest) -> HttpResponse {
        HttpResponse::Ok()
            .content_type(self.content_type)
            .private_cache()
            .no_referrer()
            .insert_header((header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
            .body(self.bytes)
    }
}
