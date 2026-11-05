//! Typed PostgreSQL `LISTEN` channels.
//!
//! PostgreSQL delivers a notification only when its transaction commits, and loses it while
//! the listening connection is down. A reconnect, like an undecodable payload, is therefore
//! reported as [`Notification::Missed`] so callers resynchronize instead of trusting a partial
//! history.

use std::marker::PhantomData;

use serde::de::DeserializeOwned;
use sqlx::postgres::PgListener;

use crate::{PgPool, error::DatabaseError};

/// A JSON payload notified on its own channel.
pub trait Channel: DeserializeOwned {
    const NAME: &'static str;
}

/// The next thing a [`Listener`] has to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notification<T> {
    Event(T),
    /// Events may have happened without a notification: whoever shows them must refetch.
    Missed(MissedReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissedReason {
    /// The listening connection was lost and has been reopened.
    Reconnected,
    /// A notification on the channel was not a valid payload.
    Undecodable,
}

/// A dedicated connection listening on `T`'s channel; it holds one pool connection for its
/// whole life.
pub struct Listener<T> {
    listener: PgListener,
    channel: PhantomData<fn() -> T>,
}

impl<T: Channel> Listener<T> {
    /// Opens a dedicated connection from `pool` and listens on `T`'s channel. Only notifications
    /// committed after this returns are reported.
    pub(crate) async fn open(pool: &PgPool) -> Result<Self, DatabaseError> {
        let mut listener = PgListener::connect_with(pool).await?;

        listener.listen(T::NAME).await?;

        Ok(Self {
            listener,
            channel: PhantomData,
        })
    }

    /// Waits for the next notification.
    ///
    /// A lost connection is reopened before this returns [`MissedReason::Reconnected`]; an error
    /// means it could not be reopened, and the listener should be replaced.
    pub async fn next(&mut self) -> Result<Notification<T>, DatabaseError> {
        let Some(notification) = self.listener.try_recv().await? else {
            return Ok(Notification::Missed(MissedReason::Reconnected));
        };

        Ok(serde_json::from_str(notification.payload()).map_or(
            Notification::Missed(MissedReason::Undecodable),
            Notification::Event,
        ))
    }
}
