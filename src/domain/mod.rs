//! Domain model for the TUI markdown viewer.
//!
//! Invalid states and invalid transitions are modelled out of the type system where practical:
//! - `LinkUrl` cannot be empty.
//! - `TerminalSize` cannot have zero dimensions.
//! - `ViewState` transitions consume `self`, so the old state cannot be reused.
//! - `LinkJumpStack` stores priors fixed at link jumps; live current state stays outside.
//! - `PreviewLoadSession` tracks per-link mermaid/image preview load phases.

mod callout;
mod checklist;
mod document_generation;
mod document_link;
mod document_prefetch;
mod front_matter;
mod link;
mod link_jump_stack;
mod markdown;
mod marks;
mod mode;
mod nav_stack;
mod nav_target;
mod navigation;
mod navigation_limits;
mod preview_load;
mod slug;
mod text_selection;
mod view;

#[cfg(test)]
mod tests;

pub use callout::{Callout, CalloutKind};
pub use checklist::{ChecklistId, ChecklistState, ChecklistStyle};
pub use document_link::{
    DocumentFs, document_link_path_part, is_remote_link_dest, resolve_document_path,
};
pub use document_prefetch::{
    DocumentPrefetchCompletion, DocumentPrefetchError, DocumentPrefetchSession,
    DocumentPrefetchSessionSnapshot, DocumentPrefetchSpawnRequest, PrefetchedDocument,
};
pub use front_matter::{FrontMatter, FrontMatterKind};
pub use link::{DocumentError, Link, LinkId, LinkKind, LinkUrl, LinkUrlError};
pub use link_jump_stack::{LinkJumpStack, LinkJumpStackFull, PriorAtLinkJump};
pub use markdown::{
    Alignment, Block, CodeBlock, DefinitionItem, DefinitionList, Document, FootnoteDefinition,
    FootnoteId, Heading, HeadingLevel, Inline, List, ListItem, MathBlock, MermaidDiagram, Table,
};
pub use marks::{MarkName, Marks};
pub use mode::{NormalSearch, PreviewKind, UiMode};
pub use nav_stack::{AnchorStackEmpty, FixedScrollPrior, NavStack};
pub use nav_target::NavTarget;
pub use navigation::{AnchorIdle, NavBackPlan, NavResetPlan, plan_back, plan_reset};
pub use navigation_limits::{
    AnchorStackFull, DOCUMENT_STACK_MAX_LAYERS, DocumentStackFull, anchor_stack_limit_message,
    document_stack_limit_message,
};
pub use preview_load::{
    ImageSource, MermaidSource, PreviewCompletion, PreviewLoadError, PreviewLoadSession,
    PreviewLoadStatus, PreviewSessionSnapshot, PreviewSource, PreviewSpawnRequest,
    mermaid_diagram_index,
};
pub use slug::{anchor_href, normalize_anchor_slug, slugify_heading};
pub use text_selection::{TextPoint, TextSelection};
pub use view::{SearchDirection, SearchMatch, TerminalSize, TerminalSizeError, ViewState};

#[cfg(test)]
pub use mode::ActiveSearch;
#[cfg(test)]
pub use navigation_limits::ANCHOR_STACK_MAX_FRAMES;
#[cfg(test)]
pub use view::{SearchQuery, SearchQueryError, SearchTransitionError};
