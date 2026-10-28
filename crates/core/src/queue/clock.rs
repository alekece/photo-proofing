use std::{
    sync::Mutex,
    time::{Duration, SystemTime},
};

/// Injected so lease and backoff logic never reads the wall clock directly.
pub trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// Manually advanced clock for deterministic tests.
#[derive(Debug)]
pub struct ManualClock {
    now: Mutex<SystemTime>,
}

impl ManualClock {
    pub fn new(start: SystemTime) -> Self {
        Self {
            now: Mutex::new(start),
        }
    }

    pub fn advance(&self, by: Duration) {
        let mut now = self.now.lock().expect("manual clock poisoned");
        *now += by;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> SystemTime {
        *self.now.lock().expect("manual clock poisoned")
    }
}
