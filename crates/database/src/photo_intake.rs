//! Coordinates photo metadata insertion and job enqueueing as one transaction.

use sqlx::{
    PgPool,
    postgres::{PgDatabaseError, PgSeverity},
};
use thiserror::Error;

use crate::{
    Repository,
    error::DatabaseError,
    jobs::{DEFAULT_MAX_ATTEMPTS, JobPayload, JobRepository},
    photo::{NewPhoto, Photo, PhotoInsertOutcome, PhotoRepository},
};

/// Failure while accepting photo metadata and its processing job.
///
/// A [`PhotoIntakeError::CommitOutcomeUnknown`] means an idempotent retry could not determine
/// whether the transaction committed. Callers must preserve the published original.
///
/// # Examples
///
/// ```
/// use pp_database::photo_intake::PhotoIntakeError;
///
/// fn may_remove_published_original(error: &PhotoIntakeError) -> bool {
///     matches!(error, PhotoIntakeError::Database(_))
/// }
/// ```
#[derive(Debug, Error)]
pub enum PhotoIntakeError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    /// The commit acknowledgement was lost and a retry could not settle the outcome.
    #[error("photo transaction commit outcome could not be confirmed")]
    CommitOutcomeUnknown {
        #[source]
        commit_error: DatabaseError,
        reconciliation_error: Option<Box<PhotoIntakeError>>,
    },
}

impl PhotoIntakeError {
    fn from_commit_error(error: sqlx::Error) -> Self {
        let severity = match &error {
            sqlx::Error::Database(database_error) => database_error
                .try_downcast_ref::<PgDatabaseError>()
                .map(PgDatabaseError::severity),
            _ => None,
        };

        if commit_error_outcome_is_unknown(severity) {
            Self::CommitOutcomeUnknown {
                commit_error: error.into(),
                reconciliation_error: None,
            }
        } else {
            Self::Database(error.into())
        }
    }

    fn resolve_reconciliation(
        commit_error: DatabaseError,
        reconciliation: Result<Option<Photo>, Self>,
    ) -> Result<Option<Photo>, Self> {
        match reconciliation {
            Ok(Some(photo)) => Ok(Some(photo)),
            Ok(None) => Err(Self::CommitOutcomeUnknown {
                commit_error,
                reconciliation_error: None,
            }),
            Err(reconciliation_error) => Err(Self::CommitOutcomeUnknown {
                commit_error,
                reconciliation_error: Some(Box::new(reconciliation_error)),
            }),
        }
    }
}

fn commit_error_outcome_is_unknown(severity: Option<PgSeverity>) -> bool {
    // PostgreSQL may panic after writing the commit record, and an unavailable severity is ambiguous.
    matches!(severity, None | Some(PgSeverity::Panic | PgSeverity::Fatal))
}

impl From<sqlx::Error> for PhotoIntakeError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error.into())
    }
}

/// Composes photo and job persistence for one accepted original.
pub struct PhotoIntake<'db> {
    pool: &'db PgPool,
}

impl<'db> Repository<'db> for PhotoIntake<'db> {
    type Options = ();

    fn open(pool: &'db PgPool, (): Self::Options) -> Self {
        Self { pool }
    }
}

