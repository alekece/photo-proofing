use std::{
    sync::{Mutex, MutexGuard},
    time::SystemTime,
};

use super::{
    Claim, FailureKind, Job, JobError, JobId, JobState, JobStore, LeaseToken, NewJob, QueueError,
    QueuePolicy,
    backoff::{Jitter, NoJitter},
    clock::Clock,
};

struct Entry {
    job: Job,
    /// Token of the live lease, or of the one that completed the job.
    token: Option<LeaseToken>,
}

#[derive(Default)]
struct State {
    entries: Vec<Entry>,
    next_token: u64,
}

pub struct InMemoryJobStore<C, J = NoJitter> {
    policy: QueuePolicy,
    clock: C,
    jitter: J,
    state: Mutex<State>,
}

impl<C: Clock> InMemoryJobStore<C> {
    pub fn new(policy: QueuePolicy, clock: C) -> Self {
        Self::with_jitter(policy, clock, NoJitter)
    }
}

impl<C: Clock, J: Jitter> InMemoryJobStore<C, J> {
    pub fn with_jitter(policy: QueuePolicy, clock: C, jitter: J) -> Self {
        Self {
            policy,
            clock,
            jitter,
            state: Mutex::new(State::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("queue state poisoned")
    }

    /// A failed attempt either schedules a retry or ends the job.
    fn settle_failure(&self, job: &mut Job, kind: FailureKind, error: JobError, now: SystemTime) {
        self.settle_failure_at(job, kind, error, now, now);
    }

    /// `failed_at` anchors the backoff: a lazily recovered job is delayed from
    /// its lease expiry, not from whenever recovery happened to run.
    fn settle_failure_at(
        &self,
        job: &mut Job,
        kind: FailureKind,
        error: JobError,
        failed_at: SystemTime,
        now: SystemTime,
    ) {
        let exhausted = job.attempts >= job.max_attempts;

        job.error = Some(error);
        job.lease_expires_at = None;
        job.updated_at = now;

        if kind == FailureKind::Terminal || exhausted {
            job.state = JobState::TerminalFailure;
            return;
        }

        let delay = self
            .policy
            .backoff
            .delay_with_jitter(job.attempts, &self.jitter);

        job.state = JobState::RetryableFailure;
        job.scheduled_at = failed_at + delay;
    }

    fn recover(&self, state: &mut State, now: SystemTime) -> usize {
        let mut recovered = 0;

        for entry in &mut state.entries {
            let expired_at = entry
                .job
                .lease_expires_at
                .filter(|at| entry.job.state == JobState::Leased && *at <= now);
            let Some(expired_at) = expired_at else {
                continue;
            };

            let error = JobError {
                code: "lease_expired".into(),
                message: "The worker stopped responding.".into(),
            };

            entry.token = None;
            self.settle_failure_at(
                &mut entry.job,
                FailureKind::Retryable,
                error,
                expired_at,
                now,
            );
            recovered += 1;
        }

        recovered
    }

    fn find_leased(
        state: &mut State,
        id: JobId,
        token: LeaseToken,
        now: SystemTime,
    ) -> Result<&mut Entry, QueueError> {
        let entry = state
            .entries
            .iter_mut()
            .find(|entry| entry.job.id == id)
            .ok_or(QueueError::NotFound)?;

        if entry.job.state != JobState::Leased || entry.token != Some(token) {
            return Err(QueueError::NotLeaseOwner);
        }
        if entry.job.lease_expires_at.is_none_or(|at| at <= now) {
            return Err(QueueError::LeaseExpired);
        }

        Ok(entry)
    }
}

impl<C: Clock, J: Jitter> JobStore for InMemoryJobStore<C, J> {
    fn enqueue(&self, new: NewJob) -> Job {
        let now = self.clock.now();
        let mut state = self.lock();

        if let Some(entry) = state
            .entries
            .iter()
            .find(|entry| entry.job.idempotency_key == new.idempotency_key)
        {
            return entry.job.clone();
        }

        let job = Job {
            id: JobId(state.entries.len() as u64 + 1),
            kind: new.kind,
            idempotency_key: new.idempotency_key,
            state: JobState::Pending,
            attempts: 0,
            max_attempts: self.policy.max_attempts,
            scheduled_at: now,
            lease_expires_at: None,
            started_at: None,
            completed_at: None,
            updated_at: now,
            error: None,
        };
        state.entries.push(Entry {
            job: job.clone(),
            token: None,
        });

        job
    }

    fn claim(&self) -> Option<Claim> {
        let now = self.clock.now();
        let mut state = self.lock();

        self.recover(&mut state, now);

        state.next_token += 1;
        let token = LeaseToken(state.next_token);

        let entry = state
            .entries
            .iter_mut()
            .filter(|entry| {
                matches!(
                    entry.job.state,
                    JobState::Pending | JobState::RetryableFailure
                ) && entry.job.scheduled_at <= now
            })
            .min_by_key(|entry| (entry.job.scheduled_at, entry.job.id))?;

        entry.token = Some(token);
        entry.job.state = JobState::Leased;
        entry.job.attempts += 1;
        entry.job.lease_expires_at = Some(now + self.policy.lease);
        entry.job.started_at.get_or_insert(now);
        entry.job.updated_at = now;

        Some(Claim {
            job: entry.job.clone(),
            token,
        })
    }

    fn heartbeat(&self, id: JobId, token: LeaseToken) -> Result<Job, QueueError> {
        let now = self.clock.now();
        let mut state = self.lock();
        let entry = Self::find_leased(&mut state, id, token, now)?;

        let extended = now + self.policy.lease;
        entry.job.lease_expires_at = entry.job.lease_expires_at.max(Some(extended));
        entry.job.updated_at = now;

        Ok(entry.job.clone())
    }

    fn complete(&self, id: JobId, token: LeaseToken) -> Result<Job, QueueError> {
        let now = self.clock.now();
        let mut state = self.lock();

        let already_done = state
            .entries
            .iter()
            .find(|entry| entry.job.id == id)
            .filter(|entry| entry.job.state == JobState::Completed && entry.token == Some(token));
        if let Some(entry) = already_done {
            return Ok(entry.job.clone());
        }

        let entry = Self::find_leased(&mut state, id, token, now)?;

        entry.job.state = JobState::Completed;
        entry.job.completed_at = Some(now);
        entry.job.lease_expires_at = None;
        entry.job.error = None;
        entry.job.updated_at = now;

        Ok(entry.job.clone())
    }

    fn fail(
        &self,
        id: JobId,
        token: LeaseToken,
        kind: FailureKind,
        error: JobError,
    ) -> Result<Job, QueueError> {
        let now = self.clock.now();
        let mut state = self.lock();
        let entry = Self::find_leased(&mut state, id, token, now)?;

        entry.token = None;
        self.settle_failure(&mut entry.job, kind, error, now);

        Ok(entry.job.clone())
    }

    fn recover_expired(&self) -> usize {
        let now = self.clock.now();
        let mut state = self.lock();

        self.recover(&mut state, now)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Arc,
        thread,
        time::{Duration, UNIX_EPOCH},
    };

    use super::*;
    use crate::queue::backoff::Backoff;
    use crate::queue::clock::ManualClock;

    const POLICY: QueuePolicy = QueuePolicy {
        backoff: Backoff {
            base: Duration::from_secs(5),
            factor: 2,
            max: Duration::from_secs(300),
        },
        max_attempts: 5,
        lease: Duration::from_secs(60),
        heartbeat_interval: Duration::from_secs(20),
    };

    type Store = InMemoryJobStore<Arc<ManualClock>>;

    impl Clock for Arc<ManualClock> {
        fn now(&self) -> SystemTime {
            ManualClock::now(self)
        }
    }

    fn setup() -> (Store, Arc<ManualClock>) {
        let clock = Arc::new(ManualClock::new(UNIX_EPOCH + Duration::from_secs(1_000)));
        (InMemoryJobStore::new(POLICY, clock.clone()), clock)
    }

    fn new_job(key: &str) -> NewJob {
        NewJob {
            kind: "proof".into(),
            idempotency_key: key.into(),
        }
    }

    fn retryable() -> (FailureKind, JobError) {
        (
            FailureKind::Retryable,
            JobError {
                code: "storage_unavailable".into(),
                message: "Storage was unavailable.".into(),
            },
        )
    }

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn duplicate_enqueue_returns_the_existing_job() {
        let (store, _) = setup();

        let first = store.enqueue(new_job("a"));
        let second = store.enqueue(new_job("a"));

        assert_eq!(first.id, second.id);
        assert_eq!(store.enqueue(new_job("b")).id, JobId(2));
    }

    #[test]
    fn claim_leases_a_job_once() {
        let (store, _) = setup();
        store.enqueue(new_job("a"));

        let claim = store.claim().expect("claim");

        assert_eq!(claim.job.state, JobState::Leased);
        assert_eq!(claim.job.attempts, 1);
        assert!(store.claim().is_none());
    }

    #[test]
    fn expired_lease_is_recovered_and_reclaimable_after_backoff() {
        let (store, clock) = setup();
        store.enqueue(new_job("a"));
        let crashed = store.claim().expect("claim");

        clock.advance(secs(61));
        assert_eq!(store.recover_expired(), 1);
        assert!(store.claim().is_none(), "backoff not elapsed");

        clock.advance(secs(5));
        let reclaimed = store.claim().expect("reclaim");

        assert_eq!(reclaimed.job.attempts, 2);
        assert_ne!(reclaimed.token, crashed.token);
    }

    #[test]
    fn claim_recovers_expired_leases_itself() {
        let (store, clock) = setup();
        store.enqueue(new_job("a"));
        store.claim().expect("claim");

        clock.advance(secs(66));

        assert!(store.claim().is_some());
    }

    #[test]
    fn stale_owner_cannot_heartbeat_complete_or_fail() {
        let (store, clock) = setup();
        let id = store.enqueue(new_job("a")).id;
        let stale = store.claim().expect("claim").token;
        clock.advance(secs(66));
        let fresh = store.claim().expect("reclaim").token;
        let (kind, error) = retryable();

        assert_eq!(store.heartbeat(id, stale), Err(QueueError::NotLeaseOwner));
        assert_eq!(store.complete(id, stale), Err(QueueError::NotLeaseOwner));
        assert_eq!(
            store.fail(id, stale, kind, error),
            Err(QueueError::NotLeaseOwner)
        );
        assert!(store.complete(id, fresh).is_ok());
    }

    #[test]
    fn owner_with_an_expired_unrecovered_lease_is_rejected() {
        let (store, clock) = setup();
        let id = store.enqueue(new_job("a")).id;
        let token = store.claim().expect("claim").token;

        clock.advance(secs(60));

        assert_eq!(store.heartbeat(id, token), Err(QueueError::LeaseExpired));
        assert_eq!(store.complete(id, token), Err(QueueError::LeaseExpired));
    }

    #[test]
    fn heartbeat_keeps_the_lease_alive() {
        let (store, clock) = setup();
        let id = store.enqueue(new_job("a")).id;
        let token = store.claim().expect("claim").token;

        for _ in 0..4 {
            clock.advance(secs(20));
            store.heartbeat(id, token).expect("heartbeat");
        }

        assert_eq!(store.recover_expired(), 0);
        assert!(store.complete(id, token).is_ok());
    }

    #[test]
    fn heartbeat_by_a_non_owner_is_rejected() {
        let (store, _) = setup();
        let first = store.enqueue(new_job("a")).id;
        store.enqueue(new_job("b"));
        store.claim().expect("claim a");
        let other = store.claim().expect("claim b").token;

        assert_eq!(
            store.heartbeat(first, other),
            Err(QueueError::NotLeaseOwner)
        );
        assert_eq!(store.heartbeat(JobId(99), other), Err(QueueError::NotFound));
    }

    #[test]
    fn completion_is_idempotent_for_the_owner_only() {
        let (store, _) = setup();
        let id = store.enqueue(new_job("a")).id;
        let token = store.claim().expect("claim").token;

        let first = store.complete(id, token).expect("complete");
        let again = store.complete(id, token).expect("repeat is a no-op");

        assert_eq!(first, again);
        assert_eq!(first.state, JobState::Completed);
        assert_eq!(
            store.complete(id, LeaseToken(999)),
            Err(QueueError::NotLeaseOwner)
        );
    }

    #[test]
    fn completed_job_is_never_claimed_again() {
        let (store, clock) = setup();
        let id = store.enqueue(new_job("a")).id;
        let token = store.claim().expect("claim").token;
        store.complete(id, token).expect("complete");

        clock.advance(secs(10_000));

        assert!(store.claim().is_none());
        assert_eq!(store.recover_expired(), 0);
    }

    #[test]
    fn retry_delays_follow_the_backoff_schedule() {
        let (store, clock) = setup();
        let id = store.enqueue(new_job("a")).id;

        for expected in [5, 10, 20, 40] {
            let token = store.claim().expect("claim").token;
            let (kind, error) = retryable();
            let job = store.fail(id, token, kind, error).expect("fail");

            assert_eq!(job.state, JobState::RetryableFailure);
            assert_eq!(
                job.scheduled_at
                    .duration_since(clock.now())
                    .expect("future"),
                secs(expected)
            );
            clock.advance(secs(expected) - Duration::from_nanos(1));
            assert!(store.claim().is_none(), "claimed before backoff elapsed");
            clock.advance(Duration::from_nanos(1));
        }
    }

    #[test]
    fn exhausted_attempts_end_in_terminal_failure() {
        let (store, clock) = setup();
        let id = store.enqueue(new_job("a")).id;
        let mut last = None;

        for _ in 0..POLICY.max_attempts {
            clock.advance(secs(300));
            let token = store.claim().expect("claim").token;
            let (kind, error) = retryable();
            last = Some(store.fail(id, token, kind, error).expect("fail"));
        }

        let job = last.expect("job");
        assert_eq!(job.state, JobState::TerminalFailure);
        assert_eq!(job.attempts, 5);

        clock.advance(secs(10_000));
        assert!(store.claim().is_none());
    }

    #[test]
    fn terminal_failure_is_not_retried() {
        let (store, clock) = setup();
        let id = store.enqueue(new_job("a")).id;
        let token = store.claim().expect("claim").token;
        let error = JobError {
            code: "unsupported_format".into(),
            message: "The file format is not supported.".into(),
        };

        let job = store
            .fail(id, token, FailureKind::Terminal, error)
            .expect("fail");

        assert_eq!(job.state, JobState::TerminalFailure);
        clock.advance(secs(10_000));
        assert!(store.claim().is_none());
    }

    #[test]
    fn repeatedly_crashing_workers_exhaust_the_job() {
        let (store, clock) = setup();
        let id = store.enqueue(new_job("a")).id;

        for _ in 0..POLICY.max_attempts {
            clock.advance(secs(1_000));
            store.claim().expect("claim");
            clock.advance(secs(61));
            store.recover_expired();
        }

        let job = store.enqueue(new_job("a"));
        assert_eq!(job.id, id);
        assert_eq!(job.state, JobState::TerminalFailure);
        assert_eq!(job.error.expect("error").code, "lease_expired");
    }

    #[test]
    fn concurrent_workers_complete_a_job_exactly_once() {
        let (store, _) = setup();
        let store = Arc::new(store);
        store.enqueue(new_job("a"));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let store = store.clone();
                thread::spawn(move || {
                    let claim = store.claim()?;
                    store.complete(claim.job.id, claim.token).ok()
                })
            })
            .collect();
        let winners = handles
            .into_iter()
            .filter_map(|handle| handle.join().expect("join"))
            .count();

        assert_eq!(winners, 1);
    }
}
