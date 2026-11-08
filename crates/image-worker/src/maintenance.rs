//! Periodic housekeeping that no job asks for: removing files nothing owns and expired exports.
//!
//! [`Maintenance::run_every`] runs one task beside the job loop, so runs never overlap, and a
//! run takes a concurrency slot like a job, so maintenance and jobs together stay within the
//! configured limit. A tick missed while a run was late is skipped, not replayed.
//!
//! Only one worker should run maintenance. Two would repeat the same sweeps, which is wasteful
//! but harmless, since every step is idempotent and spares files younger than the grace period.
//! If workers ever scale out, guard [`Maintenance::run`] with a PostgreSQL advisory lock.

use std::{sync::Arc, time::Duration};

use pp_database::{
    Database,
    export::ExportRepository,
    photo::{PhotoLocation, PhotoRepository},
};
use pp_storage::{ArchiveKey, PhotoStorage};
use tokio::{
    select,
    sync::Semaphore,
    time::{self, MissedTickBehavior},
};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

/// The maintenance steps and how often they run.
pub struct Maintenance {
    database: Database,
    storage: PhotoStorage,
    interval: Duration,
    /// A file younger than this is never swept, so an upload still being finished is safe.
    orphan_grace: Duration,
    /// How long an export is kept after it finished.
    retention: Duration,
}

impl Maintenance {
    pub const fn new(
        database: Database,
        storage: PhotoStorage,
        interval: Duration,
        orphan_grace: Duration,
        retention: Duration,
    ) -> Self {
        Self {
            database,
            storage,
            interval,
            orphan_grace,
            retention,
        }
    }

