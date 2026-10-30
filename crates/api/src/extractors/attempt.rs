use std::{
    future::{self, Ready},
    net::{IpAddr, Ipv4Addr},
};

use actix_web::{FromRequest, HttpRequest, dev::Payload, web};

use crate::{ApiState, routes::auth::AuthError};

/// Extractor for a sign-in request from a peer that is still under the failed-attempt limit.
///
/// Handlers report a failed credential check with
/// [`ApiState::record_failure`]`(attempt.peer())`; only failures count.
pub(crate) struct Attempt {
    peer: IpAddr,
}

impl Attempt {
    pub fn peer(&self) -> IpAddr {
        self.peer
    }

    pub fn peer_of(request: &HttpRequest) -> IpAddr {
        request
            .peer_addr()
            .map(|addr| addr.ip())
            .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
    }
}

impl FromRequest for Attempt {
    type Error = AuthError;
    type Future = Ready<Result<Self, AuthError>>;

    fn from_request(request: &HttpRequest, _: &mut Payload) -> Self::Future {
        let Some(state) = request.app_data::<web::Data<ApiState>>() else {
            return future::ready(Err(AuthError::Unavailable));
        };

        let peer = Self::peer_of(request);

        if !state.allow_attempt(peer) {
            return future::ready(Err(AuthError::Limited));
        }

        future::ready(Ok(Self { peer }))
    }
}
