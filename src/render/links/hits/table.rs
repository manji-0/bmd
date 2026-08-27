//! Table cell hit collection and first-line location.

use ratatui::style::Style;
use ratatui::text::Line;
use unicode_width::UnicodeWidthStr;

use crate::domain::{Alignment, Inline, LinkId, Table};

use super::super::super::context::RenderContext;
use super::super::super::inline::inlines_to_wrapped_lines;
use super::super::super::table::{
    allocate_column_widths, cell_padding, column_alignment, wrap_cell_inlines,
};
use super::super::locate::{first_link_line_in_wrapped, inlines_contain_link};
use super::inline::{collect_inline_footnote_hits_filtered, collect_inline_link_hits_filtered};
use super::{FootnoteHit, LinkHit};

struct TableRowLinkContext<'a> {
    widths: &'a [usize],
    alignments: &'a [Alignment],
    style: Style,
    ctx: &'a RenderContext<'a>,
    base_x: usize,
    row_start_line: usize,
}

pub(super) fn collect_table_link_hits(
    table: &Table,
    width: u16,
    base_x: usize,
    ctx: &RenderContext,
    hits: &mut Vec<LinkHit>,
    line_offset: &mut usize,
) {
    let col_count = table.column_count();
    if col_count == 0 || width < 3 {
        *line_offset += 1;
        return;
    }

    let widths = allocate_column_widths(table, width as usize);

    // Top border.
    *line_offset += 1;

    // Header rows.
    let header_height = table_row_height(&table.headers, &widths, ctx.theme.table_header, ctx);
    collect_table_row_link_hits(
        &table.headers,
        &TableRowLinkContext {
            widths: &widths,
            alignments: &table.alignments,
            style: ctx.theme.table_header,
            ctx,
            base_x,
            row_start_line: *line_offset,
        },
        hits,
    );
    *line_offset += header_height;

    // Separator.
    *line_offset += 1;

    // Body rows.
    for row in &table.rows {
        let row_height = table_row_height(row, &widths, ctx.theme.table_cell, ctx);
        collect_table_row_link_hits(
            row,
            &TableRowLinkContext {
                widths: &widths,
                alignments: &table.alignments,
                style: ctx.theme.table_cell,
                ctx,
                base_x,
                row_start_line: *line_offset,
            },
            hits,
        );
        *line_offset += row_height;
    }

    // Bottom border.
    *line_offset += 1;
}

pub(super) fn collect_table_footnote_hits(
    table: &Table,
    width: u16,
    base_x: usize,
    ctx: &RenderContext,
    hits: &mut Vec<FootnoteHit>,
    line_offset: &mut usize,
) {
    let col_count = table.column_count();
    if col_count == 0 || width < 3 {
        *line_offset += 1;
        return;
    }

    let widths = allocate_column_widths(table, width as usize);

    *line_offset += 1;

    let header_height = table_row_height(&table.headers, &widths, ctx.theme.table_header, ctx);
    collect_table_row_footnote_hits(
        &table.headers,
        &TableRowLinkContext {
            widths: &widths,
            alignments: &table.alignments,
            style: ctx.theme.table_header,
            ctx,
            base_x,
            row_start_line: *line_offset,
        },
        hits,
    );
    *line_offset += header_height;

    *line_offset += 1;

    for row in &table.rows {
        let row_height = table_row_height(row, &widths, ctx.theme.table_cell, ctx);
        collect_table_row_footnote_hits(
            row,
            &TableRowLinkContext {
                widths: &widths,
                alignments: &table.alignments,
                style: ctx.theme.table_cell,
                ctx,
                base_x,
                row_start_line: *line_offset,
            },
            hits,
        );
        *line_offset += row_height;
    }

    *line_offset += 1;
}

fn table_row_height(
    cells: &[Vec<Inline>],
    widths: &[usize],
    style: Style,
    ctx: &RenderContext,
) -> usize {
    widths
        .iter()
        .enumerate()
        .map(|(i, width)| {
            cells
                .get(i)
                .map(|cell| wrap_cell_inlines(cell, *width, style, ctx, 0).len())
                .unwrap_or(1)
        })
        .max()
        .unwrap_or(1)
        .max(1)
}

