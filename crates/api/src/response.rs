//! Helpers for building the HTTP responses shared by every route.

use actix_web::{
    HttpRequest, HttpResponse, HttpResponseBuilder, Responder,
    body::{BoxBody, SizedStream},
    http::header::{self, ContentDisposition, ContentType, DispositionParam, DispositionType},
};
use pp_storage::{PhotoKey, PhotoStorage, StorageError};
use tokio::{fs::File, io::AsyncReadExt};
use tokio_util::io::ReaderStream;

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
    cache: ImageCache,
}

enum ImageCache {
    Private,
    NoStore,
}

impl ImageFile {
    pub(crate) fn jpeg(bytes: Vec<u8>) -> Self {
        Self {
            content_type: ContentType::jpeg(),
            bytes,
            cache: ImageCache::Private,
        }
    }

    pub(crate) fn proof(mut self) -> Self {
        self.cache = ImageCache::NoStore;

        self
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
        let mut response = HttpResponse::Ok();

        match self.cache {
            ImageCache::Private => response.private_cache(),
            ImageCache::NoStore => response.no_store(),
        };

        response
            .content_type(self.content_type)
            .no_referrer()
            .insert_header((header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
            .body(self.bytes)
    }
}

/// A stored file streamed as a private download: saved under `name`, never cached, never
/// sniffed as another type, and never named in a `Referer`.
pub(crate) struct Attachment {
    content_type: &'static str,
    name: String,
    length: u64,
    file: File,
}

impl Attachment {
    pub(crate) async fn new(
        content_type: &'static str,
        name: String,
        file: File,
    ) -> Result<Self, ErrorResponse> {
        let length = file
            .metadata()
            .await
            .map_err(|error| ErrorResponse::from(StorageError::from(error)))?
            .len();

        Ok(Self {
            content_type,
            name,
            length,
            file,
        })
    }

    pub(crate) async fn zip(name: &str, file: File) -> Result<Self, ErrorResponse> {
        Self::new("application/zip", name.to_owned(), file).await
    }
}

impl Responder for Attachment {
    type Body = BoxBody;

    fn respond_to(self, _request: &HttpRequest) -> HttpResponse {
        HttpResponse::Ok()
            .content_type(self.content_type)
            .insert_header(ContentDisposition {
                disposition: DispositionType::Attachment,
                parameters: vec![DispositionParam::Filename(self.name)],
            })
            .insert_header((header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
            .insert_header((header::CACHE_CONTROL, "private, no-store"))
            .no_referrer()
            .body(SizedStream::new(self.length, ReaderStream::new(self.file)))
    }
}