    /// Runs the steps every interval, starting at once, until `shutdown` is cancelled. A run in
    /// progress is not interrupted by it; the caller's grace period bounds it.
    pub async fn run_every(&self, slots: Arc<Semaphore>, shutdown: CancellationToken) {
        let mut ticks = time::interval(self.interval);

        ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            select! {
                biased;
                () = shutdown.cancelled() => return,
                _ = ticks.tick() => {}
            }

            let permit = select! {
                biased;
                () = shutdown.cancelled() => return,
                permit = Arc::clone(&slots).acquire_owned() => permit,
            };
            let Ok(permit) = permit else { return };

            self.run().await;
            drop(permit);
        }
    }

    /// Runs every step once. A step that fails is logged and the next one still runs; whatever
    /// it left is picked up by the next run.
    pub async fn run(&self) {
        self.expire_exports().await;
        self.sweep_photos().await;
        self.sweep_archives().await;
    }

    /// Removes exports past their retention together with their files.
    async fn expire_exports(&self) {
        let exports = self.database.repository::<ExportRepository>();
        let expired = match exports.expire(self.retention).await {
            Ok(expired) => expired,
            Err(error) => {
                warn!(%error, "expired exports could not be deleted");

                return;
            }
        };

        for export in &expired {
            let key = ArchiveKey {
                workspace: export.workspace_id,
                gallery: export.gallery_id,
                export: export.id,
            };

            if let Err(error) = self.storage.remove_archive(&key).await {
                warn!(%error, export = %export.id.0, "expired selection ZIP could not be removed");
            }
        }

        info!(exports = expired.len(), "expired exports removed");
    }

    /// Removes old photo files that no photo row owns, whatever left them behind.
    async fn sweep_photos(&self) {
        let photos = self.database.repository::<PhotoRepository>();
        // When the database cannot say, the file is kept.
        let is_stored = async |_: PhotoLocation, photo| {
            photos
                .locate(photo)
                .await
                .map_or(true, |location| location.is_some())
        };

        match self
            .storage
            .sweep_orphans(self.orphan_grace, is_stored)
            .await
        {
            Ok(removed) => info!(removed, "orphaned photo files swept"),
            Err(error) => warn!(%error, "orphaned photo files could not be swept"),
        }
    }

    /// Removes old selection ZIPs that no export row claims.
    async fn sweep_archives(&self) {
        let exports = self.database.repository::<ExportRepository>();
        let swept = self
            .storage
            .sweep_archives(self.orphan_grace, async |key: ArchiveKey| {
                // When the database cannot say, the file is kept.
                exports.exists(key.export).await.unwrap_or(true)
            })
            .await;

        match swept {
            Ok(removed) => info!(removed, "orphaned selection ZIPs swept"),
            Err(error) => warn!(%error, "orphaned selection ZIPs could not be swept"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use pp_core::{config::PhotoDir, notifications::PhotoId, types::generate::Generate};
    use pp_database::{
        PgPool,
        export::{ExportId, ExportRequest},
        gallery::GalleryRepository,
        tests::TestRepository,
    };
    use pp_storage::{PhotoKey, StorageError, Variant};
    use tempfile::TempDir;

    use super::*;

    const DAY: Duration = Duration::from_secs(86_400);
    const TICK: Duration = Duration::from_millis(30);

    struct Fixture {
        _dir: TempDir,
        database: Database,
        storage: PhotoStorage,
        location: PhotoLocation,
    }

    impl Fixture {
        async fn new(pool: PgPool) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let storage = PhotoStorage::open(&PhotoDir::try_from(dir.path().to_owned()).unwrap())
                .await
                .unwrap();
            let database = Database::new(pool);
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
                _dir: dir,
                database,
                storage,
                location: PhotoLocation {
                    workspace_id,
                    gallery_id,
                },
            }
        }

        fn maintenance(&self, orphan_grace: Duration, retention: Duration) -> Maintenance {
            Maintenance::new(
                self.database.clone(),
                self.storage.clone(),
                TICK,
                orphan_grace,
                retention,
            )
        }

        async fn store(&self, photo: PhotoId, variant: Variant) -> PhotoKey {
            let key = PhotoKey::with_location(self.location, photo, variant);
            let mut staged = self.storage.stage().await.unwrap();

            staged.write(b"x").await.unwrap();
            staged.commit(&key).await.unwrap();

            key
        }

        async fn is_stored(&self, key: &PhotoKey) -> bool {
            !matches!(self.storage.read(key).await, Err(StorageError::NotFound))
        }

        /// A built export whose ZIP is stored.
        async fn export(&self, pool: &PgPool) -> ArchiveKey {
            sqlx::query(
                "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width,
                                     height, status, favorite)
                 VALUES ($1, $2, $3, 'a.jpg', 5, 2, 2, 'ready', true)",
            )
            .bind(PhotoId::generate())
            .bind(self.location.workspace_id)
            .bind(self.location.gallery_id)
            .execute(pool)
            .await
            .unwrap();

            let ExportRequest::Created { export, .. } = self
                .database
                .repository::<ExportRepository>()
                .request(
                    self.location.workspace_id,
                    self.location.gallery_id,
                    NonZeroU64::new(1 << 30).unwrap(),
                )
                .await
                .unwrap()
            else {
                panic!("expected a new export");
            };
            let key = ArchiveKey {
                workspace: self.location.workspace_id,
                gallery: self.location.gallery_id,
                export: export.id,
            };

            self.storage.build_archive(&key, Vec::new()).await.unwrap();

            key
        }

        async fn has_zip(&self, key: &ArchiveKey) -> bool {
            self.storage.read_archive(key).await.is_ok()
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_run_keeps_recent_files(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let key = fixture.store(PhotoId::generate(), Variant::Original).await;

        fixture.maintenance(DAY, DAY).run().await;

        assert!(fixture.is_stored(&key).await);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_run_removes_old_orphans_and_keeps_owned_files(pool: PgPool) {
        let fixture = Fixture::new(pool.clone()).await;
        let orphan = fixture.store(PhotoId::generate(), Variant::Original).await;
        let owned = PhotoId::generate();

        sqlx::query(
            "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width,
                                 height, status)
             VALUES ($1, $2, $3, 'a.jpg', 5, 2, 2, 'ready')",
        )
        .bind(owned)
        .bind(fixture.location.workspace_id)
        .bind(fixture.location.gallery_id)
        .execute(&pool)
        .await
        .unwrap();

        let kept = fixture.store(owned, Variant::Original).await;

        fixture.maintenance(Duration::ZERO, DAY).run().await;

        assert!(!fixture.is_stored(&orphan).await);
        assert!(fixture.is_stored(&kept).await);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_run_removes_expired_exports_and_unclaimed_archives(pool: PgPool) {
        let fixture = Fixture::new(pool.clone()).await;
        let expired = fixture.export(&pool).await;
        let stray = ArchiveKey {
            export: ExportId::generate(),
            ..expired
        };

        fixture
            .storage
            .build_archive(&stray, Vec::new())
            .await
            .unwrap();
        fixture.maintenance(Duration::ZERO, DAY).run().await;

        assert!(fixture.has_zip(&expired).await);
        assert!(!fixture.has_zip(&stray).await);

        fixture.maintenance(DAY, Duration::ZERO).run().await;

        assert!(!fixture.has_zip(&expired).await);
        assert!(
            !fixture
                .database
                .repository::<ExportRepository>()
                .exists(expired.export)
                .await
                .unwrap()
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn the_timer_runs_maintenance_repeatedly_until_shutdown(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let maintenance = Arc::new(fixture.maintenance(Duration::ZERO, DAY));
        let shutdown = CancellationToken::new();
        let timer = tokio::spawn({
            let maintenance = Arc::clone(&maintenance);
            let shutdown = shutdown.clone();

            async move {
                maintenance
                    .run_every(Arc::new(Semaphore::new(1)), shutdown)
                    .await;
            }
        });

        // A file that appears after the first run is swept by a later one.
        time::sleep(TICK * 3).await;
        let key = fixture.store(PhotoId::generate(), Variant::Original).await;
        let deadline = time::Instant::now() + Duration::from_secs(10);

        while fixture.is_stored(&key).await {
            assert!(time::Instant::now() < deadline, "no later run swept it");
            time::sleep(TICK).await;
        }

        shutdown.cancel();
        timer.await.unwrap();
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn maintenance_waits_for_a_free_slot(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let key = fixture.store(PhotoId::generate(), Variant::Original).await;
        let maintenance = Arc::new(fixture.maintenance(Duration::ZERO, DAY));
        let slots = Arc::new(Semaphore::new(1));
        let busy = Arc::clone(&slots).acquire_owned().await.unwrap();
        let shutdown = CancellationToken::new();
        let timer = tokio::spawn({
            let slots = Arc::clone(&slots);
            let shutdown = shutdown.clone();

            async move { maintenance.run_every(slots, shutdown).await }
        });

        time::sleep(TICK * 5).await;
        assert!(fixture.is_stored(&key).await);

        drop(busy);

        let deadline = time::Instant::now() + Duration::from_secs(10);

        while fixture.is_stored(&key).await {
            assert!(
                time::Instant::now() < deadline,
                "never ran after the slot freed"
            );
            time::sleep(TICK).await;
        }

        shutdown.cancel();
        timer.await.unwrap();
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn the_timer_stops_while_waiting_for_a_slot(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let maintenance = Arc::new(fixture.maintenance(DAY, DAY));
        let slots = Arc::new(Semaphore::new(0));
        let shutdown = CancellationToken::new();
        let timer = tokio::spawn({
            let shutdown = shutdown.clone();

            async move { maintenance.run_every(slots, shutdown).await }
        });

        time::sleep(TICK * 2).await;
        shutdown.cancel();

        time::timeout(Duration::from_secs(5), timer)
            .await
            .unwrap()
            .unwrap();
    }
}
