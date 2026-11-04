//! The worker loop: claim jobs, run them under a lease, report the outcome.
//!
//! - **Startup** ([`Worker::connect`]) applies the migrations before anything queries `jobs`;
//!   the API runs the same migrator and SQLx serializes the two with an advisory lock.
//! - **Polling** ([`Worker::run`]) claims one job at a time while a concurrency slot is free
//!   and sleeps for the poll interval when the queue is empty. This bounds the jobs in flight.
//! - **Leases**: while a handler runs, the lease is renewed three times per lease period. If a
//!   renewal shows the claim was taken over, the handler is dropped and nothing is recorded, so
//!   a stale run cannot clobber the new one.
//! - **Outcomes** go through [`JobRepository`]: completion deletes the job, a recoverable error
//!   requeues it with backoff or fails it at the attempt cap, an unreadable payload fails it
//!   at once. A handler that panics or a worker that dies reports nothing; the lease simply
//!   expires and the job is claimed again, counting toward its attempts.
//! - **Shutdown**: once the token is cancelled no new job is claimed, running jobs get
//!   [`RuntimeOptions::shutdown_grace`] to finish, and the rest is aborted and recovered by
//!   lease expiry.

use std::{num::NonZeroUsize, sync::Arc, time::Duration};

use pp_database::{
    Database,
    error::DatabaseError,
    jobs::{Backoff, Job, JobRepository},
};
use thiserror::Error;
use tokio::{select, sync::Semaphore, task::JoinSet, time};
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, error, info, info_span, warn};

use crate::{
    config::WorkerConfig,
    handlers::{HandlerError, HandlerRegistry},
};

/// How often the lease is renewed, as a fraction of the lease: two renewals may be missed
/// before the claim lapses.
const RENEWALS_PER_LEASE: u32 = 3;

/// Failure to start the [`Worker`]. [`DatabaseError`] is sanitized, so it never carries the
/// database URL and may be reported as a source.
#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("worker could not use the database")]
    Database(#[from] DatabaseError),
}

/// How the worker polls, leases and retries.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeOptions {
    /// Wait between polls once the queue is empty.
    pub poll_interval: Duration,
    /// How long a claim lasts without renewal.
    pub lease: Duration,
    /// Jobs running at once.
    pub concurrency: NonZeroUsize,
    /// How long running jobs may continue after a stop signal.
    pub shutdown_grace: Duration,
    pub backoff: Backoff,
}

pub struct Worker {
    database: Database,
    handlers: Arc<HandlerRegistry>,
    options: RuntimeOptions,
}

impl Worker {
    pub fn new(database: Database, handlers: HandlerRegistry, options: RuntimeOptions) -> Self {
        Self {
            database,
            handlers: Arc::new(handlers),
            options,
        }
    }

    /// Connects to PostgreSQL, applies pending migrations, then builds the handlers, which may
    /// keep the worker's database.
    pub async fn connect(
        config: WorkerConfig,
        handlers: impl FnOnce(&Database) -> HandlerRegistry,
    ) -> Result<Self, WorkerError> {
        let database = Database::connect_lazy(&config.database.url, config.queue.pool_size())?;

        database.ensure_reachable().await?;
        database.migrate().await?;

        let handlers = handlers(&database);

        Ok(Self::new(database, handlers, config.queue.into()))
    }

