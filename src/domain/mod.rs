//! Domain model for the TUI markdown viewer.
//!
//! Invalid states and invalid transitions are modelled out of the type system where practical:
//! - `LinkUrl` cannot be empty.
//! - `TerminalSize` cannot have zero dimensions.
//! - `ViewState` transitions consume `self`, so the old state cannot be reused.
//! - `LinkJumpStack` stores priors fixed at link jumps; live current state stays outside.
//! - `MermaidRenderSession` and `ImageRenderSession` track per-link preview phases.

mod callout;
mod checklist;
mod document_generation;
mod document_link;
mod document_prefetch;
mod front_matter;
mod image_render;
mod link;
mod link_jump_stack;
mod markdown;
mod mermaid_render;
mod mode;
mod nav_stack;
mod nav_target;
mod navigation;
mod navigation_limits;
mod preview_load;
mod text_selection;
mod view;

#[cfg(test)]
mod tests;

pub use callout::{Callout, CalloutKind};
pub use checklist::{ChecklistId, ChecklistState, ChecklistStyle};
pub use document_generation::DocumentGeneration;
pub use document_link::{
    DocumentPathError, document_link_path_part, file_modified_time, is_remote_link_dest,
    normalize_document_path, resolve_document_path,
};
pub use document_prefetch::{
    DocumentPrefetchCompletion, DocumentPrefetchCompletionApplied, DocumentPrefetchError,
    DocumentPrefetchSession, DocumentPrefetchSessionSnapshot, DocumentPrefetchSpawnRequest,
    PrefetchedDocument,
};
pub use front_matter::{FrontMatter, FrontMatterKind};
pub use image_render::{
    ImageCompletion, ImageRenderError, ImageRenderSession, ImageSessionSnapshot, ImageSource,
    ImageSpawnRequest, image_source_for_link,
};
pub use link::{DocumentError, Link, LinkId, LinkKind, LinkUrl, LinkUrlError};
pub use link_jump_stack::{LinkJumpStack, LinkJumpStackEmpty, LinkJumpStackFull, PriorAtLinkJump};
pub use markdown::{
    Alignment, Block, CodeBlock, DefinitionItem, DefinitionList, Document, FootnoteDefinition,
    FootnoteId, Heading, HeadingLevel, Inline, List, ListItem, MathBlock, MermaidDiagram, Table,
};
pub use mermaid_render::{
    MermaidCompletion, MermaidCompletionApplied, MermaidPreviewStatus, MermaidRenderError,
    MermaidRenderSession, MermaidSessionSnapshot, MermaidSource, MermaidSpawnRequest, MermaidTask,
    MermaidTaskPhase, mermaid_diagram_index, mermaid_source_for_link,
};
pub use mode::{ActiveSearch, NormalSearch, PreviewKind, UiMode};
pub use nav_stack::{AnchorStackEmpty, FixedScrollPrior, NavStack};
pub use nav_target::NavTarget;
pub use navigation::{
    AnchorIdle, NavBackPlan, NavLayer, NavResetPlan, plan_back, plan_document_back,
    plan_document_reset, plan_reset,
};
pub use navigation_limits::{
    ANCHOR_STACK_MAX_FRAMES, ANCHOR_STACK_MAX_LAYERS, AnchorStackFull, DOCUMENT_STACK_MAX_FRAMES,
    DOCUMENT_STACK_MAX_LAYERS, DocumentStackFull, anchor_stack_limit_message,
    document_stack_limit_message,
};
pub use preview_load::{
    PreviewLoadCompletionApplied, PreviewLoadPhase, PreviewLoadStatus, PreviewLoadTask,
};
pub use text_selection::{TextPoint, TextSelection};
pub use view::{
    Scroll, SearchDirection, SearchMatch, SearchQuery, SearchQueryError, SearchTransitionError,
    TerminalSize, TerminalSizeError, ViewState,
};