impl PhotoIntake<'_> {
    /// Stores photo metadata and its processing job atomically.
    ///
    /// Returns `None` when the gallery is missing or not owned by the photo's workspace.
    pub async fn accept(&self, photo: NewPhoto) -> Result<Option<Photo>, PhotoIntakeError> {
        match self.accept_in_transaction(&photo).await {
            Err(PhotoIntakeError::CommitOutcomeUnknown { commit_error, .. }) => {
                PhotoIntakeError::resolve_reconciliation(
                    commit_error,
                    self.reconcile_commit_outcome(&photo).await,
                )
            }
            result => result,
        }
    }

    async fn reconcile_commit_outcome(
        &self,
        photo: &NewPhoto,
    ) -> Result<Option<Photo>, PhotoIntakeError> {
        self.accept_in_transaction(photo).await
    }

    async fn accept_in_transaction(
        &self,
        photo: &NewPhoto,
    ) -> Result<Option<Photo>, PhotoIntakeError> {
        let mut transaction = self.pool.begin().await?;
        let outcome = PhotoRepository::open(self.pool, ())
            .insert_in(&mut transaction, photo)
            .await?;

        match outcome {
            PhotoInsertOutcome::MissingGallery => {
                transaction.rollback().await?;

                Ok(None)
            }
            PhotoInsertOutcome::Identical(existing) => {
                transaction.rollback().await?;

                Ok(Some(existing))
            }
            PhotoInsertOutcome::Inserted(inserted) => {
                let payload = JobPayload::ProcessPhoto(serde_json::json!({"photo_id": photo.id}));

                JobRepository::enqueue_in(&mut transaction, payload, DEFAULT_MAX_ATTEMPTS).await?;
                if let Err(error) = transaction.commit().await {
                    return Err(PhotoIntakeError::from_commit_error(error));
                }

                Ok(Some(inserted))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU32, NonZeroU64};

    use pp_core::notifications::PhotoId;
    use sqlx::PgPool;
    use uuid::Uuid;

    use super::*;
    use crate::{
        Database,
        auth::WorkspaceId,
        error::DatabaseError,
        gallery::{GalleryId, GalleryRepository},
        photo::{NewPhoto, PhotoDimensions, PhotoInsertOutcome, PhotoRepository},
    };

    async fn workspace(pool: &PgPool) -> WorkspaceId {
        let id = WorkspaceId(Uuid::new_v4());

        sqlx::query("INSERT INTO workspaces(id) VALUES ($1)")
            .bind(id)
            .execute(pool)
            .await
            .unwrap();

        id
    }

    fn new_photo(
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        id: PhotoId,
        original_name: &str,
    ) -> NewPhoto {
        NewPhoto {
            id,
            workspace_id,
            gallery_id,
            original_name: original_name.to_owned(),
            size: NonZeroU64::new(1024).unwrap(),
            dimensions: PhotoDimensions {
                width: NonZeroU32::new(20).unwrap(),
                height: NonZeroU32::new(10).unwrap(),
            },
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn photo_insertion_writes_metadata_without_enqueuing_a_job(pool: PgPool) {
        let database = Database::new(pool.clone());
        let workspace_id = workspace(&pool).await;
        let gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();
        let photo = new_photo(
            workspace_id,
            gallery.id,
            PhotoId::from(Uuid::new_v4()),
            "proof.jpg",
        );
        let mut transaction = pool.begin().await.unwrap();

        let outcome = database
            .repository::<PhotoRepository>()
            .insert_in(&mut transaction, &photo)
            .await
            .unwrap();

        assert!(matches!(outcome, PhotoInsertOutcome::Inserted(_)));
        let photo_count: i64 = sqlx::query_scalar("SELECT count(*) FROM photos")
            .fetch_one(&mut *transaction)
            .await
            .unwrap();
        let job_count: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
            .fetch_one(&mut *transaction)
            .await
            .unwrap();

        assert_eq!(photo_count, 1);
        assert_eq!(job_count, 0);
        transaction.commit().await.unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn accepting_a_photo_commits_its_processing_job_atomically(pool: PgPool) {
        let database = Database::new(pool.clone());
        let workspace_id = workspace(&pool).await;
        let gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();
        let photo_id = PhotoId::from(Uuid::new_v4());

        let photo = database
            .repository::<PhotoIntake>()
            .accept(new_photo(workspace_id, gallery.id, photo_id, "proof.jpg"))
            .await
            .unwrap()
            .unwrap();
        let payload: serde_json::Value = sqlx::query_scalar(
            "SELECT payload FROM jobs WHERE payload->'payload'->>'photo_id' = $1",
        )
        .bind(photo_id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(photo.id, photo_id);
        assert_eq!(
            payload,
            serde_json::json!({"kind": "process_photo", "payload": {"photo_id": photo_id.to_string()}})
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn accepting_a_missing_or_foreign_gallery_creates_no_rows(pool: PgPool) {
        let database = Database::new(pool.clone());
        let (owner, other) = (workspace(&pool).await, workspace(&pool).await);
        let gallery = database
            .repository::<GalleryRepository>()
            .create(owner)
            .await
            .unwrap();
        let intake = database.repository::<PhotoIntake>();

        assert!(
            intake
                .accept(new_photo(
                    owner,
                    GalleryId(Uuid::new_v4()),
                    PhotoId::from(Uuid::new_v4()),
                    "missing.jpg",
                ))
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            intake
                .accept(new_photo(
                    other,
                    gallery.id,
                    PhotoId::from(Uuid::new_v4()),
                    "foreign.jpg",
                ))
                .await
                .unwrap()
                .is_none()
        );

        let photo_count: i64 = sqlx::query_scalar("SELECT count(*) FROM photos")
            .fetch_one(&pool)
            .await
            .unwrap();
        let job_count: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(photo_count, 0);
        assert_eq!(job_count, 0);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn job_failure_rolls_back_photo_and_job_insertion(pool: PgPool) {
        let database = Database::new(pool.clone());
        let workspace_id = workspace(&pool).await;
        let gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();
        sqlx::query(
            "CREATE FUNCTION reject_job_insert() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN RAISE EXCEPTION 'injected test failure'; END; $$",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TRIGGER reject_job_insert BEFORE INSERT ON jobs
             FOR EACH ROW EXECUTE FUNCTION reject_job_insert()",
        )
        .execute(&pool)
        .await
        .unwrap();

        let result = database
            .repository::<PhotoIntake>()
            .accept(new_photo(
                workspace_id,
                gallery.id,
                PhotoId::from(Uuid::new_v4()),
                "proof.jpg",
            ))
            .await;

        assert!(matches!(result, Err(PhotoIntakeError::Database(_))));
        let photo_count: i64 = sqlx::query_scalar("SELECT count(*) FROM photos")
            .fetch_one(&pool)
            .await
            .unwrap();
        let job_count: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(photo_count, 0);
        assert_eq!(job_count, 0);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn reconciliation_reuses_photo_without_duplicating_job(pool: PgPool) {
        let database = Database::new(pool.clone());
        let workspace_id = workspace(&pool).await;
        let gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();
        let intake = database.repository::<PhotoIntake>();
        let photo = new_photo(
            workspace_id,
            gallery.id,
            PhotoId::from(Uuid::new_v4()),
            "proof.jpg",
        );
        let accepted = intake.accept(photo.clone()).await.unwrap().unwrap();
        let commit_error = match PhotoIntakeError::from_commit_error(sqlx::Error::Io(
            std::io::Error::from(std::io::ErrorKind::ConnectionReset),
        )) {
            PhotoIntakeError::CommitOutcomeUnknown { commit_error, .. } => commit_error,
            PhotoIntakeError::Database(error) => panic!("expected an unknown commit: {error}"),
        };
        let resolved = PhotoIntakeError::resolve_reconciliation(
            commit_error,
            intake.reconcile_commit_outcome(&photo).await,
        )
        .unwrap()
        .unwrap();
        let job_count: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(resolved, accepted);
        assert_eq!(job_count, 1);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_photo_id_collision_with_different_data_is_an_error(pool: PgPool) {
        let database = Database::new(pool.clone());
        let workspace_id = workspace(&pool).await;
        let gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();
        let intake = database.repository::<PhotoIntake>();
        let id = PhotoId::from(Uuid::new_v4());
        let photo = new_photo(workspace_id, gallery.id, id, "proof.jpg");
        intake.accept(photo.clone()).await.unwrap().unwrap();
        let mut conflicting_photo = photo;
        conflicting_photo.original_name = "different.jpg".to_owned();

        let result = intake.accept(conflicting_photo).await;

        assert!(matches!(result, Err(PhotoIntakeError::Database(_))));
        let job_count: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(job_count, 1);
    }

    #[test]
    fn connection_failure_during_commit_has_an_unknown_outcome() {
        let error = PhotoIntakeError::from_commit_error(sqlx::Error::Io(std::io::Error::from(
            std::io::ErrorKind::ConnectionReset,
        )));

        assert!(matches!(
            error,
            PhotoIntakeError::CommitOutcomeUnknown { .. }
        ));
    }

    #[test]
    fn fatal_and_panic_server_errors_during_commit_have_an_unknown_outcome() {
        assert!(commit_error_outcome_is_unknown(None));
        assert!(commit_error_outcome_is_unknown(Some(PgSeverity::Fatal)));
        assert!(commit_error_outcome_is_unknown(Some(PgSeverity::Panic)));
        assert!(!commit_error_outcome_is_unknown(Some(PgSeverity::Error)));
    }

    #[test]
    fn missing_photo_after_unknown_commit_remains_unresolved() {
        let error = PhotoIntakeError::resolve_reconciliation(
            DatabaseError::Unreachable(std::io::Error::from(std::io::ErrorKind::ConnectionReset)),
            Ok(None),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            PhotoIntakeError::CommitOutcomeUnknown { .. }
        ));
    }
}
