//! Indent-aware reST list parsing and parserst list coalescing.

use parserst::{Block as RstBlock, Inline as RstInline, ListKind};

#[derive(Debug, Clone)]
pub(crate) struct RichList {
    pub ordered: bool,
    pub items: Vec<RichListItem>,
}

#[derive(Debug, Clone)]
pub(crate) struct RichListItem {
    pub inlines: Vec<RstInline>,
    pub body: Vec<RichBody>,
}

#[derive(Debug, Clone)]
pub(crate) enum RichBody {
    Block(RstBlock),
    List(RichList),
}

#[derive(Debug, Clone)]
pub(crate) enum EnhancedBlock {
    Plain(RstBlock),
    RichList(RichList),
    BlockQuote(Vec<RstBlock>),
}

/// Rewrite parserst list blocks using source-aware indentation parsing.
pub(crate) fn enhance_blocks(content: &str, blocks: &[RstBlock]) -> Vec<EnhancedBlock> {
    let lines = line_infos(content);
    let regions = find_list_regions(&lines);
    let mut region_idx = 0;
    let mut out = Vec::new();
    let mut index = 0;

    while index < blocks.len() {
        if is_list_segment_start(&blocks[index..]) {
            let start = index;
            while index < blocks.len() && is_list_segment_block(&blocks[index]) {
                index += 1;
            }
            if region_idx < regions.len() {
                let (start_line, end_line) = regions[region_idx];
                region_idx += 1;
                if let Some(rich) = parse_list_region(&lines, start_line, end_line) {
                    out.push(EnhancedBlock::RichList(rich));
                    continue;
                }
            }
            out.push(EnhancedBlock::RichList(coalesce_parserst_lists(
                &blocks[start..index],
            )));
        } else if is_blockquote_paragraph(&blocks[index]) {
            let mut quote_blocks = Vec::new();
            while index < blocks.len() && is_blockquote_paragraph(&blocks[index]) {
                quote_blocks.push(normalize_blockquote_block(&blocks[index]));
                index += 1;
            }
            out.push(EnhancedBlock::BlockQuote(quote_blocks));
        } else {
            out.push(EnhancedBlock::Plain(blocks[index].clone()));
            index += 1;
        }
    }

    out
}

#[derive(Debug, Clone)]
struct LineInfo {
    indent: usize,
    text: String,
}

fn line_infos(content: &str) -> Vec<LineInfo> {
    content
        .lines()
        .map(|line| LineInfo {
            indent: leading_indent(line),
            text: line.to_string(),
        })
        .collect()
}

fn leading_indent(line: &str) -> usize {
    line.chars().take_while(|ch| *ch == ' ').count()
}

fn is_blank(line: &LineInfo) -> bool {
    line.text.trim().is_empty()
}

fn list_marker(line: &LineInfo) -> Option<(ListKind, &str)> {
    let trimmed = line.text[line.indent.min(line.text.len())..].trim_start();
    for prefix in ["- ", "* ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return Some((ListKind::Unordered, rest.trim_end()));
        }
    }
    let bytes = trimmed.as_bytes();
    let mut digits = 0;
    while digits < bytes.len() && bytes[digits].is_ascii_digit() {
        digits += 1;
    }
    if digits > 0 && digits + 1 < bytes.len() && bytes[digits] == b'.' && bytes[digits + 1] == b' '
    {
        let rest = trimmed[digits + 2..].trim_end();
        return Some((ListKind::Ordered, rest));
    }
    None
}

fn starts_block_boundary(line: &LineInfo) -> bool {
    let trimmed = line.text.trim_start();
    trimmed.starts_with(".. ")
        || trimmed.starts_with(':')
        || trimmed == "::"
        || trimmed.starts_with("```")
        || trimmed.starts_with('>')
        || trimmed.starts_with('+')
        || trimmed.chars().all(|ch| ch == '=')
        || trimmed.chars().all(|ch| ch == '-')
}

