//! Live changes of a gallery's favorites, announced by PostgreSQL itself whenever a photo's
//! favorite flag changes or a favorite photo is deleted. Read them with
//! [`Database::listen`](crate::Database::listen)`::<FavoriteEvent>()`.

use serde::{Deserialize, Serialize};

use crate::{auth::WorkspaceId, gallery::GalleryId, listener::Channel};

/// A gallery's selection changed. It carries no photo: whoever shows the selection refetches it,
/// so the server's list stays authoritative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FavoriteEvent {
    pub workspace_id: WorkspaceId,
    pub gallery_id: GalleryId,
}

impl Channel for FavoriteEvent {
    const NAME: &'static str = "favorite_events";
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use pp_core::notifications::PhotoId;
    use sqlx::PgPool;
    use tokio::time;
    use uuid::Uuid;

    use super::*;
    use crate::{
        Database,
        gallery::GalleryRepository,
        listener::{Listener, Notification},
        tests::TestRepository,
    };

    struct Fixture {
        database: Database,
        event: Notification<FavoriteEvent>,
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
                event: Notification::Event(FavoriteEvent {
                    workspace_id,
                    gallery_id,
                }),
                photo_id,
            }
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

    async fn next(listener: &mut Listener<FavoriteEvent>) -> Notification<FavoriteEvent> {
        time::timeout(Duration::from_secs(5), listener.next())
            .await
            .expect("a notification arrives")
            .unwrap()
    }

    async fn assert_silent(listener: &mut Listener<FavoriteEvent>) {
        let received = time::timeout(Duration::from_millis(200), listener.next()).await;

        assert!(received.is_err(), "unexpected {received:?}");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn each_change_of_the_flag_notifies_once(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let mut listener = fixture.database.listen::<FavoriteEvent>().await.unwrap();

        fixture.set_favorite(&pool, true).await;

        assert_eq!(next(&mut listener).await, fixture.event);

        fixture.set_favorite(&pool, true).await;

        assert_silent(&mut listener).await;

        fixture.set_favorite(&pool, false).await;

        assert_eq!(next(&mut listener).await, fixture.event);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn deleting_a_favorite_notifies_but_deleting_another_photo_does_not(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let mut listener = fixture.database.listen::<FavoriteEvent>().await.unwrap();

        sqlx::query("DELETE FROM photos WHERE id = $1")
            .bind(fixture.photo_id)
            .execute(&pool)
            .await
            .unwrap();

        assert_silent(&mut listener).await;

        let fixture = Fixture::new(&pool).await;
        let mut listener = fixture.database.listen::<FavoriteEvent>().await.unwrap();

        fixture.set_favorite(&pool, true).await;
        next(&mut listener).await;

        sqlx::query("DELETE FROM photos WHERE id = $1")
            .bind(fixture.photo_id)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(next(&mut listener).await, fixture.event);
    }
}
