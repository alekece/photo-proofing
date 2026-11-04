//! Handlers by job kind.

use std::{collections::HashMap, error::Error, future::Future, pin::Pin, sync::Arc};

use pp_database::jobs::{Job, JobCompletion, JobKind};
use serde::de::DeserializeOwned;
use serde_json::Error as JsonError;
use thiserror::Error;

pub mod process_photo;

type BoxError = Box<dyn Error + Send + Sync>;
type HandlerFuture = Pin<Box<dyn Future<Output = Result<JobCompletion, HandlerError>> + Send>>;
type Handler = Arc<dyn Fn(&Job) -> HandlerFuture + Send + Sync>;

/// Why a handler did not finish its job.
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

/// The handler for each job kind the worker serves.
///
/// Handlers must be idempotent: a job can run again after a crash or a lost lease. They
/// should write outputs to deterministic keys and confirm in the transaction that records the
/// result that the subject still exists, so late work cannot recreate deleted content.
///
/// A successful handler finishes its own job, together with its result when it records one
/// (see [`JobRepository::complete_photo`](pp_database::jobs::JobRepository::complete_photo));
/// the [`JobCompletion`] it returns proves it did.
#[derive(Default)]
pub struct HandlerRegistry {
    handlers: HashMap<JobKind, Handler>,
}

impl HandlerRegistry {
    /// Serves `kind` with `handler`, which receives the payload deserialized as `P` and the
    /// claimed job to finish.
    /// Registering a kind again replaces its handler.
    #[must_use]
    pub fn register<P, F, Fut>(mut self, kind: JobKind, handler: F) -> Self
    where
        P: DeserializeOwned + Send + 'static,
        F: Fn(P, Job) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<JobCompletion, HandlerError>> + Send + 'static,
    {
        let erased: Handler = Arc::new(move |job: &Job| match job.payload::<P>() {
            Ok(payload) => Box::pin(handler(payload, job.clone())),
            Err(error) => Box::pin(async move { Err(HandlerError::Payload(error)) }),
        });
        self.handlers.insert(kind, erased);

        self
    }

    /// The kinds the worker may claim; jobs of any other kind are left to other workers.
    pub fn kinds(&self) -> Vec<JobKind> {
        self.handlers.keys().copied().collect()
    }

    /// Runs the handler of the job's kind. `None` if no handler is registered, which cannot
    /// happen for jobs claimed through [`kinds`](Self::kinds).
    pub(crate) fn handle(&self, job: &Job) -> Option<HandlerFuture> {
        self.handlers.get(&job.kind()).map(|handler| handler(job))
    }
}
