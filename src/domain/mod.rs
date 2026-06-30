//! Domain model for the TUI markdown viewer.
//!
//! Invalid states and invalid transitions are modelled out of the type system where practical:
//! - `LinkUrl` cannot be empty.
//! - `TerminalSize` cannot have zero dimensions.
//! - `ViewState` transitions consume `self`, so the old state cannot be reused.

mod link;
mod markdown;
mod view;

#[cfg(test)]
mod tests;

pub use link::{DocumentError, Link, LinkId, LinkUrl, LinkUrlError};
pub use markdown::{
    Alignment, Block, CodeBlock, Document, Heading, HeadingLevel, Inline, List, ListItem,
    MermaidDiagram, Table,
};
pub use view::{
    Scroll, SearchDirection, SearchMatch, SearchQuery, SearchQueryError, SearchState, TerminalSize,
    TerminalSizeError, ViewState,
};
