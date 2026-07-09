//! Browser and external-viewer launcher adapter.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

use crate::domain::LinkUrl;
use crate::error::AppError;

/// Open `url` with the platform default handler.
///
/// This is a no-op while the crate is compiled for unit tests so `cargo test`
/// does not launch a real browser.
///
/// # Errors
///
/// Returns `AppError::OpenLink` if the opener command fails to spawn.
pub fn open_link(url: &LinkUrl) -> Result<(), AppError> {
    if cfg!(test) {
        return Ok(());
    }
    spawn_opener(url.as_str())
}

/// Open a local file `path` with the platform default handler (e.g. Preview.app
/// on macOS), used when the terminal can't render images inline.
///
/// This is a no-op while the crate is compiled for unit tests so `cargo test`
/// does not launch a real viewer.
///
/// # Errors
///
/// Returns `AppError::OpenLink` if the opener command fails to spawn.
pub fn open_path(path: &Path) -> Result<(), AppError> {
    if cfg!(test) {
        return Ok(());
    }
    spawn_opener(path)
}

fn spawn_opener(target: impl AsRef<OsStr>) -> Result<(), AppError> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let target = target.as_ref();
    Command::new(program).arg(target).spawn().map_err(|e| {
        AppError::OpenLink(format!(
            "{program} {} failed: {e}",
            target.to_string_lossy()
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::LinkUrl;

    #[test]
    fn open_link_does_not_spawn_browser_under_test() {
        let url = LinkUrl::new("https://example.com".to_string()).unwrap();
        assert!(open_link(&url).is_ok());
    }

    #[test]
    fn open_path_does_not_spawn_viewer_under_test() {
        assert!(open_path(Path::new("/tmp/does-not-matter.png")).is_ok());
    }
}
