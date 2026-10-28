//! Requires `DATABASE_URL`; the tests fail (never skip) without it. See the README.

use eg_database::{
    Error, MIGRATOR,
    gallery::{create_gallery, list_gallery_transitions, transition_gallery},
    state::GalleryState,
};
use sqlx::PgPool;
use strum::IntoEnumIterator;

/// Drives a fresh gallery into `state` through permitted transitions only.
async fn create_gallery_in(pool: &PgPool, state: GalleryState) -> uuid::Uuid {
    let gallery = create_gallery(pool, "Sample", None)
        .await
        .expect("create gallery");
    let path: &[GalleryState] = match state {
        GalleryState::Draft => &[],
        GalleryState::Proofing => &[GalleryState::Proofing],
        GalleryState::SelectionReceived => {
            &[GalleryState::Proofing, GalleryState::SelectionReceived]
        }
        GalleryState::Retouching => &[
            GalleryState::Proofing,
            GalleryState::SelectionReceived,
            GalleryState::Retouching,
        ],
        GalleryState::Delivery => &[
            GalleryState::Proofing,
            GalleryState::SelectionReceived,
            GalleryState::Retouching,
            GalleryState::Delivery,
        ],
        GalleryState::Expired => &[GalleryState::Expired],
        GalleryState::Archived => &[GalleryState::Archived],
    };

    for step in path {
        transition_gallery(pool, gallery.id, *step, None)
            .await
            .expect("reach state");
    }

    gallery.id
}

#[sqlx::test(migrations = false)]
async fn migrates_a_blank_database(pool: PgPool) {
    MIGRATOR.run(&pool).await.expect("first run");
    MIGRATOR.run(&pool).await.expect("second run is a no-op");

    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables
         WHERE table_name IN ('galleries', 'gallery_state_transitions')",
    )
    .fetch_one(&pool)
    .await
    .expect("count tables");

    assert_eq!(tables, 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn every_state_pair_is_enforced_by_rust_and_database(pool: PgPool) {
    for from in GalleryState::iter() {
        for to in GalleryState::iter() {
            let id = create_gallery_in(&pool, from).await;
            let permitted = from.can_transition_to(to);

            let via_rust = transition_gallery(&pool, id, to, None).await;
            assert_eq!(via_rust.is_ok(), permitted, "rust {from} -> {to}");
            if !permitted {
                assert!(
                    matches!(via_rust, Err(Error::InvalidTransition { .. })),
                    "{from} -> {to}"
                );
            }

            // Bypass the Rust guard: raw SQL must reach the same verdict.
            let id = create_gallery_in(&pool, from).await;
            let via_sql = sqlx::query("UPDATE galleries SET state = $2 WHERE id = $1")
                .bind(id)
                .bind(to)
                .execute(&pool)
                .await;
            assert_eq!(via_sql.is_ok(), permitted, "sql {from} -> {to}");

            let function_verdict: bool =
                sqlx::query_scalar("SELECT is_gallery_transition_allowed($1, $2)")
                    .bind(from)
                    .bind(to)
                    .fetch_one(&pool)
                    .await
                    .expect("call function");
            assert_eq!(function_verdict, permitted, "function {from} -> {to}");
        }
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn rejects_galleries_created_outside_draft(pool: PgPool) {
    let result =
        sqlx::query("INSERT INTO galleries (id, title, state) VALUES ($1, 'x', 'delivery')")
            .bind(uuid::Uuid::new_v4())
            .execute(&pool)
            .await;

    assert!(result.is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn records_every_change_with_actor(pool: PgPool) {
    let gallery = create_gallery(&pool, "Sample", Some("photographer"))
        .await
        .expect("create gallery");
    transition_gallery(
        &pool,
        gallery.id,
        GalleryState::Proofing,
        Some("photographer"),
    )
    .await
    .expect("to proofing");
    transition_gallery(&pool, gallery.id, GalleryState::Draft, None)
        .await
        .expect("back to draft");

    let history = list_gallery_transitions(&pool, gallery.id)
        .await
        .expect("list");
    let steps: Vec<_> = history
        .iter()
        .map(|row| (row.from_state, row.to_state, row.actor.as_deref()))
        .collect();

    assert_eq!(
        steps,
        [
            (None, GalleryState::Draft, Some("photographer")),
            (
                Some(GalleryState::Draft),
                GalleryState::Proofing,
                Some("photographer")
            ),
            (Some(GalleryState::Proofing), GalleryState::Draft, None),
        ]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn rejected_transitions_leave_no_audit_row(pool: PgPool) {
    let gallery = create_gallery(&pool, "Sample", None)
        .await
        .expect("create gallery");

    transition_gallery(&pool, gallery.id, GalleryState::Delivery, None)
        .await
        .expect_err("draft -> delivery is forbidden");

    let history = list_gallery_transitions(&pool, gallery.id)
        .await
        .expect("list");
    assert_eq!(history.len(), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn audit_table_is_append_only(pool: PgPool) {
    let gallery = create_gallery(&pool, "Sample", None)
        .await
        .expect("create gallery");

    for statement in [
        "UPDATE gallery_state_transitions SET actor = 'x'",
        "DELETE FROM gallery_state_transitions",
        "TRUNCATE gallery_state_transitions",
    ] {
        sqlx::query(statement)
            .execute(&pool)
            .await
            .expect_err(statement);
    }

    let history = list_gallery_transitions(&pool, gallery.id)
        .await
        .expect("list");
    assert_eq!(history.len(), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn unknown_gallery_is_reported(pool: PgPool) {
    let result =
        transition_gallery(&pool, uuid::Uuid::new_v4(), GalleryState::Proofing, None).await;

    assert!(matches!(result, Err(Error::GalleryNotFound(_))));
}
