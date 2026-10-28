use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use crate::auth::session::Clock;

#[derive(Debug, Clone)]
pub struct ManualClock(Arc<Mutex<SystemTime>>);

impl ManualClock {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(
            SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000),
        )))
    }

    pub fn advance(&self, by: Duration) {
        *self.0.lock().expect("clock lock") += by;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> SystemTime {
        *self.0.lock().expect("clock lock")
    }
}
