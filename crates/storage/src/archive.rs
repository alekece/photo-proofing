//! ZIP archives built from stored originals.
//!
//! An archive lives next to the photos of its gallery as `<export>.zip`, so deleting the gallery
//! removes it. It is built in the staging directory and published by one rename, so a reader
//! never sees a partial file. Entries are stored uncompressed (JPEGs do not shrink) and use ZIP64
//! so a large selection is not limited to 4 GiB. Entry names come from the uploader's file names
//! and are sanitized to a single safe path component; duplicates are numbered.

use std::{
    collections::HashSet,
    ffi::OsStr,
    fs::File as StdFile,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use pp_database::{auth::WorkspaceId, gallery::GalleryId, selection_export::ExportId};
use tokio::{fs, task};
use uuid::Uuid;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{PhotoKey, PhotoStorage, StagedPhoto, StorageError};

/// Longest entry name kept, in bytes; common filesystems cap a name at 255.
const MAX_NAME_BYTES: usize = 200;

/// Name used when nothing usable remains of an uploaded file name.
const FALLBACK_NAME: &str = "photo.jpg";

/// Address of one archive file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveKey {
    pub workspace: WorkspaceId,
    pub gallery: GalleryId,
    pub export: ExportId,
}

impl ArchiveKey {
    fn dir(&self, root: &Path) -> PathBuf {
        root.join(self.workspace.0.to_string())
            .join(self.gallery.0.to_string())
    }

    fn path(&self, root: &Path) -> PathBuf {
        self.dir(root).join(format!("{}.zip", self.export.0))
    }
}

/// One file to add: the stored original and the name its uploader gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub source: PhotoKey,
    pub original_name: String,
}

/// Hands out entry names that are safe and unique within one archive.
#[derive(Debug, Default)]
struct EntryNames {
    taken: HashSet<String>,
}

impl EntryNames {
    fn next(&mut self, original: &str) -> String {
        let clean = Self::sanitize(original);
        let (stem, extension) = clean
            .rsplit_once('.')
            .map_or((clean.as_str(), ""), |(stem, extension)| (stem, extension));
        let dot = if extension.is_empty() { "" } else { "." };
        let mut name = clean.clone();
        let mut counter = 1_u32;

        while !self.taken.insert(name.to_lowercase()) {
            counter += 1;
            name = format!("{stem} ({counter}){dot}{extension}");
        }

        name
    }

    /// Keeps the last path component without control characters, leading dots or spaces.
    fn sanitize(original: &str) -> String {
        let last = original.rsplit(['/', '\\']).next().unwrap_or_default();
        let cleaned: String = last.chars().filter(|c| !c.is_control()).collect();
        let cleaned = cleaned.trim().trim_start_matches('.').trim();

        if cleaned.is_empty() {
            return FALLBACK_NAME.to_owned();
        }

        Self::truncate(cleaned)
    }

    /// Shortens a long name at a character boundary, keeping its extension.
    fn truncate(name: &str) -> String {
        if name.len() <= MAX_NAME_BYTES {
            return name.to_owned();
        }

        let extension = name
            .rsplit_once('.')
            .map(|(_, extension)| extension)
            .filter(|extension| extension.len() <= 16)
            .unwrap_or_default();
        let dot = if extension.is_empty() { 0 } else { 1 };
        let mut stem: String = String::new();

        for c in name.chars() {
            if stem.len() + c.len_utf8() + dot + extension.len() > MAX_NAME_BYTES {
                break;
            }

            stem.push(c);
        }

        if extension.is_empty() {
            stem
        } else {
            format!("{stem}.{extension}")
        }
    }
}

impl PhotoStorage {
    /// Writes a ZIP of `entries`, in order, and publishes it under `key`, replacing a file left
    /// by an earlier attempt. A missing original is [`StorageError::NotFound`] and nothing is
    /// published.
    pub async fn build_archive(
        &self,
        key: &ArchiveKey,
        entries: Vec<ArchiveEntry>,
    ) -> Result<(), StorageError> {
        let staged = self.stage().await?;
        let target = staged.temporary_path().to_owned();
        let root = self.root.clone();
        let sources: Vec<(PathBuf, String)> = entries
            .iter()
            .scan(EntryNames::default(), |names, entry| {
                Some((entry.source.path(&root), names.next(&entry.original_name)))
            })
            .collect();

        task::spawn_blocking(move || Self::write_archive(&target, &sources))
            .await
            .map_err(|error| StorageError::Io(io::Error::other(error)))??;

        staged.publish_archive(key).await
    }

