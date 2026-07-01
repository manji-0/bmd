//! Markdown parser: pulldown-cmark events -> DTO.

mod block;
mod callout;
mod html;
mod inline;

use pulldown_cmark::{Options, Parser};

use crate::parse::dto::ParsedDocument;
use crate::parse::error::ParseError;
use crate::parse::format::MarkupFormat;

use block::ParserState;

pub(crate) fn syntax_error(message: impl Into<String>) -> ParseError {
    ParseError::syntax(MarkupFormat::Markdown, message)
}

/// Parse CommonMark (with tables) into a [`ParsedDocument`].
pub fn parse(content: &str) -> Result<ParsedDocument, ParseError> {
    let parser = Parser::new_ext(
        content,
        Options::all()
            | Options::ENABLE_GFM
            | Options::ENABLE_HEADING_ATTRIBUTES
            | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
            | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS,
    );
    let mut state = ParserState::new(parser);
    state.run()?;
    Ok(state.into_document())
}
