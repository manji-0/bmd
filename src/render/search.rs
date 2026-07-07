//! Search match discovery.

use ratatui::{style::Style, text::Line};
use unicode_width::UnicodeWidthStr;

use super::list_marker::list_marker_width_at;

use crate::domain::{Block, CodeBlock, DefinitionList, Document, Inline, List, SearchMatch, Table};

use super::callout::callout_inner_width;
use super::context::RenderContext;
use super::footnotes::footnote_searchable_lines;
use super::inline::{heading_styles, inlines_to_wrapped_lines};
use super::measure::measure_block_height;
use super::table::wrap_cell_inlines;

/// Find all logical lines in the rendered document that contain `query`.
///
/// Line offsets follow the same layout as [`MarkdownWidget`] rendering so that
/// selected-match highlighting lines up with navigation targets.
///
/// The returned matches are sorted by ascending line offset and can be passed
/// to `ViewState::confirm_search`.
pub fn find_search_matches(
    document: &Document,
    width: u16,
    query: &str,
    ctx: &RenderContext,
) -> Vec<SearchMatch> {
    if width == 0 || query.is_empty() {
        return Vec::new();
    }
    let query_lower = query.to_lowercase();
    let mut match_index = 0usize;
    collect_searchable_lines(document, width, ctx)
        .into_iter()
        .filter_map(|(offset, text)| {
            if text.to_lowercase().contains(&query_lower) {
                let m = SearchMatch::new(offset, match_index);
                match_index += 1;
                Some(m)
            } else {
                None
            }
        })
        .collect()
}

fn collect_searchable_lines(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut line_offset: usize = 0;
    for (idx, block) in document.blocks.iter().enumerate() {
        let gap = if idx == 0 { 0 } else { 1 };
        let block_lines = block_searchable_lines(block, width, ctx);
        let block_height = measure_block_height(block, idx, width, ctx).max(block_lines.len());
        for (i, line) in block_lines.iter().enumerate().take(block_height) {
            out.push((line_offset + i, line.clone()));
        }
        // Keep in sync with `MarkdownWidget::render`: gap rows trail block content.
        line_offset += block_height + gap;
    }

    if !document.footnote_order.is_empty() {
        line_offset += 1;
        let footnote_lines = footnote_searchable_lines(document, width, ctx);
        let footnote_body_height =
            super::footnotes::measure_footnotes_height(document, width, ctx).saturating_sub(1);
        for (i, line) in footnote_lines
            .iter()
            .enumerate()
            .take(footnote_body_height.max(footnote_lines.len()))
        {
            out.push((line_offset + i, line.clone()));
        }
    }

    out
}

fn line_plain_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

fn block_searchable_lines(block: &Block, width: u16, ctx: &RenderContext) -> Vec<String> {
    match block {
        Block::Heading(heading) => {
            let (style, _) = heading_styles(heading.level, ctx.theme);
            let prefix_width = heading.level.prefix().width();
            let content_width = if (width as usize) > prefix_width + 1 {
                (width as usize).saturating_sub(prefix_width)
            } else {
                width as usize
            };
            inlines_to_wrapped_lines(&heading.content, ctx, style, 0, content_width.max(1))
                .into_iter()
                .map(|(_, line)| line_plain_text(&line))
                .collect()
        }
        Block::Paragraph(inlines) => {
            inlines_to_wrapped_lines(inlines, ctx, ctx.theme.text, 0, width as usize)
                .into_iter()
                .map(|(_, line)| line_plain_text(&line))
                .collect()
        }
        Block::CodeBlock(cb) => code_block_searchable_lines(cb),
        Block::MathBlock(math) => vec![math.content.clone()],
        Block::BlockQuote(blocks) => {
            let inner_width = (width as usize).saturating_sub(2).max(1) as u16;
            let mut lines = Vec::new();
            for child in blocks {
                lines.extend(block_searchable_lines(child, inner_width, ctx));
            }
            // `measure_blockquote_height` adds one logical row of padding.
            lines.push(String::new());
            lines
        }
        Block::Callout(callout) => {
            let inner_width = callout_inner_width(callout, width);
            let mut lines = vec![callout.header_label()];
            for child in &callout.body {
                lines.extend(block_searchable_lines(child, inner_width, ctx));
            }
            lines
        }
        Block::List(list) => list_searchable_lines(list, width, ctx),
        Block::DefinitionList(list) => definition_list_searchable_lines(list, width, ctx),
        Block::Table(table) => table_searchable_lines(table, width, ctx),
        Block::Rule => Vec::new(),
    }
}

