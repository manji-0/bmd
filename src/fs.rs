//! Standard-library filesystem adapter for [`crate::domain::DocumentFs`].

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::domain::DocumentFs;

/// Production filesystem: canonicalize, metadata, and file existence via `std::fs`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StdDocumentFs;

impl DocumentFs for StdDocumentFs {
    fn identity(&self, path: PathBuf) -> PathBuf {
        std::fs::canonicalize(&path).unwrap_or(path)
    }

    fn modified_time(&self, path: &Path) -> Option<SystemTime> {
        std::fs::metadata(path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }
}

/// Canonicalize a resolved path for cache keys and prefetch lookup.
pub fn normalize_document_path(path: PathBuf) -> PathBuf {
    StdDocumentFs.identity(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_uses_canonical_path_when_available() {
        let dir = std::env::temp_dir().join(format!("bmd-norm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("note.md");
        std::fs::write(&file, "# note\n").unwrap();
        let normalized = normalize_document_path(file.clone());
        assert!(normalized.is_absolute());
        let _ = std::fs::remove_dir_all(dir);
    }
}
