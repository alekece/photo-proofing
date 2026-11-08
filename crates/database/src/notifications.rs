//! The photographer's notifications about client activity.
//!
//! Database triggers keep at most one unread notification per gallery: a selection change or the
//! first download of a final adds to it, and activity after it was read opens a new one (see
//! migration `0021_notifications.sql`). Reading is workspace-wide and persistent. A change is
//! announced on [`NotificationEvent`]'s channel.

use std::time::Duration;

use pp_core::types::timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Row, Type, postgres::PgRow};
use uuid::Uuid;

use crate::{
    Repository, auth::WorkspaceId, error::DatabaseError, gallery::GalleryId, photo::PhotoId,
};

/// The most read notifications the bell lists.
pub const LIST_LIMIT: i64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct NotificationId(pub Uuid);

/// A workspace's notifications changed. It carries no content: whoever shows them refetches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationEvent {
    pub workspace_id: WorkspaceId,
    pub gallery_id: GalleryId,
}

impl NotificationEvent {
    pub(crate) const CHANNEL: &'static str = "notification_events";
}

/// What a client did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum NotificationKind {
    Selection,
    Download,
}

/// A notification with what the bell shows of its gallery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notification {
    pub id: NotificationId,
    pub gallery_id: GalleryId,
    pub title: Option<String>,
    pub hero_id: Option<PhotoId>,
    pub kind: NotificationKind,
    /// Finals downloaded since the notification opened; 0 for a selection or a ZIP of photos
    /// that were already downloaded.
    pub count: usize,
    /// The gallery's current selection, so the text never goes stale.
    pub selected: usize,
    pub updated_at: Timestamp,
    /// `None` while unread.
    pub read_at: Option<Timestamp>,
}

impl FromRow<'_, PgRow> for Notification {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let count = |column: &str| {
            usize::try_from(row.try_get::<i64, _>(column)?)
                .map_err(|error| sqlx::Error::Decode(error.into()))
        };

        Ok(Self {
            id: row.try_get("id")?,
            gallery_id: row.try_get("gallery_id")?,
            title: row.try_get("title")?,
            hero_id: row.try_get("hero_id")?,
            kind: row.try_get("kind")?,
            count: count("count")?,
            selected: count("selected")?,
            updated_at: row.try_get("updated_at")?,
            read_at: row.try_get("read_at")?,
        })
    }
}

/// The newest notifications and how many are unread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotificationList {
    pub notifications: Vec<Notification>,
    pub unread: usize,
}

#[derive(Clone, Copy)]
pub struct NotificationRepository<'db> {
    pool: &'db PgPool,
}

impl<'db> Repository<'db> for NotificationRepository<'db> {
    type Options = ();

    fn open(pool: &'db PgPool, (): Self::Options) -> Self {
        Self { pool }
    }
}

