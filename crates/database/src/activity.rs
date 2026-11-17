//! The workspace activity log: what a client did across galleries, extended into one line per
//! gallery, contact and kind while repeats stay inside [`ActivityOptions::window`]. A database
//! trigger derives the photographer's bell notification from the change in each line's count, so
//! the two can never disagree.

use std::time::Duration;

use pp_core::config::DEFAULT_ACTIVITY_WINDOW;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Type};

use crate::{
    contact::ContactId,
    error::DatabaseError,
    photo::{PhotoId, PhotoLocation},
};

/// The contact behind the personal link an activity was recorded through, or anonymous (`None`)
/// for the gallery's general link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Client {
    pub contact_id: Option<ContactId>,
}

/// Settings for one [`ActivityRecorder::record_activity`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivityOptions {
    /// Repeated activity of the same gallery, contact and kind within this window extends one
    /// line instead of opening a new one.
    pub window: Duration,
    /// `None` is the photographer, whose own actions record nothing.
    pub client: Option<Client>,
}

impl Default for ActivityOptions {
    fn default() -> Self {
        Self {
            window: DEFAULT_ACTIVITY_WINDOW,
            client: None,
        }
    }
}

impl ActivityOptions {
    #[must_use]
    pub fn client(mut self, client: Option<Client>) -> Self {
        self.client = client;

        self
    }
}

/// What a client did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum ActivityKind {
    Open,
    Favorite,
    Unfavorite,
    Comment,
    Download,
    Email,
}

impl ActivityKind {
    /// How much one call adds to the line's `count`. A download call logs the number of photos
    /// it newly affected, which a ZIP's first download may make zero; every other kind is one
    /// flip, comment or open, regardless of how many photos it names for the thumbnails.
    fn delta(self, photo_ids: &[PhotoId]) -> i32 {
        match self {
            Self::Download => i32::try_from(photo_ids.len()).unwrap_or(i32::MAX),
            Self::Open | Self::Favorite | Self::Unfavorite | Self::Comment | Self::Email => 1,
        }
    }
}

/// Writes one client action into the gallery's activity log.
///
/// A no-op when `options.client` is `None`. Otherwise the call extends the line of the same
/// gallery, contact and kind whose `last_at` is still inside `options.window`, or opens a new
/// one: `count` grows by [`ActivityKind::delta`], `photo_ids` grows up to four entries, and
/// `detail`, when given, replaces the line's. A transaction-scoped advisory lock on the gallery,
/// contact and kind orders concurrent calls on the same line, so none of them are lost and the
/// window is never split.
pub(crate) trait ActivityRecorder {
    async fn record_activity(
        &mut self,
        options: &ActivityOptions,
        location: PhotoLocation,
        kind: ActivityKind,
        photo_ids: &[PhotoId],
        detail: Option<&str>,
    ) -> Result<(), DatabaseError>;
}

