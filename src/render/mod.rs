//! Rendering: domain model -> ratatui widgets.

mod blocks;
mod cache;
mod callout;
mod context;
mod document;
mod footnotes;
mod headings;
mod hits;
mod image;
mod inline;
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

#[cfg(test)]
mod invariant_tests;
#[cfg(test)]
mod tests;

pub use cache::{CachedMarkdownView, DocumentRenderCache};
pub use context::RenderContext;
pub use footnotes::{footnote_preview_title, render_footnote_preview};
pub use headings::{
    HeadingCatalogCache, find_heading_line_by_anchor, next_heading_line, prev_heading_line,
};
pub use hits::{Hit, HitTarget, hit_at, nav_targets, target_line, visible_links};
pub(crate) use image::render_markdown_image_from_src;
pub(crate) use image::{
    PREVIEW_POPUP_PERCENT, centered_rect, render_floating_image, resolve_image_path,
};
pub(crate) use measure::block_tops;
pub use measure::measure_document_height;
pub use mermaid::RenderedDocument;
pub(crate) use mermaid::{render_mermaid_from_source, save_mermaid_png};
pub use preview_cache::PreviewRenderCache;
pub use search::find_search_matches;
pub use selection::{extract_selected_text, paint_selection_overlay};
pub use syntax::SyntaxAssets;
pub use theme::{DEFAULT_PRESET, Theme};
