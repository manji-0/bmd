//! Local document link path resolution.

use std::path::{Path, PathBuf};

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
}