    fn write_archive(target: &Path, sources: &[(PathBuf, String)]) -> Result<(), StorageError> {
        let file = StdFile::options().write(true).open(target)?;
        let mut writer = ZipWriter::new(BufWriter::new(file));
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .large_file(true);

        for (path, name) in sources {
            let mut source = StdFile::open(path)?;

            writer.start_file(name, options).map_err(io::Error::other)?;
            io::copy(&mut source, &mut writer)?;
        }

        let mut buffered = writer.finish().map_err(io::Error::other)?;

        buffered.flush()?;
        buffered.get_ref().sync_all()?;

        Ok(())
    }

    /// Opens a published archive; [`StorageError::NotFound`] if it does not exist.
    pub async fn read_archive(&self, key: &ArchiveKey) -> Result<fs::File, StorageError> {
        Ok(fs::File::open(key.path(&self.root)).await?)
    }

    /// Deletes an archive. A file already gone counts as removed.
    pub async fn remove_archive(&self, key: &ArchiveKey) -> Result<(), StorageError> {
        match fs::remove_file(key.path(&self.root)).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Deletes archives older than `grace` that `is_wanted` does not claim, such as leftovers of
    /// a crash or of an export row deleted by expiry. Returns how many files went.
    pub async fn sweep_archives<Fut>(
        &self,
        grace: Duration,
        is_wanted: impl Fn(ArchiveKey) -> Fut,
    ) -> Result<usize, StorageError>
    where
        Fut: std::future::Future<Output = bool>,
    {
        let mut removed = 0;
        let mut workspaces = fs::read_dir(&self.root).await?;

        while let Some(workspace_entry) = workspaces.next_entry().await? {
            let Some(workspace) = Self::uuid_name(&workspace_entry.file_name()) else {
                continue;
            };
            let mut galleries = fs::read_dir(workspace_entry.path()).await?;

            while let Some(gallery_entry) = galleries.next_entry().await? {
                let Some(gallery) = Self::uuid_name(&gallery_entry.file_name()) else {
                    continue;
                };
                let mut files = fs::read_dir(gallery_entry.path()).await?;

                while let Some(file) = files.next_entry().await? {
                    let Some(export) = Self::archive_name(&file.file_name()) else {
                        continue;
                    };
                    let key = ArchiveKey {
                        workspace: WorkspaceId(workspace),
                        gallery: GalleryId(gallery),
                        export: ExportId(export),
                    };
                    let age = file
                        .metadata()
                        .await?
                        .modified()?
                        .elapsed()
                        .unwrap_or_default();

                    if age < grace || is_wanted(key).await {
                        continue;
                    }

                    match fs::remove_file(file.path()).await {
                        Ok(()) => removed += 1,
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error.into()),
                    }
                }
            }
        }

        Ok(removed)
    }

    fn archive_name(name: &OsStr) -> Option<Uuid> {
        Self::uuid_name(name.to_str()?.strip_suffix(".zip")?.as_ref())
    }
}

