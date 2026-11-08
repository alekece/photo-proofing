//! Live photo changes, announced by PostgreSQL itself for every insert, status change and
//! delete of a photo, whichever process made it. Read them with
//! [`Database::listen`](crate::Database::listen)`()`.

use serde::{Deserialize, Serialize};

use crate::photo::PhotoId;
use crate::{auth::WorkspaceId, gallery::GalleryId, photo::PhotoKind};

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
    #[serde(default)]
    pub kind: PhotoKind,
    pub status: PhotoChange,
}

impl PhotoEvent {
    pub(crate) const CHANNEL: &'static str = "photo_events";
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sqlx::{PgExecutor, PgPool};
    use tokio::time;
    use uuid::Uuid;

    use super::*;
    use crate::{
        Database,
        gallery::GalleryRepository,
        listener::{Listener, MissedReason, Notification},
        photo::PhotoStatus,
        tests::TestRepository,
    };

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
                kind: PhotoKind::Proof,
                status,
            })
        }
    }

    async fn next(listener: &mut Listener) -> Notification {
        time::timeout(Duration::from_secs(5), listener.next())
            .await
            .expect("a notification arrives")
            .unwrap()
    }

    async fn assert_silent(listener: &mut Listener) {
        let received = time::timeout(Duration::from_millis(200), listener.next()).await;

        assert!(received.is_err(), "unexpected {received:?}");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn insert_status_change_and_delete_each_notify_once(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let mut listener = fixture.database.listen().await.unwrap();
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
        let mut listener = fixture.database.listen().await.unwrap();
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
        let mut listener = fixture.database.listen().await.unwrap();

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
        let mut listener = database.listen().await.unwrap();

        sqlx::query("SELECT pg_notify($1, 'not json')")
            .bind(PhotoEvent::CHANNEL)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(
            next(&mut listener).await,
            Notification::Missed(MissedReason::Undecodable)
        );
    }
}
