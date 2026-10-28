use std::{error::Error, fmt, time::Duration};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use thiserror::Error;

use super::{
    GalleryId, LinkToken, LinkTokenHash, ShareLink, ShareLinkStore,
    rate_limit::{Locked, RateLimitKey, RateLimiter},
};
use crate::auth::{
    password::{Password, PasswordError, PasswordHasher},
    session::{Clock, SystemClock},
};

const KEY_SCOPE_HASH_BYTES: usize = 6;
const MALFORMED_TOKEN_SCOPE: &str = "malformed";

/// Every way a link can be refused except throttling and infrastructure faults
/// collapses into `Invalid`: unknown, malformed, expired, revoked, wrong or missing
/// passcode. Do not log or answer with anything finer-grained than this, and map
/// it to one generic response so nothing about the link or gallery leaks.
/// `RateLimited` depends only on the caller's own failed attempts, never on whether
/// a link exists.
#[derive(Debug, Error)]
pub enum ShareError<S: Error + 'static, L: Error + 'static> {
    #[error("share link is not available")]
    Invalid,
    #[error("too many failed attempts")]
    RateLimited { retry_after: Duration },
    #[error("share link store failure")]
    Store(#[source] S),
    #[error("rate limiter failure")]
    Limiter(#[source] L),
    #[error("passcode handling failure")]
    Passcode(#[source] PasswordError),
}

/// What a successful verification discloses: nothing beyond the gallery to open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedLink {
    pub gallery_id: GalleryId,
}

pub struct ShareLinkService<S, L, C = SystemClock> {
    store: S,
    client_limiter: L,
    link_limiter: L,
    hasher: PasswordHasher,
    clock: C,
}

impl<S, L, C> fmt::Debug for ShareLinkService<S, L, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareLinkService").finish_non_exhaustive()
    }
}

impl<S: ShareLinkStore, L: RateLimiter> ShareLinkService<S, L> {
    pub fn new(store: S, client_limiter: L, link_limiter: L, hasher: PasswordHasher) -> Self {
        Self::with_clock(store, client_limiter, link_limiter, hasher, SystemClock)
    }
}

impl<S: ShareLinkStore, L: RateLimiter, C: Clock> ShareLinkService<S, L, C> {
    pub fn with_clock(
        store: S,
        client_limiter: L,
        link_limiter: L,
        hasher: PasswordHasher,
        clock: C,
    ) -> Self {
        Self {
            store,
            client_limiter,
            link_limiter,
            hasher,
            clock,
        }
    }

    /// Returns the raw token exactly once; only its hash is stored.
    pub async fn create_link(
        &self,
        gallery_id: GalleryId,
        passcode: Option<&Password>,
        ttl: Duration,
    ) -> Result<(LinkToken, ShareLink), ShareError<S::Error, L::Error>> {
        let passcode_hash = passcode
            .map(|passcode| self.hasher.hash(passcode))
            .transpose()
            .map_err(ShareError::Passcode)?;
        let token = LinkToken::generate();
        let now = self.clock.now();
        let link = ShareLink {
            gallery_id,
            token_hash: token.hash(),
            passcode_hash,
            created_at: now,
            expires_at: now + ttl,
            revoked_at: None,
        };

        self.store
            .insert(link.clone())
            .await
            .map_err(ShareError::Store)?;

        Ok((token, link))
    }

    pub async fn revoke_link(
        &self,
        token_hash: &LinkTokenHash,
    ) -> Result<bool, ShareError<S::Error, L::Error>> {
        self.store
            .revoke(token_hash, self.clock.now())
            .await
            .map_err(ShareError::Store)
    }