impl StagedPhoto {
    /// Atomically publishes the staged file as the archive `key`, replacing an earlier one.
    async fn publish_archive(self, key: &ArchiveKey) -> Result<(), StorageError> {
        fs::create_dir_all(key.dir(&self.root)).await?;
        fs::rename(&self.path, key.path(&self.root)).await?;

        let directory = fs::File::open(key.dir(&self.root)).await?;

        directory.sync_all().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use tempfile::TempDir;
    use zip::ZipArchive;

    use super::*;
    use crate::Variant;

    fn names(originals: &[&str]) -> Vec<String> {
        let mut names = EntryNames::default();

        originals.iter().map(|name| names.next(name)).collect()
    }

    #[test]
    fn names_keep_only_a_safe_final_component() {
        assert_eq!(
            names(&[
                "../../etc/passwd",
                "C:\\x\\y.jpg",
                ".hidden.jpg",
                "  ",
                "a\nb.jpg"
            ]),
            ["passwd", "y.jpg", "hidden.jpg", "photo.jpg", "ab.jpg"]
        );
    }

    #[test]
    fn duplicate_names_are_numbered_ignoring_case() {
        assert_eq!(
            names(&["a.jpg", "A.JPG", "a.jpg", "b"]),
            ["a.jpg", "A (2).JPG", "a (3).jpg", "b"]
        );
    }

    #[test]
    fn long_names_are_shortened_on_a_character_boundary_and_keep_the_extension() {
        let long = format!("{}.jpg", "é".repeat(300));
        let name = names(&[&long]).remove(0);

        assert!(name.len() <= MAX_NAME_BYTES);
        assert!(name.ends_with(".jpg"));
    }

    async fn storage() -> (TempDir, PhotoStorage) {
        let dir = tempfile::tempdir().unwrap();
        let photo_dir = dir.path().to_str().unwrap().parse().unwrap();
        let storage = PhotoStorage::open(&photo_dir).await.unwrap();

        (dir, storage)
    }

    fn source(photo: u128) -> PhotoKey {
        PhotoKey {
            workspace: WorkspaceId(Uuid::from_u128(1)),
            gallery: GalleryId(Uuid::from_u128(2)),
            photo: Uuid::from_u128(photo).into(),
            variant: Variant::Original,
        }
    }

    fn archive_key() -> ArchiveKey {
        ArchiveKey {
            workspace: WorkspaceId(Uuid::from_u128(1)),
            gallery: GalleryId(Uuid::from_u128(2)),
            export: ExportId(Uuid::from_u128(9)),
        }
    }

    async fn store(storage: &PhotoStorage, key: &PhotoKey, bytes: &[u8]) {
        let mut staged = storage.stage().await.unwrap();

        staged.write(bytes).await.unwrap();
        staged.commit(key).await.unwrap();
    }

    fn entry(photo: u128, name: &str) -> ArchiveEntry {
        ArchiveEntry {
            source: source(photo),
            original_name: name.to_owned(),
        }
    }

    #[tokio::test]
    async fn the_archive_holds_every_original_byte_for_byte_in_order() {
        let (_dir, storage) = storage().await;

        store(&storage, &source(3), b"first original").await;
        store(&storage, &source(4), b"second original").await;
        storage
            .build_archive(&archive_key(), vec![entry(3, "a.jpg"), entry(4, "a.jpg")])
            .await
            .unwrap();

        let path = archive_key().path(&storage.root);
        let mut zip = ZipArchive::new(StdFile::open(path).unwrap()).unwrap();
        let mut first = String::new();
        let mut second = String::new();

        zip.by_index(0).unwrap().read_to_string(&mut first).unwrap();
        zip.by_index(1)
            .unwrap()
            .read_to_string(&mut second)
            .unwrap();

        assert_eq!(zip.name_for_index(0).unwrap().unwrap(), "a.jpg");
        assert_eq!(zip.name_for_index(1).unwrap().unwrap(), "a (2).jpg");
        assert_eq!(first, "first original");
        assert_eq!(second, "second original");
    }

    #[tokio::test]
    async fn a_missing_original_publishes_nothing_and_leaves_no_temp_file() {
        let (dir, storage) = storage().await;

        store(&storage, &source(3), b"present").await;

        let result = storage
            .build_archive(&archive_key(), vec![entry(3, "a.jpg"), entry(4, "b.jpg")])
            .await;

        assert!(matches!(result, Err(StorageError::NotFound)));
        assert!(matches!(
            storage.read_archive(&archive_key()).await,
            Err(StorageError::NotFound)
        ));
        assert_eq!(
            std::fs::read_dir(dir.path().join(".tmp")).unwrap().count(),
            0
        );
    }

    #[tokio::test]
    async fn removing_an_archive_is_idempotent() {
        let (_dir, storage) = storage().await;

        store(&storage, &source(3), b"x").await;
        storage
            .build_archive(&archive_key(), vec![entry(3, "a.jpg")])
            .await
            .unwrap();
        storage.remove_archive(&archive_key()).await.unwrap();
        storage.remove_archive(&archive_key()).await.unwrap();

        assert!(storage.read_archive(&archive_key()).await.is_err());
    }

    #[tokio::test]
    async fn sweeping_removes_only_old_unwanted_archives() {
        let (_dir, storage) = storage().await;

        store(&storage, &source(3), b"x").await;
        storage
            .build_archive(&archive_key(), vec![entry(3, "a.jpg")])
            .await
            .unwrap();

        let young = storage
            .sweep_archives(Duration::from_secs(3600), |_| async { false })
            .await
            .unwrap();
        let wanted = storage
            .sweep_archives(Duration::ZERO, |_| async { true })
            .await
            .unwrap();
        let orphan = storage
            .sweep_archives(Duration::ZERO, |_| async { false })
            .await
            .unwrap();

        assert_eq!((young, wanted, orphan), (0, 0, 1));
        assert!(storage.read(&source(3)).await.is_ok());
    }
}
