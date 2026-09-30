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
pub fn parse_with_path(path: Option<&Path>, content: &str) -> Result<Document, AppError> {
    let format = MarkupFormat::detect(path, content);
    parse_document(format, content)
}

/// Parse markup content into the shared DTO.
pub fn parse_dto(format: MarkupFormat, content: &str) -> Result<ParsedDocument, ParseError> {
    match format {
        MarkupFormat::Markdown => markdown::parse(content),
        MarkupFormat::Rest => rst::parse(content),
        MarkupFormat::AsciiDoc => asciidoc::parse(content),
    }
}

/// Parse CommonMark content; test shorthand for [`parse_document`].
#[cfg(test)]
pub fn parse(content: &str) -> Result<Document, AppError> {
    parse_document(MarkupFormat::Markdown, content)
}