impl NotificationRepository<'_> {
    /// Every unread notification of the workspace, then its newest [`LIST_LIMIT`] read ones.
    pub async fn list(&self, workspace_id: WorkspaceId) -> Result<NotificationList, DatabaseError> {
        let notifications = sqlx::query_as(
            "SELECT n.id, n.gallery_id, g.title, g.hero_id, n.kind, n.count::bigint AS count, n.updated_at, n.read_at,
                    (SELECT count(*) FROM photos p
                     WHERE p.gallery_id = g.id AND p.status = 'ready' AND p.kind = 'proof'
                       AND p.favorite) AS selected
             FROM notifications n JOIN galleries g ON g.id = n.gallery_id
             WHERE n.workspace_id = $1
               AND (n.read_at IS NULL OR n.id IN (
                   SELECT id FROM notifications
                   WHERE workspace_id = $1 AND read_at IS NOT NULL
                   ORDER BY updated_at DESC LIMIT $2))
             ORDER BY (n.read_at IS NOT NULL), n.updated_at DESC",
        )
        .bind(workspace_id)
        .bind(LIST_LIMIT)
        .fetch_all(self.pool)
        .await?;
        let unread: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM notifications WHERE workspace_id = $1 AND read_at IS NULL",
        )
        .bind(workspace_id)
        .fetch_one(self.pool)
        .await?;

        Ok(NotificationList {
            notifications,
            unread: usize::try_from(unread).unwrap_or_default(),
        })
    }

    /// Marks one notification read; false when the workspace has no such notification. Reading
    /// one already read changes nothing.
    pub async fn mark_read(
        &self,
        workspace_id: WorkspaceId,
        id: NotificationId,
    ) -> Result<bool, DatabaseError> {
        let found = sqlx::query(
            "UPDATE notifications SET read_at = coalesce(read_at, now())
             WHERE id = $1 AND workspace_id = $2",
        )
        .bind(id)
        .bind(workspace_id)
        .execute(self.pool)
        .await?;

        Ok(found.rows_affected() == 1)
    }

    pub async fn mark_all_read(&self, workspace_id: WorkspaceId) -> Result<(), DatabaseError> {
        sqlx::query(
            "UPDATE notifications SET read_at = now() WHERE workspace_id = $1 AND read_at IS NULL",
        )
        .bind(workspace_id)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Deletes notifications read longer ago than `retention`.
    pub async fn expire(&self, retention: Duration) -> Result<u64, DatabaseError> {
        let removed = sqlx::query(
            "DELETE FROM notifications WHERE read_at < now() - make_interval(secs => $1)",
        )
        .bind(retention.as_secs_f64())
        .execute(self.pool)
        .await?;

        Ok(removed.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Database, export::ExportRepository, gallery::GalleryRepository, tests::TestRepository,
    };

    struct Fixture {
        database: Database,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        pool: PgPool,
    }

    impl Fixture {
        async fn new(pool: PgPool) -> Self {
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
                pool,
            }
        }

        fn notifications(&self) -> NotificationRepository<'_> {
            self.database.repository()
        }

        async fn photo(&self, kind: &str) -> PhotoId {
            let id = PhotoId::from(Uuid::new_v4());

            sqlx::query(
                "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height, kind, status)
                 VALUES ($1, $2, $3, 'a.jpg', 1, 1, 1, $4::photo_kind, 'ready')",
            )
            .bind(id)
            .bind(self.workspace_id)
            .bind(self.gallery_id)
            .bind(kind)
            .execute(&self.pool)
            .await
            .unwrap();

            id
        }

        async fn favorite(&self, id: PhotoId, favorite: bool) {
            sqlx::query("UPDATE photos SET favorite = $2 WHERE id = $1")
                .bind(id)
                .bind(favorite)
                .execute(&self.pool)
                .await
                .unwrap();
        }

        async fn download(&self, id: PhotoId) {
            sqlx::query(
                "UPDATE photos SET downloaded_at = now() WHERE id = $1 AND downloaded_at IS NULL",
            )
            .bind(id)
            .execute(&self.pool)
            .await
            .unwrap();
        }

        /// The next notification event; the listener also reports other channels.
        async fn heard(listener: &mut crate::listener::Listener) -> crate::listener::Notification {
            loop {
                let heard = listener.next().await.unwrap();

                if matches!(heard, crate::listener::Notification::Notification(_)) {
                    return heard;
                }
            }
        }

        async fn list(&self) -> NotificationList {
            self.notifications().list(self.workspace_id).await.unwrap()
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn selection_changes_keep_one_unread_notification(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let (first, second) = (fixture.photo("proof").await, fixture.photo("proof").await);

        fixture.favorite(first, true).await;
        fixture.favorite(second, true).await;

        let list = fixture.list().await;

        assert_eq!(list.unread, 1);
        assert_eq!(list.notifications.len(), 1);
        assert_eq!(list.notifications[0].kind, NotificationKind::Selection);
        assert_eq!(list.notifications[0].selected, 2);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn activity_after_a_read_opens_a_new_notification(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let photo = fixture.photo("proof").await;

        fixture.favorite(photo, true).await;

        let id = fixture.list().await.notifications[0].id;

        assert!(
            fixture
                .notifications()
                .mark_read(fixture.workspace_id, id)
                .await
                .unwrap()
        );

        let list = fixture.list().await;

        assert_eq!(list.unread, 0);
        assert!(list.notifications[0].read_at.is_some());

        fixture.favorite(photo, false).await;

        let list = fixture.list().await;

        assert_eq!(list.unread, 1);
        assert_eq!(list.notifications.len(), 2);
        assert_ne!(list.notifications[0].id, id, "unread first");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn downloads_accumulate_once_per_photo(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let (first, second) = (fixture.photo("final").await, fixture.photo("final").await);

        fixture.download(first).await;
        fixture.download(first).await;

        assert_eq!(fixture.list().await.notifications[0].count, 1);

        fixture.download(second).await;

        let list = fixture.list().await;

        assert_eq!(list.notifications.len(), 1);
        assert_eq!(list.notifications[0].kind, NotificationKind::Download);
        assert_eq!(list.notifications[0].count, 2);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_kind_change_restarts_the_count(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let (proof, first, second) = (
            fixture.photo("proof").await,
            fixture.photo("final").await,
            fixture.photo("final").await,
        );

        fixture.download(first).await;
        fixture.favorite(proof, true).await;

        let list = fixture.list().await;

        assert_eq!(list.notifications.len(), 1);
        assert_eq!(
            (list.notifications[0].kind, list.notifications[0].count),
            (NotificationKind::Selection, 0)
        );

        fixture.download(second).await;

        assert_eq!(fixture.list().await.notifications[0].count, 1);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_zip_of_downloaded_photos_still_notifies(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let photo = fixture.photo("final").await;

        fixture.download(photo).await;
        fixture
            .notifications()
            .mark_all_read(fixture.workspace_id)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO exports (id, workspace_id, gallery_id, kind, photo_count, size)
             VALUES (gen_random_uuid(), $1, $2, 'delivery', 1, 1)",
        )
        .bind(fixture.workspace_id)
        .bind(fixture.gallery_id)
        .execute(&fixture.pool)
        .await
        .unwrap();

        let export_id: crate::export::ExportId =
            sqlx::query_scalar("SELECT id FROM exports WHERE gallery_id = $1")
                .bind(fixture.gallery_id)
                .fetch_one(&fixture.pool)
                .await
                .unwrap();

        fixture
            .database
            .repository::<ExportRepository>()
            .mark_downloaded(export_id)
            .await
            .unwrap();

        let list = fixture.list().await;

        assert_eq!(list.unread, 1);
        assert_eq!(list.notifications[0].count, 0);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn mark_all_read_and_isolation_between_workspaces(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let other = fixture
            .database
            .repository::<TestRepository>()
            .create_workspace()
            .await
            .unwrap();
        let photo = fixture.photo("proof").await;

        fixture.favorite(photo, true).await;

        let id = fixture.list().await.notifications[0].id;

        assert!(
            fixture
                .notifications()
                .list(other)
                .await
                .unwrap()
                .notifications
                .is_empty()
        );
        assert!(!fixture.notifications().mark_read(other, id).await.unwrap());
        assert_eq!(fixture.list().await.unread, 1);

        fixture
            .notifications()
            .mark_all_read(fixture.workspace_id)
            .await
            .unwrap();

        assert_eq!(fixture.list().await.unread, 0);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn deleting_a_gallery_removes_its_notifications(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let photo = fixture.photo("proof").await;

        fixture.favorite(photo, true).await;
        fixture
            .database
            .repository::<TestRepository>()
            .delete_gallery(fixture.gallery_id)
            .await
            .unwrap();

        assert!(fixture.list().await.notifications.is_empty());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn every_unread_notification_is_listed_beyond_the_read_cap(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let galleries = LIST_LIMIT + 5;

        for _ in 0..galleries {
            let gallery_id = fixture
                .database
                .repository::<GalleryRepository>()
                .create(fixture.workspace_id)
                .await
                .unwrap()
                .id;

            sqlx::query("SELECT record_notification($1, $2, 'selection', 0)")
                .bind(fixture.workspace_id)
                .bind(gallery_id)
                .execute(&fixture.pool)
                .await
                .unwrap();
        }

        let list = fixture.list().await;

        assert_eq!(i64::try_from(list.unread).unwrap(), galleries);
        assert_eq!(i64::try_from(list.notifications.len()).unwrap(), galleries);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn old_read_notifications_expire(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let photo = fixture.photo("proof").await;

        fixture.favorite(photo, true).await;
        fixture
            .notifications()
            .mark_all_read(fixture.workspace_id)
            .await
            .unwrap();

        assert_eq!(
            fixture
                .notifications()
                .expire(Duration::from_secs(3600))
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            fixture
                .notifications()
                .expire(Duration::ZERO)
                .await
                .unwrap(),
            1
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn activity_and_reads_are_announced(pool: PgPool) {
        use crate::listener::Notification as Heard;

        let fixture = Fixture::new(pool).await;
        let photo = fixture.photo("proof").await;
        let mut listener = fixture.database.listen().await.unwrap();
        let event = Heard::Notification(NotificationEvent {
            workspace_id: fixture.workspace_id,
            gallery_id: fixture.gallery_id,
        });

        fixture.favorite(photo, true).await;

        assert_eq!(Fixture::heard(&mut listener).await, event);

        fixture
            .notifications()
            .mark_all_read(fixture.workspace_id)
            .await
            .unwrap();

        assert_eq!(Fixture::heard(&mut listener).await, event);
    }
}
