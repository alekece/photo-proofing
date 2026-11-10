use std::{future::Future, pin::Pin};

use actix_web::{FromRequest, HttpRequest, dev::Payload, web::Data};
use pp_database::{
    comments::CommentAuthor,
    gallery::{GalleryId, GalleryRepository},
    photo::PhotoLocation,
};
use uuid::Uuid;

use crate::{
    api::ApiState,
    error::ErrorResponse,
    extractors::{client_access::ClientAccess, identity::Authenticated},
};

/// The comment thread scope of a request: a client under a share link, or the photographer
/// under their gallery. The author comes from the credentials, never from the input.
pub(crate) struct CommentAccess {
    pub location: PhotoLocation,
    pub author: CommentAuthor,
}

impl CommentAccess {
    pub(crate) fn pace(&self, state: &ApiState) -> Result<(), ErrorResponse> {
        if self.author == CommentAuthor::Client {
            state.pace_comments(self.location.gallery_id)?;
        }

        Ok(())
    }
}

impl FromRequest for CommentAccess {
    type Error = ErrorResponse;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(request: &HttpRequest, payload: &mut Payload) -> Self::Future {
        if request.match_info().get("token").is_some() {
            let access = ClientAccess::from_request(request, payload);

            return Box::pin(async move {
                let access = access.await?;

                Ok(Self {
                    location: access.open_for_comments()?,
                    author: CommentAuthor::Client,
                })
            });
        }

        let identity = Authenticated::from_request(request, payload);
        let id = request
            .match_info()
            .get("id")
            .and_then(|id| id.parse::<Uuid>().ok())
            .map(GalleryId);
        let state = request.app_data::<Data<ApiState>>().cloned();

        Box::pin(async move {
            let Authenticated(identity) = identity.await?;
            let gallery_id = id.ok_or_else(ErrorResponse::not_found)?;
            let state = state.ok_or_else(ErrorResponse::unavailable)?;

            state
                .database()
                .repository::<GalleryRepository>()
                .get(identity.workspace_id, gallery_id)
                .await?
                .ok_or_else(ErrorResponse::not_found)?;

            Ok(Self {
                location: PhotoLocation {
                    workspace_id: identity.workspace_id,
                    gallery_id,
                },
                author: CommentAuthor::Photographer,
            })
        })
    }
}
