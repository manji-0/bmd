//! Markup parsing: format-specific parsers -> DTO -> domain.

mod asciidoc;
mod autolink;
mod dto;
mod error;
mod format;
mod into_domain;
mod markdown;
mod parity;
mod rst;
mod subsup;

#[cfg(test)]
mod tests;

pub use dto::ParsedDocument;
pub use error::ParseError;
pub use format::MarkupFormat;

use std::path::Path;

use crate::domain::Document;
use crate::error::AppError;

/// Parse markup content with an explicit format into a domain [`Document`].
pub fn parse_document(format: MarkupFormat, content: &str) -> Result<Document, AppError> {
    let dto = parse_dto(format, content)?;
    dto.into_domain().map_err(AppError::from)
}

/// Parse markup using path-based (and content) format detection.
///
/// `path` is a local file: AsciiDoc `include::` may read files beneath its
/// directory. Pass `None` for content with no local home (stdin, remote).
pub fn parse_with_path(path: Option<&Path>, content: &str) -> Result<Document, AppError> {
    let format = MarkupFormat::detect(path, content);
    let include_base = path.map(|path| match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    });
    let dto = parse_dto_in(format, content, include_base)?;
    dto.into_domain().map_err(AppError::from)
}

/// Parse markup content into the shared DTO. No local files are read.
pub fn parse_dto(format: MarkupFormat, content: &str) -> Result<ParsedDocument, ParseError> {
    parse_dto_in(format, content, None)
}

/// Parse into the shared DTO, letting AsciiDoc `include::` read files beneath
/// `include_base`. With `None`, includes are not read.
fn parse_dto_in(
    format: MarkupFormat,
    content: &str,
    include_base: Option<&Path>,
) -> Result<ParsedDocument, ParseError> {
    match format {
        MarkupFormat::Markdown => markdown::parse(content),
        MarkupFormat::Rest => rst::parse(content),
        MarkupFormat::AsciiDoc => asciidoc::parse(content, include_base),
    }
}

/// Parse CommonMark content; test shorthand for [`parse_document`].
#[cfg(test)]
pub fn parse(content: &str) -> Result<Document, AppError> {
    parse_document(MarkupFormat::Markdown, content)
}
