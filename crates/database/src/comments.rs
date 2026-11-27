//! Photo threads scoped to a workspace and gallery. Writes lock delivery and the photo so a
//! concurrent delivery cannot accept a late client write, and concurrent posts cannot exceed
//! the thread cap. Deleted bodies are redacted in SQL for client reads.

use std::num::NonZeroU16;

use pp_core::types::{bounded::BoundedText, timestamp::Timestamp};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, Transaction, Type};
use uuid::Uuid;

use crate::{
    Repository,
    activity::{ActivityKind, ActivityOptions, ActivityRecorder, Client},
    contact::{ContactId, ContactName},
    error::DatabaseError,
    photo::{PhotoId, PhotoLocation},
};

pub const MAX_BODY_CHARS: usize = 500;

pub type CommentBody = BoundedText<1, MAX_BODY_CHARS>;

/// How much of the latest client comment an Activity line quotes.
const ACTIVITY_PREVIEW_CHARS: usize = 140;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct CommentId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum CommentAuthor {
    Client,
    Photographer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
enum CommentOrigin {
    General,
    Contact,
    Photographer,
}

/// Non-secret contact identity included only in photographer comment reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentContact {
    pub id: ContactId,
    pub name: ContactName,
}

#[derive(Debug, FromRow)]
struct StoredComment {
    id: CommentId,
    photo_id: PhotoId,
    author: CommentAuthor,
    body: Option<String>,
    created_at: Timestamp,
    edited_at: Option<Timestamp>,
    deleted_at: Option<Timestamp>,
    contact_id: Option<ContactId>,
    contact_name: Option<ContactName>,
    editable: bool,
}

impl From<StoredComment> for Comment {
    fn from(comment: StoredComment) -> Self {
        Self {
            id: comment.id,
            photo_id: comment.photo_id,
            author: comment.author,
            body: comment.body,
            created_at: comment.created_at,
            edited_at: comment.edited_at,
            deleted_at: comment.deleted_at,
            editable: comment.editable,
            contact: comment
                .contact_id
                .zip(comment.contact_name)
                .map(|(id, name)| CommentContact { id, name }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Comment {
    pub id: CommentId,
    pub photo_id: PhotoId,
    pub author: CommentAuthor,
    pub body: Option<String>,
    pub created_at: Timestamp,
    pub edited_at: Option<Timestamp>,
    pub deleted_at: Option<Timestamp>,
    /// The current viewer may mutate this comment.
    pub editable: bool,
    /// Present for an owner read of a personal-link client comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<CommentContact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentCount {
    pub photo_id: PhotoId,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_body: Option<String>,
}

#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentChange<T> {
    Applied(T),
    NotFound,
    Delivered,
    Full,
}

#[derive(Clone, Copy)]
pub struct CommentRepository<'db> {
    pool: &'db PgPool,
    options: ActivityOptions,
}

impl<'db> Repository<'db> for CommentRepository<'db> {
    type Options = ActivityOptions;

    fn open(pool: &'db PgPool, options: Self::Options) -> Self {
        Self { pool, options }
    }
}

impl CommentRepository<'_> {
    pub async fn thread(
        &self,
        location: PhotoLocation,
        photo: PhotoId,
        viewer: CommentAuthor,
        client: Option<Client>,
    ) -> Result<Option<Vec<Comment>>, DatabaseError> {
        let found: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                 SELECT 1 FROM photos
                 WHERE workspace_id = $1 AND gallery_id = $2 AND id = $3
                   AND kind = 'proof' AND clean_ready
             )",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(photo)
        .fetch_one(self.pool)
        .await?;

        if !found {
            return Ok(None);
        }