fn code_block_searchable_lines(cb: &CodeBlock) -> Vec<String> {
    let mut lines = Vec::new();
    let label = cb
        .language
        .as_ref()
        .map(|l| format!(" {l} "))
        .unwrap_or_else(|| " code ".to_string());
    lines.push(label);
    if cb.content.is_empty() {
        lines.push(String::new());
    } else {
        for line in cb.content.lines() {
            lines.push(line.to_string());
        }
    }
    lines
}

fn list_searchable_lines(list: &List, width: u16, ctx: &RenderContext) -> Vec<String> {
    let mut lines = Vec::new();
    for (idx, item) in list.items.iter().enumerate() {
        let marker_width = list_marker_width_at(list, idx, item, ctx.checklist_state);
        let inner_width = (width as usize).saturating_sub(marker_width).max(1) as u16;
        if item.content.is_empty() {
            lines.push(String::new());
            continue;
        }
        for child in &item.content {
            lines.extend(block_searchable_lines(child, inner_width, ctx));
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn definition_list_searchable_lines(
    list: &DefinitionList,
    width: u16,
    ctx: &RenderContext,
) -> Vec<String> {
    let inner_width = (width as usize).saturating_sub(2).max(1) as u16;
    let mut lines = Vec::new();
    for item in &list.items {
        if !item.term.is_empty() {
            lines.extend(
                inlines_to_wrapped_lines(&item.term, ctx, ctx.theme.text, 0, width as usize)
                    .into_iter()
                    .map(|(_, line)| line_plain_text(&line)),
            );
        }
        for definition in &item.definitions {
            for child in definition {
                lines.extend(block_searchable_lines(child, inner_width, ctx));
            }
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn table_searchable_lines(table: &Table, width: u16, ctx: &RenderContext) -> Vec<String> {
    let col_count = table
        .headers
        .len()
        .max(table.rows.first().map(|r| r.len()).unwrap_or(0));
    if col_count == 0 {
        return Vec::new();
    }
    let widths = table.allocate_column_widths(width as usize);
    let mut lines = Vec::new();

    // `measure_table_height` counts: top border + header + separator + body + bottom border.
    lines.push(String::new());
    lines.extend(render_table_search_lines(
        &table.headers,
        &widths,
        ctx.theme.table_header,
        ctx,
    ));
    lines.push(String::new());
    for row in &table.rows {
        lines.extend(render_table_search_lines(
            row,
            &widths,
            ctx.theme.table_cell,
            ctx,
        ));
    }
    lines.push(String::new());

    lines
}

fn render_table_search_lines(
    cells: &[Vec<Inline>],
    widths: &[usize],
    style: Style,
    ctx: &RenderContext,
) -> Vec<String> {
    let col_count = widths.len();
    let mut wrapped_columns: Vec<Vec<String>> = Vec::with_capacity(col_count);
    for (i, width) in widths.iter().enumerate() {
        let wrapped = cells
            .get(i)
            .map(|cell| wrap_cell_inlines(cell, *width, style, ctx, 0))
            .unwrap_or_else(|| vec![Line::from(" ")]);
        wrapped_columns.push(
            wrapped
                .into_iter()
                .map(|line| line_plain_text(&line))
                .collect(),
        );
    }
    let max_height = wrapped_columns
        .iter()
        .map(|c| c.len())
        .max()
        .unwrap_or(1)
        .max(1);

    let mut lines = Vec::with_capacity(max_height);
    for row_line in 0..max_height {
        let mut row_text = String::new();
        for (i, _width) in widths.iter().enumerate() {
            let cell = wrapped_columns
                .get(i)
                .and_then(|col| col.get(row_line))
                .map(|s| s.as_str())
                .unwrap_or(" ");
            if !row_text.is_empty() {
                row_text.push(' ');
            }
            row_text.push_str(cell);
        }
        lines.push(row_text);
    }
    lines
}