fn find_list_regions(lines: &[LineInfo]) -> Vec<(usize, usize)> {
    let mut regions = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if is_blank(&lines[index]) {
            index += 1;
            continue;
        }
        if starts_block_boundary(&lines[index]) {
            index += 1;
            continue;
        }
        let Some((kind, _)) = list_marker(&lines[index]) else {
            index += 1;
            continue;
        };
        if index > 0 && !is_blank(&lines[index - 1]) && list_marker(&lines[index - 1]).is_none() {
            index += 1;
            continue;
        }
        let base_indent = lines[index].indent;
        let end = list_region_end(lines, index, base_indent, kind);
        if end > index + 1 || list_marker(&lines[index]).is_some() {
            regions.push((index, end));
        }
        index = end;
    }
    regions
}

fn list_region_end(lines: &[LineInfo], start: usize, base_indent: usize, kind: ListKind) -> usize {
    let mut index = start + 1;
    while index < lines.len() {
        if is_blank(&lines[index]) {
            index += 1;
            continue;
        }
        let line = &lines[index];
        if line.indent < base_indent {
            break;
        }
        if line.indent == base_indent {
            if let Some((line_kind, _)) = list_marker(line) {
                if line_kind == kind {
                    index += 1;
                    continue;
                }
                break;
            }
            if starts_block_boundary(line) {
                break;
            }
            index += 1;
            continue;
        }
        if list_marker(line).is_some() || line.indent > base_indent {
            index += 1;
            continue;
        }
        break;
    }
    index
}

fn parse_list_region(lines: &[LineInfo], start: usize, end: usize) -> Option<RichList> {
    let (kind, _) = list_marker(&lines[start])?;
    let base_indent = lines[start].indent;
    let mut cursor = start;
    let mut list = RichList {
        ordered: matches!(kind, ListKind::Ordered),
        items: Vec::new(),
    };
    parse_list_items(lines, end, &mut cursor, base_indent, kind, &mut list.items);
    if list.items.is_empty() {
        None
    } else {
        Some(list)
    }
}

fn parse_list_items(
    lines: &[LineInfo],
    end: usize,
    cursor: &mut usize,
    min_indent: usize,
    kind: ListKind,
    items: &mut Vec<RichListItem>,
) {
    while *cursor < end {
        while *cursor < end && is_blank(&lines[*cursor]) {
            *cursor += 1;
        }
        if *cursor >= end {
            break;
        }
        let line = &lines[*cursor];
        if line.indent < min_indent {
            break;
        }
        if let Some((line_kind, content)) = list_marker(line) {
            if line.indent == min_indent {
                if line_kind != kind {
                    break;
                }
                items.push(RichListItem {
                    inlines: parse_rst_inlines(content),
                    body: Vec::new(),
                });
                *cursor += 1;
                continue;
            }
            if line.indent > min_indent {
                let mut nested = RichList {
                    ordered: matches!(line_kind, ListKind::Ordered),
                    items: Vec::new(),
                };
                parse_list_items(
                    lines,
                    end,
                    cursor,
                    line.indent,
                    line_kind,
                    &mut nested.items,
                );
                if let Some(last) = items.last_mut() {
                    last.body.push(RichBody::List(nested));
                } else {
                    items.push(RichListItem {
                        inlines: Vec::new(),
                        body: vec![RichBody::List(nested)],
                    });
                }
                continue;
            }
        }
        if line.indent > min_indent
            && let Some(last) = items.last_mut()
        {
            let (body, consumed) = parse_continuation(lines, end, *cursor, min_indent);
            last.body.extend(body);
            *cursor += consumed;
            continue;
        }
        break;
    }
}

fn parse_continuation(
    lines: &[LineInfo],
    end: usize,
    start: usize,
    parent_indent: usize,
) -> (Vec<RichBody>, usize) {
    let mut index = start;
    let mut chunks = Vec::new();
    while index < end {
        if is_blank(&lines[index]) {
            let next = next_non_blank(lines, end, index + 1);
            match next {
                Some(next_idx) if lines[next_idx].indent > parent_indent => {
                    index += 1;
                    continue;
                }
                _ => break,
            }
        }
        let line = &lines[index];
        if line.indent <= parent_indent || list_marker(line).is_some() {
            break;
        }
        if line.text.trim() == "::" {
            let (block, consumed) = parse_literal_block(lines, end, index, line.indent);
            chunks.push(RichBody::Block(block));
            index += consumed;
            continue;
        }
        if line.text.trim_start().starts_with("```") {
            let (block, consumed) = parse_code_fence(lines, end, index);
            chunks.push(RichBody::Block(block));
            index += consumed;
            continue;
        }
        let (text, consumed) = collect_indented_paragraph(lines, end, index, parent_indent);
        if text.trim().is_empty() {
            break;
        }
        chunks.push(RichBody::Block(RstBlock::Paragraph(parse_rst_inlines(
            text.trim(),
        ))));
        index += consumed;
    }
    (chunks, index - start)
}