        let comments: Vec<StoredComment> = sqlx::query_as(
            "SELECT comment.id, comment.photo_id, comment.author,
                    CASE WHEN comment.deleted_at IS NOT NULL AND $4 = 'client'
                         THEN NULL ELSE comment.body END AS body,
                    comment.created_at, comment.edited_at, comment.deleted_at,
                    CASE WHEN $4 = 'photographer' THEN contact.id END AS contact_id,
                    CASE WHEN $4 = 'photographer' THEN contact.name END AS contact_name,
                    CASE
                        WHEN $4 = 'photographer' THEN comment.author = 'photographer'
                        WHEN $5 IS NULL THEN comment.contact_origin = 'general'
                        ELSE comment.contact_origin = 'contact'
                             AND coalesce(comment.contact_id = $5, false)
                    END AS editable
             FROM photo_comments AS comment
             LEFT JOIN contacts AS contact
               ON contact.id = comment.contact_id
              AND contact.workspace_id = comment.workspace_id
             WHERE comment.workspace_id = $1 AND comment.gallery_id = $2
               AND comment.photo_id = $3
             ORDER BY comment.created_at, comment.id",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(photo)
        .bind(viewer)
        .bind(client.and_then(|client| client.contact_id))
        .fetch_all(self.pool)
        .await?;

        Ok(Some(comments.into_iter().map(Comment::from).collect()))
    }

