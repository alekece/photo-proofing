//! The workspace activity log: what a client did across galleries, extended into one line per
//! gallery, contact and kind while repeats stay inside [`ActivityOptions::window`]. A database
//! trigger derives the photographer's bell notification from the change in each line's count, so
//! the two can never disagree.

use std::{fmt, str::FromStr, time::Duration};

use pp_core::{config::DEFAULT_ACTIVITY_WINDOW, types::timestamp::Timestamp};
use serde::{Deserialize, Serialize};
use sqlx::{AssertSqlSafe, FromRow, PgConnection, PgPool, Row, Type, postgres::PgRow};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use crate::{
    Repository,
    auth::WorkspaceId,
    contact::ContactId,
    error::DatabaseError,
    gallery::GalleryId,
    jobs::{JobId, JobRepository},
    photo::{PhotoId, PhotoLocation},
};

/// Lines in one page of [`ActivityRepository::list`].
pub const PAGE_SIZE: usize = 30;

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
    EmailFailed,
}

impl ActivityKind {
    /// How much one call adds to the line's `count`. A download call logs the number of photos
    /// it newly affected, which a ZIP's first download may make zero; every other kind is one
    /// flip, comment or open, regardless of how many photos it names for the thumbnails.
    fn delta(self, photo_ids: &[PhotoId]) -> i32 {
        match self {
            Self::Download => i32::try_from(photo_ids.len()).unwrap_or(i32::MAX),
            Self::Open
            | Self::Favorite
            | Self::Unfavorite
            | Self::Comment
            | Self::Email
            | Self::EmailFailed => 1,
        }
    }
}

/// Writes one client action into the gallery's activity log.
///
/// A no-op when `options.client` is `None`. Otherwise the call extends the line of the same
/// gallery, contact and kind whose `last_at` is still inside `options.window`, or opens a new
/// one: `count` grows by [`ActivityKind::delta`], and `detail`, when given, replaces the line's.
/// Favorite and unfavorite lines keep distinct photo IDs in first-seen order; other lines keep
/// only the first four thumbnail IDs. A transaction-scoped advisory lock on the gallery, contact
/// and kind orders concurrent calls on the same line, so none are lost and the window is never split.
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
                     photo_ids = CASE WHEN kind IN ('favorite', 'unfavorite') THEN ARRAY(
                         SELECT id
                         FROM unnest(photo_ids || $2::uuid[]) WITH ORDINALITY AS photos(id, position)
                         GROUP BY id
                         ORDER BY min(position)
                     ) ELSE (photo_ids || $2::uuid[])[1:4] END,
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
             VALUES ($1, $2, $3, $4, $5,
                     CASE WHEN $4 IN ('favorite', 'unfavorite') THEN ARRAY(
                         SELECT id
                         FROM unnest($6::uuid[]) WITH ORDINALITY AS photos(id, position)
                         GROUP BY id
                         ORDER BY min(position)
                     ) ELSE $6[1:4] END, $7)",
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

/// Identifies one activity line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct ActivityEventId(pub Uuid);

/// Why a string is not an [`ActivityCursor`] or a [`ContactFilter`].
#[derive(Debug, Error)]
#[error("not a valid activity {0}")]
pub struct InvalidActivityParameter(&'static str);

/// Where the next page of the list starts: just after one line, which a line that is extended
/// meanwhile can no longer be. Serialized as an opaque string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct ActivityCursor {
    last_at: Timestamp,
    id: ActivityEventId,
}

impl fmt::Display for ActivityCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let last_at = self.last_at.0.format(&Rfc3339).map_err(|_| fmt::Error)?;

        write!(formatter, "{last_at}_{}", self.id.0)
    }
}

impl FromStr for ActivityCursor {
    type Err = InvalidActivityParameter;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || InvalidActivityParameter("cursor");
        let (last_at, id) = text.split_once('_').ok_or_else(invalid)?;
        let last_at = OffsetDateTime::parse(last_at, &Rfc3339).map_err(|_| invalid())?;
        let id = id.parse().map_err(|_| invalid())?;