fn next_non_blank(lines: &[LineInfo], end: usize, start: usize) -> Option<usize> {
    (start..end).find(|&idx| !is_blank(&lines[idx]))
}

fn collect_indented_paragraph(
    lines: &[LineInfo],
    end: usize,
    start: usize,
    parent_indent: usize,
) -> (String, usize) {
    let mut index = start;
    let mut parts = Vec::new();
    while index < end {
        if is_blank(&lines[index]) {
            break;
        }
        let line = &lines[index];
        if line.indent <= parent_indent || list_marker(line).is_some() || line.text.trim() == "::" {
            break;
        }
        parts.push(line.text[line.indent.min(line.text.len())..].trim_start());
        index += 1;
    }
    (parts.join(" "), index - start)
}

fn parse_literal_block(
    lines: &[LineInfo],
    end: usize,
    start: usize,
    marker_indent: usize,
) -> (RstBlock, usize) {
    let mut index = start + 1;
    if index < end && is_blank(&lines[index]) {
        index += 1;
    }
    let content_indent = lines
        .get(index)
        .map(|line| line.indent)
        .unwrap_or(marker_indent + 1);
    let mut body = String::new();
    while index < end {
        if is_blank(&lines[index]) {
            if let Some(next_idx) = next_non_blank(lines, end, index + 1)
                && lines[next_idx].indent < content_indent
            {
                break;
            }
            body.push('\n');
            index += 1;
            continue;
        }
        let line = &lines[index];
        if line.indent < content_indent {
            break;
        }
        let stripped = line.text[content_indent.min(line.text.len())..].trim_end();
        body.push_str(stripped);
        body.push('\n');
        index += 1;
    }
    (
        RstBlock::LiteralBlock(body.trim_end().to_string()),
        index - start,
    )
}

pub(crate) fn parse_rst_inlines(text: &str) -> Vec<RstInline> {
    match parserst::parse(&format!("{text}\n")) {
        Ok(blocks) => match blocks.first() {
            Some(RstBlock::Paragraph(inlines)) => inlines.clone(),
            _ => vec![RstInline::Text(text.to_string())],
        },
        Err(_) => vec![RstInline::Text(text.to_string())],
    }
}

fn parse_code_fence(lines: &[LineInfo], end: usize, start: usize) -> (RstBlock, usize) {
    let mut index = start + 1;
    let mut body = String::new();
    while index < end {
        if lines[index].text.trim() == "```" {
            index += 1;
            break;
        }
        body.push_str(&lines[index].text);
        body.push('\n');
        index += 1;
    }
    (
        RstBlock::CodeBlock(body.trim_end().to_string()),
        index - start,
    )
}

fn is_list_segment_start(blocks: &[RstBlock]) -> bool {
    matches!(blocks.first(), Some(RstBlock::List { .. }))
}

fn is_list_segment_block(block: &RstBlock) -> bool {
    match block {
        RstBlock::List { .. } => true,
        RstBlock::Paragraph(inlines) => paragraph_is_indented(inlines),
        RstBlock::LiteralBlock(_) | RstBlock::CodeBlock(_) => true,
        _ => false,
    }
}

fn paragraph_is_indented(inlines: &[RstInline]) -> bool {
    paragraph_plain(inlines).starts_with("  ")
}

fn is_blockquote_paragraph(block: &RstBlock) -> bool {
    matches!(block, RstBlock::Paragraph(inlines) if paragraph_is_indented(inlines))
}

