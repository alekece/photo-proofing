//! Best-effort, per-process pacing for client writes. Share-link users share one allowance.

use std::{
    collections::HashMap,
    sync::{Mutex, PoisonError},
    time::{Duration, Instant},
};

use pp_database::gallery::GalleryId;

pub(crate) struct CommentLimiter {
    interval: Duration,
    windows: Mutex<HashMap<GalleryId, Instant>>,
}

impl CommentLimiter {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            windows: Mutex::default(),
        }
    }

    pub fn check(&self, gallery: GalleryId) -> Result<(), Duration> {
        self.check_at(gallery, Instant::now())
    }

    fn check_at(&self, gallery: GalleryId, now: Instant) -> Result<(), Duration> {
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);

        windows.retain(|_, start| now.saturating_duration_since(*start) < self.interval);

        if let Some(start) = windows.get(&gallery) {
            return Err(self
                .interval
                .saturating_sub(now.saturating_duration_since(*start)));
        }

        windows.insert(gallery, now);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use pp_database::gallery::GalleryId;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn writes_are_paced_per_gallery_and_expire() {
        let limiter = CommentLimiter::new(Duration::from_secs(2));
        let gallery = GalleryId(Uuid::new_v4());
        let other = GalleryId(Uuid::new_v4());
        let now = Instant::now();

        assert_eq!(limiter.check_at(gallery, now), Ok(()));
        assert_eq!(
            limiter.check_at(gallery, now + Duration::from_secs(1)),
            Err(Duration::from_secs(1))
        );
        assert_eq!(limiter.check_at(other, now), Ok(()));
        assert_eq!(
            limiter.check_at(gallery, now + Duration::from_secs(2)),
            Ok(())
        );
        assert_eq!(limiter.windows.lock().unwrap().len(), 1);
    }
}