        Ok(Self {
            last_at: Timestamp(last_at),
            id: ActivityEventId(id),
        })
    }
}

impl From<ActivityCursor> for String {
    fn from(cursor: ActivityCursor) -> Self {
        cursor.to_string()
    }
}

impl TryFrom<String> for ActivityCursor {
    type Error = InvalidActivityParameter;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

/// Which clients' lines to list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub enum ContactFilter {
    /// Lines of the general link's visitors, who are not a contact.
    Anonymous,
    Contact(ContactId),
}

impl TryFrom<String> for ContactFilter {
    type Error = InvalidActivityParameter;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        if text == "anonymous" {
            return Ok(Self::Anonymous);
        }

        text.parse()
            .map(|id| Self::Contact(ContactId(id)))
            .map_err(|_| InvalidActivityParameter("contact"))
    }
}

/// How far back to list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ActivityRange {
    /// The last 24 hours.
    #[serde(rename = "today")]
    Today,
    #[serde(rename = "7d")]
    Week,
    #[serde(rename = "30d")]
    Month,
}

impl ActivityRange {
    fn duration(self) -> Duration {
        const DAY: Duration = Duration::from_secs(24 * 60 * 60);

        match self {
            Self::Today => DAY,
            Self::Week => 7 * DAY,
            Self::Month => 30 * DAY,
        }
    }
}

/// The timeline and cursor use the same order, including for lines with equal timestamps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivityOrder {
    #[default]
    Newest,
    Oldest,
}

/// What [`ActivityRepository::list`] lists; every filter that is `None` lets all lines through.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ActivityQuery {
    pub gallery: Option<GalleryId>,
    pub contact: Option<ContactFilter>,
    pub kind: Option<ActivityKind>,
    pub range: Option<ActivityRange>,
    pub cursor: Option<ActivityCursor>,
    pub sort: ActivityOrder,
    pub include_archived: bool,
}

/// The contact a line is attributed to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActivityContact {
    pub id: ContactId,
    pub name: String,
}

/// One line of the log, with what the timeline shows of its gallery and contact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActivityEvent {
    pub id: ActivityEventId,
    pub gallery_id: GalleryId,
    pub gallery_title: Option<String>,
    /// `None` for the general link's visitors.
    pub contact: Option<ActivityContact>,
    pub kind: ActivityKind,
    pub can_retry: bool,
    pub count: usize,
    /// Recorded photo IDs that still exist, in first-seen order. Other than favorite and
    /// unfavorite lines, these are a sample of at most four photos.
    pub photo_ids: Vec<PhotoId>,
    pub detail: Option<String>,
    pub first_at: Timestamp,
    pub last_at: Timestamp,
}

impl FromRow<'_, PgRow> for ActivityEvent {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let contact = row
            .try_get::<Option<ContactId>, _>("contact_id")?
            .zip(row.try_get::<Option<String>, _>("contact_name")?)
            .map(|(id, name)| ActivityContact { id, name });
        let count = usize::try_from(row.try_get::<i32, _>("count")?)
            .map_err(|error| sqlx::Error::Decode(error.into()))?;

        Ok(Self {
            id: row.try_get("id")?,
            gallery_id: row.try_get("gallery_id")?,
            gallery_title: row.try_get("gallery_title")?,
            contact,
            kind: row.try_get("kind")?,
            can_retry: row.try_get("can_retry")?,
            count,
            photo_ids: row.try_get("photo_ids")?,
            detail: row.try_get("detail")?,
            first_at: row.try_get("first_at")?,
            last_at: row.try_get("last_at")?,
        })
    }
}

/// One page of lines in the order requested.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActivityPage {
    pub events: Vec<ActivityEvent>,
    /// Where the next page starts; `None` on the last one.
    pub next: Option<ActivityCursor>,
    /// Boundary of a completed oldest-first list, for fetching newer activity on refresh.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail: Option<ActivityCursor>,
}

/// Logs what the worker itself does, which has no repository write to piggyback on, and prunes
/// old lines.
#[derive(Clone, Copy)]
pub struct ActivityRepository<'db> {
    pool: &'db PgPool,
    options: ActivityOptions,
}

