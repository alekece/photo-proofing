//! Successful JSON responses shared by every route.

use actix_web::{
    HttpRequest, HttpResponse, Responder,
    body::BoxBody,
    http::{StatusCode, header},
};
use serde::Serialize;

/// A JSON body that must never be cached: every response here is specific to the session.
pub(crate) struct NoStore<T> {
    status: StatusCode,
    body: T,
}

impl<T> NoStore<T> {
    pub fn ok(body: T) -> Self {
        Self {
            status: StatusCode::OK,
            body,
        }
    }

    pub fn created(body: T) -> Self {
        Self {
            status: StatusCode::CREATED,
            body,
        }
    }
}

impl<T: Serialize> Responder for NoStore<T> {
    type Body = BoxBody;

    fn respond_to(self, _: &HttpRequest) -> HttpResponse {
        HttpResponse::build(self.status)
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(self.body)
    }
}
