//! Rendering: domain model -> ratatui widgets.

mod blocks;
mod cache;
pub(crate) mod checklist;
mod context;
mod headings;
mod image;
mod inline;
mod links;
mod list_marker;
mod measure;
mod mermaid;
mod preview_cache;
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
pub use headings::{
    collect_heading_offsets, find_heading_line_by_anchor, next_heading_line, prev_heading_line,
    slugify_heading,
};
pub(crate) use image::render_markdown_image_from_src;
pub(crate) use image::{PREVIEW_POPUP_PERCENT, centered_rect};
pub use links::{collect_visible_links, find_link_line_offset};
pub use measure::{measure_block_height, measure_document_height};
pub use mermaid::RenderedDocument;
pub(crate) use mermaid::render_mermaid_from_source;
pub use preview_cache::PreviewRenderCache;
pub use search::find_search_matches;
pub use syntax::SyntaxAssets;
pub use theme::Theme;
pub use widget::MarkdownWidget;
