//! Markdown parser adapter: pulldown-cmark events -> domain model.

mod block;
mod html;
mod inline;

#[cfg(test)]
mod tests;

use pulldown_cmark::{Options, Parser};

use crate::domain::Document;
use crate::error::AppError;

use block::ParserState;

/// Parse CommonMark (with tables) into a `Document`.
pub fn parse(markdown: &str) -> Result<Document, AppError> {
    let parser = Parser::new_ext(markdown, Options::all());
    let mut state = ParserState::new(parser);
    state.run()?;
    let (blocks, links) = state.into_parts();
    Document::new(blocks, links).map_err(AppError::Document)
}
