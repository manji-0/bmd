//! Domain model for the TUI markdown viewer.
//!
//! Invalid states and invalid transitions are modelled out of the type system where practical:
//! - `LinkUrl` cannot be empty.
//! - `TerminalSize` cannot have zero dimensions.
//! - `ViewState` transitions consume `self`, so the old state cannot be reused.

mod checklist;
mod document_link;
mod link;
mod markdown;
mod mode;
mod nav_stack;
mod view;

#[cfg(test)]
mod tests;

pub use checklist::{ChecklistId, ChecklistState, ChecklistStyle};
pub use document_link::{
    DocumentPathError, document_link_path_part, is_remote_link_dest, resolve_document_path,
};
pub use link::{DocumentError, Link, LinkId, LinkKind, LinkUrl, LinkUrlError};
pub use markdown::{
    Alignment, Block, CodeBlock, Document, Heading, HeadingLevel, Inline, List, ListItem,
    MermaidDiagram, Table,
};
pub use mode::{NormalSearch, UiMode};
pub use nav_stack::NavStack;
pub use view::{
    Scroll, SearchDirection, SearchMatch, SearchQuery, SearchQueryError, TerminalSize,
    TerminalSizeError, ViewState,
};
