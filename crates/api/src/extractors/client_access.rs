use std::{future::Future, pin::Pin};

use actix_web::{FromRequest, HttpRequest, dev::Payload, web::Data};
use pp_core::notifications::PhotoId;
use pp_database::{
    gallery::{GalleryRepository, ShareToken, SharedAccess},
    photo::{FavoriteChange, PhotoRepository},
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
}

impl ClientAccess {
    pub(crate) fn photo_set(&self) -> ClientPhotoSet {
        ClientPhotoSet::Proofs
    }

    /// Adds or removes a favorite; a photo outside the link's gallery is a 404 and a delivered
    /// gallery a 409.
    pub(crate) async fn set_favorite(
        &self,
        state: &ApiState,
        photo_id: PhotoId,
        favorite: bool,
    ) -> Result<(), ErrorResponse> {
        let change = state
            .database()
            .repository::<PhotoRepository>()
            .set_favorite(self.0.location, photo_id, favorite)
            .await?;

        match change {
            FavoriteChange::Applied => Ok(()),
            FavoriteChange::NotFound => Err(ErrorResponse::not_found()),
            FavoriteChange::Delivered => Err(ErrorResponse::conflict("gallery already delivered")),
        }
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