impl<'db> Repository<'db> for ActivityRepository<'db> {
    type Options = ActivityOptions;

    fn open(pool: &'db PgPool, options: Self::Options) -> Self {
        Self { pool, options }
    }
}

impl ActivityRepository<'_> {
    /// Requeues this line's failed invitation only while its gallery, stage and recipient are active.
    pub async fn retry_email(
        &self,
        workspace: WorkspaceId,
        gallery: GalleryId,
        event: ActivityEventId,
    ) -> Result<bool, DatabaseError> {
        let mut tx = self.pool.begin().await?;
        let eligible: Option<(JobId, Option<ContactId>)> = sqlx::query_as(
            "SELECT job_id, contact_id FROM retryable_email_activity
             WHERE workspace_id=$1 AND gallery_id=$2 AND event_id=$3 FOR UPDATE",
        )
        .bind(workspace)
        .bind(gallery)
        .bind(event)
        .fetch_optional(&mut *tx)
        .await?;

        let Some((job, contact)) = eligible else {
            return Ok(false);
        };

        if let Some(contact) = contact {
            let link = sqlx::query(
                "SELECT FROM gallery_contacts WHERE gallery_id=$1 AND contact_id=$2 FOR KEY SHARE",
            )
            .bind(gallery)
            .bind(contact)
            .fetch_optional(&mut *tx)
            .await?;

            if link.is_none() {
                return Ok(false);
            }
        }

        if !JobRepository::retry_in(&mut *tx, job).await? {
            return Ok(false);
        }

        // Wake both timelines without moving this historical line or signalling the bell.
        sqlx::query("UPDATE activity_events SET last_at=last_at WHERE id=$1")
            .bind(event)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        Ok(true)
    }

    /// Logs a sent email on the line of `contact_id`, or of nobody (`None`) when it went to a
    /// bare address. The options' own client is ignored: the recipient owns the line.
    pub async fn record_email(
        &self,
        location: PhotoLocation,
        contact_id: Option<ContactId>,
        detail: &str,
    ) -> Result<(), DatabaseError> {
        let options = self.options.client(Some(Client { contact_id }));
        let mut transaction = self.pool.begin().await?;

        transaction
            .record_activity(&options, location, ActivityKind::Email, &[], Some(detail))
            .await?;
        transaction.commit().await?;

        Ok(())
    }

    /// Lists the workspace's matching lines in the requested order, [`PAGE_SIZE`] at a time.
    /// Archived galleries are left out unless requested. Keep the sort unchanged when paging.
    pub async fn list(
        &self,
        workspace_id: WorkspaceId,
        query: ActivityQuery,
    ) -> Result<ActivityPage, DatabaseError> {
        let contact = match query.contact {
            Some(ContactFilter::Contact(id)) => Some(id),
            _ => None,
        };
        let anonymous = query.contact == Some(ContactFilter::Anonymous);
        let limit = i64::try_from(PAGE_SIZE + 1).unwrap_or(i64::MAX);

        let (after_cursor, direction) = match query.sort {
            ActivityOrder::Newest => ("<", "DESC"),
            ActivityOrder::Oldest => (">", "ASC"),
        };
        let statement = format!(
            "SELECT e.id, e.gallery_id, g.title AS gallery_title, e.contact_id,
                    c.name AS contact_name, e.kind, e.count,
                    e.failed_email_job IS NOT NULL AND EXISTS
                        (SELECT FROM retryable_email_activity r WHERE r.event_id=e.id) AS can_retry,
                    ARRAY(SELECT p.id
                          FROM unnest(e.photo_ids) WITH ORDINALITY AS line(photo_id, position)
                          JOIN photos p ON p.id = line.photo_id
                          ORDER BY line.position) AS photo_ids,
                    e.detail, e.first_at, e.last_at
             FROM activity_events e
             JOIN galleries g ON g.id = e.gallery_id
             LEFT JOIN contacts c ON c.id = e.contact_id
             WHERE e.workspace_id = $1
               AND ($2::uuid IS NULL OR e.gallery_id = $2)
               AND ($3::uuid IS NULL OR e.contact_id = $3)
               AND (NOT $4 OR e.contact_id IS NULL)
               AND ($5::text IS NULL OR e.kind = $5)
               AND ($6::float8 IS NULL OR e.last_at >= now() - make_interval(secs => $6))
               AND ($7::timestamptz IS NULL OR (e.last_at, e.id) {after_cursor} ($7, $8::uuid))
               AND ($9 OR g.archived_at IS NULL)
             ORDER BY e.last_at {direction}, e.id {direction}
             LIMIT $10",
        );
        // Only fixed operators and keywords from ActivityOrder are interpolated.
        let mut events: Vec<ActivityEvent> = sqlx::query_as(AssertSqlSafe(statement))
            .bind(workspace_id)
            .bind(query.gallery)
            .bind(contact)
            .bind(anonymous)
            .bind(query.kind)
            .bind(query.range.map(|range| range.duration().as_secs_f64()))
            .bind(query.cursor.map(|cursor| cursor.last_at))
            .bind(query.cursor.map(|cursor| cursor.id))
            .bind(query.include_archived)
            .bind(limit)
            .fetch_all(self.pool)
            .await?;
        let has_more = events.len() > PAGE_SIZE;

        events.truncate(PAGE_SIZE);

        let boundary = events.last().map(|event| ActivityCursor {
            last_at: event.last_at,
            id: event.id,
        });
        let next = boundary.filter(|_| has_more);
        let tail = boundary.filter(|_| !has_more && query.sort == ActivityOrder::Oldest);

        Ok(ActivityPage { events, next, tail })
    }

    /// Deletes lines whose last activity is older than `retention`.
    pub async fn expire(&self, retention: Duration) -> Result<u64, DatabaseError> {
        let removed = sqlx::query(
            "DELETE FROM activity_events WHERE last_at < now() - make_interval(secs => $1)",
        )
        .bind(retention.as_secs_f64())
        .execute(self.pool)
        .await?;

        Ok(removed.rows_affected())
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
        contact::ContactRepository,
        gallery::{GalleryId, GalleryRepository, InvitationBatch, InvitationContact},
        jobs::{Backoff, JobKind, JobOptions, JobState},
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

        async fn insert_photo(&self) -> PhotoId {
            let id = PhotoId::generate();

            sqlx::query(
                "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width,
                                     height, status, kind, clean_ready)
                 VALUES ($1, $2, $3, 'a.jpg', 1, 1, 1, 'ready', 'proof', true)",
            )
            .bind(id)
            .bind(self.location.workspace_id)
            .bind(self.location.gallery_id)
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
    async fn favorite_lines_keep_all_distinct_photo_ids_in_order(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let mut photos = Vec::new();

        for _ in 0..12 {
            photos.push(fixture.insert_photo().await);
        }

        for kind in [ActivityKind::Favorite, ActivityKind::Unfavorite] {
            for photo in &photos {
                fixture.record(CLIENT, kind, &[*photo]).await;
            }

            fixture.record(CLIENT, kind, &[photos[2]]).await;
        }

        let lines = fixture.lines().await;

        assert_eq!(lines.len(), 2);

        for kind in [ActivityKind::Favorite, ActivityKind::Unfavorite] {
            let line = lines.iter().find(|line| line.kind == kind).unwrap();

            assert_eq!(line.count, 13);
            assert_eq!(line.photo_ids, photos);
        }

        let deleted = photos.remove(4);

        sqlx::query("DELETE FROM photos WHERE id = $1")
            .bind(deleted)
            .execute(&pool)
            .await
            .unwrap();

        let events = fixture.list(ActivityQuery::default()).await.events;

        assert_eq!(events.len(), 2);

        for event in events {
            assert_eq!(event.count, 13);
            assert_eq!(event.photo_ids, photos);
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn favorite_insert_deduplicates_before_any_update(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let [first, second] = [PhotoId::generate(), PhotoId::generate()];

        fixture
            .record(CLIENT, ActivityKind::Favorite, &[first, second, first])
            .await;

        assert_eq!(fixture.lines().await[0].photo_ids, [first, second]);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn download_lines_keep_only_four_thumbnail_ids(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let photos: Vec<_> = (0..13).map(|_| PhotoId::generate()).collect();

        fixture
            .record(CLIENT, ActivityKind::Download, &photos[..12])
            .await;
        fixture
            .record(CLIENT, ActivityKind::Download, &photos[12..])
            .await;

        let lines = fixture.lines().await;

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].count, 13);
        assert_eq!(lines[0].photo_ids, photos[..4]);
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

    #[sqlx::test(migrations = "./migrations")]
    async fn revoked_obsolete_and_removed_lines_cannot_retry_an_invitation(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let PhotoLocation {
            workspace_id,
            gallery_id,
        } = fixture.location;
        let db = Database::new(pool.clone());
        let galleries = db.repository::<GalleryRepository>();
        let jobs = db.repository::<JobRepository>();
        let activity = db.repository::<ActivityRepository>();
        let batch = InvitationBatch::new(
            vec![InvitationContact {
                email: "client@example.test".parse().unwrap(),
                name: None,
            }],
            vec![],
            "Gallery".parse().unwrap(),
            "Welcome".parse().unwrap(),
        );
        galleries
            .queue_invitations(
                workspace_id,
                gallery_id,
                &batch,
                JobOptions {
                    max_attempts: 2.try_into().unwrap(),
                    ..JobOptions::default()
                },
            )
            .await
            .unwrap();

        for expected in [JobState::Queued, JobState::Failed] {
            let job = jobs
                .claim(&[JobKind::SendEmail], Duration::from_secs(60))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                jobs.fail(
                    &job,
                    "temporary",
                    Backoff::new(Duration::ZERO, Duration::ZERO)
                )
                .await
                .unwrap(),
                Some(expected)
            );
            if expected == JobState::Queued {
                assert!(fixture.lines().await.is_empty());
            }
        }

        let line = fixture
            .list(ActivityQuery::default())
            .await
            .events
            .remove(0);
        let contact = line.contact.unwrap().id;
        let retry = || activity.retry_email(workspace_id, gallery_id, line.id);
        assert!(line.can_retry);
        galleries.archive(workspace_id, gallery_id).await.unwrap();
        assert!(!retry().await.unwrap());
        galleries.restore(workspace_id, gallery_id).await.unwrap();
        let contacts = db.repository::<ContactRepository>();
        contacts
            .unlink_gallery(workspace_id, gallery_id, contact)
            .await
            .unwrap();
        assert!(!retry().await.unwrap());
        contacts
            .link_gallery(workspace_id, gallery_id, contact)
            .await
            .unwrap();
        galleries.deliver(workspace_id, gallery_id).await.unwrap();
        assert!(!retry().await.unwrap());
        assert_eq!(activity.expire(Duration::ZERO).await.unwrap(), 1);
        assert!(!retry().await.unwrap());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_sent_email_records_an_email_line_for_its_contact(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let contact = fixture.insert_contact("alice@example.test").await;
        let activity = ActivityRepository::open(&pool, ActivityOptions::default());

        activity
            .record_email(fixture.location, Some(contact), "invitation")
            .await
            .unwrap();
        activity
            .record_email(fixture.location, None, "invitation")
            .await
            .unwrap();

        let lines = fixture.lines().await;

        assert_eq!(
            lines
                .iter()
                .map(|line| (line.contact_id, line.kind, line.count))
                .collect::<Vec<_>>(),
            vec![
                (Some(contact), ActivityKind::Email, 1),
                (None, ActivityKind::Email, 1)
            ]
        );
        assert_eq!(fixture.bell().await, None);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn expire_deletes_only_lines_older_than_the_retention(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let activity = ActivityRepository::open(&pool, ActivityOptions::default());

        fixture.record(CLIENT, ActivityKind::Open, &[]).await;
        fixture.record(CLIENT, ActivityKind::Comment, &[]).await;
        sqlx::query(
            "UPDATE activity_events SET last_at = now() - interval '91 days' WHERE kind = 'open'",
        )
        .execute(&pool)
        .await
        .unwrap();

        let removed = activity
            .expire(Duration::from_secs(90 * 24 * 60 * 60))
            .await
            .unwrap();
        let lines = fixture.lines().await;

        assert_eq!(removed, 1);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].kind, ActivityKind::Comment);
    }

    impl Fixture {
        async fn insert_gallery(&self) -> GalleryId {
            GalleryRepository::open(&self.pool, ActivityOptions::default())
                .create(self.location.workspace_id)
                .await
                .unwrap()
                .id
        }

        async fn insert_line(
            &self,
            gallery_id: GalleryId,
            contact_id: Option<ContactId>,
            kind: ActivityKind,
            age: &str,
            photo_ids: &[PhotoId],
        ) {
            sqlx::query(
                "INSERT INTO activity_events (workspace_id, gallery_id, contact_id, kind, count,
                                              photo_ids, first_at, last_at)
                 VALUES ($1, $2, $3, $4, 1, $5, now() - $6::interval, now() - $6::interval)",
            )
            .bind(self.location.workspace_id)
            .bind(gallery_id)
            .bind(contact_id)
            .bind(kind)
            .bind(photo_ids)
            .bind(age)
            .execute(&self.pool)
            .await
            .unwrap();
        }

        async fn list(&self, query: ActivityQuery) -> ActivityPage {
            ActivityRepository::open(&self.pool, ActivityOptions::default())
                .list(self.location.workspace_id, query)
                .await
                .unwrap()
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn lines_are_listed_newest_first_and_paged_by_cursor(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let gallery_id = fixture.location.gallery_id;

        for minutes in 0..=PAGE_SIZE {
            fixture
                .insert_line(
                    gallery_id,
                    None,
                    ActivityKind::Open,
                    &format!("{minutes} minutes"),
                    &[],
                )
                .await;
        }

        let first = fixture.list(ActivityQuery::default()).await;

        assert_eq!(first.events.len(), PAGE_SIZE);
        assert!(
            first
                .events
                .windows(2)
                .all(|pair| pair[0].last_at > pair[1].last_at)
        );

        let second = fixture
            .list(ActivityQuery {
                cursor: first.next,
                ..ActivityQuery::default()
            })
            .await;

        assert_eq!(second.events.len(), 1);
        assert_eq!(second.next, None);
        assert!(second.events[0].last_at < first.events[PAGE_SIZE - 1].last_at);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn oldest_first_pages_through_equal_timestamps_once(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let gallery_id = fixture.location.gallery_id;

        for minutes in 0..=PAGE_SIZE {
            fixture
                .insert_line(
                    gallery_id,
                    None,
                    ActivityKind::Open,
                    &format!("{minutes} minutes"),
                    &[],
                )
                .await;
        }

        sqlx::query("UPDATE activity_events SET last_at = now() WHERE gallery_id = $1")
            .bind(gallery_id)
            .execute(&pool)
            .await
            .unwrap();

        let query = ActivityQuery {
            sort: ActivityOrder::Oldest,
            ..ActivityQuery::default()
        };
        let first = fixture.list(query).await;
        let second = fixture
            .list(ActivityQuery {
                cursor: first.next,
                ..query
            })
            .await;
        let got: Vec<_> = first
            .events
            .into_iter()
            .chain(second.events)
            .map(|event| event.id.0)
            .collect();
        let mut expected = got.clone();

        expected.sort_unstable();
        expected.dedup();
        assert_eq!(expected.len(), PAGE_SIZE + 1);
        assert_eq!(got, expected);
        assert_eq!(second.next, None);

        fixture
            .insert_line(gallery_id, None, ActivityKind::Comment, "-1 minute", &[])
            .await;

        let resumed = fixture
            .list(ActivityQuery {
                cursor: second.tail,
                ..query
            })
            .await;

        assert_eq!(resumed.events.len(), 1);
        assert_eq!(resumed.events[0].kind, ActivityKind::Comment);
        assert_eq!(resumed.next, None);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn each_filter_narrows_the_list(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let alice = fixture.insert_contact("alice@example.test").await;
        let first = fixture.location.gallery_id;
        let second = fixture.insert_gallery().await;

        fixture
            .insert_line(first, Some(alice), ActivityKind::Favorite, "1 hour", &[])
            .await;
        fixture
            .insert_line(first, None, ActivityKind::Comment, "3 days", &[])
            .await;
        fixture
            .insert_line(second, Some(alice), ActivityKind::Open, "40 days", &[])
            .await;

        let kinds = async |query: ActivityQuery| {
            fixture
                .list(query)
                .await
                .events
                .into_iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            kinds(ActivityQuery::default()).await,
            [
                ActivityKind::Favorite,
                ActivityKind::Comment,
                ActivityKind::Open
            ]
        );
        assert_eq!(
            kinds(ActivityQuery {
                gallery: Some(second),
                ..ActivityQuery::default()
            })
            .await,
            [ActivityKind::Open]
        );
        assert_eq!(
            kinds(ActivityQuery {
                contact: Some(ContactFilter::Contact(alice)),
                ..ActivityQuery::default()
            })
            .await,
            [ActivityKind::Favorite, ActivityKind::Open]
        );
        assert_eq!(
            kinds(ActivityQuery {
                contact: Some(ContactFilter::Anonymous),
                ..ActivityQuery::default()
            })
            .await,
            [ActivityKind::Comment]
        );
        assert_eq!(
            kinds(ActivityQuery {
                kind: Some(ActivityKind::Comment),
                ..ActivityQuery::default()
            })
            .await,
            [ActivityKind::Comment]
        );
        assert_eq!(
            kinds(ActivityQuery {
                range: Some(ActivityRange::Today),
                ..ActivityQuery::default()
            })
            .await,
            [ActivityKind::Favorite]
        );
        assert_eq!(
            kinds(ActivityQuery {
                range: Some(ActivityRange::Week),
                ..ActivityQuery::default()
            })
            .await,
            [ActivityKind::Favorite, ActivityKind::Comment]
        );
        assert_eq!(
            kinds(ActivityQuery {
                range: Some(ActivityRange::Month),
                ..ActivityQuery::default()
            })
            .await,
            [ActivityKind::Favorite, ActivityKind::Comment]
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn archived_galleries_are_hidden_unless_asked_for(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        fixture
            .insert_line(
                fixture.location.gallery_id,
                None,
                ActivityKind::Open,
                "1 hour",
                &[],
            )
            .await;
        sqlx::query("UPDATE galleries SET archived_at = now() WHERE id = $1")
            .bind(fixture.location.gallery_id)
            .execute(&pool)
            .await
            .unwrap();

        assert!(
            fixture
                .list(ActivityQuery::default())
                .await
                .events
                .is_empty()
        );
        assert_eq!(
            fixture
                .list(ActivityQuery {
                    include_archived: true,
                    ..ActivityQuery::default()
                })
                .await
                .events
                .len(),
            1
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_workspace_only_lists_its_own_lines(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let other = Fixture::new(&pool).await;

        fixture
            .insert_line(
                fixture.location.gallery_id,
                None,
                ActivityKind::Open,
                "1 hour",
                &[],
            )
            .await;

        assert!(other.list(ActivityQuery::default()).await.events.is_empty());
        assert_eq!(fixture.list(ActivityQuery::default()).await.events.len(), 1);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_line_names_its_contact_and_only_the_photos_that_still_exist(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let alice = fixture.insert_contact("alice@example.test").await;
        let [kept, deleted] = [fixture.insert_photo().await, fixture.insert_photo().await];

        fixture
            .insert_line(
                fixture.location.gallery_id,
                Some(alice),
                ActivityKind::Favorite,
                "1 hour",
                &[deleted, kept],
            )
            .await;
        sqlx::query("DELETE FROM photos WHERE id = $1")
            .bind(deleted)
            .execute(&pool)
            .await
            .unwrap();

        let events = fixture.list(ActivityQuery::default()).await.events;

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].photo_ids, [kept]);
        assert_eq!(
            events[0]
                .contact
                .as_ref()
                .map(|contact| (contact.id, contact.name.as_str())),
            Some((alice, "A Client"))
        );
    }
}
