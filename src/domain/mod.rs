//! Domain model for the TUI markdown viewer.
//!
//! Invalid states and invalid transitions are modelled out of the type system where practical:
//! - `LinkUrl` cannot be empty.
//! - `TerminalSize` cannot have zero dimensions.
//! - `ViewState` transitions consume `self`, so the old state cannot be reused.
//! - `MermaidRenderSession` and `ImageRenderSession` track per-link preview phases.

mod checklist;
mod document_generation;
mod document_link;
mod image_render;
mod link;
mod markdown;
mod mermaid_render;
mod mode;
mod nav_stack;
mod preview_load;
mod view;

#[cfg(test)]
mod tests;

pub use checklist::{ChecklistId, ChecklistState, ChecklistStyle};
pub use document_generation::DocumentGeneration;
pub use document_link::{
    DocumentPathError, document_link_path_part, is_remote_link_dest, resolve_document_path,
};
pub use image_render::{
    ImageCompletion, ImageRenderError, ImageRenderSession, ImageSessionSnapshot, ImageSource,
    ImageSpawnRequest, image_source_for_link,
};
pub use link::{DocumentError, Link, LinkId, LinkKind, LinkUrl, LinkUrlError};
pub use markdown::{
    Alignment, Block, CodeBlock, Document, Heading, HeadingLevel, Inline, List, ListItem,
    MermaidDiagram, Table,
};
pub use mermaid_render::{
    MermaidCompletion, MermaidCompletionApplied, MermaidPreviewStatus, MermaidRenderError,
    MermaidRenderSession, MermaidSessionSnapshot, MermaidSource, MermaidSpawnRequest, MermaidTask,
    MermaidTaskPhase, mermaid_diagram_index, mermaid_source_for_link,
};
pub use mode::{NormalSearch, UiMode};
pub use nav_stack::NavStack;
pub use preview_load::{
    PreviewLoadCompletionApplied, PreviewLoadPhase, PreviewLoadStatus, PreviewLoadTask,
};
pub use view::{
    Scroll, SearchDirection, SearchMatch, SearchQuery, SearchQueryError, TerminalSize,
    TerminalSizeError, ViewState,
};
