//! Footnote section measurement and rendering at the document tail.

use ratatui::{buffer::Buffer, layout::Rect, text::Line};

use crate::domain::{Block, Document, FootnoteId, Heading};

use super::blocks::render_block;
use super::context::RenderContext;
use super::inline::{footnote_marker_style, inlines_to_wrapped_lines};
use super::measure::measure_block_height;
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
    let marker_width = footnote_marker_width(document, footnote_id);
    let inner_width = (width as usize).saturating_sub(marker_width).max(1) as u16;
    if def.content.is_empty() {
        return 1;
    }
    def.content
        .iter()
        .map(|block| measure_block_height(block, usize::MAX, inner_width, ctx))
        .sum::<usize>()
        .max(1)
}

pub(crate) fn render_footnotes_section(
    document: &Document,
    area: Rect,
    buf: &mut Buffer,
    scroll: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    if document.footnote_order.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }

    let section_height = measure_footnotes_height(document, area.width, ctx);
    if scroll >= line_offset + section_height {
        return;
    }

    let mut y = area.y;
    let max_y = area.y + area.height;
    let mut current_offset = line_offset;

    // Leading gap row between body and footnotes.
    if scroll <= current_offset && y < max_y {
        y += 1;
    }
    current_offset += 1;

    for &footnote_id in &document.footnote_order {
        let entry_height = measure_footnote_entry_height(document, footnote_id, area.width, ctx);
        if scroll >= current_offset + entry_height {
            current_offset += entry_height;
            continue;
        }

        let skip_rows = scroll.saturating_sub(current_offset);
        let visible_height = entry_height
            .saturating_sub(skip_rows)
            .min((max_y - y) as usize);
        if visible_height == 0 {
            break;
        }

        let entry_area = Rect {
            x: area.x,
            y,
            width: area.width,
            height: visible_height as u16,
        };
        render_footnote_entry(
            document,
            footnote_id,
            entry_area,
            buf,
            skip_rows,
            ctx,
            current_offset,
        );
        y += visible_height as u16;
        current_offset += entry_height;
        if y >= max_y {
            break;
        }
    }
}

fn render_footnote_entry(
    document: &Document,
    footnote_id: FootnoteId,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    let Some(def) = document.footnotes.get(footnote_id.0) else {
        return;
    };

    let marker = footnote_marker_label(document, footnote_id);
    let marker_width = marker.width();
    let marker_style = footnote_marker_style(ctx);
    let inner_width = (area.width as usize).saturating_sub(marker_width).max(1) as u16;

    if def.content.is_empty() {
        if skip_rows == 0 && area.height > 0 {
            buf.set_stringn(area.x, area.y, &marker, marker_width, marker_style);
        }
        return;
    }

    let mut y = area.y;
    let max_y = area.y + area.height;
    let mut line_offset_inner = 0usize;
    let scroll = skip_rows;
    let mut drew_marker = false;

    for block in &def.content {
        let height = measure_block_height(block, usize::MAX, inner_width, ctx);
        if line_offset_inner.saturating_add(height) <= scroll {
            line_offset_inner += height;
            continue;
        }
        let block_skip = scroll.saturating_sub(line_offset_inner);
        let block_line_offset = line_offset + line_offset_inner;
        let remaining = (max_y - y) as usize;
        let render_height = height.saturating_sub(block_skip).min(remaining);
        if render_height == 0 {
            break;
        }

        if !drew_marker && line_offset_inner + height > scroll {
            let marker_y = y + block_skip as u16;
            if marker_y < max_y {
                buf.set_stringn(area.x, marker_y, &marker, marker_width, marker_style);
            }
            drew_marker = true;
        }

        let block_area = Rect {
            x: area.x + marker_width as u16,
            y,
            width: inner_width,
            height: render_height as u16,
        };
        render_block(
            block,
            usize::MAX,
            block_area,
            buf,
            block_skip,
            ctx,
            block_line_offset,
        );
        y += render_height as u16;
        line_offset_inner += height;
        if y >= max_y {
            break;
        }
    }
}

pub(crate) fn footnote_searchable_lines(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
) -> Vec<String> {
    if document.footnote_order.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for &footnote_id in &document.footnote_order {
        let Some(def) = document.footnotes.get(footnote_id.0) else {
            continue;
        };
        for block in &def.content {
            out.extend(block_searchable_lines(block, width, ctx));
        }
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

/// First logical line offset of a footnote definition in the bottom section.
pub fn find_footnote_definition_line_offset(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
    footnote_id: FootnoteId,
) -> Option<usize> {
    if width == 0 || document.footnote_order.is_empty() {
        return None;
    }
    let body_height: usize = document
        .blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| {
            let gap = if idx == 0 { 0 } else { 1 };
            super::measure::measure_block_height(block, idx, width, ctx) + gap
        })
        .sum();
    let mut line = body_height.saturating_add(1);
    for &id in &document.footnote_order {
        if id == footnote_id {
            return Some(line);
        }
        line += measure_footnote_entry_height(document, id, width, ctx);
    }
    None
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

    let mut y = area.y;
    let max_y = area.y + area.height;
    let mut line_offset = 0usize;

    for (block_idx, block) in def.content.iter().enumerate() {
        let gap = if block_idx == 0 { 0 } else { 1 };
        if gap > 0 && y < max_y {
            y += 1;
        }
        let height = measure_block_height(block, usize::MAX, area.width, ctx);
        let visible_height = height.min((max_y - y) as usize);
        if visible_height == 0 {
            break;
        }
        let block_area = Rect {
            x: area.x,
            y,
            width: area.width,
            height: visible_height as u16,
        };
        render_block(block, usize::MAX, block_area, buf, 0, ctx, line_offset);
        y += visible_height as u16;
        line_offset += height + gap;
        if y >= max_y {
            break;
        }
    }
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
