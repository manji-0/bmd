//! Rendering: domain model -> ratatui widgets.

mod blocks;
mod cache;
pub(crate) mod checklist;
mod context;
mod image;
mod inline;
mod links;
mod list_marker;
mod measure;
mod mermaid;
mod search;
mod search_state;
pub(crate) mod subpixel;
mod syntax;
mod table;
mod theme;
mod widget;

#[cfg(test)]
mod tests;

pub use cache::{CachedMarkdownView, DocumentRenderCache};
pub use context::RenderContext;
pub(crate) use image::{PREVIEW_POPUP_PERCENT, render_floating_image};
pub use links::find_link_line_offset;
pub use measure::{measure_block_height, measure_document_height};
pub use mermaid::RenderedDocument;
pub use search::find_search_matches;
pub use syntax::SyntaxAssets;
pub use theme::Theme;
pub use widget::MarkdownWidget;