impl ActivityRecorder for PgConnection {
    async fn record_activity(
        &mut self,
        options: &ActivityOptions,
        location: PhotoLocation,
        kind: ActivityKind,
        photo_ids: &[PhotoId],
        detail: Option<&str>,
    ) -> Result<(), DatabaseError> {
        let Some(client) = options.client else {
            return Ok(());
        };

        // A sliding window has no natural unique key to upsert on, so the gallery, contact and
        // kind are locked first: a concurrent writer on the same key waits instead of racing the
        // read-then-write below, which would otherwise lose a count or split the window.
        let lock_key = format!(
            "{}:{}:{kind:?}",
            location.gallery_id.0,
            client
                .contact_id
                .map_or_else(|| "anonymous".to_owned(), |id| id.0.to_string()),
        );

        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(lock_key)
            .execute(&mut *self)
            .await?;

        let added = kind.delta(photo_ids);
        let updated = sqlx::query(
            "UPDATE activity_events
                 SET count = count + $1,
                     photo_ids = (photo_ids || $2::uuid[])[1:4],
                     detail = coalesce($3, detail),
                     last_at = now()
             WHERE workspace_id = $4
               AND gallery_id = $5
               AND contact_id IS NOT DISTINCT FROM $6
               AND kind = $7
               AND last_at >= now() - make_interval(secs => $8)",
        )
        .bind(added)
        .bind(photo_ids)
        .bind(detail)
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(client.contact_id)
        .bind(kind)
        .bind(options.window.as_secs_f64())
        .execute(&mut *self)
        .await?;

        if updated.rows_affected() > 0 {
            return Ok(());
        }

        sqlx::query(
            "INSERT INTO activity_events (workspace_id, gallery_id, contact_id, kind, count, photo_ids, detail)
             VALUES ($1, $2, $3, $4, $5, $6[1:4], $7)",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(client.contact_id)
        .bind(kind)
        .bind(added)
        .bind(photo_ids)
        .bind(detail)
        .execute(&mut *self)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use pp_core::types::generate::Generate;
    use sqlx::{FromRow, PgPool};
    use tokio::time;

    use super::*;
    use crate::{
        Database, Repository,
        gallery::GalleryRepository,
        listener::{Listener, Notification},
        notifications::{NotificationEvent, NotificationKind},
        tests::TestRepository,
    };

    #[derive(Debug, FromRow)]
    struct ActivityRow {
        contact_id: Option<ContactId>,
        kind: ActivityKind,
        count: i32,
        photo_ids: Vec<PhotoId>,
    }

    struct Fixture {
        pool: PgPool,
        location: PhotoLocation,
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
                pool: pool.clone(),
                location: PhotoLocation {
                    workspace_id,
                    gallery_id,
                },
            }
        }

        async fn insert_contact(&self, email: &str) -> ContactId {
            let id = ContactId::generate();

            sqlx::query(
                "INSERT INTO contacts (id, workspace_id, name, email) VALUES ($1, $2, 'A Client', $3)",
            )
            .bind(id)
            .bind(self.location.workspace_id)
            .bind(email)
            .execute(&self.pool)
            .await
            .unwrap();

            id
        }

        async fn record(&self, client: Option<Client>, kind: ActivityKind, photo_ids: &[PhotoId]) {
            let options = ActivityOptions::default().client(client);
            let mut transaction = self.pool.begin().await.unwrap();

            transaction
                .record_activity(&options, self.location, kind, photo_ids, None)
                .await
                .unwrap();
            transaction.commit().await.unwrap();
        }

        async fn backdate_past_the_window(&self) {
            sqlx::query(
                "UPDATE activity_events SET last_at = now() - interval '1 hour'
                 WHERE gallery_id = $1",
            )
            .bind(self.location.gallery_id)
            .execute(&self.pool)
            .await
            .unwrap();
        }

        async fn lines(&self) -> Vec<ActivityRow> {
            sqlx::query_as(
                "SELECT contact_id, kind, count, photo_ids FROM activity_events
                 WHERE gallery_id = $1 ORDER BY first_at, id",
            )
            .bind(self.location.gallery_id)
            .fetch_all(&self.pool)
            .await
            .unwrap()
        }

        async fn bell(&self) -> Option<(NotificationKind, i32)> {
            sqlx::query_as(
                "SELECT kind, count FROM notifications
                 WHERE gallery_id = $1 AND read_at IS NULL",
            )
            .bind(self.location.gallery_id)
            .fetch_optional(&self.pool)
            .await
            .unwrap()
        }

        async fn delete_gallery(&self) {
            TestRepository::open(&self.pool, ())
                .delete_gallery(self.location.gallery_id)
                .await
                .unwrap();
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

    const CLIENT: Option<Client> = Some(Client { contact_id: None });

    #[sqlx::test(migrations = "./migrations")]
    async fn events_within_the_window_extend_one_line(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        fixture.record(CLIENT, ActivityKind::Open, &[]).await;
        fixture.record(CLIENT, ActivityKind::Open, &[]).await;

        let lines = fixture.lines().await;

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].count, 2);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn an_event_after_the_window_opens_a_new_line(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        fixture.record(CLIENT, ActivityKind::Open, &[]).await;
        fixture.backdate_past_the_window().await;
        fixture.record(CLIENT, ActivityKind::Open, &[]).await;

        let lines = fixture.lines().await;

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].count, 1);
        assert_eq!(lines[1].count, 1);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn contacts_and_kinds_have_separate_lines(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let a = Client {
            contact_id: Some(fixture.insert_contact("a@example.test").await),
        };
        let b = Client {
            contact_id: Some(fixture.insert_contact("b@example.test").await),
        };

        fixture.record(Some(a), ActivityKind::Open, &[]).await;
        fixture.record(Some(b), ActivityKind::Open, &[]).await;
        fixture.record(Some(a), ActivityKind::Favorite, &[]).await;

        let lines: Vec<_> = fixture
            .lines()
            .await
            .into_iter()
            .map(|line| (line.contact_id, line.kind, line.count))
            .collect();

        assert_eq!(lines.len(), 3);
        assert!(lines.contains(&(a.contact_id, ActivityKind::Open, 1)));
        assert!(lines.contains(&(a.contact_id, ActivityKind::Favorite, 1)));
        assert!(lines.contains(&(b.contact_id, ActivityKind::Open, 1)));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn anonymous_lines_coalesce_apart_from_named_contacts(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let named = Client {
            contact_id: Some(fixture.insert_contact("named@example.test").await),
        };

        fixture.record(CLIENT, ActivityKind::Open, &[]).await;
        fixture.record(CLIENT, ActivityKind::Open, &[]).await;
        fixture.record(Some(named), ActivityKind::Open, &[]).await;

        let mut lines = fixture.lines().await;
        lines.sort_by_key(|line| line.count);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].contact_id, named.contact_id);
        assert_eq!(lines[0].count, 1);
        assert_eq!(lines[1].contact_id, None);
        assert_eq!(lines[1].count, 2);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn photo_ids_keep_at_most_four(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        for _ in 0..6 {
            let photo_id = PhotoId::from(uuid::Uuid::new_v4());
            fixture
                .record(CLIENT, ActivityKind::Favorite, &[photo_id])
                .await;
        }

        let lines = fixture.lines().await;

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].count, 6);
        assert_eq!(lines[0].photo_ids.len(), 4);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn the_photographer_records_nothing(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        fixture.record(None, ActivityKind::Open, &[]).await;

        assert!(fixture.lines().await.is_empty());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn concurrent_events_make_one_line(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        tokio::join!(
            fixture.record(CLIENT, ActivityKind::Favorite, &[]),
            fixture.record(CLIENT, ActivityKind::Favorite, &[]),
            fixture.record(CLIENT, ActivityKind::Favorite, &[]),
            fixture.record(CLIENT, ActivityKind::Favorite, &[]),
            fixture.record(CLIENT, ActivityKind::Favorite, &[]),
            fixture.record(CLIENT, ActivityKind::Favorite, &[]),
            fixture.record(CLIENT, ActivityKind::Favorite, &[]),
            fixture.record(CLIENT, ActivityKind::Favorite, &[]),
        );

        let lines = fixture.lines().await;

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].count, 8);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn deleting_a_contact_keeps_its_lines_anonymous(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let contact_id = fixture.insert_contact("gone@example.test").await;
        let client = Some(Client {
            contact_id: Some(contact_id),
        });

        fixture.record(client, ActivityKind::Open, &[]).await;
        sqlx::query("DELETE FROM contacts WHERE id = $1")
            .bind(contact_id)
            .execute(&pool)
            .await
            .unwrap();

        let lines = fixture.lines().await;

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].contact_id, None);
        assert_eq!(lines[0].count, 1);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn deleting_a_gallery_removes_its_lines(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        fixture.record(CLIENT, ActivityKind::Open, &[]).await;
        fixture.delete_gallery().await;

        assert!(fixture.lines().await.is_empty());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn the_bell_is_derived_from_activity(pool: PgPool) {
        let database = Database::new(pool.clone());
        let mut listener = database.listen().await.unwrap();

        for (kind, photo_ids, expected) in [
            (
                ActivityKind::Favorite,
                vec![],
                Some((NotificationKind::Selection, 0)),
            ),
            (
                ActivityKind::Comment,
                vec![],
                Some((NotificationKind::Comment, 1)),
            ),
            (
                ActivityKind::Open,
                vec![],
                Some((NotificationKind::Open, 1)),
            ),
            (
                ActivityKind::Download,
                vec![PhotoId::from(uuid::Uuid::new_v4())],
                Some((NotificationKind::Download, 1)),
            ),
            (ActivityKind::Email, vec![], None),
        ] {
            let fixture = Fixture::new(&pool).await;

            fixture.record(CLIENT, kind, &photo_ids).await;

            assert_eq!(fixture.bell().await, expected, "{kind:?}");
            assert_eq!(
                next(&mut listener).await,
                Notification::Notification(NotificationEvent {
                    workspace_id: fixture.location.workspace_id,
                    gallery_id: fixture.location.gallery_id,
                }),
                "{kind:?}"
            );
            assert_silent(&mut listener).await;
        }
    }
}
