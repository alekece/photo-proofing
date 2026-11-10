use std::{future::Future, pin::Pin};

use actix_web::{FromRequest, HttpRequest, dev::Payload, web::Data};
use pp_database::{
    gallery::{GalleryRepository, ShareToken, SharedAccess},
    photo::{PhotoKind, PhotoLocation, Watermarked},
};

use crate::{api::ApiState, error::ErrorResponse};

/// The gallery behind the `{token}` path segment, which a client may see right now.
///
/// Every client route takes it. A malformed, unknown, unshared or expired link fails with the
/// same 404, decided at each request, so a page loaded before expiry loses its images too.
pub(crate) struct ClientAccess(pub SharedAccess);

/// The photos a client may see, chosen from the gallery state, never from the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientPhotoSet {
    Proofs,
    Finals,
}

impl ClientPhotoSet {
    pub(crate) fn kind(self) -> PhotoKind {
        match self {
            Self::Proofs => PhotoKind::Proof,
            Self::Finals => PhotoKind::Final,
        }
    }
}

impl ClientAccess {
    pub(crate) fn open_for_comments(&self) -> Result<PhotoLocation, ErrorResponse> {
        if self.0.gallery.delivered {
            return Err(ErrorResponse::gallery_delivered());
        }

        Ok(self.0.location)
    }

    pub(crate) fn photo_set(&self) -> ClientPhotoSet {
        if self.0.gallery.delivered {
            ClientPhotoSet::Finals
        } else {
            ClientPhotoSet::Proofs
        }
    }

    /// The files the client sees: proofs are marked when the gallery's switch is on, finals
    /// never are. Never chosen by the request.
    pub(crate) fn watermarked(&self) -> Watermarked {
        Watermarked(self.0.watermark_enabled && self.photo_set() == ClientPhotoSet::Proofs)
    }
}

impl FromRequest for ClientAccess {
    type Error = ErrorResponse;
    type Future = Pin<Box<dyn Future<Output = Result<Self, ErrorResponse>>>>;

    fn from_request(request: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let token = request
            .match_info()
            .get("token")
            .and_then(|token| token.parse::<ShareToken>().ok());
        let state = request.app_data::<Data<ApiState>>().cloned();

        Box::pin(async move {
            let token = token.ok_or_else(ErrorResponse::not_found)?;
            let state = state.ok_or_else(ErrorResponse::unavailable)?;

            state
                .database()
                .repository::<GalleryRepository>()
                .find_shared(&token)
                .await?
                .map(Self)
                .ok_or_else(ErrorResponse::not_found)
        })
    }
}
