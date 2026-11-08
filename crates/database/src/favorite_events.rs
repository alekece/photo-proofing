//! Live changes of a gallery's favorites, announced by PostgreSQL itself whenever a photo's
//! favorite flag changes or a favorite photo is deleted. Read them with
//! [`Database::listen`](crate::Database::listen)`()`.

use serde::{Deserialize, Serialize};

use crate::{auth::WorkspaceId, gallery::GalleryId};

/// A gallery's selection changed. It carries no photo: whoever shows the selection refetches it,
/// so the server's list stays authoritative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FavoriteEvent {
    pub workspace_id: WorkspaceId,
    pub gallery_id: GalleryId,
}

impl FavoriteEvent {
    pub(crate) const CHANNEL: &'static str = "favorite_events";
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sqlx::PgPool;
    use tokio::time;
    use uuid::Uuid;

    use super::*;
    use crate::photo::PhotoId;
    use crate::{
        Database,
        gallery::GalleryRepository,
        listener::{Listener, Notification},
        tests::TestRepository,
    };

    struct Fixture {
        database: Database,
        event: Notification,
        photo_id: PhotoId,
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
            let photo_id = PhotoId::from(Uuid::new_v4());

            sqlx::query(
                "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height)
                 VALUES ($1, $2, $3, 'a.jpg', 1, 1, 1)",
            )
            .bind(photo_id)
            .bind(workspace_id)
            .bind(gallery_id)
            .execute(pool)
            .await
            .unwrap();

            Self {
                database,
                event: Notification::Favorites(FavoriteEvent {
                    workspace_id,
                    gallery_id,
                }),
                photo_id,
            }
        }

        /// The next favorite notification; the listener also reports other channels.
        async fn next(listener: &mut Listener) -> Notification {
            time::timeout(Duration::from_secs(5), async {
                loop {
                    let notification = listener.next().await.unwrap();

                    if matches!(notification, Notification::Favorites(_)) {
                        return notification;
                    }
                }
            })
            .await
            .expect("a notification arrives")
        }

        async fn assert_silent(listener: &mut Listener) {
            let received = time::timeout(Duration::from_millis(200), Self::next(listener)).await;

            assert!(received.is_err(), "unexpected {received:?}");
        }

        async fn set_favorite(&self, pool: &PgPool, favorite: bool) {
            sqlx::query("UPDATE photos SET favorite = $2 WHERE id = $1")
                .bind(self.photo_id)
                .bind(favorite)
                .execute(pool)
                .await
                .unwrap();
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn each_change_of_the_flag_notifies_once(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let mut listener = fixture.database.listen().await.unwrap();

        fixture.set_favorite(&pool, true).await;

        assert_eq!(Fixture::next(&mut listener).await, fixture.event);

        fixture.set_favorite(&pool, true).await;

        Fixture::assert_silent(&mut listener).await;

        fixture.set_favorite(&pool, false).await;

        assert_eq!(Fixture::next(&mut listener).await, fixture.event);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn deleting_a_favorite_notifies_but_deleting_another_photo_does_not(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let mut listener = fixture.database.listen().await.unwrap();

        sqlx::query("DELETE FROM photos WHERE id = $1")
            .bind(fixture.photo_id)
            .execute(&pool)
            .await
            .unwrap();

        Fixture::assert_silent(&mut listener).await;

        let fixture = Fixture::new(&pool).await;
        let mut listener = fixture.database.listen().await.unwrap();

        fixture.set_favorite(&pool, true).await;
        Fixture::next(&mut listener).await;

        sqlx::query("DELETE FROM photos WHERE id = $1")
            .bind(fixture.photo_id)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(Fixture::next(&mut listener).await, fixture.event);
    }
}
