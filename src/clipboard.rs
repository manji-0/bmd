//! System clipboard adapter.

use crate::error::AppError;

/// Copy plain text to the system clipboard.
///
/// This is a no-op while the crate is compiled for unit tests so `cargo test`
/// does not touch the host clipboard.
pub fn copy_to_clipboard(text: &str) -> Result<(), AppError> {
    if cfg!(test) {
        return Ok(());
    }
    arboard::Clipboard::new()
        .map_err(|e| AppError::Clipboard(e.to_string()))?
        .set_text(text.to_string())
        .map_err(|e| AppError::Clipboard(e.to_string()))
}
