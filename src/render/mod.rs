//! Rendering: domain model -> ratatui widgets.

mod blocks;
mod cache;
mod callout;
pub(crate) mod checklist;
mod context;
mod footnotes;
mod headings;
mod image;
mod inline;
mod links;
mod list_marker;
mod math;
mod measure;
mod mermaid;
mod preview_cache;
mod search;
mod search_state;
mod selection;
pub(crate) mod subpixel;
mod syntax;
mod table;
mod theme;
mod widget;

#[cfg(test)]
mod tests;

pub use cache::{CachedMarkdownView, DocumentRenderCache};
pub use context::RenderContext;
pub use footnotes::{footnote_preview_title, render_footnote_preview};
pub use headings::{
    HeadingOffsetCache, find_heading_line_by_anchor, next_heading_line, prev_heading_line,
};
pub(crate) use image::render_markdown_image_from_src;
pub(crate) use image::{
    PREVIEW_POPUP_PERCENT, centered_rect, render_floating_image, resolve_image_path,
};
pub use links::{collect_visible_links, collect_visible_nav_targets, link_at_click};
pub use measure::{measure_block_height, measure_document_height};
pub use mermaid::RenderedDocument;
pub(crate) use mermaid::{render_mermaid_from_source, save_mermaid_png};
pub use preview_cache::PreviewRenderCache;
pub use search::find_search_matches;
pub use selection::{extract_selected_text, paint_selection_overlay};
pub use syntax::SyntaxAssets;
pub use theme::{DEFAULT_PRESET, Theme};