fn paragraph_plain(inlines: &[RstInline]) -> String {
    inlines
        .iter()
        .map(|inline| match inline {
            RstInline::Text(text) => text.clone(),
            RstInline::Code(code) => code.clone(),
            RstInline::Em(children) | RstInline::Strong(children) => paragraph_plain(children),
            RstInline::Link { text, .. } => paragraph_plain(text),
        })
        .collect()
}

fn normalize_blockquote_block(block: &RstBlock) -> RstBlock {
    let RstBlock::Paragraph(inlines) = block else {
        return block.clone();
    };
    let plain = paragraph_plain(inlines);
    let trimmed = plain.trim_start();
    RstBlock::Paragraph(parse_rst_inlines(trimmed))
}

fn coalesce_parserst_lists(blocks: &[RstBlock]) -> RichList {
    let mut ordered = false;
    let mut items = Vec::new();

    for block in blocks {
        match block {
            RstBlock::List {
                kind,
                items: list_items,
            } => {
                ordered = matches!(kind, ListKind::Ordered);
                for item in list_items {
                    items.push(RichListItem {
                        inlines: item.clone(),
                        body: Vec::new(),
                    });
                }
            }
            RstBlock::Paragraph(inlines) => {
                if let Some(last) = items.last_mut() {
                    let text = inlines
                        .iter()
                        .filter_map(|inline| match inline {
                            RstInline::Text(text) => Some(text.trim().to_string()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("");
                    last.body
                        .push(RichBody::Block(RstBlock::Paragraph(vec![RstInline::Text(
                            text,
                        )])));
                }
            }
            RstBlock::LiteralBlock(content) | RstBlock::CodeBlock(content) => {
                if let Some(last) = items.last_mut() {
                    last.body.push(RichBody::Block(block.clone()));
                } else {
                    let _ = content;
                }
            }
            _ => {}
        }
    }

    RichList { ordered, items }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parserst::parse as parse_rst;

    fn rich_list_items(content: &str) -> RichList {
        let blocks = parse_rst(content).unwrap();
        let enhanced = enhance_blocks(content, &blocks);
        match &enhanced[0] {
            EnhancedBlock::RichList(list) => list.clone(),
            other => panic!("expected rich list, got {other:?}"),
        }
    }

    #[test]
    fn rest_list_coalesces_continuation_paragraph() {
        let list = rich_list_items("- One\n\n  Continuation.\n\n- Two");
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].body.len(), 1);
        assert!(matches!(
            &list.items[0].body[0],
            RichBody::Block(RstBlock::Paragraph(inlines))
                if inlines.iter().any(|inline| matches!(inline, RstInline::Text(t) if t == "Continuation."))
        ));
    }

    #[test]
    fn rest_list_parses_nested_items() {
        let list = rich_list_items("- One\n  - Nested\n- Two");
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].body.len(), 1);
        let RichBody::List(nested) = &list.items[0].body[0] else {
            panic!("expected nested list");
        };
        assert_eq!(nested.items.len(), 1);
        assert!(matches!(
            &nested.items[0].inlines[0],
            RstInline::Text(t) if t == "Nested"
        ));
    }

    #[test]
    fn rest_list_coalesces_literal_block_continuation() {
        let list = rich_list_items("- One\n\n  ::\n\n    code\n\n- Two");
        assert_eq!(list.items.len(), 2);
        assert!(matches!(
            &list.items[0].body[0],
            RichBody::Block(RstBlock::LiteralBlock(content)) if content == "code"
        ));
    }

    #[test]
    fn rest_enhance_blocks_groups_indented_paragraphs_as_blockquote() {
        let blocks = parse_rst("Intro.\n\n    quoted text\n").unwrap();
        let enhanced = enhance_blocks("Intro.\n\n    quoted text\n", &blocks);
        assert_eq!(enhanced.len(), 2);
        assert!(matches!(enhanced[0], EnhancedBlock::Plain(_)));
        let EnhancedBlock::BlockQuote(quote_blocks) = &enhanced[1] else {
            panic!("expected blockquote, got {:?}", enhanced[1]);
        };
        assert_eq!(quote_blocks.len(), 1);
        assert!(matches!(
            &quote_blocks[0],
            RstBlock::Paragraph(inlines)
                if inlines.iter().any(|inline| matches!(inline, RstInline::Text(t) if t == "quoted text"))
        ));
    }
}
