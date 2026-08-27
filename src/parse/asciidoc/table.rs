//! AsciiDoc table grid materialization.

use acdc_parser::{Block as AdocBlock, HorizontalAlignment, Table, TableColumn};

use crate::parse::dto::{ParsedAlignment, ParsedBlock, ParsedInline, ParsedTable};
use crate::parse::error::ParseError;

use super::inline::map_inlines;
use super::{AsciiDocState, map_block};

pub(super) fn map_table(
    table: &Table<'_>,
    state: &mut AsciiDocState<'_>,
) -> Result<ParsedBlock, ParseError> {
    let mut grid: Vec<Vec<Option<Vec<ParsedInline>>>> = Vec::new();
    if let Some(header) = &table.header {
        place_table_row(&mut grid, 0, &header.columns, state)?;
    }
    for (row_idx, row) in table.rows.iter().enumerate() {
        let grid_row = if table.header.is_some() {
            row_idx + 1
        } else {
            row_idx
        };
        place_table_row(&mut grid, grid_row, &row.columns, state)?;
    }

    let headers = materialize_table_row(grid.first());
    let body_start = usize::from(table.header.is_some());
    let rows = grid
        .into_iter()
        .skip(body_start)
        .map(|row| materialize_table_row(Some(&row)))
        .collect();
    let alignments = table
        .columns
        .iter()
        .map(|column| map_horizontal_alignment(Some(column.halign)))
        .collect();
    Ok(ParsedBlock::Table(ParsedTable {
        headers,
        rows,
        alignments,
    }))
}

fn materialize_table_row(row: Option<&Vec<Option<Vec<ParsedInline>>>>) -> Vec<Vec<ParsedInline>> {
    row.map(|cells| {
        cells
            .iter()
            .map(|cell| cell.clone().unwrap_or_default())
            .collect()
    })
    .unwrap_or_default()
}

fn place_table_row(
    grid: &mut Vec<Vec<Option<Vec<ParsedInline>>>>,
    row_idx: usize,
    columns: &[TableColumn<'_>],
    state: &mut AsciiDocState<'_>,
) -> Result<(), ParseError> {
    while grid.len() <= row_idx {
        grid.push(Vec::new());
    }
    let mut col_idx = 0;
    for column in columns {
        col_idx = next_free_table_col(&grid[row_idx], col_idx);
        let cell = cell_to_inlines(&column.content, state)?;
        let colspan = column.colspan.max(1);
        let rowspan = column.rowspan.max(1);
        for row_offset in 0..rowspan {
            let target_row = row_idx + row_offset;
            while grid.len() <= target_row {
                grid.push(Vec::new());
            }
            for col_offset in 0..colspan {
                let target_col = col_idx + col_offset;
                while grid[target_row].len() <= target_col {
                    grid[target_row].push(None);
                }
                if row_offset == 0 && col_offset == 0 {
                    grid[target_row][target_col] = Some(cell.clone());
                } else {
                    grid[target_row][target_col] = Some(Vec::new());
                }
            }
        }
        col_idx += colspan;
    }
    Ok(())
}

fn next_free_table_col(row: &[Option<Vec<ParsedInline>>], start: usize) -> usize {
    let mut col = start;
    while col < row.len() && row[col].is_some() {
        col += 1;
    }
    col
}

fn cell_to_inlines(
    blocks: &[AdocBlock<'_>],
    state: &mut AsciiDocState<'_>,
) -> Result<Vec<ParsedInline>, ParseError> {
    let mut paragraphs = Vec::new();
    for block in blocks {
        match block {
            AdocBlock::Paragraph(paragraph) => {
                paragraphs.push(map_inlines(&paragraph.content, state));
            }
            other => {
                for mapped in map_block(other, state)? {
                    if let ParsedBlock::Paragraph(inlines) = mapped {
                        paragraphs.push(inlines);
                    }
                }
            }
        }
    }
    if paragraphs.is_empty() {
        return Ok(Vec::new());
    }
    if paragraphs.len() == 1 {
        return Ok(paragraphs.pop().unwrap());
    }
    let mut out = paragraphs.remove(0);
    for paragraph in paragraphs {
        out.push(ParsedInline::HardBreak);
        out.extend(paragraph);
    }
    Ok(out)
}

fn map_horizontal_alignment(alignment: Option<HorizontalAlignment>) -> ParsedAlignment {
    match alignment {
        Some(HorizontalAlignment::Left) => ParsedAlignment::Left,
        Some(HorizontalAlignment::Center) => ParsedAlignment::Center,
        Some(HorizontalAlignment::Right) => ParsedAlignment::Right,
        _ => ParsedAlignment::None,
    }
}
