//! Private photo files on the local filesystem, shared by the API and the worker.
//!
//! Layout (see ADR 0003): `<photo-dir>/<workspace>/<gallery>/<photo>-<variant>`. Every path is
//! derived from a [`PhotoKey`] made of UUIDs and a [`Variant`], never from a user-supplied
//! string, so a path cannot escape the directory or cross workspaces by construction.
//!
//! Writes go through [`PhotoStorage::stage`]: bytes land in a temp file under `<photo-dir>/.tmp`
//! (the same filesystem, so the final `rename` is atomic) and only [`StagedPhoto::commit`] makes
//! them visible. Failed original publication removes its new target unless cleanup also fails.
//! Cancellation after publication can leave an orphan; it cannot roll back asynchronously.

use std::{
    future::Future,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    time::Duration,
};

use pp_core::{config::PhotoDir, notifications::PhotoId};
use pp_database::{auth::WorkspaceId, gallery::GalleryId, photo::PhotoLocation};
use strum::Display;
use thiserror::Error;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};
use uuid::Uuid;

const STAGING_DIR: &str = ".tmp";

/// Failure of a storage operation. Callers branch on the variant; causes stay as sources.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("photo directory {path} is not a writable directory")]
    Unusable { path: PathBuf, source: io::Error },
    #[error("no space left on the photo volume")]
    DiskFull(#[source] io::Error),
    #[error("photo is not stored")]
    NotFound,
    #[error("photo is already stored")]
    AlreadyExists,
    /// Directory durability failed. A new original was removed; a replaced derivative remains.
    #[error("photo directory sync failed")]
    DirectorySync(#[source] io::Error),
    /// Recovery failed after directory sync failed; the new original may remain on disk.
    #[error("failed to remove original after directory sync failure")]
    PublicationCleanup(#[source] io::Error),
    #[error("photo storage I/O failed")]
    Io(#[source] io::Error),
}

impl From<io::Error> for StorageError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            ErrorKind::StorageFull | ErrorKind::QuotaExceeded => Self::DiskFull(error),
            ErrorKind::NotFound => Self::NotFound,
            ErrorKind::AlreadyExists => Self::AlreadyExists,
            _ => Self::Io(error),
        }
    }
}

/// Which file of a photo is addressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
#[strum(serialize_all = "lowercase")]
pub enum Variant {
    /// The accepted upload, written once. A finished photo is also stored as an `Original`
    /// under its own photo id.
    Original,
    Thumbnail,
    Preview,
}

/// Address of one stored file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhotoKey {
    pub workspace: WorkspaceId,
    pub gallery: GalleryId,
    pub photo: PhotoId,
    pub variant: Variant,
}

impl PhotoKey {
    pub const fn with_location(location: PhotoLocation, photo: PhotoId, variant: Variant) -> Self {
        Self {
            workspace: location.workspace_id,
            gallery: location.gallery_id,
            photo,
            variant,
        }
    }

    fn dir(&self, root: &Path) -> PathBuf {
        root.join(self.workspace.0.to_string())
            .join(self.gallery.0.to_string())
    }

    fn path(&self, root: &Path) -> PathBuf {
        self.dir(root)
            .join(format!("{}-{}", self.photo, self.variant))
    }
}

/// Handle to the photo directory, checked at [`open`](Self::open).
#[derive(Debug, Clone)]
pub struct PhotoStorage {
    root: PathBuf,
}

impl PhotoStorage {
    /// Fails unless `dir` is an existing directory in which files can be created, so a missing
    /// or read-only volume stops startup instead of failing the first upload.
    pub async fn open(dir: &PhotoDir) -> Result<Self, StorageError> {
        let root = dir.as_path().to_owned();
        let unusable = |source| StorageError::Unusable {
            path: root.clone(),
            source,
        };

        let metadata = fs::metadata(&root).await.map_err(unusable)?;

        if !metadata.is_dir() {
            return Err(unusable(ErrorKind::NotADirectory.into()));
        }

        fs::create_dir_all(root.join(STAGING_DIR))
            .await
            .map_err(unusable)?;

        // Creating the staging directory succeeds when it already exists, so probe with a file.
        let probe = root.join(STAGING_DIR).join(Uuid::new_v4().to_string());

        File::create(&probe).await.map_err(unusable)?;
        fs::remove_file(&probe).await.map_err(unusable)?;

        Ok(Self { root })
    }

