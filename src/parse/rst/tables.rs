//! Source-aware reST table alignment parsing.

use crate::parse::dto::ParsedAlignment;

#[derive(Debug, Clone)]
pub(crate) struct TableRegionMeta {
    pub alignments: Vec<ParsedAlignment>,
}

pub(crate) fn find_table_regions(content: &str) -> Vec<TableRegionMeta> {
    let lines: Vec<&str> = content.lines().collect();
    let mut regions = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if is_simple_table_boundary(lines[index]) {
            if let Some((meta, end)) = parse_simple_table_region(&lines, index) {
                regions.push(meta);
                index = end;
                continue;
            }
        }
        if is_grid_border(lines[index]) {
            if let Some((meta, end)) = parse_grid_table_region(&lines, index) {
                regions.push(meta);
                index = end;
                continue;
            }
        }
        index += 1;
    }
    regions
}

fn is_simple_table_boundary(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && trimmed.chars().all(|c| c == '=' || c == ' ')
}

fn is_grid_border(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && trimmed.starts_with('+')
        && trimmed
            .chars()
            .all(|c| c == '+' || c == '-' || c == '=' || c == ' ')
}

fn is_alignment_row(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && trimmed.contains('-')
        && trimmed
            .chars()
            .all(|c| c == '-' || c == ':' || c == ' ' || c == '\t')
}

fn split_table_row(line: &str) -> Vec<String> {
    line.split("  ")
        .map(str::trim)
        .filter(|cell| !cell.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_alignment_cell(cell: &str) -> ParsedAlignment {
    let trimmed = cell.trim();
    if !trimmed.chars().any(|c| c == '-') {
        return ParsedAlignment::None;
    }
    let left = trimmed.starts_with(':');
    let right = trimmed.ends_with(':');
    match (left, right) {
        (true, true) => ParsedAlignment::Center,
        (false, true) => ParsedAlignment::Right,
        _ => ParsedAlignment::Left,
    }
}

fn parse_alignment_row(line: &str) -> Vec<ParsedAlignment> {
    split_table_row(line)
        .iter()
        .map(|cell| parse_alignment_cell(cell))
        .collect()
}

fn parse_simple_table_region(lines: &[&str], start: usize) -> Option<(TableRegionMeta, usize)> {
    let mut index = start + 1;
    if index >= lines.len() {
        return None;
    }
    let header = lines[index].trim();
    if header.is_empty() {
        return None;
    }
    index += 1;
    if index >= lines.len() || !is_simple_table_boundary(lines[index]) {
        return None;
    }
    index += 1;

    let column_count = split_table_row(header).len().max(1);
    let mut alignments = vec![ParsedAlignment::Left; column_count];

    while index < lines.len() && !is_simple_table_boundary(lines[index]) {
        if lines[index].trim().is_empty() {
            index += 1;
            continue;
        }
        if is_alignment_row(lines[index]) {
            alignments = parse_alignment_row(lines[index]);
            while alignments.len() < column_count {
                alignments.push(ParsedAlignment::Left);
            }
            alignments.truncate(column_count);
        }
        index += 1;
    }

    if index < lines.len() {
        index += 1;
    }

    Some((TableRegionMeta { alignments }, index))
}

fn parse_grid_table_region(lines: &[&str], start: usize) -> Option<(TableRegionMeta, usize)> {
    let columns = lines[start]
        .char_indices()
        .filter_map(|(index, ch)| (ch == '+').then_some(index))
        .count()
        .saturating_sub(1)
        .max(1);
    let mut index = start + 1;
    while index < lines.len() {
        if is_grid_border(lines[index]) {
            index += 1;
            if index >= lines.len() || !is_grid_border(lines[index]) {
                break;
            }
        } else if lines[index].trim().is_empty() {
            index += 1;
            break;
        } else {
            index += 1;
        }
    }
    Some((
        TableRegionMeta {
            alignments: vec![ParsedAlignment::Left; columns],
        },
        index,
    ))
}

pub(crate) fn is_alignment_separator_row(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let trimmed = cell.trim();
            trimmed.contains('-')
                && trimmed
                    .chars()
                    .all(|c| c == '-' || c == ':' || c == ' ' || c == '\t')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_table_region_reads_column_alignments() {
        let source = "=====  =====\nLeft   Right\n=====  =====\nA      B\n-----  ------:\nC       D\n=====  =====\n";
        let regions = find_table_regions(source);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].alignments, vec![ParsedAlignment::Left, ParsedAlignment::Right]);
    }
}
