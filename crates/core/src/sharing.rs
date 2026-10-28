use std::{
    collections::HashMap,
    convert::Infallible,
    error::Error,
    future::Future,
    sync::{Mutex, MutexGuard, PoisonError},
    time::SystemTime,
};

use crate::auth::{
    password::PasswordHash,
    session::{SessionToken, SessionTokenHash},
};

pub mod rate_limit;
pub mod service;
#[cfg(test)]
mod testing;

/// Same 256-bit CSPRNG token as sessions; share links reuse its generation,
/// parsing, redaction and hashing so there is one audited implementation.
pub type LinkToken = SessionToken;
pub type LinkTokenHash = SessionTokenHash;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GalleryId(pub i64);

/// Persisted share link. Holds only the token hash and the passcode's Argon2id
/// hash; both redact themselves in `Debug`.
#[derive(Debug, Clone)]
pub struct ShareLink {
    pub gallery_id: GalleryId,
    pub token_hash: LinkTokenHash,
    pub passcode_hash: Option<PasswordHash>,
    pub created_at: SystemTime,
    pub expires_at: SystemTime,
    pub revoked_at: Option<SystemTime>,
}

impl ShareLink {
    pub fn is_usable_at(&self, now: SystemTime) -> bool {
        self.revoked_at.is_none() && now < self.expires_at
    }
}

pub trait ShareLinkStore: Send + Sync {
    type Error: Error + Send + Sync + 'static;

    fn insert(&self, link: ShareLink) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn find(
        &self,
        token_hash: &LinkTokenHash,
    ) -> impl Future<Output = Result<Option<ShareLink>, Self::Error>> + Send;

    /// Returns whether a live link was newly revoked.
    fn revoke(
        &self,
        token_hash: &LinkTokenHash,
        at: SystemTime,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send;
}

#[derive(Debug, Default)]
pub struct InMemoryShareLinkStore {
    links: Mutex<HashMap<LinkTokenHash, ShareLink>>,
}

impl InMemoryShareLinkStore {
    fn lock(&self) -> MutexGuard<'_, HashMap<LinkTokenHash, ShareLink>> {
        self.links.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl ShareLinkStore for InMemoryShareLinkStore {
    type Error = Infallible;

    async fn insert(&self, link: ShareLink) -> Result<(), Self::Error> {
        self.lock().insert(link.token_hash.clone(), link);

        Ok(())
    }

    async fn find(&self, token_hash: &LinkTokenHash) -> Result<Option<ShareLink>, Self::Error> {
        Ok(self.lock().get(token_hash).cloned())
    }

    async fn revoke(
        &self,
        token_hash: &LinkTokenHash,
        at: SystemTime,
    ) -> Result<bool, Self::Error> {
        let mut links = self.lock();
        let Some(link) = links
            .get_mut(token_hash)
            .filter(|link| link.revoked_at.is_none())
        else {
            return Ok(false);
        };

        link.revoked_at = Some(at);

        Ok(true)
    }
}
