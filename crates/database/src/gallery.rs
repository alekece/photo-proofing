use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::{Error, Result, state::GalleryState};

#[derive(Debug, Clone, FromRow)]
pub struct Gallery {
    pub id: Uuid,
    pub title: String,
    pub state: GalleryState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct GalleryStateTransition {
    pub gallery_id: Uuid,
    pub from_state: Option<GalleryState>,
    pub to_state: GalleryState,
    pub occurred_at: DateTime<Utc>,
    pub actor: Option<String>,
}

/// Creates a gallery in `Draft`; the database records the initial audit row.
pub async fn create_gallery(pool: &PgPool, title: &str, actor: Option<&str>) -> Result<Gallery> {
    let mut tx = pool.begin().await?;

    set_actor(&mut tx, actor).await?;

    let gallery = sqlx::query_as(
        "INSERT INTO galleries (id, title) VALUES ($1, $2)
         RETURNING id, title, state, created_at, updated_at",
    )
    .bind(Uuid::new_v4())
    .bind(title)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(gallery)
}

/// Moves a gallery to `to`, rejecting forbidden transitions before and (as a backstop)
/// inside the database. The audit row is written by a database trigger.
pub async fn transition_gallery(
    pool: &PgPool,
    id: Uuid,
    to: GalleryState,
    actor: Option<&str>,
) -> Result<Gallery> {
    let mut tx = pool.begin().await?;

    let from: GalleryState =
        sqlx::query_scalar("SELECT state FROM galleries WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error::GalleryNotFound(id))?;

    if !from.can_transition_to(to) {
        return Err(Error::InvalidTransition { from, to });
    }

    set_actor(&mut tx, actor).await?;

    let gallery = sqlx::query_as(
        "UPDATE galleries SET state = $2, updated_at = now() WHERE id = $1
         RETURNING id, title, state, created_at, updated_at",
    )
    .bind(id)
    .bind(to)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(gallery)
}

pub async fn list_gallery_transitions(
    pool: &PgPool,
    id: Uuid,
) -> Result<Vec<GalleryStateTransition>> {
    Ok(sqlx::query_as(
        "SELECT gallery_id, from_state, to_state, occurred_at, actor
         FROM gallery_state_transitions WHERE gallery_id = $1 ORDER BY id",
    )
    .bind(id)
    .fetch_all(pool)
    .await?)
}

async fn set_actor(tx: &mut sqlx::PgConnection, actor: Option<&str>) -> Result<()> {
    sqlx::query("SELECT set_config('eg.actor', $1, true)")
        .bind(actor.unwrap_or_default())
        .execute(tx)
        .await?;

    Ok(())
}
