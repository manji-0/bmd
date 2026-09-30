//! Whole-document rendering: body blocks, then the footnotes section.

use ratatui::{buffer::Buffer, layout::Rect};

use crate::domain::Document;

use super::blocks::{render_stacked, rows_below};
use super::context::RenderContext;
use super::footnotes::render_footnotes_section;
use super::measure::BLOCK_GAP;

/// Render `document` from its first row into `area`, clipping at the bottom.
pub(crate) fn render_document(
    document: &Document,
    area: Rect,
    buf: &mut Buffer,
    ctx: &RenderContext,
) {
    let body_rows = render_stacked(&document.blocks, area, buf, ctx, 0, BLOCK_GAP);
    render_footnotes_section(document, rows_below(area, body_rows), buf, ctx, body_rows);
}