fn collect_table_row_link_hits(
    cells: &[Vec<Inline>],
    row: &TableRowLinkContext<'_>,
    hits: &mut Vec<LinkHit>,
) {
    let max_height = table_row_height(cells, row.widths, row.style, row.ctx);

    for row_line in 0..max_height {
        let line = row.row_start_line + row_line;
        let mut col_x = row.base_x + 1;
        for (i, width) in row.widths.iter().enumerate() {
            let cell = cells.get(i);
            let wrapped = cell
                .map(|c| wrap_cell_inlines(c, *width, row.style, row.ctx, row.row_start_line))
                .unwrap_or_else(|| vec![Line::from(" ")]);
            let line_content = wrapped
                .get(row_line)
                .cloned()
                .unwrap_or_else(|| Line::from(" "));
            let rendered_width = line_content
                .spans
                .iter()
                .map(|s| s.content.width())
                .sum::<usize>();
            let alignment = column_alignment(row.alignments, i);
            let (pad_left, _) = cell_padding(alignment, rendered_width, *width);
            let content_x = col_x + 1 + pad_left;

            if let Some(cell_inlines) = cell {
                collect_inline_link_hits_filtered(
                    cell_inlines,
                    *width,
                    content_x,
                    row.row_start_line,
                    Some(line),
                    hits,
                );
            }

            col_x += 1 + width + 1 + 1;
        }
    }
}

fn collect_table_row_footnote_hits(
    cells: &[Vec<Inline>],
    row: &TableRowLinkContext<'_>,
    hits: &mut Vec<FootnoteHit>,
) {
    let max_height = table_row_height(cells, row.widths, row.style, row.ctx);

    for row_line in 0..max_height {
        let line = row.row_start_line + row_line;
        let mut col_x = row.base_x + 1;
        for (i, width) in row.widths.iter().enumerate() {
            let cell = cells.get(i);
            let wrapped = cell
                .map(|c| wrap_cell_inlines(c, *width, row.style, row.ctx, row.row_start_line))
                .unwrap_or_else(|| vec![Line::from(" ")]);
            let line_content = wrapped
                .get(row_line)
                .cloned()
                .unwrap_or_else(|| Line::from(" "));
            let rendered_width = line_content
                .spans
                .iter()
                .map(|s| s.content.width())
                .sum::<usize>();
            let alignment = column_alignment(row.alignments, i);
            let (pad_left, _) = cell_padding(alignment, rendered_width, *width);
            let content_x = col_x + 1 + pad_left;

            if let Some(cell_inlines) = cell {
                collect_inline_footnote_hits_filtered(
                    cell_inlines,
                    *width,
                    content_x,
                    row.row_start_line,
                    Some(line),
                    hits,
                );
            }

            col_x += 1 + width + 1 + 1;
        }
    }
}

pub fn table_first_link_line(
    table: &Table,
    width: u16,
    ctx: &RenderContext,
    link_id: LinkId,
) -> Option<usize> {
    let col_count = table.column_count();
    if col_count == 0 || width < 3 {
        return None;
    }

    let widths = allocate_column_widths(table, width as usize);
    let mut line = 0usize;

    // Top border.
    line += 1;

    // Header rows.
    if let Some(local) = first_link_line_in_table_row(
        &table.headers,
        &widths,
        &table.alignments,
        ctx.theme.table_header,
        ctx,
        link_id,
    ) {
        return Some(line + local);
    }
    line += table_row_height(&table.headers, &widths, ctx.theme.table_header, ctx);

    // Separator.
    line += 1;

    // Body rows.
    for row in &table.rows {
        if let Some(local) = first_link_line_in_table_row(
            row,
            &widths,
            &table.alignments,
            ctx.theme.table_cell,
            ctx,
            link_id,
        ) {
            return Some(line + local);
        }
        line += table_row_height(row, &widths, ctx.theme.table_cell, ctx);
    }

    None
}

fn first_link_line_in_table_row(
    cells: &[Vec<Inline>],
    widths: &[usize],
    _alignments: &[Alignment],
    style: Style,
    ctx: &RenderContext,
    link_id: LinkId,
) -> Option<usize> {
    let mut first: Option<usize> = None;
    for (i, cell) in cells.iter().enumerate() {
        if !inlines_contain_link(cell, link_id) {
            continue;
        }
        let width = widths.get(i).copied().unwrap_or(1);
        let wrapped = inlines_to_wrapped_lines(cell, ctx, style, 0, width);
        if let Some(local) = first_link_line_in_wrapped(&wrapped, cell, link_id) {
            first = Some(match first {
                Some(f) => f.min(local),
                None => local,
            });
        }
    }
    first
}