    /// Starts a write. Nothing is visible under a photo name until [`StagedPhoto::commit`].
    pub async fn stage(&self) -> Result<StagedPhoto, StorageError> {
        let path = self.root.join(STAGING_DIR).join(Uuid::new_v4().to_string());
        let file = File::create_new(&path).await?;

        Ok(StagedPhoto {
            root: self.root.clone(),
            file,
            path,
        })
    }

    /// Opens a stored file for reading; [`StorageError::NotFound`] if it does not exist.
    pub async fn read(&self, key: &PhotoKey) -> Result<File, StorageError> {
        Ok(File::open(key.path(&self.root)).await?)
    }

    /// Deletes one stored file. Removing a gallery's directory belongs to gallery deletion.
    pub async fn remove(&self, key: &PhotoKey) -> Result<(), StorageError> {
        Ok(fs::remove_file(key.path(&self.root)).await?)
    }

    /// Deletes every file of the given photos, going on past failures. A file already gone
    /// counts as removed; the others that could not be removed are returned with their error.
    pub async fn remove_many(
        &self,
        location: PhotoLocation,
        photos: &[PhotoId],
    ) -> Vec<(PhotoKey, StorageError)> {
        let mut failures = Vec::new();

        for &photo in photos {
            for variant in [Variant::Original, Variant::Thumbnail, Variant::Preview] {
                let key = PhotoKey::with_location(location, photo, variant);

                match self.remove(&key).await {
                    Ok(()) | Err(StorageError::NotFound) => {}
                    Err(error) => failures.push((key, error)),
                }
            }
        }

        failures
    }

