pub mod backoff;
pub mod clock;
pub mod memory;

use std::time::{Duration, SystemTime};

use strum::Display;
use thiserror::Error;

use self::backoff::Backoff;

/// Validated queue settings; build one through `QueueConfig::policy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueuePolicy {
    pub backoff: Backoff,
    pub max_attempts: u32,
    pub lease: Duration,
    pub heartbeat_interval: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JobId(pub u64);

/// Proves ownership of one lease. A new token is issued on every claim, so a
/// worker whose lease was recovered can never act on the job again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LeaseToken(pub u64);

#[derive(Display, Debug, Clone, Copy, PartialEq, Eq)]
#[strum(serialize_all = "snake_case")]
pub enum JobState {
    Pending,
    Leased,
    Completed,
    RetryableFailure,
    TerminalFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    Retryable,
    Terminal,
}

/// Must stay free of credentials, storage paths, tokens and private URLs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewJob {
    pub kind: String,
    /// Logical identity of the work: enqueueing the same key returns the
    /// existing job instead of creating a duplicate.
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: JobId,
    pub kind: String,
    pub idempotency_key: String,
    pub state: JobState,
    pub attempts: u32,
    pub max_attempts: u32,
    pub scheduled_at: SystemTime,
    pub lease_expires_at: Option<SystemTime>,
    pub started_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
    pub updated_at: SystemTime,
    pub error: Option<JobError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub job: Job,
    pub token: LeaseToken,
}

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    #[error("job not found")]
    NotFound,
    /// The token is not the current lease: stale, superseded or never issued.
    #[error("caller does not own the job lease")]
    NotLeaseOwner,
    /// Rejected even for the owner: once expired another worker may claim the
    /// job at any moment, so late work must not be acknowledged.
    #[error("job lease has expired")]
    LeaseExpired,
}

/// Storage boundary of the queue. Every method is atomic with respect to the
/// others; the Postgres implementation will use row locks to guarantee it.
pub trait JobStore: Send + Sync {
    /// Returns the existing job when the idempotency key is already known.
    fn enqueue(&self, job: NewJob) -> Job;

    /// Leases the oldest due job after recovering expired leases.
    fn claim(&self) -> Option<Claim>;

    /// Extends the lease to `now + lease`; never shortens it.
    fn heartbeat(&self, id: JobId, token: LeaseToken) -> Result<Job, QueueError>;

    /// Completing an already completed job with the same token is a no-op.
    fn complete(&self, id: JobId, token: LeaseToken) -> Result<Job, QueueError>;

    /// A retryable failure becomes terminal once attempts are exhausted.
    fn fail(
        &self,
        id: JobId,
        token: LeaseToken,
        kind: FailureKind,
        error: JobError,
    ) -> Result<Job, QueueError>;

    /// Reclaims jobs whose lease expired; returns how many were recovered.
    fn recover_expired(&self) -> usize;
}
