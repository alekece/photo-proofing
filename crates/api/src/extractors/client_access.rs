use std::{future::Future, pin::Pin};

use actix_web::{FromRequest, HttpRequest, dev::Payload, web::Data};
use pp_core::types::access_code::UNLOCK_COOKIE_NAME;
use pp_database::{
    gallery::{GalleryRepository, ShareToken, SharedAccess},
    photo::{PhotoKind, PhotoLocation, Watermarked},
};

use crate::{api::ApiState, error::ErrorResponse};

/// Active shared gallery behind `{token}`, together with whether the viewer has unlocked it.
///
/// Only the client gate routes (`GET /shared/{token}`, `/cover`, `/logo`, and `POST /unlock`)
/// take [`SharedLinkAccess`]. Malformed, unknown, unshared, archived, and expired tokens always
/// fail with `404` before any lock state is inspected.
pub(crate) struct SharedLinkAccess {
    pub(crate) access: SharedAccess,
    pub(crate) unlocked: bool,
}

impl FromRequest for SharedLinkAccess {
    type Error = ErrorResponse;
    type Future = Pin<Box<dyn Future<Output = Result<Self, ErrorResponse>>>>;

    fn from_request(request: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let token = request
            .match_info()
            .get("token")
            .and_then(|token| token.parse::<ShareToken>().ok());
        let cookie_value = request
            .cookie(UNLOCK_COOKIE_NAME)
            .map(|cookie| cookie.value().to_owned());
        let state = request.app_data::<Data<ApiState>>().cloned();

        Box::pin(async move {
            let token = token.ok_or_else(ErrorResponse::not_found)?;
            let state = state.ok_or_else(ErrorResponse::unavailable)?;
            let access = state
                .database()
                .repository::<GalleryRepository>()
                .find_shared(&token)
                .await?
                .ok_or_else(ErrorResponse::not_found)?;
            let unlocked = match &access.access_code_ciphertext {
                None => true,
                Some(ciphertext) => state.access_code_cipher().is_some_and(|cipher| {
                    cipher.is_unlocked(
                        cookie_value.as_deref(),
                        access.location.gallery_id,
                        ciphertext,
                    )
                }),
            };

            Ok(Self { access, unlocked })
        })
    }
}

/// The unlocked gallery behind the `{token}` path segment, which a client may browse right now.
///
/// Every content, favorite, comment, download, and open route takes it. Unavailable links fail
/// with `404`, and code-protected galleries without a valid unlock cookie fail with `401`.
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

    fn from_request(request: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let link = SharedLinkAccess::from_request(request, payload);

        Box::pin(async move {
            let link = link.await?;

            if !link.unlocked {
                return Err(ErrorResponse::gallery_locked());
            }

            Ok(Self(link.access))
        })
    }
}
