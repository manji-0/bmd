//! Source-aware reST table alignment parsing.

use crate::parse::dto::ParsedAlignment;

#[derive(Debug, Clone)]
pub(crate) struct TableRegionMeta {
    pub alignments: Vec<ParsedAlignment>,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
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
            .all(|c| c == '+' || c == '-' || c == '=' || c == ' ' || c == ':')
}

fn is_alignment_row(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && trimmed.contains('-')
        && trimmed
            .chars()
            .all(|c| c == '-' || c == ':' || c == ' ' || c == '\t')
}

fn split_grid_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') {
        return split_table_row(line);
    }
    trimmed
        .split('|')
        .map(str::trim)
        .filter(|cell| !cell.is_empty())
        .map(str::to_string)
        .collect()
}

fn is_grid_data_row(line: &str) -> bool {
    line.trim().starts_with('|') && line.contains('|')
}
fn split_table_row(line: &str) -> Vec<String> {
    line.split("  ")
        .map(str::trim)
        .filter(|cell| !cell.is_empty())
        .map(str::to_string)
        .collect()
}

fn pad_row(mut row: Vec<String>, column_count: usize) -> Vec<String> {
    while row.len() < column_count {
        row.push(String::new());
    }
    row.truncate(column_count);
    row
}

fn parse_alignment_cell(cell: &str) -> ParsedAlignment {
    let trimmed = cell.trim();
    if !trimmed.chars().any(|c| c == '-' || c == '=') {
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
    let headers = split_table_row(header);
    index += 1;
    if index >= lines.len() || !is_simple_table_boundary(lines[index]) {
        return None;
    }
    index += 1;

    let column_count = headers.len().max(1);
    let mut alignments = vec![ParsedAlignment::Left; column_count];
    let mut rows = Vec::new();

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
        } else {
            rows.push(pad_row(split_table_row(lines[index]), column_count));
        }
        index += 1;
    }

    if index < lines.len() {
        index += 1;
    }

    Some((
        TableRegionMeta {
            alignments,
            headers,
            rows,
        },
        index,
    ))
}

fn parse_grid_separator_alignments(line: &str) -> Vec<ParsedAlignment> {
    line.split('+')
        .filter(|part| !part.trim().is_empty())
        .map(parse_alignment_cell)
        .collect()
}

fn is_grid_header_separator(line: &str) -> bool {
    is_grid_border(line) && line.contains('=')
}

fn parse_grid_table_region(lines: &[&str], start: usize) -> Option<(TableRegionMeta, usize)> {
    let column_count = lines[start]
        .split('+')
        .filter(|part| !part.trim().is_empty())
        .count()
        .max(1);
    let mut alignments = vec![ParsedAlignment::Left; column_count];
    let mut headers = Vec::new();
    let mut rows = Vec::new();
    let mut index = start + 1;
    let mut end = start + 1;
    let mut saw_header_separator = false;

    while index < lines.len() {
        if lines[index].trim().is_empty() {
            end = index + 1;
            break;
        }
        if is_grid_header_separator(lines[index]) {
            let parsed = parse_grid_separator_alignments(lines[index]);
            if !parsed.is_empty() {
                alignments = parsed;
                while alignments.len() < column_count {
                    alignments.push(ParsedAlignment::Left);
                }
                alignments.truncate(column_count);
            }
            saw_header_separator = true;
        } else if is_grid_data_row(lines[index]) {
            let row = pad_row(split_grid_row(lines[index]), column_count);
            if headers.is_empty() {
                headers = row;
            } else if saw_header_separator {
                rows.push(row);
            }
        }
        if is_grid_border(lines[index]) {
            end = index + 1;
            index += 1;
            if index < lines.len() && is_grid_border(lines[index]) && !lines[index].contains('|') {
                end = index + 1;
                break;
            }
            continue;
        }
        index += 1;
        end = index;
    }

    Some((
        TableRegionMeta {
            alignments,
            headers,
            rows,
        },
        end,
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
        assert_eq!(
            regions[0].alignments,
            vec![ParsedAlignment::Left, ParsedAlignment::Right]
        );
    }

    #[test]
    fn rest_grid_table_region_reads_column_alignments() {
        let source = "+-------+--------+\n| Left  | Right  |\n+=======+=======:+\n| A     |      B |\n+-------+--------+\n";
        let regions = find_table_regions(source);
        assert_eq!(regions.len(), 1);
        assert_eq!(
            regions[0].alignments,
            vec![ParsedAlignment::Left, ParsedAlignment::Right]
        );
        assert_eq!(regions[0].headers, vec!["Left", "Right"]);
        assert_eq!(
            regions[0].rows,
            vec![vec!["A".to_string(), "B".to_string()]]
        );
    }

    #[test]
    fn rest_simple_table_region_reads_body_cells() {
        let source = "=====  =====\nLeft   Right\n=====  =====\n`link <https://example.com>`_  text\n-----  -----\n=====  =====\n";
        let regions = find_table_regions(source);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].headers, vec!["Left", "Right"]);
        assert_eq!(regions[0].rows.len(), 1);
        assert!(regions[0].rows[0][0].contains("link"));
        assert_eq!(regions[0].rows[0][1], "text");
    }
}
