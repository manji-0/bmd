//! Rendering: domain model -> ratatui widgets.

mod blocks;
mod cache;
mod context;
mod image;
mod inline;
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
pub use measure::{measure_block_height, measure_document_height};
pub use mermaid::RenderedDocument;
pub use search::find_search_matches;
pub use syntax::SyntaxAssets;
pub use theme::Theme;
pub use widget::MarkdownWidget;