    /// `client` identifies the caller (e.g. the peer IP). Attempts are throttled per
    /// client, per client and link (token-hash prefix), and per link alone, the last
    /// through `link_limiter`, which should be configured with a higher threshold:
    /// it stops a leaked link's passcode being guessed from many addresses, at the
    /// cost that heavy guessing can temporarily lock legitimate viewers out of that
    /// link. Attempts are reserved before any work and refunded on success, so
    /// concurrent guesses cannot exceed the limits.
    ///
    /// Every refusal costs one Argon2 verification (real or decoy) so response time
    /// does not reveal whether the link exists.
    pub async fn verify(
        &self,
        token: &str,
        passcode: Option<&Password>,
        client: &RateLimitKey,
    ) -> Result<VerifiedLink, ShareError<S::Error, L::Error>> {
        let token = LinkToken::parse(token);
        let token_hash = token.as_ref().map(LinkToken::hash);
        let scope = token_hash
            .as_ref()
            .map_or(MALFORMED_TOKEN_SCOPE.to_owned(), key_scope);
        let reservations = [
            (&self.client_limiter, client.clone()),
            (&self.client_limiter, client.scoped(&scope)),
            (
                &self.link_limiter,
                RateLimitKey::new(format!("link|{scope}")),
            ),
        ];

        self.reserve_attempts(&reservations).await?;

        let link = match &token_hash {
            Some(hash) => self.store.find(hash).await.map_err(ShareError::Store)?,
            None => None,
        };
        let passcode_ok = self.check_passcode(link.as_ref(), passcode)?;
        let now = self.clock.now();

        match link {
            Some(link) if passcode_ok && link.is_usable_at(now) => {
                self.refund_attempts(&reservations).await?;

                Ok(VerifiedLink {
                    gallery_id: link.gallery_id,
                })
            }
            _ => Err(ShareError::Invalid),
        }
    }

    async fn reserve_attempts(
        &self,
        reservations: &[(&L, RateLimitKey)],
    ) -> Result<(), ShareError<S::Error, L::Error>> {
        for (index, (limiter, key)) in reservations.iter().enumerate() {
            let outcome = limiter.reserve(key).await.map_err(ShareError::Limiter)?;

            if let Err(Locked { retry_after }) = outcome {
                self.refund_attempts(&reservations[..index]).await?;

                return Err(ShareError::RateLimited { retry_after });
            }
        }

        Ok(())
    }

    async fn refund_attempts(
        &self,
        reservations: &[(&L, RateLimitKey)],
    ) -> Result<(), ShareError<S::Error, L::Error>> {
        for (limiter, key) in reservations {
            limiter.refund(key).await.map_err(ShareError::Limiter)?;
        }

        Ok(())
    }

    /// Always performs exactly one Argon2 verification, whatever the link state,
    /// so expired, revoked, unknown and wrong-passcode cases cost the same.
    fn check_passcode(
        &self,
        link: Option<&ShareLink>,
        passcode: Option<&Password>,
    ) -> Result<bool, ShareError<S::Error, L::Error>> {
        let empty = Password::new("");
        let supplied = passcode.unwrap_or(&empty);

        match link {
            Some(ShareLink {
                passcode_hash: Some(hash),
                ..
            }) => self
                .hasher
                .verify(supplied, hash)
                .map_err(ShareError::Passcode),
            Some(_) => {
                self.hasher.verify_decoy(supplied);

                Ok(true)
            }
            None => {
                self.hasher.verify_decoy(supplied);

                Ok(false)
            }
        }
    }
}

