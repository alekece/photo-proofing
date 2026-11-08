//! Handlers by job kind.

use std::{collections::HashMap, error::Error, future::Future, pin::Pin, sync::Arc};

use pp_database::{
    Database,
    jobs::{Job, JobCompletion, JobKind, JobRepository},
};
use serde::de::DeserializeOwned;
use serde_json::Error as JsonError;
use thiserror::Error;

pub mod cleanup;
pub mod export;
pub mod process_photo;

type BoxError = Box<dyn Error + Send + Sync>;
type FinishFuture = Pin<Box<dyn Future<Output = Result<JobCompletion, HandlerError>> + Send>>;
type HandlerFuture = Pin<Box<dyn Future<Output = Result<FinishFuture, HandlerError>> + Send>>;
type Handler = Arc<dyn Fn(&Job) -> HandlerFuture + Send + Sync>;

/// Why a handler did not produce an output; the runtime records the job's outcome.
#[derive(Debug, Error)]
pub enum HandlerError {
    /// The payload is not what the handler expects. Retrying cannot help, so the job fails
    /// immediately.
    #[error("job payload is unreadable: {0}")]
    Payload(#[source] JsonError),
    /// A recoverable failure: the job is retried with backoff until its attempts run out.
    #[error("job failed: {0}")]
    Failed(#[source] BoxError),
}

impl HandlerError {
    /// A recoverable failure caused by `error`.
    pub fn failed(error: impl Error + Send + Sync + 'static) -> Self {
        Self::Failed(Box::new(error))
    }
}

/// Records what a handler produced: the job is deleted and the domain result saved in one
/// transaction, so a crash can never leave one without the other.
///
/// The runtime calls it once the handler returns and the claim is still held. Implementations
/// must check that the run still owns the job inside that transaction: a lost claim records
/// nothing and reports [`JobCompletion::is_current`] as `false`. An error is retried like a
/// handler failure.
pub trait Finalizer: Send + Sync + 'static {
    /// What the handler hands over: the result to save, or a user-visible failure.
    type Output: Send + 'static;

    fn finalize(
        &self,
        job: &Job,
        output: Self::Output,
    ) -> impl Future<Output = Result<JobCompletion, HandlerError>> + Send;
}

/// The finalizer of jobs with no domain result: it only deletes the job.
pub struct CompleteJob {
    database: Database,
}

impl CompleteJob {
    pub const fn new(database: Database) -> Self {
        Self { database }
    }
}

impl Finalizer for CompleteJob {
    type Output = ();

    async fn finalize(&self, job: &Job, (): ()) -> Result<JobCompletion, HandlerError> {
        self.database
            .repository::<JobRepository>()
            .complete(job)
            .await
            .map_err(HandlerError::failed)
    }
}

/// The handler and finalizer of each job kind the worker serves.
///
/// Handlers do the work and return an output; they never finish their job. They must be
/// idempotent: a job can run again after a crash or a lost lease. They should write outputs to
/// deterministic keys, and the finalizer confirms in the transaction that records the result
/// that the subject still exists, so late work cannot recreate deleted content.
#[derive(Default)]
pub struct HandlerRegistry {
    handlers: HashMap<JobKind, Handler>,
}

impl HandlerRegistry {
    /// Serves `kind`: `handler` receives the payload deserialized as `P`, and its output goes to
    /// `finalizer`.
    /// Registering a kind again replaces it.
    #[must_use]
    pub fn register<P, F, Fut, Z>(mut self, kind: JobKind, handler: F, finalizer: Z) -> Self
    where
        P: DeserializeOwned + Send + 'static,
        F: Fn(P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Z::Output, HandlerError>> + Send + 'static,
        Z: Finalizer,
    {
        let finalizer = Arc::new(finalizer);
        let erased: Handler = Arc::new(move |job: &Job| -> HandlerFuture {
            let payload = match job.payload::<P>() {
                Ok(payload) => payload,
                Err(error) => return Box::pin(async move { Err(HandlerError::Payload(error)) }),
            };
            let handling = handler(payload);
            let finalizer = Arc::clone(&finalizer);
            let job = job.clone();

            Box::pin(async move {
                let output = handling.await?;
                let finishing: FinishFuture =
                    Box::pin(async move { finalizer.finalize(&job, output).await });

                Ok(finishing)
            })
        });

        self.handlers.insert(kind, erased);

        self
    }

    /// The kinds the worker may claim; jobs of any other kind are left to other workers.
    pub fn kinds(&self) -> Vec<JobKind> {
        self.handlers.keys().copied().collect()
    }

    /// Runs the handler of the job's kind. `None` if no handler is registered, which cannot
    /// happen for jobs claimed through [`kinds`](Self::kinds). The handler resolves to the
    /// finalization the runtime runs while it still holds the claim.
    pub(crate) fn handle(&self, job: &Job) -> Option<HandlerFuture> {
        self.handlers.get(&job.kind()).map(|handler| handler(job))
    }
}