    pub async fn counts(
        &self,
        location: PhotoLocation,
        viewer: CommentAuthor,
    ) -> Result<Vec<CommentCount>, DatabaseError> {
        let rows: Vec<(PhotoId, i64, Option<String>)> = sqlx::query_as(
            "SELECT DISTINCT ON (c.photo_id) c.photo_id, count(*) OVER (PARTITION BY c.photo_id),
                    CASE WHEN $3 = 'photographer' THEN c.body END AS latest_body
             FROM photo_comments AS c JOIN photos AS p ON p.id = c.photo_id
             WHERE c.workspace_id = $1 AND c.gallery_id = $2 AND c.deleted_at IS NULL
               AND p.kind = 'proof' AND p.clean_ready
             ORDER BY c.photo_id, c.created_at DESC, c.id DESC",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(viewer)
        .fetch_all(self.pool)
        .await?;

        rows.into_iter()
            .map(|(photo_id, count, latest_body)| {
                let count =
                    usize::try_from(count).map_err(|error| sqlx::Error::Decode(error.into()))?;

                Ok(CommentCount {
                    photo_id,
                    count,
                    latest_body,
                })
            })
            .collect()
    }

    /// Locks the gallery against delivery and serializes changes to this photo's thread.
    async fn writable(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        location: PhotoLocation,
        photo: PhotoId,
        author: CommentAuthor,
    ) -> Result<CommentChange<()>, DatabaseError> {
        let delivered: Option<bool> = sqlx::query_scalar(
            "SELECT g.delivered_at IS NOT NULL
             FROM galleries AS g JOIN photos AS p ON p.gallery_id = g.id
             WHERE g.workspace_id = $1 AND g.id = $2 AND p.workspace_id = $1 AND p.id = $3
               AND p.kind = 'proof' AND p.clean_ready
             FOR SHARE OF g FOR UPDATE OF p",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(photo)
        .fetch_optional(&mut **tx)
        .await?;

        Ok(match delivered {
            None => CommentChange::NotFound,
            Some(true) if author == CommentAuthor::Client => CommentChange::Delivered,
            Some(_) => CommentChange::Applied(()),
        })
    }

    pub async fn add(
        &self,
        location: PhotoLocation,
        photo: PhotoId,
        author: CommentAuthor,
        body: &CommentBody,
        client_limit: NonZeroU16,
    ) -> Result<CommentChange<Comment>, DatabaseError> {
        let mut tx = self.pool.begin().await?;

        match self.writable(&mut tx, location, photo, author).await? {
            CommentChange::Applied(()) => {}
            CommentChange::Delivered => return Ok(CommentChange::Delivered),
            _ => return Ok(CommentChange::NotFound),
        }

        if author == CommentAuthor::Client {
            let count: i64 =
                sqlx::query_scalar("SELECT count(*) FROM photo_comments WHERE photo_id = $1")
                    .bind(photo)
                    .fetch_one(&mut *tx)
                    .await?;

            if count >= i64::from(client_limit.get()) {
                return Ok(CommentChange::Full);
            }
        }

        let (contact_id, origin) = match author {
            CommentAuthor::Photographer => (None, CommentOrigin::Photographer),
            CommentAuthor::Client => match self.options.client.and_then(|client| client.contact_id)
            {
                Some(contact_id) => (Some(contact_id), CommentOrigin::Contact),
                None => (None, CommentOrigin::General),
            },
        };
        let comment: StoredComment = sqlx::query_as(
            "INSERT INTO photo_comments
                 (workspace_id, gallery_id, photo_id, author, body, contact_id, contact_origin)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id, photo_id, author, body, created_at, edited_at, deleted_at,
                       NULL::uuid AS contact_id, NULL::text AS contact_name, TRUE AS editable",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(photo)
        .bind(author)
        .bind(body)
        .bind(contact_id)
        .bind(origin)
        .fetch_one(&mut *tx)
        .await?;

        if author == CommentAuthor::Client {
            let preview = body
                .as_str()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(ACTIVITY_PREVIEW_CHARS)
                .collect::<String>();

            tx.record_activity(
                &self.options,
                location,
                ActivityKind::Comment,
                &[photo],
                Some(&preview),
            )
            .await?;
        }

        tx.commit().await?;

        Ok(CommentChange::Applied(comment.into()))
    }

    /// Finds the photo of comment `id` in this gallery, if `author` wrote it.
    async fn photo_of(
        &self,
        location: PhotoLocation,
        id: CommentId,
        author: CommentAuthor,
        client: Option<Client>,
    ) -> Result<Option<PhotoId>, DatabaseError> {
        let photo = sqlx::query_scalar(
            "SELECT photo_id FROM photo_comments
             WHERE workspace_id = $1 AND gallery_id = $2 AND id = $3 AND author = $4
               AND (
                   $4 = 'photographer'
                   OR contact_origin = 'general' AND $5 IS NULL
                   OR contact_origin = 'contact' AND contact_id = $5
               )",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(id)
        .bind(author)
        .bind(client.and_then(|client| client.contact_id))
        .fetch_optional(self.pool)
        .await?;

        Ok(photo)
    }

    pub async fn edit(
        &self,
        location: PhotoLocation,
        id: CommentId,
        author: CommentAuthor,
        client: Option<Client>,
        body: &CommentBody,
    ) -> Result<CommentChange<Comment>, DatabaseError> {
        let Some(photo) = self.photo_of(location, id, author, client).await? else {
            return Ok(CommentChange::NotFound);
        };

        let mut tx = self.pool.begin().await?;

        match self.writable(&mut tx, location, photo, author).await? {
            CommentChange::Applied(()) => {}
            CommentChange::Delivered => return Ok(CommentChange::Delivered),
            _ => return Ok(CommentChange::NotFound),
        }

        let comment: Option<StoredComment> = sqlx::query_as(
            "UPDATE photo_comments SET body = $5, edited_at = now()
             WHERE workspace_id = $1 AND gallery_id = $2 AND id = $3 AND author = $4
               AND deleted_at IS NULL
               AND (
                   $4 = 'photographer'
                   OR contact_origin = 'general' AND $6 IS NULL
                   OR contact_origin = 'contact' AND contact_id = $6
               )
             RETURNING id, photo_id, author, body, created_at, edited_at, deleted_at,
                       NULL::uuid AS contact_id, NULL::text AS contact_name, TRUE AS editable",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(id)
        .bind(author)
        .bind(body)
        .bind(client.and_then(|client| client.contact_id))
        .fetch_optional(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(comment.map_or(CommentChange::NotFound, |comment| {
            CommentChange::Applied(comment.into())
        }))
    }

    pub async fn delete(
        &self,
        location: PhotoLocation,
        id: CommentId,
        author: CommentAuthor,
        client: Option<Client>,
    ) -> Result<CommentChange<()>, DatabaseError> {
        let Some(photo) = self.photo_of(location, id, author, client).await? else {
            return Ok(CommentChange::NotFound);
        };

        let mut tx = self.pool.begin().await?;

        match self.writable(&mut tx, location, photo, author).await? {
            CommentChange::Applied(()) => {}
            CommentChange::Delivered => return Ok(CommentChange::Delivered),
            _ => return Ok(CommentChange::NotFound),
        }

        // The client's deleted comment stays as a tombstone for the photographer; the
        // photographer's own comment is removed for good.
        let query = match author {
            CommentAuthor::Client => {
                "UPDATE photo_comments SET deleted_at = coalesce(deleted_at, now())
                 WHERE workspace_id = $1 AND gallery_id = $2 AND id = $3 AND author = $4
                   AND (
                       contact_origin = 'general' AND $5 IS NULL
                       OR contact_origin = 'contact' AND contact_id = $5
                   )"
            }
            CommentAuthor::Photographer => {
                "DELETE FROM photo_comments
                 WHERE workspace_id = $1 AND gallery_id = $2 AND id = $3 AND author = $4
                   AND (
                       $4 = 'photographer'
                       OR contact_origin = 'general' AND $5 IS NULL
                       OR contact_origin = 'contact' AND contact_id = $5
                   )"
            }
        };
        let changed = sqlx::query(query)
            .bind(location.workspace_id)
            .bind(location.gallery_id)
            .bind(id)
            .bind(author)
            .bind(client.and_then(|client| client.contact_id))
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(if changed.rows_affected() == 1 {
            CommentChange::Applied(())
        } else {
            CommentChange::NotFound
        })
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU16;

    use sqlx::{AssertSqlSafe, PgPool};
    use uuid::Uuid;

    use super::*;
    use crate::{
        Database,
        activity::Client,
        gallery::GalleryRepository,
        notifications::{NotificationKind, NotificationRepository},
        tests::TestRepository,
    };

    struct Fixture {
        database: Database,
        location: PhotoLocation,
        proof: PhotoId,
        final_photo: PhotoId,
    }

    const CLIENT: Option<Client> = Some(Client { contact_id: None });

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
            let proof = PhotoId::from(Uuid::new_v4());
            let final_photo = PhotoId::from(Uuid::new_v4());

            for (id, kind) in [(proof, "proof"), (final_photo, "final")] {
                sqlx::query(
                    "INSERT INTO photos
                         (id, workspace_id, gallery_id, original_name, size, width, height,
                          kind, status, clean_ready)
                     VALUES ($1, $2, $3, 'a.jpg', 1, 1, 1, $4::photo_kind, 'ready', true)",
                )
                .bind(id)
                .bind(workspace_id)
                .bind(gallery_id)
                .bind(kind)
                .execute(pool)
                .await
                .unwrap();
            }

            Self {
                database,
                location: PhotoLocation {
                    workspace_id,
                    gallery_id,
                },
                proof,
                final_photo,
            }
        }

        async fn add(&self, author: CommentAuthor) -> Comment {
            self.add_text(author, "Nice").await
        }

        async fn add_text(&self, author: CommentAuthor, text: &str) -> Comment {
            let options = ActivityOptions::default().client(Some(Client { contact_id: None }));
            let CommentChange::Applied(comment) = self
                .database
                .repository_with::<CommentRepository>(options)
                .add(
                    self.location,
                    self.proof,
                    author,
                    &text.to_owned().try_into().unwrap(),
                    NonZeroU16::new(100).unwrap(),
                )
                .await
                .unwrap()
            else {
                panic!("comment should apply")
            };

            comment
        }
    }

    #[test]
    fn bodies_are_trimmed_and_bounded() {
        for body in ["".to_owned(), " \n ".to_owned(), "x".repeat(501)] {
            assert!(CommentBody::try_from(body).is_err());
        }

        assert_eq!(
            CommentBody::try_from(" hi ".to_owned()).unwrap().as_str(),
            "hi"
        );
        assert_eq!(
            CommentBody::try_from("é".repeat(500))
                .unwrap()
                .as_str()
                .chars()
                .count(),
            500
        );
    }

    #[sqlx::test(migrations = false)]
    async fn comment_contact_migration_preserves_legacy_comments(pool: PgPool) {
        for migration in crate::MIGRATOR.iter().take_while(|m| m.version < 50) {
            sqlx::raw_sql(AssertSqlSafe(migration.sql.as_ref()))
                .execute(&pool)
                .await
                .unwrap();
        }

        let fixture = Fixture::new(&pool).await;
        let comment_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO photo_comments
                 (id, workspace_id, gallery_id, photo_id, author, body)
             VALUES ($1, $2, $3, $4, 'client', 'Legacy')",
        )
        .bind(comment_id)
        .bind(fixture.location.workspace_id)
        .bind(fixture.location.gallery_id)
        .bind(fixture.proof)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::raw_sql(include_str!("../migrations/0050_comment_contact.sql"))
            .execute(&pool)
            .await
            .unwrap();

        let legacy: (String, Option<Uuid>, String) = sqlx::query_as(
            "SELECT body, contact_id, contact_origin FROM photo_comments WHERE id = $1",
        )
        .bind(comment_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(legacy, ("Legacy".to_owned(), None, "general".to_owned()));

        let contact_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO contacts(id, workspace_id, name, email)
             VALUES ($1, $2, 'Claire', 'claire@example.test')",
        )
        .bind(contact_id)
        .bind(fixture.location.workspace_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE photo_comments SET contact_id = $1, contact_origin = 'contact' WHERE id = $2",
        )
        .bind(contact_id)
        .bind(comment_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("DELETE FROM contacts WHERE id = $1")
            .bind(contact_id)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(
            sqlx::query_as::<_, (Option<Uuid>, String)>(
                "SELECT contact_id, contact_origin FROM photo_comments WHERE id = $1",
            )
            .bind(comment_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            (None, "contact".to_owned())
        );

        let invalid_contact_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO contacts(id, workspace_id, name, email)
             VALUES ($1, $2, 'Alex', 'alex@example.test')",
        )
        .bind(invalid_contact_id)
        .bind(fixture.location.workspace_id)
        .execute(&pool)
        .await
        .unwrap();
        let invalid = sqlx::query(
            "INSERT INTO photo_comments
                 (workspace_id, gallery_id, photo_id, author, body, contact_id, contact_origin)
             VALUES ($1, $2, $3, 'photographer', 'Wrong', $4, 'contact')",
        )
        .bind(fixture.location.workspace_id)
        .bind(fixture.location.gallery_id)
        .bind(fixture.proof)
        .bind(invalid_contact_id)
        .execute(&pool)
        .await;
        assert!(invalid.is_err());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_client_comment_appears_in_the_thread_and_counts(pool: PgPool) {
        let f = Fixture::new(&pool).await;
        let comment = f.add(CommentAuthor::Client).await;
        let repo = f.database.repository::<CommentRepository>();

        assert_eq!(comment.body.as_deref(), Some("Nice"));
        assert_eq!(comment.author, CommentAuthor::Client);
        assert!(comment.edited_at.is_none());
        assert_eq!(
            repo.thread(f.location, f.proof, CommentAuthor::Client, CLIENT)
                .await
                .unwrap()
                .unwrap(),
            vec![comment]
        );
        assert_eq!(
            repo.counts(f.location, CommentAuthor::Client)
                .await
                .unwrap(),
            vec![CommentCount {
                photo_id: f.proof,
                count: 1,
                latest_body: None,
            }]
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn only_the_photographer_gets_the_latest_live_body_in_the_summary(pool: PgPool) {
        let f = Fixture::new(&pool).await;
        let first = f.add(CommentAuthor::Client).await;
        let second = f.add(CommentAuthor::Photographer).await;
        let repo = f.database.repository::<CommentRepository>();

        let edited = repo
            .edit(
                f.location,
                second.id,
                CommentAuthor::Photographer,
                None,
                &"Warmer, then?".to_owned().try_into().unwrap(),
            )
            .await
            .unwrap();
        assert!(matches!(edited, CommentChange::Applied(_)));
        sqlx::query(
            "UPDATE photo_comments SET created_at = now() - interval '1 minute' WHERE id = $1",
        )
        .bind(first.id)
        .execute(&pool)
        .await
        .unwrap();

        let owner = repo
            .counts(f.location, CommentAuthor::Photographer)
            .await
            .unwrap();
        let client = repo
            .counts(f.location, CommentAuthor::Client)
            .await
            .unwrap();

        assert_eq!(owner[0].count, 2);
        assert_eq!(owner[0].latest_body.as_deref(), Some("Warmer, then?"));
        assert_eq!(client[0].count, 2);
        assert_eq!(client[0].latest_body, None);

        assert_eq!(
            repo.delete(f.location, second.id, CommentAuthor::Photographer, None)
                .await
                .unwrap(),
            CommentChange::Applied(())
        );
        let owner = repo
            .counts(f.location, CommentAuthor::Photographer)
            .await
            .unwrap();

        assert_eq!(owner[0].count, 1);
        assert_eq!(owner[0].latest_body.as_deref(), Some("Nice"));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn only_proofs_take_comments(pool: PgPool) {
        let f = Fixture::new(&pool).await;
        let other = Fixture::new(&pool).await;
        let repo = f.database.repository::<CommentRepository>();

        for photo in [f.final_photo, other.proof, PhotoId::from(Uuid::new_v4())] {
            assert_eq!(
                repo.add(
                    f.location,
                    photo,
                    CommentAuthor::Client,
                    &"hi".to_owned().try_into().unwrap(),
                    NonZeroU16::new(100).unwrap()
                )
                .await
                .unwrap(),
                CommentChange::NotFound
            );
            assert!(
                repo.thread(f.location, photo, CommentAuthor::Client, CLIENT)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_delivered_gallery_refuses_clients_but_not_the_photographer(pool: PgPool) {
        let f = Fixture::new(&pool).await;
        let comment = f.add(CommentAuthor::Client).await;
        let repo = f.database.repository::<CommentRepository>();
        let body = "changed".to_owned().try_into().unwrap();

        sqlx::query("UPDATE galleries SET delivered_at=now() WHERE id=$1")
            .bind(f.location.gallery_id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            repo.add(
                f.location,
                f.proof,
                CommentAuthor::Client,
                &body,
                NonZeroU16::new(100).unwrap()
            )
            .await
            .unwrap(),
            CommentChange::Delivered
        );
        assert_eq!(
            repo.edit(f.location, comment.id, CommentAuthor::Client, CLIENT, &body)
                .await
                .unwrap(),
            CommentChange::Delivered
        );
        assert_eq!(
            repo.delete(f.location, comment.id, CommentAuthor::Client, CLIENT)
                .await
                .unwrap(),
            CommentChange::Delivered
        );
        f.add(CommentAuthor::Photographer).await;
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn authors_only_change_their_own_comments(pool: PgPool) {
        let f = Fixture::new(&pool).await;
        let client = f.add(CommentAuthor::Client).await;
        let photographer = f.add(CommentAuthor::Photographer).await;
        let repo = f.database.repository::<CommentRepository>();
        let body = "changed".to_owned().try_into().unwrap();

        for (comment, author) in [
            (client.id, CommentAuthor::Photographer),
            (photographer.id, CommentAuthor::Client),
        ] {
            assert_eq!(
                repo.edit(
                    f.location,
                    comment,
                    author,
                    if author == CommentAuthor::Client {
                        CLIENT
                    } else {
                        None
                    },
                    &body,
                )
                .await
                .unwrap(),
                CommentChange::NotFound
            );
            assert_eq!(
                repo.delete(
                    f.location,
                    comment,
                    author,
                    if author == CommentAuthor::Client {
                        CLIENT
                    } else {
                        None
                    },
                )
                .await
                .unwrap(),
                CommentChange::NotFound
            );
        }

        let other = Fixture::new(&pool).await;

        assert_eq!(
            repo.edit(
                other.location,
                client.id,
                CommentAuthor::Client,
                CLIENT,
                &body,
            )
            .await
            .unwrap(),
            CommentChange::NotFound
        );
        assert_eq!(
            repo.delete(other.location, client.id, CommentAuthor::Client, CLIENT)
                .await
                .unwrap(),
            CommentChange::NotFound
        );

        let CommentChange::Applied(edited) = repo
            .edit(f.location, client.id, CommentAuthor::Client, CLIENT, &body)
            .await
            .unwrap()
        else {
            panic!("edit applies")
        };
        assert_eq!(edited.body.as_deref(), Some("changed"));
        assert!(edited.edited_at.is_some());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn deleting_keeps_the_text_for_the_photographer_only(pool: PgPool) {
        let f = Fixture::new(&pool).await;
        let comment = f.add(CommentAuthor::Client).await;
        let repo = f.database.repository::<CommentRepository>();

        for _ in 0..2 {
            assert_eq!(
                repo.delete(f.location, comment.id, CommentAuthor::Client, CLIENT)
                    .await
                    .unwrap(),
                CommentChange::Applied(())
            );
        }

        let client = repo
            .thread(f.location, f.proof, CommentAuthor::Client, CLIENT)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(client[0].body, None);
        assert!(client[0].deleted_at.is_some());
        assert_eq!(
            repo.thread(f.location, f.proof, CommentAuthor::Photographer, None)
                .await
                .unwrap()
                .unwrap()[0]
                .body
                .as_deref(),
            Some("Nice")
        );
        assert!(
            repo.counts(f.location, CommentAuthor::Photographer)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            repo.edit(
                f.location,
                comment.id,
                CommentAuthor::Client,
                CLIENT,
                &"edited".to_owned().try_into().unwrap()
            )
            .await
            .unwrap(),
            CommentChange::NotFound
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn deleting_a_photographer_comment_removes_it_for_good(pool: PgPool) {
        let f = Fixture::new(&pool).await;
        let comment = f.add(CommentAuthor::Photographer).await;
        let repo = f.database.repository::<CommentRepository>();

        assert_eq!(
            repo.delete(f.location, comment.id, CommentAuthor::Photographer, None)
                .await
                .unwrap(),
            CommentChange::Applied(())
        );
        assert_eq!(
            repo.delete(f.location, comment.id, CommentAuthor::Photographer, None)
                .await
                .unwrap(),
            CommentChange::NotFound
        );

        for viewer in [CommentAuthor::Client, CommentAuthor::Photographer] {
            assert!(
                repo.thread(
                    f.location,
                    f.proof,
                    viewer,
                    if viewer == CommentAuthor::Client {
                        CLIENT
                    } else {
                        None
                    },
                )
                .await
                .unwrap()
                .unwrap()
                .is_empty()
            );
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn deleting_a_photo_removes_its_comments(pool: PgPool) {
        let f = Fixture::new(&pool).await;

        f.add(CommentAuthor::Client).await;
        sqlx::query("DELETE FROM photos WHERE id=$1")
            .bind(f.proof)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM photo_comments")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_full_thread_refuses_clients_only(pool: PgPool) {
        let f = Fixture::new(&pool).await;
        let repo = f.database.repository::<CommentRepository>();
        let limit = NonZeroU16::new(3).unwrap();
        let body = "hi".to_owned().try_into().unwrap();
        let comment = f.add(CommentAuthor::Client).await;

        assert_eq!(
            repo.delete(f.location, comment.id, CommentAuthor::Client, CLIENT)
                .await
                .unwrap(),
            CommentChange::Applied(())
        );

        for _ in 0..2 {
            assert!(matches!(
                repo.add(f.location, f.proof, CommentAuthor::Client, &body, limit)
                    .await
                    .unwrap(),
                CommentChange::Applied(_)
            ));
        }

        assert_eq!(
            repo.add(f.location, f.proof, CommentAuthor::Client, &body, limit)
                .await
                .unwrap(),
            CommentChange::Full
        );
        f.add(CommentAuthor::Photographer).await;
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn concurrent_posts_cannot_exceed_the_cap(pool: PgPool) {
        let f = Fixture::new(&pool).await;
        let repo = f.database.repository::<CommentRepository>();
        let body = "hi".to_owned().try_into().unwrap();
        let limit = NonZeroU16::new(1).unwrap();
        let (a, b) = tokio::join!(
            repo.add(f.location, f.proof, CommentAuthor::Client, &body, limit),
            repo.add(f.location, f.proof, CommentAuthor::Client, &body, limit)
        );
        let results = [a.unwrap(), b.unwrap()];

        assert_eq!(
            results
                .iter()
                .filter(|v| matches!(v, CommentChange::Applied(_)))
                .count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter(|v| matches!(v, CommentChange::Full))
                .count(),
            1
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn only_a_new_client_comment_notifies(pool: PgPool) {
        let f = Fixture::new(&pool).await;
        let comment = f.add(CommentAuthor::Client).await;

        f.add(CommentAuthor::Client).await;
        f.add(CommentAuthor::Photographer).await;
        let repo = f.database.repository::<CommentRepository>();

        assert!(matches!(
            repo.edit(
                f.location,
                comment.id,
                CommentAuthor::Client,
                CLIENT,
                &"edited".to_owned().try_into().unwrap(),
            )
            .await
            .unwrap(),
            CommentChange::Applied(_)
        ));
        assert_eq!(
            repo.delete(f.location, comment.id, CommentAuthor::Client, CLIENT)
                .await
                .unwrap(),
            CommentChange::Applied(())
        );
        let notifications = f
            .database
            .repository::<NotificationRepository>()
            .list(f.location.workspace_id)
            .await
            .unwrap();

        assert_eq!(notifications.unread, 1);
        assert_eq!(
            notifications.notifications[0].kind,
            NotificationKind::Comment
        );
        assert_eq!(notifications.notifications[0].count, 2);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn add_records_client_comments_only(pool: PgPool) {
        let f = Fixture::new(&pool).await;

        f.add(CommentAuthor::Photographer).await;
        f.add(CommentAuthor::Client).await;

        let lines: Vec<(ActivityKind, i32)> = sqlx::query_as(
            "SELECT kind, count FROM activity_events
             WHERE gallery_id = $1 ORDER BY first_at, id",
        )
        .bind(f.location.gallery_id)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(lines, [(ActivityKind::Comment, 1)]);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn the_activity_line_quotes_the_latest_client_comment(pool: PgPool) {
        let f = Fixture::new(&pool).await;
        let long = format!("  First\n{}  ", "é".repeat(300));
        let quote = || async {
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT detail FROM activity_events WHERE gallery_id = $1",
            )
            .bind(f.location.gallery_id)
            .fetch_one(&pool)
            .await
            .unwrap()
        };

        f.add_text(CommentAuthor::Client, &long).await;

        let first = quote().await.unwrap();

        assert!(first.starts_with("First é"));
        assert_eq!(first.chars().count(), ACTIVITY_PREVIEW_CHARS);

        f.add_text(CommentAuthor::Client, "Warmer, please").await;

        assert_eq!(quote().await.as_deref(), Some("Warmer, please"));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn the_activity_line_points_at_the_photo_of_the_latest_comment(pool: PgPool) {
        let f = Fixture::new(&pool).await;
        let photos = || async {
            sqlx::query_scalar::<_, Vec<PhotoId>>(
                "SELECT photo_ids FROM activity_events WHERE gallery_id = $1",
            )
            .bind(f.location.gallery_id)
            .fetch_one(&pool)
            .await
            .unwrap()
        };

        f.add_text(CommentAuthor::Client, "On the first photo")
            .await;

        assert_eq!(photos().await, [f.proof]);

        let options = ActivityOptions::default().client(Some(Client { contact_id: None }));
        let other = PhotoId::from(Uuid::new_v4());

        sqlx::query(
            "INSERT INTO photos
                 (id, workspace_id, gallery_id, original_name, size, width, height,
                  kind, status, clean_ready)
             VALUES ($1, $2, $3, 'b.jpg', 1, 1, 1, 'proof'::photo_kind, 'ready', true)",
        )
        .bind(other)
        .bind(f.location.workspace_id)
        .bind(f.location.gallery_id)
        .execute(&pool)
        .await
        .unwrap();
        let change = f
            .database
            .repository_with::<CommentRepository>(options)
            .add(
                f.location,
                other,
                CommentAuthor::Client,
                &"On the second".to_owned().try_into().unwrap(),
                NonZeroU16::new(100).unwrap(),
            )
            .await
            .unwrap();

        assert!(matches!(change, CommentChange::Applied(_)));
        assert_eq!(photos().await, [other]);
    }
}
