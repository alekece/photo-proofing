//! ZIP archives built from stored originals.
//!
//! An archive lives next to the photos of its gallery as `<export>.zip`, so deleting the gallery
//! removes it. It is built in the staging directory and published by one rename, so a reader
//! never sees a partial file. Entries are stored uncompressed (JPEGs do not shrink) and use ZIP64
//! so a large selection is not limited to 4 GiB. Entry names come from the uploader's file names
//! and are sanitized to a single safe path component; duplicates are numbered.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use pp_database::{auth::WorkspaceId, export::ExportId, gallery::GalleryId};

use crate::PhotoKey;

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
    pub(crate) fn dir(&self, root: &Path) -> PathBuf {
        root.join(self.workspace.0.to_string())
            .join(self.gallery.0.to_string())
    }

    pub(crate) fn path(&self, root: &Path) -> PathBuf {
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
pub(crate) struct EntryNames {
    taken: HashSet<String>,
}

impl EntryNames {
    pub(crate) fn next(&mut self, original: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