    /// Runs until `shutdown` is cancelled, then waits for running jobs as described in the
    /// module documentation.
    pub async fn run(self, shutdown: CancellationToken) {
        let jobs = self.database.repository::<JobRepository>();
        let kinds = self.handlers.kinds();
        let slots = Arc::new(Semaphore::new(self.options.concurrency.get()));
        let mut running = JoinSet::new();

        info!(kinds = kinds.len(), "worker polling for jobs");

        loop {
            let permit = select! {
                biased;
                () = shutdown.cancelled() => break,
                permit = Arc::clone(&slots).acquire_owned() => permit,
            };
            let Ok(permit) = permit else { break };

            while let Some(finished) = running.try_join_next() {
                log_task_end(finished);
            }

            match jobs.claim(&kinds, self.options.lease).await {
                Ok(Some(job)) => {
                    let span = info_span!("job", job_id = %job.id.0, kind = ?job.kind(), attempt = job.attempts);
                    let database = self.database.clone();
                    let handlers = Arc::clone(&self.handlers);
                    let options = self.options;

                    running.spawn(
                        async move {
                            run_job(&database, &handlers, options, job).await;
                            drop(permit);
                        }
                        .instrument(span),
                    );

                    continue;
                }
                Ok(None) => {}
                Err(error) => warn!(%error, "claiming a job failed"),
            }

            drop(permit);
            select! {
                () = shutdown.cancelled() => break,
                () = time::sleep(self.options.poll_interval) => {}
            }
        }

        self.drain(running).await;
    }

    async fn drain(&self, mut running: JoinSet<()>) {
        if running.is_empty() {
            return;
        }

        info!(jobs = running.len(), "waiting for running jobs to finish");

        let finished = time::timeout(self.options.shutdown_grace, async {
            while let Some(result) = running.join_next().await {
                log_task_end(result);
            }
        })
        .await;

        if finished.is_err() {
            warn!(
                jobs = running.len(),
                "grace period over; aborting jobs, which become claimable when their lease expires"
            );
            running.abort_all();
            while running.join_next().await.is_some() {}
        }
    }
}

fn log_task_end(result: Result<(), tokio::task::JoinError>) {
    if let Err(error) = result
        && error.is_panic()
    {
        error!(%error, "job handler panicked; its lease will expire and the job be retried");
    }
}

/// Runs one claimed job while keeping its lease alive, then records the outcome.
async fn run_job(
    database: &Database,
    handlers: &HandlerRegistry,
    options: RuntimeOptions,
    job: Job,
) {
    let jobs = database.repository::<JobRepository>();
    let Some(handling) = handlers.handle(&job) else {
        error!("no handler registered for a claimed job");
        return;
    };

    let outcome = select! {
        outcome = handling => outcome,
        () = hold_lease(&jobs, &job, options.lease) => {
            warn!("lease lost to another claim; dropping this run");
            return;
        }
    };

    let recorded = match outcome {
        Ok(completion) => Ok((completion.is_current(), "completed")),
        Err(error @ HandlerError::Payload(_)) => {
            warn!(%error, "job abandoned");
            jobs.abandon(&job, &error.to_string())
                .await
                .map(|current| (current, "abandoned"))
        }
        Err(error) => {
            warn!(%error, "job failed");
            jobs.fail(&job, &error.to_string(), options.backoff)
                .await
                .map(|state| (state.is_some(), "failed"))
        }
    };

    match recorded {
        Ok((true, what)) => info!(what, "job finished"),
        Ok((false, _)) => warn!("lease lost before the outcome could be recorded"),
        Err(error) => {
            error!(%error, "recording the job outcome failed; it will be retried after its lease expires")
        }
    }
}

