//! Live photo changes, announced by PostgreSQL itself.
//!
//! Migration `0007_photo_events` adds triggers on `photos` that `NOTIFY` on [`CHANNEL`] for
//! every insert, status change and delete, whichever process made it. PostgreSQL delivers a
//! notification only when its transaction commits. [`PhotoEventListener`] decodes them. A
//! notification is lost while the listening connection is down, so a reconnect, like an
//! undecodable payload, is reported as [`Notification::Missed`] for callers to resynchronize.

use pp_core::notifications::PhotoId;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgListener;

use crate::{Database, auth::WorkspaceId, error::DatabaseError, gallery::GalleryId};

/// Channel the `photos` triggers notify on.
pub const CHANNEL: &str = "photo_events";

/// What happened to a photo: its new processing status, or its deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhotoChange {
    Waiting,
    Processing,
    Ready,
    Failed,
    Deleted,
}

/// One committed change to one photo. It carries ids only: names and failure reasons are read
/// from the photo list, so they never travel through notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhotoEvent {
    pub workspace_id: WorkspaceId,
    pub gallery_id: GalleryId,
    pub photo_id: PhotoId,
    pub status: PhotoChange,
}

/// The next thing a [`PhotoEventListener`] has to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notification {
    Photo(PhotoEvent),
    /// Changes may have happened without an event; whoever shows photos must refetch them.
    Missed(MissedReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissedReason {
    /// The listening connection was lost and has been reopened.
    Reconnected,
    /// A notification on the channel was not a valid photo event.
    Undecodable,
}

/// A dedicated connection listening on [`CHANNEL`]; it holds one pool connection for its
/// whole life.
pub struct PhotoEventListener {
    listener: PgListener,
}

impl PhotoEventListener {
    /// Waits for the next notification.
    ///
    /// A lost connection is reopened before this returns [`MissedReason::Reconnected`]; an error
    /// means it could not be reopened, and the listener should be replaced.
    pub async fn next(&mut self) -> Result<Notification, DatabaseError> {
        let Some(notification) = self.listener.try_recv().await? else {
            return Ok(Notification::Missed(MissedReason::Reconnected));
        };

        Ok(serde_json::from_str(notification.payload()).map_or(
            Notification::Missed(MissedReason::Undecodable),
            Notification::Photo,
        ))
    }
}

impl Database {
    /// Opens a [`PhotoEventListener`]. Only changes committed after this returns are reported.
    pub async fn listen_photo_events(&self) -> Result<PhotoEventListener, DatabaseError> {
        let mut listener = PgListener::connect_with(&self.pool).await?;

        listener.listen(CHANNEL).await?;

        Ok(PhotoEventListener { listener })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sqlx::{PgExecutor, PgPool};
    use tokio::time;
    use uuid::Uuid;

    use super::*;
    use crate::{gallery::GalleryRepository, photo::PhotoStatus, tests::TestRepository};

    struct Fixture {
        database: Database,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
    }

    impl Fixture {
        async fn new(pool: &PgPool) -> Self {
            let database = Database::new(pool.clone());
            let workspace_id = database
                .repository::<TestRepository>()
                .create_workspace()
                .await
                .unwrap();
            let gallery_id = database
                .repository::<GalleryRepository>()
                .create(workspace_id)
                .await
                .unwrap()
                .id;

            Self {
                database,
                workspace_id,
                gallery_id,
            }
        }

        async fn insert_photo(&self, executor: impl PgExecutor<'_>) -> PhotoId {
            let id = PhotoId::from(Uuid::new_v4());

            sqlx::query(
                "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height)
                 VALUES ($1, $2, $3, 'a.jpg', 1, 1, 1)",
            )
            .bind(id)
            .bind(self.workspace_id)
            .bind(self.gallery_id)
            .execute(executor)
            .await
            .unwrap();

            id
        }

        fn event(&self, photo_id: PhotoId, status: PhotoChange) -> Notification {
            Notification::Photo(PhotoEvent {
                workspace_id: self.workspace_id,
                gallery_id: self.gallery_id,
                photo_id,
                status,
            })
        }
    }

    async fn next(listener: &mut PhotoEventListener) -> Notification {
        time::timeout(Duration::from_secs(5), listener.next())
            .await
            .expect("a notification arrives")
            .unwrap()
    }

    async fn assert_silent(listener: &mut PhotoEventListener) {
        let received = time::timeout(Duration::from_millis(200), listener.next()).await;

        assert!(received.is_err(), "unexpected {received:?}");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn insert_status_change_and_delete_each_notify_once(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let mut listener = fixture.database.listen_photo_events().await.unwrap();
        let tests = fixture.database.repository::<TestRepository>();
        let id = fixture.insert_photo(&pool).await;

        assert_eq!(
            next(&mut listener).await,
            fixture.event(id, PhotoChange::Waiting)
        );

        tests
            .force_photo_status(id, &PhotoStatus::Processing)
            .await
            .unwrap();

        assert_eq!(
            next(&mut listener).await,
            fixture.event(id, PhotoChange::Processing)
        );

        tests
            .force_photo_status(id, &PhotoStatus::Failed("broken".to_owned()))
            .await
            .unwrap();

        assert_eq!(
            next(&mut listener).await,
            fixture.event(id, PhotoChange::Failed)
        );

        sqlx::query("DELETE FROM photos WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(
            next(&mut listener).await,
            fixture.event(id, PhotoChange::Deleted)
        );
        assert_silent(&mut listener).await;
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn unchanged_status_and_rolled_back_changes_stay_silent(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let id = fixture.insert_photo(&pool).await;
        let mut listener = fixture.database.listen_photo_events().await.unwrap();
        let mut transaction = pool.begin().await.unwrap();

        sqlx::query("UPDATE photos SET status = 'waiting', original_name = 'b.jpg' WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        fixture.insert_photo(&mut *transaction).await;
        transaction.rollback().await.unwrap();

        assert_silent(&mut listener).await;
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_gallery_deletion_announces_each_photo(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let id = fixture.insert_photo(&pool).await;
        let mut listener = fixture.database.listen_photo_events().await.unwrap();

        fixture
            .database
            .repository::<TestRepository>()
            .delete_gallery(fixture.gallery_id)
            .await
            .unwrap();

        assert_eq!(
            next(&mut listener).await,
            fixture.event(id, PhotoChange::Deleted)
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn an_undecodable_payload_is_reported_as_missed(pool: PgPool) {
        let database = Database::new(pool.clone());
        let mut listener = database.listen_photo_events().await.unwrap();

        sqlx::query("SELECT pg_notify($1, 'not json')")
            .bind(CHANNEL)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(
            next(&mut listener).await,
            Notification::Missed(MissedReason::Undecodable)
        );
    }
}