    /// Deletes a gallery's whole directory. A directory already gone counts as removed.
    pub async fn remove_gallery(
        &self,
        workspace: WorkspaceId,
        gallery: GalleryId,
    ) -> Result<(), StorageError> {
        let dir = self
            .root
            .join(workspace.0.to_string())
            .join(gallery.0.to_string());

        match fs::remove_dir_all(dir).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Deletes photo files no stored photo owns, such as leftovers of a crash between a
    /// deletion and its cleanup. Only files older than `grace` are considered, so an upload in
    /// flight is never taken for an orphan; `is_stored` is asked about each remaining photo.
    /// Names that are not `<photo-uuid>-<variant>` are left alone. Returns how many files went.
    pub async fn sweep_orphans<Fut>(
        &self,
        grace: Duration,
        is_stored: impl Fn(PhotoLocation, PhotoId) -> Fut,
    ) -> Result<usize, StorageError>
    where
        Fut: Future<Output = bool>,
    {
        let mut removed = 0;
        let mut workspaces = fs::read_dir(&self.root).await?;

        while let Some(workspace_entry) = workspaces.next_entry().await? {
            let Some(workspace) = Self::uuid_name(&workspace_entry) else {
                continue;
            };
            let mut galleries = fs::read_dir(workspace_entry.path()).await?;

            while let Some(gallery_entry) = galleries.next_entry().await? {
                let Some(gallery) = Self::uuid_name(&gallery_entry) else {
                    continue;
                };
                let location = PhotoLocation {
                    workspace_id: WorkspaceId(workspace),
                    gallery_id: GalleryId(gallery),
                };

                removed += self
                    .sweep_gallery(&gallery_entry.path(), location, grace, &is_stored)
                    .await?;
            }
        }

        Ok(removed)
    }

    async fn sweep_gallery<Fut>(
        &self,
        dir: &Path,
        location: PhotoLocation,
        grace: Duration,
        is_stored: &impl Fn(PhotoLocation, PhotoId) -> Fut,
    ) -> Result<usize, StorageError>
    where
        Fut: Future<Output = bool>,
    {
        let mut removed = 0;
        let mut files = fs::read_dir(dir).await?;

        while let Some(file) = files.next_entry().await? {
            let Some(photo) = Self::photo_name(&file) else {
                continue;
            };
            let age = file
                .metadata()
                .await?
                .modified()?
                .elapsed()
                .unwrap_or_default();

            if age < grace || is_stored(location, photo).await {
                continue;
            }

            match fs::remove_file(file.path()).await {
                Ok(()) => removed += 1,
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }

        Ok(removed)
    }

    fn uuid_name(entry: &fs::DirEntry) -> Option<Uuid> {
        entry.file_name().to_str()?.parse().ok()
    }

    fn photo_name(entry: &fs::DirEntry) -> Option<PhotoId> {
        let name = entry.file_name();
        let (photo, variant) = name.to_str()?.rsplit_once('-')?;

        if !["original", "thumbnail", "preview"].contains(&variant) {
            return None;
        }

        photo.parse::<Uuid>().ok().map(PhotoId::from)
    }
}

/// A file being written. Dropping it, committed or not, deletes its temp file.
#[must_use = "a staged photo is discarded unless committed"]
#[derive(Debug)]
pub struct StagedPhoto {
    root: PathBuf,
    file: File,
    path: PathBuf,
}

impl StagedPhoto {
    /// The random, storage-owned path of this staged file.
    ///
    /// This path is for internal metadata inspection only; it is not derived from user input and
    /// does not identify a published photo key.
    pub fn temporary_path(&self) -> &Path {
        &self.path
    }

    /// Appends bytes. Writes are buffered, so a full volume may only be reported by a later
    /// `write` or by [`commit`](Self::commit), which never succeeds after a failed write.
    pub async fn write(&mut self, bytes: &[u8]) -> Result<(), StorageError> {
        Ok(self.file.write_all(bytes).await?)
    }

    /// Flushes staged bytes so another reader can inspect the temporary file.
    pub async fn flush(&mut self) -> Result<(), StorageError> {
        Ok(self.file.flush().await?)
    }

    /// Syncs the file and atomically publishes it under `key`.
    ///
    /// An `Original` is written once: if one exists, nothing is replaced and
    /// [`StorageError::AlreadyExists`] is returned. Any other variant replaces its previous
    /// file, as a retried processing job regenerates it.
    ///
    /// If directory sync fails after publication, storage removes this attempt's new original.
    /// [`StorageError::PublicationCleanup`] reports failed removal; callers must not try to
    /// remove targets for other errors, since they might predate this attempt. A replaced
    /// derivative cannot be restored and stays available even if directory sync fails.
    ///
    /// Cancellation after publication can leave a file without confirmed durability.
    ///
    /// ```
    /// # async fn store(storage: &pp_storage::PhotoStorage, key: &pp_storage::PhotoKey)
    /// #     -> Result<(), pp_storage::StorageError> {
    /// let mut staged = storage.stage().await?;
    /// staged.write(b"original bytes").await?;
    /// staged.commit(key).await
    /// # }
    /// ```
    pub async fn commit(mut self, key: &PhotoKey) -> Result<(), StorageError> {
        self.file.flush().await?;
        self.file.sync_all().await?;

        let target = key.path(&self.root);

        fs::create_dir_all(key.dir(&self.root)).await?;

        match key.variant {
            // `hard_link` fails instead of replacing, unlike `rename`.
            Variant::Original => fs::hard_link(&self.path, &target).await?,
            Variant::Thumbnail | Variant::Preview => fs::rename(&self.path, &target).await?,
        }

        // Persist the directory entry so a crash cannot lose a photo that was reported stored.
        let durability = async {
            let directory = File::open(key.dir(&self.root)).await?;

            directory.sync_all().await
        }
        .await;

        if let Err(error) = durability {
            let error = match key.variant {
                Variant::Original => rollback_original(&target, error).await,
                Variant::Thumbnail | Variant::Preview => StorageError::DirectorySync(error),
            };

            return Err(error);
        }

        // `Drop` then unlinks the staged name that a hard-linked original leaves behind.
        Ok(())
    }
}

async fn rollback_original(target: &Path, source: io::Error) -> StorageError {
    match fs::remove_file(target).await {
        Ok(()) => StorageError::DirectorySync(source),
        Err(source) => StorageError::PublicationCleanup(source),
    }
}

impl Drop for StagedPhoto {
    fn drop(&mut self) {
        // Synchronous because `Drop` cannot await; unlinking one file is quick. The error is
        // ignored because a renamed file is already gone.
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use std::{fs as std_fs, os::unix::fs::PermissionsExt};

    use tempfile::TempDir;

    use super::*;

    fn key(variant: Variant) -> PhotoKey {
        PhotoKey {
            workspace: WorkspaceId(Uuid::from_u128(1)),
            gallery: GalleryId(Uuid::from_u128(2)),
            photo: Uuid::from_u128(3).into(),
            variant,
        }
    }

    async fn storage() -> (TempDir, PhotoStorage) {
        let dir = tempfile::tempdir().unwrap();
        let photo_dir = dir.path().to_str().unwrap().parse().unwrap();
        let storage = PhotoStorage::open(&photo_dir).await.unwrap();

        (dir, storage)
    }

    async fn store(storage: &PhotoStorage, key: &PhotoKey, bytes: &[u8]) {
        let mut staged = storage.stage().await.unwrap();

        staged.write(bytes).await.unwrap();
        staged.commit(key).await.unwrap();
    }

    async fn read_all(storage: &PhotoStorage, key: &PhotoKey) -> Vec<u8> {
        let mut bytes = Vec::new();

        tokio::io::AsyncReadExt::read_to_end(&mut storage.read(key).await.unwrap(), &mut bytes)
            .await
            .unwrap();

        bytes
    }

    fn staged_files(root: &Path) -> usize {
        std_fs::read_dir(root.join(STAGING_DIR)).unwrap().count()
    }

    #[test]
    fn paths_are_derived_from_ids_and_stay_inside_the_root() {
        let root = Path::new("/photos");

        for variant in [Variant::Original, Variant::Thumbnail, Variant::Preview] {
            let path = key(variant).path(root);
            let relative = path.strip_prefix(root).unwrap();

            assert_eq!(relative.components().count(), 3);
            assert!(
                relative
                    .components()
                    .all(|part| matches!(part, std::path::Component::Normal(_)))
            );
            assert!(!relative.starts_with(STAGING_DIR));
        }

        assert_eq!(
            key(Variant::Preview).path(root),
            Path::new(
                "/photos/00000000-0000-0000-0000-000000000001/\
                 00000000-0000-0000-0000-000000000002/\
                 00000000-0000-0000-0000-000000000003-preview"
            )
        );
    }

    #[test]
    fn keys_of_different_workspaces_never_share_a_path() {
        let root = Path::new("/photos");
        let other = PhotoKey {
            workspace: WorkspaceId(Uuid::from_u128(9)),
            ..key(Variant::Original)
        };

        assert_ne!(other.path(root), key(Variant::Original).path(root));
    }

    #[tokio::test]
    async fn file_is_invisible_until_commit_and_readable_after() {
        let (dir, storage) = storage().await;
        let key = key(Variant::Original);
        let mut staged = storage.stage().await.unwrap();

        staged.write(b"pixels").await.unwrap();

        assert!(matches!(
            storage.read(&key).await,
            Err(StorageError::NotFound)
        ));

        staged.commit(&key).await.unwrap();

        assert_eq!(read_all(&storage, &key).await, b"pixels");
        assert_eq!(staged_files(dir.path()), 0);
    }

    #[tokio::test]
    async fn dropping_without_commit_leaves_nothing() {
        let (dir, storage) = storage().await;
        let mut staged = storage.stage().await.unwrap();

        staged.write(b"half").await.unwrap();
        drop(staged);

        assert_eq!(staged_files(dir.path()), 0);
        assert!(matches!(
            storage.read(&key(Variant::Original)).await,
            Err(StorageError::NotFound)
        ));
    }

    #[tokio::test]
    async fn staged_path_points_to_the_storage_owned_temporary_file() {
        let (dir, storage) = storage().await;
        let staged = storage.stage().await.unwrap();
        let path = staged.temporary_path();

        assert!(path.is_file());
        assert!(path.starts_with(dir.path().join(STAGING_DIR)));
        assert_ne!(path, key(Variant::Original).path(dir.path()));
        assert!(matches!(
            storage.read(&key(Variant::Original)).await,
            Err(StorageError::NotFound)
        ));
    }

    #[tokio::test]
    async fn flush_makes_staged_bytes_available_for_metadata_inspection() {
        let (_dir, storage) = storage().await;
        let mut staged = storage.stage().await.unwrap();
        let path = staged.temporary_path().to_owned();

        staged.write(b"header bytes").await.unwrap();
        staged.flush().await.unwrap();

        assert_eq!(std_fs::read(path).unwrap(), b"header bytes");
    }

    #[tokio::test]
    async fn derivatives_are_replaced_but_originals_are_written_once() {
        let (dir, storage) = storage().await;
        let thumbnail = key(Variant::Thumbnail);
        let original = key(Variant::Original);

        store(&storage, &thumbnail, b"old").await;
        store(&storage, &thumbnail, b"new").await;
        store(&storage, &original, b"first").await;

        let mut second = storage.stage().await.unwrap();

        second.write(b"second").await.unwrap();

        assert!(matches!(
            second.commit(&original).await,
            Err(StorageError::AlreadyExists)
        ));
        assert_eq!(read_all(&storage, &thumbnail).await, b"new");
        assert_eq!(read_all(&storage, &original).await, b"first");
        assert_eq!(staged_files(dir.path()), 0);
    }

    #[tokio::test]
    async fn failure_before_publication_preserves_an_existing_original() {
        let (dir, storage) = storage().await;
        let key = key(Variant::Original);

        store(&storage, &key, b"existing original").await;

        let mut duplicate = storage.stage().await.unwrap();

        duplicate.write(b"new upload").await.unwrap();
        duplicate.flush().await.unwrap();
        std_fs::remove_file(duplicate.temporary_path()).unwrap();

        assert!(duplicate.commit(&key).await.is_err());
        assert_eq!(read_all(&storage, &key).await, b"existing original");
        assert_eq!(staged_files(dir.path()), 0);
    }

    #[tokio::test]
    async fn failure_after_publication_removes_only_the_new_original() {
        let (dir, storage) = storage().await;
        let original = key(Variant::Original);
        let older = PhotoKey {
            photo: Uuid::from_u128(4).into(),
            ..original
        };

        store(&storage, &older, b"older original").await;

        let parent = original.dir(dir.path());
        let mut staged = storage.stage().await.unwrap();

        staged.write(b"new original").await.unwrap();
        staged.flush().await.unwrap();
        // Linking needs write/search permissions; opening the directory for fsync needs read.
        std_fs::set_permissions(&parent, std_fs::Permissions::from_mode(0o300)).unwrap();

        let enforced = File::open(&parent).await.is_err();
        let result = staged.commit(&original).await;

        std_fs::set_permissions(&parent, std_fs::Permissions::from_mode(0o700)).unwrap();

        if enforced {
            assert!(result.is_err());
            assert!(matches!(
                storage.read(&original).await,
                Err(StorageError::NotFound)
            ));
        } else {
            // Permission bits do not bind root.
            result.unwrap();
        }

        assert_eq!(read_all(&storage, &older).await, b"older original");
        assert_eq!(staged_files(dir.path()), 0);
    }

    #[tokio::test]
    async fn failed_publication_cleanup_reports_the_removal_error() {
        let (dir, _storage) = storage().await;
        let target = key(Variant::Original).path(dir.path());
        let error = rollback_original(&target, io::ErrorKind::PermissionDenied.into()).await;

        assert!(matches!(
            error,
            StorageError::PublicationCleanup(source) if source.kind() == io::ErrorKind::NotFound
        ));
    }

    #[tokio::test]
    async fn directory_sync_failure_keeps_the_replaced_derivative() {
        let (dir, storage) = storage().await;
        let thumbnail = key(Variant::Thumbnail);

        store(&storage, &thumbnail, b"old thumbnail").await;

        let mut staged = storage.stage().await.unwrap();
        let parent = thumbnail.dir(dir.path());

        staged.write(b"new thumbnail").await.unwrap();
        std_fs::set_permissions(&parent, std_fs::Permissions::from_mode(0o300)).unwrap();

        let enforced = File::open(&parent).await.is_err();
        let result = staged.commit(&thumbnail).await;

        std_fs::set_permissions(&parent, std_fs::Permissions::from_mode(0o700)).unwrap();

        if enforced {
            assert!(matches!(result, Err(StorageError::DirectorySync(_))));
        } else {
            result.unwrap();
        }

        assert_eq!(read_all(&storage, &thumbnail).await, b"new thumbnail");
        assert_eq!(staged_files(dir.path()), 0);
    }

    #[tokio::test]
    async fn remove_deletes_one_file() {
        let (_dir, storage) = storage().await;
        let key = key(Variant::Preview);

        store(&storage, &key, b"x").await;
        storage.remove(&key).await.unwrap();

        assert!(matches!(
            storage.read(&key).await,
            Err(StorageError::NotFound)
        ));
        assert!(matches!(
            storage.remove(&key).await,
            Err(StorageError::NotFound)
        ));
    }

    #[tokio::test]
    async fn remove_many_deletes_every_file_of_the_photos() {
        let (dir, storage) = storage().await;
        let location = PhotoLocation {
            workspace_id: WorkspaceId(Uuid::new_v4()),
            gallery_id: GalleryId(Uuid::new_v4()),
        };
        let removed: PhotoId = Uuid::new_v4().into();
        let kept: PhotoId = Uuid::new_v4().into();

        for variant in [Variant::Original, Variant::Thumbnail] {
            store(
                &storage,
                &PhotoKey::with_location(location, removed, variant),
                b"x",
            )
            .await;
        }

        let kept_key = PhotoKey::with_location(location, kept, Variant::Original);

        store(&storage, &kept_key, b"x").await;

        assert!(storage.remove_many(location, &[removed]).await.is_empty());
        assert_eq!(read_all(&storage, &kept_key).await, b"x");
        assert_eq!(
            std_fs::read_dir(kept_key.dir(dir.path())).unwrap().count(),
            1
        );
    }

    #[test]
    fn full_disk_is_reported_as_such() {
        assert!(matches!(
            StorageError::from(io::Error::from(ErrorKind::StorageFull)),
            StorageError::DiskFull(_)
        ));
        assert!(matches!(
            StorageError::from(io::Error::from_raw_os_error(28)),
            StorageError::DiskFull(_)
        ));
        assert!(matches!(
            StorageError::from(io::Error::from(ErrorKind::PermissionDenied)),
            StorageError::Io(_)
        ));
    }

    #[tokio::test]
    async fn write_to_a_full_device_fails_and_leaves_nothing() {
        let (dir, storage) = storage().await;
        let mut staged = storage.stage().await.unwrap();

        // `/dev/full` fails every write with ENOSPC, as a full volume would.
        staged.file = File::options().write(true).open("/dev/full").await.unwrap();
        let written = staged.write(&[0; 4096]).await;

        // Writes are buffered, so the failure may only surface at commit; it must never be hidden.
        let outcome = match written {
            Ok(()) => staged.commit(&key(Variant::Original)).await,
            Err(error) => Err(error),
        };

        assert!(matches!(outcome, Err(StorageError::DiskFull(_))));
        assert_eq!(staged_files(dir.path()), 0);
        assert!(matches!(
            storage.read(&key(Variant::Original)).await,
            Err(StorageError::NotFound)
        ));
    }

    #[tokio::test]
    async fn open_rejects_missing_and_non_directory_paths() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file");

        std_fs::write(&file, "x").unwrap();

        for path in [dir.path().join("missing"), file] {
            let photo_dir = path.to_str().unwrap().parse().unwrap();

            assert!(matches!(
                PhotoStorage::open(&photo_dir).await,
                Err(StorageError::Unusable { .. })
            ));
        }
    }

    #[tokio::test]
    async fn open_rejects_a_read_only_directory() {
        let dir = tempfile::tempdir().unwrap();

        std_fs::set_permissions(dir.path(), std_fs::Permissions::from_mode(0o555)).unwrap();

        // Permission bits do not bind root, so only assert when they are enforced.
        let enforced = std_fs::write(dir.path().join("probe"), "x").is_err();
        let photo_dir = dir.path().to_str().unwrap().parse().unwrap();
        let result = PhotoStorage::open(&photo_dir).await;

        std_fs::set_permissions(dir.path(), std_fs::Permissions::from_mode(0o755)).unwrap();

        if enforced {
            assert!(matches!(result, Err(StorageError::Unusable { .. })));
        }
    }

    fn at(workspace: u128, gallery: u128, photo: u128, variant: Variant) -> PhotoKey {
        PhotoKey {
            workspace: WorkspaceId(Uuid::from_u128(workspace)),
            gallery: GalleryId(Uuid::from_u128(gallery)),
            photo: Uuid::from_u128(photo).into(),
            variant,
        }
    }

    #[tokio::test]
    async fn remove_gallery_deletes_only_that_gallery_and_tolerates_a_missing_one() {
        let (_dir, storage) = storage().await;
        let doomed = at(1, 2, 3, Variant::Original);
        let sibling = at(1, 9, 3, Variant::Original);

        store(&storage, &doomed, b"a").await;
        store(&storage, &sibling, b"b").await;

        storage
            .remove_gallery(doomed.workspace, doomed.gallery)
            .await
            .unwrap();
        storage
            .remove_gallery(doomed.workspace, doomed.gallery)
            .await
            .unwrap();

        assert!(matches!(
            storage.read(&doomed).await,
            Err(StorageError::NotFound)
        ));
        assert_eq!(read_all(&storage, &sibling).await, b"b");
    }

    #[tokio::test]
    async fn sweep_removes_only_old_unowned_photo_files() {
        let (dir, storage) = storage().await;
        let owned = at(1, 2, 3, Variant::Original);
        let orphan = at(1, 2, 4, Variant::Thumbnail);
        let emptied = at(1, 7, 5, Variant::Preview);

        for key in [&owned, &orphan, &emptied] {
            store(&storage, key, b"x").await;
        }

        std_fs::write(dir.path().join("1").parent().unwrap().join("stray"), b"x").unwrap();

        let owned_photo = owned.photo;
        let is_stored = |_: PhotoLocation, photo: PhotoId| async move { photo == owned_photo };

        assert_eq!(
            storage
                .sweep_orphans(Duration::from_secs(3600), is_stored)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            storage
                .sweep_orphans(Duration::ZERO, is_stored)
                .await
                .unwrap(),
            2
        );

        assert_eq!(read_all(&storage, &owned).await, b"x");
        assert!(matches!(
            storage.read(&orphan).await,
            Err(StorageError::NotFound)
        ));
        assert!(matches!(
            storage.read(&emptied).await,
            Err(StorageError::NotFound)
        ));
        assert!(dir.path().join(STAGING_DIR).exists());
        assert!(dir.path().join("stray").exists());
    }
}
