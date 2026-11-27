//! The photographer's notifications about client activity.
//!
//! A database trigger keeps at most one unread notification per gallery: a recorded
//! [`crate::activity`] event adds to it, and activity after it was read opens a new one (see
//! migration `0037_activity_events.sql`). Reading is workspace-wide and persistent. A change is
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
    Open,
    Comment,
}

/// A notification with what the bell shows of its gallery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notification {
    pub id: NotificationId,
    pub gallery_id: GalleryId,
    pub title: Option<String>,
    pub hero_id: Option<PhotoId>,
    pub kind: NotificationKind,
    /// Finals downloaded, or client opens, since the notification opened; 0 for a selection or a
    /// ZIP of photos that were already downloaded.
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
                       AND p.favorited_at IS NOT NULL) AS selected
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

    /// Marks the gallery's unread notification read, as the photographer has opened the gallery.
    /// A gallery with nothing unread, or of another workspace, changes nothing.
    pub async fn mark_gallery_read(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            "UPDATE notifications SET read_at = now()
             WHERE workspace_id = $1 AND gallery_id = $2 AND read_at IS NULL",
        )
        .bind(workspace_id)
        .bind(gallery_id)
        .execute(self.pool)
        .await?;

        Ok(())
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

    /// Deletes one notification, read or not; false when the workspace has no such notification.
    pub async fn delete(
        &self,
        workspace_id: WorkspaceId,
        id: NotificationId,
    ) -> Result<bool, DatabaseError> {
        let deleted = sqlx::query("DELETE FROM notifications WHERE id = $1 AND workspace_id = $2")
            .bind(id)
            .bind(workspace_id)
            .execute(self.pool)
            .await?;

        Ok(deleted.rows_affected() == 1)
    }

    /// Deletes every notification of the workspace, read or not.
    pub async fn delete_all(&self, workspace_id: WorkspaceId) -> Result<(), DatabaseError> {
        sqlx::query("DELETE FROM notifications WHERE workspace_id = $1")
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
    use sqlx::{AssertSqlSafe, PgConnection};

    use super::*;
    use crate::{
        Database,
        activity::{ActivityKind, ActivityOptions, ActivityRecorder, Client},
        export::{ExportOptions, ExportRepository},
        gallery::GalleryRepository,
        photo::PhotoLocation,
        tests::TestRepository,
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
            sqlx::query(
                "UPDATE photos SET favorited_at = CASE WHEN $2 THEN now() END WHERE id = $1",
            )
            .bind(id)
            .bind(favorite)
            .execute(&self.pool)
            .await
            .unwrap();

            let kind = if favorite {
                ActivityKind::Favorite
            } else {
                ActivityKind::Unfavorite
            };

            self.record(kind, &[id]).await;
        }

        async fn download(&self, id: PhotoId) {
            let marked = sqlx::query(
                "UPDATE photos SET downloaded_at = now() WHERE id = $1 AND downloaded_at IS NULL",
            )
            .bind(id)
            .execute(&self.pool)
            .await
            .unwrap();

            if marked.rows_affected() == 1 {
                self.record(ActivityKind::Download, &[id]).await;
            }
        }

        async fn open(&self) {
            sqlx::query("UPDATE galleries SET last_opened_at = now() WHERE id = $1")
                .bind(self.gallery_id)
                .execute(&self.pool)
                .await
                .unwrap();

            self.record(ActivityKind::Open, &[]).await;
        }

        /// Records a client-attributed activity event, as a repository write would in the same
        /// transaction as its own change; the writes above make theirs separately, since the bell
        /// here is only exercising the trigger on `activity_events`, not a repository's own
        /// throttling or readiness rules.
        async fn record(&self, kind: ActivityKind, photo_ids: &[PhotoId]) {
            let location = PhotoLocation {
                workspace_id: self.workspace_id,
                gallery_id: self.gallery_id,
            };
            let options = ActivityOptions::default().client(Some(Client { contact_id: None }));
            let mut transaction = self.pool.begin().await.unwrap();

            transaction
                .record_activity(&options, location, kind, photo_ids, None)
                .await
                .unwrap();
            transaction.commit().await.unwrap();
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
            .repository_with::<ExportRepository>(ExportOptions {
                activity: ActivityOptions::default().client(Some(Client { contact_id: None })),
                ..Default::default()
            })
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
    async fn deleting_notifications_is_scoped_and_announced(pool: PgPool) {
        use crate::listener::Notification as Heard;

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
        let mut listener = fixture.database.listen().await.unwrap();

        assert!(!fixture.notifications().delete(other, id).await.unwrap());
        assert_eq!(fixture.list().await.notifications.len(), 1);

        fixture.notifications().delete_all(other).await.unwrap();

        assert_eq!(fixture.list().await.notifications.len(), 1);
        assert!(
            fixture
                .notifications()
                .delete(fixture.workspace_id, id)
                .await
                .unwrap()
        );
        assert_eq!(
            Fixture::heard(&mut listener).await,
            Heard::Notification(NotificationEvent {
                workspace_id: fixture.workspace_id,
                gallery_id: fixture.gallery_id,
            })
        );
        assert!(fixture.list().await.notifications.is_empty());
        assert!(
            !fixture
                .notifications()
                .delete(fixture.workspace_id, id)
                .await
                .unwrap()
        );

        fixture.favorite(photo, false).await;
        fixture.favorite(photo, true).await;
        fixture
            .notifications()
            .delete_all(fixture.workspace_id)
            .await
            .unwrap();

        assert!(fixture.list().await.notifications.is_empty());
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

    #[sqlx::test(migrations = "./migrations")]
    async fn opens_accumulate_until_the_notification_is_read(pool: PgPool) {
        let fixture = Fixture::new(pool).await;

        fixture.open().await;
        sqlx::query("UPDATE galleries SET last_opened_at = now() - interval '1 hour'")
            .execute(&fixture.pool)
            .await
            .unwrap();

        assert_eq!(fixture.list().await.notifications[0].count, 1);

        fixture.open().await;
        let list = fixture.list().await;

        assert_eq!(list.unread, 1);
        assert_eq!(list.notifications.len(), 1);
        assert_eq!(list.notifications[0].kind, NotificationKind::Open);
        assert_eq!(list.notifications[0].count, 2);

        let id = list.notifications[0].id;

        fixture
            .notifications()
            .mark_read(fixture.workspace_id, id)
            .await
            .unwrap();
        fixture.open().await;

        let list = fixture.list().await;

        assert_eq!(list.unread, 1);
        assert_eq!(list.notifications.len(), 2);
        assert_eq!(list.notifications[0].count, 1);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn opened_switches_kind_like_the_others(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let photo = fixture.photo("proof").await;

        fixture.favorite(photo, true).await;
        fixture.open().await;

        let list = fixture.list().await;

        assert_eq!(list.notifications.len(), 1);
        assert_eq!(
            (list.notifications[0].kind, list.notifications[0].count),
            (NotificationKind::Open, 1)
        );

        fixture.favorite(photo, false).await;

        assert_eq!(
            fixture.list().await.notifications[0].kind,
            NotificationKind::Selection
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn migration_accepts_the_opened_kind_and_keeps_existing_rows(pool: PgPool) {
        let mut transaction = pool.begin().await.unwrap();
        let schema = format!("notification_opened_migration_{}", Uuid::new_v4().simple());

        sqlx::query(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query(AssertSqlSafe(format!("SET LOCAL search_path TO {schema}")))
            .execute(&mut *transaction)
            .await
            .unwrap();

        for migration in [
            include_str!("../migrations/0001_accounts_sessions.sql"),
            include_str!("../migrations/0002_galleries.sql"),
            include_str!("../migrations/0003_share_tokens.sql"),
            include_str!("../migrations/0004_jobs.sql"),
            include_str!("../migrations/0005_job_payload_enum.sql"),
            include_str!("../migrations/0006_photos.sql"),
            include_str!("../migrations/0007_photo_events.sql"),
            include_str!("../migrations/0008_gallery_states.sql"),
            include_str!("../migrations/0009_gallery_hero.sql"),
            include_str!("../migrations/0010_photo_favorites.sql"),
            include_str!("../migrations/0011_job_runs.sql"),
            include_str!("../migrations/0012_gallery_last_opened.sql"),
            include_str!("../migrations/0013_finals_draft.sql"),
            include_str!("../migrations/0014_exports.sql"),
            include_str!("../migrations/0015_workspace_watermark.sql"),
            include_str!("../migrations/0016_photo_render_revision.sql"),
            include_str!("../migrations/0017_proof_variants.sql"),
            include_str!("../migrations/0018_drop_job_runs.sql"),
            include_str!("../migrations/0019_deliver_step.sql"),
            include_str!("../migrations/0020_client_downloads.sql"),
            include_str!("../migrations/0021_notifications.sql"),
            include_str!("../migrations/0022_workspace_language.sql"),
            include_str!("../migrations/0023_export_failure_codes.sql"),
            include_str!("../migrations/0024_photo_favorite_dates.sql"),
            include_str!("../migrations/0025_proof_quality.sql"),
        ] {
            sqlx::raw_sql(migration)
                .execute(&mut *transaction)
                .await
                .unwrap();
        }

        let workspace = Uuid::new_v4();
        let gallery = Uuid::new_v4();

        sqlx::query("INSERT INTO workspaces(id) VALUES ($1)")
            .bind(workspace)
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query("INSERT INTO galleries(id, workspace_id) VALUES ($1, $2)")
            .bind(gallery)
            .bind(workspace)
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO notifications (workspace_id, gallery_id, kind, read_at)
             VALUES ($1, $2, 'selection', now())",
        )
        .bind(workspace)
        .bind(gallery)
        .execute(&mut *transaction)
        .await
        .unwrap();

        let opened = "INSERT INTO notifications (workspace_id, gallery_id, kind)
                      VALUES ($1, $2, 'open')";

        sqlx::query("SAVEPOINT before_migration")
            .execute(&mut *transaction)
            .await
            .unwrap();

        let rejected = sqlx::query(opened)
            .bind(workspace)
            .bind(gallery)
            .execute(&mut *transaction)
            .await;

        assert!(
            rejected.is_err(),
            "the kind is unknown before the migration"
        );

        sqlx::query("ROLLBACK TO SAVEPOINT before_migration")
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0026_notification_opened.sql"))
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query(opened)
            .bind(workspace)
            .bind(gallery)
            .execute(&mut *transaction)
            .await
            .unwrap();

        let kinds: Vec<String> =
            sqlx::query_scalar("SELECT kind FROM notifications ORDER BY (read_at IS NULL)")
                .fetch_all(&mut *transaction)
                .await
                .unwrap();

        assert_eq!(kinds, ["selection", "open"]);

        sqlx::query("SAVEPOINT after_migration")
            .execute(&mut *transaction)
            .await
            .unwrap();

        let unknown = sqlx::query(
            "INSERT INTO notifications (workspace_id, gallery_id, kind)
             VALUES ($1, $2, 'unknown')",
        )
        .bind(workspace)
        .bind(gallery)
        .execute(&mut *transaction)
        .await;

        assert!(unknown.is_err(), "other kinds stay rejected");

        transaction.rollback().await.unwrap();
    }

    /// Inserts a gallery and returns its id, so each scenario below keeps its own unread row.
    async fn insert_gallery(transaction: &mut PgConnection, workspace: WorkspaceId) -> GalleryId {
        let id = GalleryId(Uuid::new_v4());

        sqlx::query("INSERT INTO galleries (id, workspace_id) VALUES ($1, $2)")
            .bind(id)
            .bind(workspace)
            .execute(&mut *transaction)
            .await
            .unwrap();

        id
    }

    /// The gallery's one notification row, as `(kind, count, unread)`.
    async fn snapshot_notification(
        transaction: &mut PgConnection,
        gallery: GalleryId,
    ) -> (String, i32, bool) {
        sqlx::query_as(
            "SELECT kind, count, read_at IS NULL FROM notifications WHERE gallery_id = $1",
        )
        .bind(gallery)
        .fetch_one(&mut *transaction)
        .await
        .unwrap()
    }

    /// Drives each of the five old triggers (migration `0021`/`0026`/`0029`) with the base-table
    /// write it fires on, in its own gallery, and returns the resulting notification snapshots in
    /// a fixed scenario order.
    async fn drive_old_triggers(
        transaction: &mut PgConnection,
        workspace: WorkspaceId,
    ) -> Vec<(String, i32, bool)> {
        let favorite = insert_gallery(transaction, workspace).await;
        let photo = PhotoId::from(Uuid::new_v4());
        sqlx::query(
            "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height)
             VALUES ($1, $2, $3, 'a.jpg', 1, 1, 1)",
        )
        .bind(photo)
        .bind(workspace)
        .bind(favorite)
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query("UPDATE photos SET favorited_at = now() WHERE id = $1")
            .bind(photo)
            .execute(&mut *transaction)
            .await
            .unwrap();

        let unfavorite = insert_gallery(transaction, workspace).await;
        let photo = PhotoId::from(Uuid::new_v4());
        sqlx::query(
            "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height, favorited_at)
             VALUES ($1, $2, $3, 'a.jpg', 1, 1, 1, now())",
        )
        .bind(photo)
        .bind(workspace)
        .bind(unfavorite)
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query("UPDATE photos SET favorited_at = NULL WHERE id = $1")
            .bind(photo)
            .execute(&mut *transaction)
            .await
            .unwrap();

        let download_photo = insert_gallery(transaction, workspace).await;
        let photo = PhotoId::from(Uuid::new_v4());
        sqlx::query(
            "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height, kind)
             VALUES ($1, $2, $3, 'a.jpg', 1, 1, 1, 'final')",
        )
        .bind(photo)
        .bind(workspace)
        .bind(download_photo)
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query("UPDATE photos SET downloaded_at = now() WHERE id = $1")
            .bind(photo)
            .execute(&mut *transaction)
            .await
            .unwrap();

        let download_zip = insert_gallery(transaction, workspace).await;
        let export = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO exports (id, workspace_id, gallery_id, photo_count, size, kind)
             VALUES ($1, $2, $3, 1, 100, 'delivery')",
        )
        .bind(export)
        .bind(workspace)
        .bind(download_zip)
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query("UPDATE exports SET downloaded_at = now() WHERE id = $1")
            .bind(export)
            .execute(&mut *transaction)
            .await
            .unwrap();

        let open = insert_gallery(transaction, workspace).await;
        sqlx::query("UPDATE galleries SET last_opened_at = now() WHERE id = $1")
            .bind(open)
            .execute(&mut *transaction)
            .await
            .unwrap();

        let comment = insert_gallery(transaction, workspace).await;
        let photo = PhotoId::from(Uuid::new_v4());
        sqlx::query(
            "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height)
             VALUES ($1, $2, $3, 'a.jpg', 1, 1, 1)",
        )
        .bind(photo)
        .bind(workspace)
        .bind(comment)
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO photo_comments (workspace_id, gallery_id, photo_id, author, body)
             VALUES ($1, $2, $3, 'client', 'Love these!')",
        )
        .bind(workspace)
        .bind(comment)
        .bind(photo)
        .execute(&mut *transaction)
        .await
        .unwrap();

        let mut snapshots = Vec::new();
        for gallery in [
            favorite,
            unfavorite,
            download_photo,
            download_zip,
            open,
            comment,
        ] {
            snapshots.push(snapshot_notification(transaction, gallery).await);
        }

        snapshots
    }

    /// Drives the new path (migration `0038` onward) with one [`ActivityRecorder::record_activity`]
    /// call per scenario, in the same fixed order as [`drive_old_triggers`].
    async fn drive_activity_events(
        transaction: &mut PgConnection,
        workspace: WorkspaceId,
    ) -> Vec<(String, i32, bool)> {
        let options = ActivityOptions::default().client(Some(Client { contact_id: None }));
        let mut galleries = Vec::new();

        for kind in [
            ActivityKind::Favorite,
            ActivityKind::Unfavorite,
            ActivityKind::Download,
            ActivityKind::Download,
            ActivityKind::Open,
            ActivityKind::Comment,
        ] {
            let gallery = insert_gallery(transaction, workspace).await;
            let location = PhotoLocation {
                workspace_id: workspace,
                gallery_id: gallery,
            };
            // The ZIP scenario (the second download) marks no photo as newly downloaded.
            let photo_ids = if kind == ActivityKind::Download && galleries.len() == 3 {
                vec![]
            } else {
                vec![PhotoId::from(Uuid::new_v4())]
            };

            transaction
                .record_activity(&options, location, kind, &photo_ids, None)
                .await
                .unwrap();
            galleries.push(gallery);
        }

        let mut snapshots = Vec::new();
        for gallery in galleries {
            snapshots.push(snapshot_notification(transaction, gallery).await);
        }

        snapshots
    }

    /// Regression test for migration `0038`: the bell, read only from its stored `kind`, `count`
    /// and `read_at` columns, must react identically whether client activity comes from the five
    /// old per-table triggers it drops or from [`ActivityRecorder::record_activity`] writing
    /// `activity_events` afterwards. The photos table's live `selected` count is not part of this
    /// comparison; only the notification row itself is.
    #[sqlx::test(migrations = "./migrations")]
    async fn the_bell_matches_the_old_triggers(pool: PgPool) {
        let mut transaction = pool.begin().await.unwrap();
        let schema = format!("bell_matches_old_triggers_{}", Uuid::new_v4().simple());

        sqlx::query(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query(AssertSqlSafe(format!("SET LOCAL search_path TO {schema}")))
            .execute(&mut *transaction)
            .await
            .unwrap();

        for migration in [
            include_str!("../migrations/0001_accounts_sessions.sql"),
            include_str!("../migrations/0002_galleries.sql"),
            include_str!("../migrations/0003_share_tokens.sql"),
            include_str!("../migrations/0004_jobs.sql"),
            include_str!("../migrations/0005_job_payload_enum.sql"),
            include_str!("../migrations/0006_photos.sql"),
            include_str!("../migrations/0007_photo_events.sql"),
            include_str!("../migrations/0008_gallery_states.sql"),
            include_str!("../migrations/0009_gallery_hero.sql"),
            include_str!("../migrations/0010_photo_favorites.sql"),
            include_str!("../migrations/0011_job_runs.sql"),
            include_str!("../migrations/0012_gallery_last_opened.sql"),
            include_str!("../migrations/0013_finals_draft.sql"),
            include_str!("../migrations/0014_exports.sql"),
            include_str!("../migrations/0015_workspace_watermark.sql"),
            include_str!("../migrations/0016_photo_render_revision.sql"),
            include_str!("../migrations/0017_proof_variants.sql"),
            include_str!("../migrations/0018_drop_job_runs.sql"),
            include_str!("../migrations/0019_deliver_step.sql"),
            include_str!("../migrations/0020_client_downloads.sql"),
            include_str!("../migrations/0021_notifications.sql"),
            include_str!("../migrations/0022_workspace_language.sql"),
            include_str!("../migrations/0023_export_failure_codes.sql"),
            include_str!("../migrations/0024_photo_favorite_dates.sql"),
            include_str!("../migrations/0025_proof_quality.sql"),
            include_str!("../migrations/0026_notification_opened.sql"),
            include_str!("../migrations/0027_ten_size_steps.sql"),
            include_str!("../migrations/0028_gallery_shoot_date.sql"),
            include_str!("../migrations/0029_photo_comments.sql"),
            include_str!("../migrations/0030_cover_layout_and_focal_point.sql"),
            include_str!("../migrations/0031_contacts.sql"),
            include_str!("../migrations/0032_gallery_typography_and_grid_presets.sql"),
            include_str!("../migrations/0033_gallery_access_code.sql"),
            include_str!("../migrations/0034_gallery_favorite.sql"),
            include_str!("../migrations/0035_plain_gallery_access_code.sql"),
            include_str!("../migrations/0036_gallery_expires_at_date.sql"),
            include_str!("../migrations/0037_activity_events.sql"),
        ] {
            sqlx::raw_sql(migration)
                .execute(&mut *transaction)
                .await
                .unwrap();
        }

        let workspace = WorkspaceId(Uuid::new_v4());
        sqlx::query("INSERT INTO workspaces (id) VALUES ($1)")
            .bind(workspace)
            .execute(&mut *transaction)
            .await
            .unwrap();

        let before = drive_old_triggers(&mut transaction, workspace).await;

        sqlx::raw_sql(include_str!(
            "../migrations/0038_drop_notification_triggers.sql"
        ))
        .execute(&mut *transaction)
        .await
        .unwrap();

        let after = drive_activity_events(&mut transaction, workspace).await;

        assert_eq!(
            before,
            [
                ("selection".to_owned(), 0, true),
                ("selection".to_owned(), 0, true),
                ("download".to_owned(), 1, true),
                ("download".to_owned(), 0, true),
                ("open".to_owned(), 1, true),
                ("comment".to_owned(), 1, true),
            ],
            "documents the old triggers' behaviour this migration must preserve"
        );
        assert_eq!(
            after, before,
            "the new activity-events path must notify exactly like the old triggers did"
        );

        transaction.rollback().await.unwrap();
    }
}