/// Short, non-reversible tag of the token hash; safe to embed in a limiter key.
fn key_scope(hash: &LinkTokenHash) -> String {
    URL_SAFE_NO_PAD.encode(&hash.as_bytes()[..KEY_SCOPE_HASH_BYTES])
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use super::*;
    use crate::{
        auth::password::HashingParams,
        sharing::{
            InMemoryShareLinkStore,
            rate_limit::{InMemoryRateLimiter, RateLimitConfig},
            testing::ManualClock,
        },
    };

    const TTL: Duration = Duration::from_secs(3600);
    const PASSCODE: &str = "open sesame please";
    const GALLERY: GalleryId = GalleryId(7);
    const CONFIG: RateLimitConfig = RateLimitConfig {
        max_failures: 3,
        window: Duration::from_secs(60),
        lockout: Duration::from_secs(300),
        max_keys: 100,
    };

    const LINK_CONFIG: RateLimitConfig = RateLimitConfig {
        max_failures: 6,
        ..CONFIG
    };

    type TestService =
        ShareLinkService<InMemoryShareLinkStore, InMemoryRateLimiter<ManualClock>, ManualClock>;

    fn service() -> (TestService, ManualClock) {
        let clock = ManualClock::new();
        let hasher = PasswordHasher::new(HashingParams::insecure_for_tests()).expect("test params");
        let service = ShareLinkService::with_clock(
            InMemoryShareLinkStore::default(),
            InMemoryRateLimiter::with_clock(CONFIG, clock.clone()),
            InMemoryRateLimiter::with_clock(LINK_CONFIG, clock.clone()),
            hasher,
            clock.clone(),
        );

        (service, clock)
    }

    fn client() -> RateLimitKey {
        RateLimitKey::new("203.0.113.9")
    }

    fn passcode(value: &str) -> Password {
        Password::new(value)
    }

    async fn open(
        service: &TestService,
        token: &str,
        passcode: Option<&Password>,
    ) -> Result<VerifiedLink, ShareError<Infallible, Infallible>> {
        service.verify(token, passcode, &client()).await
    }

    #[tokio::test]
    async fn valid_link_without_passcode_opens_its_gallery() {
        let (service, _) = service();
        let (token, _) = service
            .create_link(GALLERY, None, TTL)
            .await
            .expect("link is created");

        let verified = open(&service, token.expose(), None)
            .await
            .expect("link verifies");

        assert_eq!(verified.gallery_id, GALLERY);
    }

    #[tokio::test]
    async fn valid_link_with_right_passcode_opens() {
        let (service, _) = service();
        let (token, link) = service
            .create_link(GALLERY, Some(&passcode(PASSCODE)), TTL)
            .await
            .expect("link is created");

        assert!(link.passcode_hash.is_some());
        assert!(
            open(&service, token.expose(), Some(&passcode(PASSCODE)))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn stored_link_keeps_only_hashes() {
        let (service, _) = service();
        let (token, link) = service
            .create_link(GALLERY, Some(&passcode(PASSCODE)), TTL)
            .await
            .expect("link is created");

        assert_eq!(link.token_hash, token.hash());
        assert!(
            !link
                .passcode_hash
                .as_ref()
                .expect("passcode hash")
                .as_phc()
                .contains(PASSCODE)
        );
    }

    #[tokio::test]
    async fn every_refusal_is_the_same_invalid_error() {
        let (service, clock) = service();
        let (unprotected, _) = service
            .create_link(GALLERY, None, TTL)
            .await
            .expect("link is created");
        let (protected, _) = service
            .create_link(GALLERY, Some(&passcode(PASSCODE)), TTL)
            .await
            .expect("link is created");
        let (revoked, revoked_link) = service
            .create_link(GALLERY, None, TTL)
            .await
            .expect("link is created");
        let (short, _) = service
            .create_link(GALLERY, None, Duration::from_secs(10))
            .await
            .expect("link is created");

        service
            .revoke_link(&revoked_link.token_hash)
            .await
            .expect("revoke works");
        clock.advance(Duration::from_secs(11));

        let unknown = LinkToken::generate();
        let wrong = passcode("wrong passcode!");
        let requests = [
            (unknown.expose(), None),
            ("not-a-token", None),
            (protected.expose(), Some(&wrong)),
            (protected.expose(), None),
            (revoked.expose(), None),
            (short.expose(), None),
        ];
        let mut attempts = Vec::new();

        for (index, (token, passcode)) in requests.into_iter().enumerate() {
            let client = RateLimitKey::new(format!("client-{index}"));

            attempts.push(service.verify(token, passcode, &client).await);
        }

        for attempt in attempts {
            let error = attempt.expect_err("must be refused");

            assert!(matches!(error, ShareError::Invalid), "got {error:?}");
            assert_eq!(error.to_string(), "share link is not available");
        }

        assert!(open(&service, unprotected.expose(), None).await.is_ok());
    }

    #[tokio::test]
    async fn expired_link_is_refused_even_with_the_right_passcode() {
        let (service, clock) = service();
        let (token, _) = service
            .create_link(GALLERY, Some(&passcode(PASSCODE)), TTL)
            .await
            .expect("link is created");

        clock.advance(TTL);

        assert!(matches!(
            open(&service, token.expose(), Some(&passcode(PASSCODE))).await,
            Err(ShareError::Invalid)
        ));
    }

    #[tokio::test]
    async fn revoked_link_is_refused_and_revocation_is_idempotent() {
        let (service, _) = service();
        let (token, link) = service
            .create_link(GALLERY, None, TTL)
            .await
            .expect("link is created");

        assert!(service.revoke_link(&link.token_hash).await.expect("revoke"));
        assert!(!service.revoke_link(&link.token_hash).await.expect("revoke"));
        assert!(matches!(
            open(&service, token.expose(), None).await,
            Err(ShareError::Invalid)
        ));
    }

    #[tokio::test]
    async fn repeated_wrong_passcodes_lock_out_even_the_right_one() {
        let (service, _) = service();
        let (token, _) = service
            .create_link(GALLERY, Some(&passcode(PASSCODE)), TTL)
            .await
            .expect("link is created");

        for _ in 0..CONFIG.max_failures {
            let _ = open(&service, token.expose(), Some(&passcode("wrong passcode!"))).await;
        }

        assert!(matches!(
            open(&service, token.expose(), Some(&passcode(PASSCODE))).await,
            Err(ShareError::RateLimited { .. })
        ));
    }

    #[tokio::test]
    async fn token_guessing_is_throttled_across_different_tokens() {
        let (service, _) = service();

        for _ in 0..CONFIG.max_failures {
            let guess = LinkToken::generate();

            assert!(matches!(
                open(&service, guess.expose(), None).await,
                Err(ShareError::Invalid)
            ));
        }

        let guess = LinkToken::generate();

        assert!(matches!(
            open(&service, guess.expose(), None).await,
            Err(ShareError::RateLimited { .. })
        ));
    }

    #[tokio::test]
    async fn guessing_a_passcode_from_many_clients_locks_the_link() {
        let (service, _) = service();
        let (token, _) = service
            .create_link(GALLERY, Some(&passcode(PASSCODE)), TTL)
            .await
            .expect("link is created");
        let wrong = passcode("wrong passcode!");

        for index in 0..LINK_CONFIG.max_failures {
            let client = RateLimitKey::new(format!("client-{index}"));

            assert!(matches!(
                service.verify(token.expose(), Some(&wrong), &client).await,
                Err(ShareError::Invalid)
            ));
        }

        let fresh = RateLimitKey::new("fresh-client");

        assert!(matches!(
            service
                .verify(token.expose(), Some(&passcode(PASSCODE)), &fresh)
                .await,
            Err(ShareError::RateLimited { .. })
        ));
    }

    #[tokio::test]
    async fn successful_opens_do_not_use_up_the_budget() {
        let (service, _) = service();
        let (token, _) = service
            .create_link(GALLERY, None, TTL)
            .await
            .expect("link is created");

        for _ in 0..(LINK_CONFIG.max_failures * 3) {
            assert!(open(&service, token.expose(), None).await.is_ok());
        }
    }

    #[tokio::test]
    async fn access_recovers_after_the_lockout() {
        let (service, clock) = service();
        let (token, _) = service
            .create_link(GALLERY, Some(&passcode(PASSCODE)), TTL)
            .await
            .expect("link is created");

        for _ in 0..CONFIG.max_failures {
            let _ = open(&service, token.expose(), None).await;
        }

        clock.advance(CONFIG.lockout);

        assert!(
            open(&service, token.expose(), Some(&passcode(PASSCODE)))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn other_clients_are_not_locked_out() {
        let (service, _) = service();
        let (token, _) = service
            .create_link(GALLERY, None, TTL)
            .await
            .expect("link is created");

        for _ in 0..CONFIG.max_failures {
            let _ = open(&service, "not-a-token", None).await;
        }

        let other = RateLimitKey::new("198.51.100.1");

        assert!(service.verify(token.expose(), None, &other).await.is_ok());
    }

    #[tokio::test]
    async fn secrets_never_appear_in_debug_or_display_output() {
        let (service, _) = service();
        let (token, link) = service
            .create_link(GALLERY, Some(&passcode(PASSCODE)), TTL)
            .await
            .expect("link is created");
        let supplied = passcode(PASSCODE);
        let error = open(&service, token.expose(), Some(&passcode("wrong passcode!")))
            .await
            .expect_err("wrong passcode");
        let output = [
            format!("{token:?}"),
            format!("{link:?}"),
            format!("{supplied:?}"),
            format!("{service:?}"),
            format!("{error:?}"),
            error.to_string(),
            format!("{:?}", client()),
        ]
        .join("\n");

        for secret in [
            token.expose(),
            PASSCODE,
            "wrong passcode!",
            link.passcode_hash.as_ref().expect("hash").as_phc(),
            "203.0.113.9",
        ] {
            assert!(!output.contains(secret), "leaked {secret}");
        }
    }
}
