//! Local document link path resolution.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Split a link destination into path and optional `#fragment`.
pub fn document_link_path_part(dest: &str) -> (&str, Option<&str>) {
    match dest.split_once('#') {
        Some((path, frag)) if !path.is_empty() => (path, Some(frag)),
        Some((_, frag)) => ("", Some(frag)),
        None => (dest, None),
    }
}

/// Returns true when the destination should open in an external handler.
pub fn is_remote_link_dest(dest: &str) -> bool {
    let path_part = dest.split('#').next().unwrap_or(dest);
    let lower = path_part.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
}

/// Last modification time for a local file, when available.
pub fn file_modified_time(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
}

/// Canonicalize a resolved path for cache keys and prefetch lookup.
///
/// Falls back to `path` when canonicalization fails (e.g. the file was removed).
pub fn normalize_document_path(path: PathBuf) -> PathBuf {
    std::fs::canonicalize(&path).unwrap_or(path)
}

/// Resolve a relative or absolute markdown file path against the current file.
///
/// # Errors
///
/// Returns `DocumentPathError` when the path is empty or no base file is available.
pub fn resolve_document_path(
    current_file: Option<&Path>,
    dest: &str,
) -> Result<PathBuf, DocumentPathError> {
    let (path_part, _) = document_link_path_part(dest);
    if path_part.is_empty() {
        return Err(DocumentPathError::EmptyPath);
    }
    let path = Path::new(path_part);
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let Some(base) = current_file.and_then(|p| p.parent()) else {
        return Err(DocumentPathError::NoBasePath);
    };
    Ok(base.join(path))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DocumentPathError {
    #[error("document link path is empty")]
    EmptyPath,
    #[error("relative document links require a file-backed document")]
    NoBasePath,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_path_and_fragment() {
        assert_eq!(
            document_link_path_part("other.md#section"),
            ("other.md", Some("section"))
        );
        assert_eq!(document_link_path_part("#only"), ("", Some("only")));
        assert_eq!(document_link_path_part("plain.md"), ("plain.md", None));
    }

    #[test]
    fn remote_dest_detection() {
        assert!(is_remote_link_dest("https://example.com"));
        assert!(is_remote_link_dest("http://x/a.md#b"));
        assert!(!is_remote_link_dest("./local.md"));
    }

    #[test]
    fn resolves_relative_to_current_file() {
        let base = PathBuf::from("/docs/readme.md");
        let resolved = resolve_document_path(Some(&base), "guide/other.md").unwrap();
        assert_eq!(resolved, PathBuf::from("/docs/guide/other.md"));
    }

    #[test]
    fn rejects_relative_path_without_base_file() {
        let err = resolve_document_path(None, "other.md").unwrap_err();
        assert_eq!(err, DocumentPathError::NoBasePath);
    }

    #[test]
    fn rejects_empty_path_part() {
        let base = PathBuf::from("/docs/readme.md");
        let err = resolve_document_path(Some(&base), "#only-anchor").unwrap_err();
        assert_eq!(err, DocumentPathError::EmptyPath);
    }

    #[test]
    fn strips_fragment_before_resolving_path() {
        let base = PathBuf::from("/docs/readme.md");
        let resolved = resolve_document_path(Some(&base), "guide/other.md#section").unwrap();
        assert_eq!(resolved, PathBuf::from("/docs/guide/other.md"));
    }

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
