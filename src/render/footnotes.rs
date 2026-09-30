//! Footnote section measurement and rendering at the document tail.

use ratatui::{buffer::Buffer, layout::Rect, text::Line};

use crate::domain::{Block, Document, FootnoteId, Heading};

use super::blocks::{render_stacked, rows_below};
use super::context::RenderContext;
use super::inline::{footnote_marker_style, inlines_to_wrapped_lines};
use super::measure::{BLOCK_GAP, measure_block_height};
use unicode_width::UnicodeWidthStr;

pub(crate) fn measure_footnotes_height(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
) -> usize {
    if document.footnote_order.is_empty() || width == 0 {
        return 0;
    }
    let mut total = 1usize;
    for &footnote_id in &document.footnote_order {
        total += measure_footnote_entry_height(document, footnote_id, width, ctx);
    }
    total
}

fn measure_footnote_entry_height(
    document: &Document,
    footnote_id: FootnoteId,
    width: u16,
    ctx: &RenderContext,
) -> usize {
    let Some(def) = document.footnotes.get(footnote_id.0) else {
        return 0;
    };
    let inner_width = footnote_inner_width(document, footnote_id, width);
    if def.content.is_empty() {
        return 1;
    }
    def.content
        .iter()
        .map(|block| measure_block_height(block, inner_width, ctx))
        .sum::<usize>()
        .max(1)
}

/// Render the footnotes section (a leading blank row, then one entry per footnote)
/// into `area`, the rows below the body.
pub(crate) fn render_footnotes_section(
    document: &Document,
    area: Rect,
    buf: &mut Buffer,
    ctx: &RenderContext,
    line_offset: usize,
) {
    let mut row = 1usize;
    for &footnote_id in &document.footnote_order {
        let entry_area = rows_below(area, row);
        if entry_area.height == 0 {
            break;
        }
        render_footnote_entry(
            document,
            footnote_id,
            entry_area,
            buf,
            ctx,
            line_offset + row,
        );
        row += measure_footnote_entry_height(document, footnote_id, area.width, ctx);
    }
}

fn footnote_inner_width(document: &Document, footnote_id: FootnoteId, width: u16) -> u16 {
    (width as usize)
        .saturating_sub(footnote_marker_width(document, footnote_id))
        .max(1) as u16
}

fn render_footnote_entry(
    document: &Document,
    footnote_id: FootnoteId,
    area: Rect,
    buf: &mut Buffer,
    ctx: &RenderContext,
    line_offset: usize,
) {
    let Some(def) = document.footnotes.get(footnote_id.0) else {
        return;
    };
    let marker = footnote_marker_label(document, footnote_id);
    let marker_width = marker.width();
    buf.set_stringn(
        area.x,
        area.y,
        &marker,
        marker_width,
        footnote_marker_style(ctx),
    );
    let body = Rect {
        x: area.x + marker_width as u16,
        width: footnote_inner_width(document, footnote_id, area.width),
        ..area
    };
    render_stacked(&def.content, body, buf, ctx, line_offset, 0);
}

/// Plain text of each footnote-section row after the leading blank row, laid out
/// exactly as [`render_footnotes_section`] wraps it.
pub(crate) fn footnote_searchable_lines(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
) -> Vec<String> {
    let mut out = Vec::new();
    for &footnote_id in &document.footnote_order {
        let Some(def) = document.footnotes.get(footnote_id.0) else {
            continue;
        };
        let inner_width = footnote_inner_width(document, footnote_id, width);
        let mut entry = Vec::new();
        for block in &def.content {
            let mut lines = block_searchable_lines(block, inner_width, ctx);
            lines.resize(measure_block_height(block, inner_width, ctx), String::new());
            entry.extend(lines);
        }
        entry.resize(
            measure_footnote_entry_height(document, footnote_id, width, ctx),
            String::new(),
        );
        out.extend(entry);
    }
    out
}

fn block_searchable_lines(block: &Block, width: u16, ctx: &RenderContext) -> Vec<String> {
    match block {
        Block::Paragraph(inlines) => {
            inlines_to_wrapped_lines(inlines, ctx, ctx.theme.text, 0, width as usize)
                .into_iter()
                .map(|(_, line)| line_plain_text(&line))
                .collect()
        }
        Block::Heading(Heading { content, .. }) => {
            inlines_to_wrapped_lines(content, ctx, ctx.theme.text, 0, width as usize)
                .into_iter()
                .map(|(_, line)| line_plain_text(&line))
                .collect()
        }
        _ => Vec::new(),
    }
}

fn line_plain_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

fn footnote_marker_label(document: &Document, footnote_id: FootnoteId) -> String {
    let display = document
        .footnote_order
        .iter()
        .position(|&id| id == footnote_id)
        .map(|pos| pos + 1)
        .unwrap_or(footnote_id.0 + 1);
    format!("[{display}] ")
}

fn footnote_marker_width(document: &Document, footnote_id: FootnoteId) -> usize {
    footnote_marker_label(document, footnote_id).width()
}

/// Render a footnote definition into `area`. Returns false when the footnote is missing.
pub fn render_footnote_preview(
    document: &Document,
    footnote_id: FootnoteId,
    area: Rect,
    buf: &mut Buffer,
    ctx: &RenderContext,
) -> bool {
    if area.width == 0 || area.height == 0 {
        return true;
    }
    let Some(def) = document.footnotes.get(footnote_id.0) else {
        return false;
    };
    if def.content.is_empty() {
        return true;
    }

    render_stacked(&def.content, area, buf, ctx, 0, BLOCK_GAP);
    true
}

pub fn footnote_preview_title(document: &Document, footnote_id: FootnoteId) -> String {
    let display = document
        .footnote_order
        .iter()
        .position(|&id| id == footnote_id)
        .map(|pos| pos + 1)
        .unwrap_or(footnote_id.0 + 1);
    let label = document
        .footnotes
        .get(footnote_id.0)
        .map(|def| def.label.as_str())
        .unwrap_or("");
    if label.is_empty() {
        format!("[{display}]")
    } else {
        format!("[{display}] {label}")
    }
}
