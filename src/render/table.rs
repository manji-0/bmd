//! Table layout and rendering.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::domain::{Alignment, Inline, Table};

use super::context::RenderContext;
use super::inline::inlines_to_wrapped_lines;

pub(crate) fn allocate_column_widths(table: &Table, total_width: usize) -> Vec<usize> {
    table.allocate_column_widths(total_width)
}

pub(crate) fn render_table(
    table: &Table,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    let col_count = table
        .headers
        .len()
        .max(table.rows.first().map(|r| r.len()).unwrap_or(0));
    if col_count == 0 || area.width < 3 {
        return;
    }

    let widths = allocate_column_widths(table, area.width as usize);

    // Pre-wrap all cell content, including borders, into terminal rows.
    let mut rows: Vec<(usize, Line)> = Vec::new();

    // Top border.
    rows.push((
        line_offset,
        Line::styled(
            horizontal_table_border(&widths, '┌', '┬', '┐'),
            ctx.theme.table_border,
        ),
    ));

    // Header rows (multi-line cells produce multiple terminal rows).
    let header_rows = render_table_row(
        &table.headers,
        &widths,
        &table.alignments,
        ctx.theme.table_header,
        ctx,
        line_offset + 1,
    );
    for (i, line) in header_rows.into_iter().enumerate() {
        rows.push((line_offset + 1 + i, line));
    }

    // Separator.
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
                line_offset + 1,
            )
            .len()
        })
        .max()
        .unwrap_or(1);
    let separator_offset = line_offset + 1 + header_height;
    rows.push((
        separator_offset,
        Line::styled(
            horizontal_table_border(&widths, '├', '┼', '┤'),
            ctx.theme.table_border,
        ),
    ));

    // Body rows.
    let mut body_offset = separator_offset + 1;
    for row in &table.rows {
        let row_height = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                wrap_cell_inlines(
                    cell,
                    widths.get(i).copied().unwrap_or(1),
                    ctx.theme.table_cell,
                    ctx,
                    body_offset,
                )
                .len()
            })
            .max()
            .unwrap_or(1);
        let row_lines = render_table_row(
            row,
            &widths,
            &table.alignments,
            ctx.theme.table_cell,
            ctx,
            body_offset,
        );
        for (i, line) in row_lines.into_iter().enumerate() {
            rows.push((body_offset + i, line));
        }
        body_offset += row_height;
    }

    // Bottom border.
    rows.push((
        body_offset,
        Line::styled(
            horizontal_table_border(&widths, '└', '┴', '┘'),
            ctx.theme.table_border,
        ),
    ));

    // Render directly into the buffer, skipping scrolled rows and clipping to area.
    for (row_line_offset, line) in rows.iter() {
        let row_idx = row_line_offset.saturating_sub(line_offset);
        let screen_y = area.y as usize + row_idx;
        if row_idx < skip_rows || screen_y >= (area.y + area.height) as usize {
            continue;
        }
        buf.set_line(area.x, screen_y as u16, line, area.width);
    }
}

pub(crate) fn render_table_row(
    cells: &[Vec<Inline>],
    widths: &[usize],
    alignments: &[Alignment],
    style: Style,
    ctx: &RenderContext,
    row_start_line_offset: usize,
) -> Vec<Line<'static>> {
    let col_count = widths.len();
    let max_height = (0..col_count)
        .map(|i| {
            cells
                .get(i)
                .map(|c| wrap_cell_inlines(c, widths[i], style, ctx, row_start_line_offset).len())
                .unwrap_or(1)
        })
        .max()
        .unwrap_or(1);

    let mut result = Vec::with_capacity(max_height);

    // Build per-column wrapped lines.
    let mut wrapped_columns: Vec<Vec<Line<'static>>> = Vec::with_capacity(col_count);
    for (i, width) in widths.iter().enumerate() {
        let cell = cells.get(i);
        let wrapped = cell
            .map(|c| wrap_cell_inlines(c, *width, style, ctx, row_start_line_offset))
            .unwrap_or_else(|| vec![Line::from(" ")]);
        wrapped_columns.push(wrapped);
    }

    for row_line in 0..max_height {
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(Span::styled("│".to_string(), ctx.theme.table_border));
        for (i, width) in widths.iter().enumerate() {
            let line = wrapped_columns
                .get(i)
                .and_then(|col| col.get(row_line))
                .cloned()
                .unwrap_or_else(|| Line::from(" "));
            let rendered_width = line.spans.iter().map(|s| s.content.width()).sum::<usize>();
            let alignment = column_alignment(alignments, i);
            let (pad_left, pad_right) = cell_padding(alignment, rendered_width, *width);
            spans.push(Span::styled(" ".to_string(), style));
            if pad_left > 0 {
                spans.push(Span::styled(" ".repeat(pad_left), style));
            }
            spans.extend(line.spans);
            if pad_right > 0 {
                spans.push(Span::styled(" ".repeat(pad_right), style));
            }
            spans.push(Span::styled(" ".to_string(), style));
            spans.push(Span::styled("│".to_string(), ctx.theme.table_border));
        }
        result.push(Line::from(spans));
    }

    result
}

fn horizontal_table_border(widths: &[usize], left: char, cross: char, right: char) -> String {
    let mut s = String::new();
    s.push(left);
    for (i, w) in widths.iter().enumerate() {
        s.extend(std::iter::repeat_n('─', *w + 2));
        if i + 1 < widths.len() {
            s.push(cross);
        }
    }
    s.push(right);
    s
}

pub(crate) fn wrap_cell_inlines(
    inlines: &[Inline],
    width: usize,
    style: Style,
    ctx: &RenderContext,
    start_line_offset: usize,
) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![Line::from(" ")];
    }
    let lines = inlines_to_wrapped_lines(inlines, ctx, style, start_line_offset, width);
    if lines.is_empty() {
        return vec![Line::from(" ")];
    }
    lines.into_iter().map(|(_, line)| line).collect()
}

pub(crate) fn column_alignment(alignments: &[Alignment], column: usize) -> Alignment {
    alignments.get(column).copied().unwrap_or(Alignment::None)
}

pub(crate) fn cell_padding(alignment: Alignment, content_width: usize, cell_width: usize) -> (usize, usize) {
    let pad_total = cell_width.saturating_sub(content_width);
    match alignment {
        Alignment::Right => (pad_total, 0),
        Alignment::Center => {
            let pad_left = pad_total / 2;
            (pad_left, pad_total - pad_left)
        }
        Alignment::Left | Alignment::None => (0, pad_total),
    }
}
