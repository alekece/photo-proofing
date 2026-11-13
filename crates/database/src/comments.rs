//! Photo threads scoped to a workspace and gallery. Writes lock delivery and the photo so a
//! concurrent delivery cannot accept a late client write, and concurrent posts cannot exceed
//! the thread cap. Deleted bodies are redacted in SQL for client reads.

use std::num::NonZeroU16;

use pp_core::types::timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, Transaction, Type};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    Repository,
    error::DatabaseError,
    photo::{PhotoId, PhotoLocation},
};

pub const MAX_BODY_CHARS: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("comment must contain between 1 and 500 characters")]
pub struct CommentBodyError;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(try_from = "String")]
#[sqlx(transparent)]
pub struct CommentBody(String);

impl CommentBody {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommentBody {
    type Error = CommentBodyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let value = value.trim();

        if !(1..=MAX_BODY_CHARS).contains(&value.chars().count()) || value.contains('\0') {
            return Err(CommentBodyError);
        }

        Ok(Self(value.to_owned()))
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromRow)]
pub struct Comment {
    pub id: CommentId,
    pub photo_id: PhotoId,
    pub author: CommentAuthor,
    pub body: Option<String>,
    pub created_at: Timestamp,
    pub edited_at: Option<Timestamp>,
    pub deleted_at: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentCount {
    pub photo_id: PhotoId,
    pub count: usize,
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
}

impl<'db> Repository<'db> for CommentRepository<'db> {
    type Options = ();

    fn open(pool: &'db PgPool, (): ()) -> Self {
        Self { pool }
    }
}

