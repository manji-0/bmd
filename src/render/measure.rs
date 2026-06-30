//! Document height measurement.

use unicode_width::UnicodeWidthStr;

use crate::domain::{Block, CodeBlock, Document, Heading, Inline, List, Table};

use super::context::RenderContext;
use super::inline::{heading_styles, inlines_to_wrapped_lines};
use super::table::{allocate_column_widths, wrap_cell_inlines};

/// Total logical height of the whole document, including one-row gaps between
/// consecutive block-level elements.
///
/// This must stay in lock-step with `MarkdownWidget::render`: any change to how
/// blocks are laid out must be reflected here, otherwise scrolling will truncate
/// or overshoot the document.
pub fn measure_document_height(document: &Document, width: u16, ctx: &RenderContext) -> usize {
    if width == 0 {
        return 0;
    }
    document
        .blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| {
            let gap = if idx == 0 { 0 } else { 1 };
            measure_block_height(block, idx, width, ctx) + gap
        })
        .sum()
}

/// Number of logical rows a block occupies at the given width.
///
/// This must stay in lock-step with the `render_*` functions: any change to how a
/// block is drawn must be reflected here, otherwise scrolling will truncate or
/// overshoot the document.
pub fn measure_block_height(
    block: &Block,
    _block_idx: usize,
    width: u16,
    ctx: &RenderContext,
) -> usize {
    if width == 0 {
        return 0;
    }
    match block {
        Block::Heading(h) => measure_heading_height(h, width, ctx),
        Block::Paragraph(inlines) => measure_paragraph_height(inlines, width, ctx),
        Block::CodeBlock(cb) => measure_code_block_height(cb, width),
        Block::BlockQuote(blocks) => measure_blockquote_height(blocks, width, ctx),
        Block::List(list) => measure_list_height(list, width, ctx),
        Block::Table(table) => measure_table_height(table, width, ctx),
        Block::Rule => 1,
    }
}

fn measure_paragraph_height(inlines: &[Inline], width: u16, ctx: &RenderContext) -> usize {
    inlines_to_wrapped_lines(inlines, ctx, ctx.theme.text, 0, width as usize)
        .len()
        .max(1)
}

fn measure_heading_height(heading: &Heading, width: u16, ctx: &RenderContext) -> usize {
    let (style, _prefix_style) = heading_styles(heading.level, ctx.theme);
    let prefix_width = heading.level.prefix().width();
    let total_width = width as usize;
    // Keep this condition in sync with render_heading.
    let content_width = if total_width > prefix_width + 1 {
        total_width.saturating_sub(prefix_width)
    } else {
        total_width
    };
    inlines_to_wrapped_lines(&heading.content, ctx, style, 0, content_width.max(1))
        .len()
        .max(1)
}

pub(crate) fn measure_code_block_height(cb: &CodeBlock, width: u16) -> usize {
    let _ = width; // width only matters for rendering, not for height.
    cb.logical_height()
}

fn measure_blockquote_height(blocks: &[Block], width: u16, ctx: &RenderContext) -> usize {
    let inner_width = (width as usize).saturating_sub(2).max(1) as u16;
    let content_height: usize = blocks
        .iter()
        .map(|b| measure_block_height(b, usize::MAX, inner_width, ctx))
        .sum();
    content_height.saturating_add(1)
}

fn measure_list_height(list: &List, width: u16, ctx: &RenderContext) -> usize {
    let w = width as usize;
    let mut total = 0usize;
    for (idx, item) in list.items.iter().enumerate() {
        let marker_width = if list.ordered {
            format!("{}.", idx + 1).width() + 1
        } else {
            2
        };
        let inner_width = w.saturating_sub(marker_width).max(1) as u16;
        if item.content.is_empty() {
            total += 1;
        } else {
            total += item
                .content
                .iter()
                .map(|b| measure_block_height(b, usize::MAX, inner_width, ctx))
                .sum::<usize>();
        }
    }
    total.max(1)
}

fn measure_table_height(table: &Table, width: u16, ctx: &RenderContext) -> usize {
    let col_count = table
        .headers
        .len()
        .max(table.rows.first().map(|r| r.len()).unwrap_or(0));
    if col_count == 0 {
        return 1;
    }
    let widths = allocate_column_widths(table, width as usize);
    let header_height = table
        .headers
        .iter()
        .enumerate()
        .map(|(i, cell)| {
            wrap_cell_inlines(
                cell,
                widths.get(i).copied().unwrap_or(1),
                ctx.theme.table_header,
                ctx,
                0,
            )
            .len()
        })
        .max()
        .unwrap_or(1);
    let body_height = table
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, cell)| {
                    wrap_cell_inlines(
                        cell,
                        widths.get(i).copied().unwrap_or(1),
                        ctx.theme.table_cell,
                        ctx,
                        0,
                    )
                    .len()
                })
                .max()
                .unwrap_or(1)
        })
        .sum::<usize>();
    // Top border + header + separator + body rows + bottom border.
    1 + header_height + 1 + body_height + 1
}