/// Renews the lease periodically. Returns only when the claim is no longer ours.
async fn hold_lease(jobs: &JobRepository<'_>, job: &Job, lease: Duration) {
    let mut interval = time::interval(lease / RENEWALS_PER_LEASE);
    interval.tick().await;

    loop {
        interval.tick().await;

        match jobs.renew(job, lease).await {
            Ok(true) => {}
            Ok(false) => return,
            Err(error) => warn!(%error, "renewing the lease failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use pp_core::{notifications::PhotoId, types::generate::Generate};
    use pp_database::{
        PgPool,
        jobs::{DEFAULT_MAX_ATTEMPTS, JobCompletion, JobKind, JobPayload, JobState, ProcessPhoto},
        tests::TestRepository,
    };
    use serde_json::json;

    use super::*;

    const KIND: JobKind = JobKind::ProcessPhoto;

    fn options() -> RuntimeOptions {
        RuntimeOptions {
            poll_interval: Duration::from_millis(30),
            lease: Duration::from_millis(600),
            concurrency: NonZeroUsize::new(2).unwrap(),
            shutdown_grace: Duration::from_millis(200),
            backoff: Backoff::new(Duration::from_millis(50), Duration::from_millis(100)),
        }
    }

    fn start(
        pool: &PgPool,
        handlers: HandlerRegistry,
        options: RuntimeOptions,
    ) -> (CancellationToken, tokio::task::JoinHandle<()>) {
        let token = CancellationToken::new();
        let worker = Worker::new(Database::new(pool.clone()), handlers, options);
        let task = tokio::spawn(worker.run(token.clone()));

        (token, task)
    }

    async fn enqueue(pool: &PgPool, attempts: u16) {
        let payload = JobPayload::ProcessPhoto(ProcessPhoto {
            photo_id: PhotoId::generate(),
        });

        Database::new(pool.clone())
            .repository::<JobRepository>()
            .enqueue(payload, attempts.try_into().unwrap())
            .await
            .unwrap();
    }

    async fn job_count(pool: &PgPool, state: Option<JobState>) -> i64 {
        Database::new(pool.clone())
            .repository::<TestRepository>()
            .count_jobs(state)
            .await
            .unwrap()
    }

    /// Finishes `job` the way a successful handler does.
    async fn finish(pool: PgPool, job: Job) -> Result<JobCompletion, HandlerError> {
        Database::new(pool)
            .repository::<JobRepository>()
            .complete(&job)
            .await
            .map_err(|error| HandlerError::Failed(error.into()))
    }

    async fn wait_for(mut condition: impl AsyncFnMut() -> bool) {
        let deadline = time::Instant::now() + Duration::from_secs(10);
        while !condition().await {
            assert!(time::Instant::now() < deadline, "condition not reached");
            time::sleep(Duration::from_millis(20)).await;
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn runs_a_job_and_deletes_it(pool: PgPool) {
        let runs = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&runs);
        let handlers = HandlerRegistry::default().register(KIND, {
            let pool = pool.clone();

            move |_: ProcessPhoto, job| {
                seen.fetch_add(1, Ordering::SeqCst);
                finish(pool.clone(), job)
            }
        });
        enqueue(&pool, 3).await;

        let (token, task) = start(&pool, handlers, options());
        wait_for(async || job_count(&pool, None).await == 0).await;
        token.cancel();
        task.await.unwrap();

        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn failing_job_is_retried_then_kept_as_failed(pool: PgPool) {
        let runs = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&runs);
        let handlers = HandlerRegistry::default().register(KIND, move |_: ProcessPhoto, _| {
            seen.fetch_add(1, Ordering::SeqCst);
            future::ready(Err(HandlerError::Failed("disk full".into())))
        });
        enqueue(&pool, 3).await;

        let (token, task) = start(&pool, handlers, options());
        wait_for(async || job_count(&pool, Some(JobState::Failed)).await == 1).await;
        token.cancel();
        task.await.unwrap();

        assert_eq!(runs.load(Ordering::SeqCst), 3);
        let failed = Database::new(pool)
            .repository::<JobRepository>()
            .list_failed()
            .await
            .unwrap();
        assert_eq!(
            failed[0].last_error.as_deref(),
            Some("job failed: disk full")
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn unreadable_payload_fails_without_retrying(pool: PgPool) {
        let handlers = HandlerRegistry::default().register(KIND, |_: ProcessPhoto, _| {
            future::ready(Err(HandlerError::Failed("never called".into())))
        });
        let database = Database::new(pool.clone());
        let testing = database.repository::<TestRepository>();
        let id = testing
            .insert_raw_job(json!({"kind": "process_photo", "payload": {"unexpected": true}}))
            .await
            .unwrap();

        let (token, task) = start(&pool, handlers, options());
        wait_for(async || job_count(&pool, Some(JobState::Failed)).await == 1).await;
        token.cancel();
        task.await.unwrap();

        assert_eq!(testing.job_attempts(id).await.unwrap(), 1);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn leases_are_renewed_so_a_slow_job_runs_once(pool: PgPool) {
        let runs = Arc::new(AtomicUsize::new(0));
        let handlers = || {
            let seen = Arc::clone(&runs);
            let pool = pool.clone();

            HandlerRegistry::default().register(KIND, move |_: ProcessPhoto, job| {
                seen.fetch_add(1, Ordering::SeqCst);
                let pool = pool.clone();

                async {
                    time::sleep(Duration::from_millis(1800)).await;
                    finish(pool, job).await
                }
            })
        };
        enqueue(&pool, 3).await;

        // Two workers: the second would take the job over if the lease lapsed.
        let (token, first) = start(&pool, handlers(), options());
        let second = tokio::spawn(
            Worker::new(Database::new(pool.clone()), handlers(), options()).run(token.clone()),
        );
        wait_for(async || job_count(&pool, None).await == 0).await;
        token.cancel();
        first.await.unwrap();
        second.await.unwrap();

        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn concurrency_is_bounded(pool: PgPool) {
        let current = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let (c, p) = (Arc::clone(&current), Arc::clone(&peak));
        let finisher = pool.clone();
        let handlers = HandlerRegistry::default().register(KIND, move |_: ProcessPhoto, job| {
            let (c, p) = (Arc::clone(&c), Arc::clone(&p));
            let pool = finisher.clone();

            async move {
                p.fetch_max(c.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                time::sleep(Duration::from_millis(150)).await;
                c.fetch_sub(1, Ordering::SeqCst);
                finish(pool, job).await
            }
        });
        for _ in 0..6 {
            enqueue(&pool, DEFAULT_MAX_ATTEMPTS.get()).await;
        }

        let (token, task) = start(&pool, handlers, options());
        wait_for(async || job_count(&pool, None).await == 0).await;
        token.cancel();
        task.await.unwrap();

        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn shutdown_waits_for_a_job_that_finishes_within_the_grace(pool: PgPool) {
        let finisher = pool.clone();
        let handlers = HandlerRegistry::default().register(KIND, move |_: ProcessPhoto, job| {
            let pool = finisher.clone();

            async {
                time::sleep(Duration::from_millis(100)).await;
                finish(pool, job).await
            }
        });
        enqueue(&pool, 3).await;

        let (token, task) = start(&pool, handlers, options());
        wait_for(async || job_count(&pool, Some(JobState::Running)).await == 1).await;
        token.cancel();
        task.await.unwrap();

        assert_eq!(job_count(&pool, None).await, 0);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn shutdown_during_a_stuck_job_leaves_it_recoverable(pool: PgPool) {
        let handlers = HandlerRegistry::default().register(KIND, |_: ProcessPhoto, _| {
            future::pending::<Result<JobCompletion, HandlerError>>()
        });
        enqueue(&pool, 3).await;

        let (token, task) = start(&pool, handlers, options());
        wait_for(async || job_count(&pool, Some(JobState::Running)).await == 1).await;
        token.cancel();
        task.await.unwrap();

        // Still leased by the stopped worker, then claimable once the lease passes.
        let jobs_pool = Database::new(pool.clone());
        let jobs = jobs_pool.repository::<JobRepository>();
        assert!(
            jobs.claim(&[KIND], options().lease)
                .await
                .unwrap()
                .is_none()
        );
        time::sleep(options().lease + Duration::from_millis(200)).await;
        let recovered = jobs.claim(&[KIND], options().lease).await.unwrap().unwrap();
        assert_eq!(recovered.attempts, 2);
        assert_eq!(job_count(&pool, Some(JobState::Running)).await, 1);
    }
}