impl CommentRepository<'_> {
    pub async fn thread(
        &self,
        location: PhotoLocation,
        photo: PhotoId,
        viewer: CommentAuthor,
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

        let comments = sqlx::query_as(
            "SELECT id, photo_id, author,
                    CASE WHEN deleted_at IS NOT NULL AND $4 = 'client' THEN NULL ELSE body END AS body,
                    created_at, edited_at, deleted_at
             FROM photo_comments
             WHERE workspace_id = $1 AND gallery_id = $2 AND photo_id = $3
             ORDER BY created_at, id",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(photo)
        .bind(viewer)
        .fetch_all(self.pool)
        .await?;

        Ok(Some(comments))
    }

    pub async fn counts(
        &self,
        location: PhotoLocation,
    ) -> Result<Vec<CommentCount>, DatabaseError> {
        let rows: Vec<(PhotoId, i64)> = sqlx::query_as(
            "SELECT c.photo_id, count(*)
             FROM photo_comments AS c JOIN photos AS p ON p.id = c.photo_id
             WHERE c.workspace_id = $1 AND c.gallery_id = $2 AND c.deleted_at IS NULL
               AND p.kind = 'proof' AND p.clean_ready
             GROUP BY c.photo_id
             ORDER BY c.photo_id",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .fetch_all(self.pool)
        .await?;

        rows.into_iter()
            .map(|(photo_id, count)| {
                let count =
                    usize::try_from(count).map_err(|error| sqlx::Error::Decode(error.into()))?;

                Ok(CommentCount { photo_id, count })
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

        let comment = sqlx::query_as(
            "INSERT INTO photo_comments (workspace_id, gallery_id, photo_id, author, body)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING id, photo_id, author, body, created_at, edited_at, deleted_at",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(photo)
        .bind(author)
        .bind(body)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(CommentChange::Applied(comment))
    }

    /// Finds the photo of comment `id` in this gallery, if `author` wrote it.
    async fn photo_of(
        &self,
        location: PhotoLocation,
        id: CommentId,
        author: CommentAuthor,
    ) -> Result<Option<PhotoId>, DatabaseError> {
        let photo = sqlx::query_scalar(
            "SELECT photo_id FROM photo_comments
             WHERE workspace_id = $1 AND gallery_id = $2 AND id = $3 AND author = $4",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(id)
        .bind(author)
        .fetch_optional(self.pool)
        .await?;

        Ok(photo)
    }

    pub async fn edit(
        &self,
        location: PhotoLocation,
        id: CommentId,
        author: CommentAuthor,
        body: &CommentBody,
    ) -> Result<CommentChange<Comment>, DatabaseError> {
        let Some(photo) = self.photo_of(location, id, author).await? else {
            return Ok(CommentChange::NotFound);
        };

        let mut tx = self.pool.begin().await?;

        match self.writable(&mut tx, location, photo, author).await? {
            CommentChange::Applied(()) => {}
            CommentChange::Delivered => return Ok(CommentChange::Delivered),
            _ => return Ok(CommentChange::NotFound),
        }

        let comment = sqlx::query_as(
            "UPDATE photo_comments SET body = $5, edited_at = now()
             WHERE workspace_id = $1 AND gallery_id = $2 AND id = $3 AND author = $4
               AND deleted_at IS NULL
             RETURNING id, photo_id, author, body, created_at, edited_at, deleted_at",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(id)
        .bind(author)
        .bind(body)
        .fetch_optional(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(comment.map_or(CommentChange::NotFound, CommentChange::Applied))
    }

    pub async fn delete(
        &self,
        location: PhotoLocation,
        id: CommentId,
        author: CommentAuthor,
    ) -> Result<CommentChange<()>, DatabaseError> {
        let Some(photo) = self.photo_of(location, id, author).await? else {
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
                 WHERE workspace_id = $1 AND gallery_id = $2 AND id = $3 AND author = $4"
            }
            CommentAuthor::Photographer => {
                "DELETE FROM photo_comments
                 WHERE workspace_id = $1 AND gallery_id = $2 AND id = $3 AND author = $4"
            }
        };
        let changed = sqlx::query(query)
            .bind(location.workspace_id)
            .bind(location.gallery_id)
            .bind(id)
            .bind(author)
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

    use sqlx::PgPool;
    use uuid::Uuid;

    use super::*;
    use crate::{
        Database,
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
            let CommentChange::Applied(comment) = self
                .database
                .repository::<CommentRepository>()
                .add(
                    self.location,
                    self.proof,
                    author,
                    &"Nice".to_owned().try_into().unwrap(),
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

    #[sqlx::test(migrations = "./migrations")]
    async fn a_client_comment_appears_in_the_thread_and_counts(pool: PgPool) {
        let f = Fixture::new(&pool).await;
        let comment = f.add(CommentAuthor::Client).await;
        let repo = f.database.repository::<CommentRepository>();

        assert_eq!(comment.body.as_deref(), Some("Nice"));
        assert_eq!(comment.author, CommentAuthor::Client);
        assert!(comment.edited_at.is_none());
        assert_eq!(
            repo.thread(f.location, f.proof, CommentAuthor::Client)
                .await
                .unwrap()
                .unwrap(),
            vec![comment]
        );
        assert_eq!(
            repo.counts(f.location).await.unwrap(),
            vec![CommentCount {
                photo_id: f.proof,
                count: 1
            }]
        );
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
                repo.thread(f.location, photo, CommentAuthor::Client)
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
            repo.edit(f.location, comment.id, CommentAuthor::Client, &body)
                .await
                .unwrap(),
            CommentChange::Delivered
        );
        assert_eq!(
            repo.delete(f.location, comment.id, CommentAuthor::Client)
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
                repo.edit(f.location, comment, author, &body).await.unwrap(),
                CommentChange::NotFound
            );
            assert_eq!(
                repo.delete(f.location, comment, author).await.unwrap(),
                CommentChange::NotFound
            );
        }

        let other = Fixture::new(&pool).await;

        assert_eq!(
            repo.edit(other.location, client.id, CommentAuthor::Client, &body)
                .await
                .unwrap(),
            CommentChange::NotFound
        );
        assert_eq!(
            repo.delete(other.location, client.id, CommentAuthor::Client)
                .await
                .unwrap(),
            CommentChange::NotFound
        );

        let CommentChange::Applied(edited) = repo
            .edit(f.location, client.id, CommentAuthor::Client, &body)
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
                repo.delete(f.location, comment.id, CommentAuthor::Client)
                    .await
                    .unwrap(),
                CommentChange::Applied(())
            );
        }

        let client = repo
            .thread(f.location, f.proof, CommentAuthor::Client)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(client[0].body, None);
        assert!(client[0].deleted_at.is_some());
        assert_eq!(
            repo.thread(f.location, f.proof, CommentAuthor::Photographer)
                .await
                .unwrap()
                .unwrap()[0]
                .body
                .as_deref(),
            Some("Nice")
        );
        assert!(repo.counts(f.location).await.unwrap().is_empty());
        assert_eq!(
            repo.edit(
                f.location,
                comment.id,
                CommentAuthor::Client,
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
            repo.delete(f.location, comment.id, CommentAuthor::Photographer)
                .await
                .unwrap(),
            CommentChange::Applied(())
        );
        assert_eq!(
            repo.delete(f.location, comment.id, CommentAuthor::Photographer)
                .await
                .unwrap(),
            CommentChange::NotFound
        );

        for viewer in [CommentAuthor::Client, CommentAuthor::Photographer] {
            assert!(
                repo.thread(f.location, f.proof, viewer)
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
            repo.delete(f.location, comment.id, CommentAuthor::Client)
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
                &"edited".to_owned().try_into().unwrap(),
            )
            .await
            .unwrap(),
            CommentChange::Applied(_)
        ));
        assert_eq!(
            repo.delete(f.location, comment.id, CommentAuthor::Client)
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
}
