//! Browser launcher adapter.

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

fn spawn_opener(url: &str) -> Result<(), AppError> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(program)
        .arg(url)
        .spawn()
        .map_err(|e| AppError::OpenLink(format!("{program} {url} failed: {e}")))?;
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
}
