//! Browser launcher adapter for macOS.

use crate::domain::LinkUrl;
use crate::error::AppError;
use std::process::Command;

/// Open `url` with the macOS `open` command.
///
/// # Errors
///
/// Returns `AppError::OpenLink` if the `open` command fails to spawn.
pub fn open_link(url: &LinkUrl) -> Result<(), AppError> {
    Command::new("open")
        .arg(url.as_str())
        .spawn()
        .map_err(|e| AppError::OpenLink(format!("open {} failed: {}", url.as_str(), e)))?;
    Ok(())
}
