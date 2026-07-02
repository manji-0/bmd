//! reStructuredText parser: parserst AST -> DTO.

mod lists;
mod tables;

use std::collections::HashMap;

use lists::{EnhancedBlock, RichBody, RichList, RichListItem};
use parserst::{Block as RstBlock, Field, Inline as RstInline, ListKind};
use tables::TableRegionMeta;

use crate::parse::dto::{
    ParsedBlock, ParsedCodeBlock, ParsedDefinitionItem, ParsedDefinitionList, ParsedDocument,
    ParsedDocumentParts, ParsedFootnoteDefinition, ParsedFrontMatter, ParsedFrontMatterKind,
    ParsedHeading, ParsedInline, ParsedLink, ParsedLinkKind, ParsedList, ParsedListItem,
    ParsedMathBlock, ParsedTable, ParsedAlignment,
};
use crate::parse::error::ParseError;
use crate::parse::format::MarkupFormat;
use crate::parse::slug::{anchor_href, slugify_heading};

const ADMONITIONS: &[&str] = &[
    "admonition",
    "attention",
    "caution",
    "danger",
    "error",
    "hint",
    "important",
    "note",
    "tip",
    "warning",
];

struct RestState {
    parts: ParsedDocumentParts,
    footnotes: Vec<ParsedFootnoteDefinition>,
    footnote_order: Vec<usize>,
    footnote_label_to_id: HashMap<String, usize>,
    front_matter: Option<ParsedFrontMatter>,
    table_regions: Vec<TableRegionMeta>,
    table_region_index: usize,
}

impl RestState {
    fn new(front_matter: Option<ParsedFrontMatter>, table_regions: Vec<TableRegionMeta>) -> Self {
        Self {
            parts: ParsedDocumentParts::default(),
            footnotes: Vec::new(),
            footnote_order: Vec::new(),
            footnote_label_to_id: HashMap::new(),
            front_matter,
            table_regions,
            table_region_index: 0,
        }
    }

    fn next_table_meta(&mut self, columns: usize) -> TableRegionMeta {
        let meta = self
            .table_regions
            .get(self.table_region_index)
            .cloned()
            .unwrap_or_else(|| TableRegionMeta {
                alignments: vec![ParsedAlignment::Left; columns.max(1)],
            });
        self.table_region_index += 1;
        meta
    }

    fn into_document(self, blocks: Vec<ParsedBlock>) -> ParsedDocument {
        ParsedDocument::new(
            blocks,
            self.parts.links,
            self.parts.mermaid_diagrams,
            self.footnotes,
            self.footnote_order,
            self.front_matter,
        )
    }

    fn footnote_id_for_label(&mut self, label: &str) -> usize {
        if let Some(&id) = self.footnote_label_to_id.get(label) {
            return id;
        }
        let id = self.footnotes.len();
        self.footnote_label_to_id.insert(label.to_string(), id);
        self.footnotes.push(ParsedFootnoteDefinition {
            label: label.to_string(),
            blocks: Vec::new(),
        });
        id
    }

    fn footnote_display_for(&mut self, footnote_id: usize) -> usize {
        if let Some(pos) = self.footnote_order.iter().position(|&id| id == footnote_id) {
            pos + 1
        } else {
            self.footnote_order.push(footnote_id);
            self.footnote_order.len()
        }
    }
}

/// Parse reStructuredText into a [`ParsedDocument`].
pub fn parse(content: &str) -> Result<ParsedDocument, ParseError> {
    let blocks = parserst::parse(content)
        .map_err(|error| ParseError::syntax(MarkupFormat::Rest, error.to_string()))?;
    let (front_matter, body_start) = extract_leading_front_matter(&blocks);
    let table_regions = tables::find_table_regions(content);
    let enhanced = lists::enhance_blocks(content, &blocks[body_start..]);
    let mut state = RestState::new(front_matter, table_regions);
    let parsed_blocks = map_enhanced_blocks(&enhanced, &mut state)?;
    Ok(state.into_document(parsed_blocks))
}

fn extract_leading_front_matter(blocks: &[RstBlock]) -> (Option<ParsedFrontMatter>, usize) {
    let mut index = 0;
    let mut lines = Vec::new();
    while index < blocks.len() {
        let RstBlock::FieldList { fields } = &blocks[index] else {
            break;
        };
        for field in fields {
            if !field_list_is_front_matter_candidate(field) {
                return (build_front_matter(lines), index);
            }
            let key = field_list_key(field);
            let value = field_list_value_plain(field);
            if value.is_empty() {
                lines.push(format!("{key}:"));
            } else {
                lines.push(format!("{key}: {value}"));
            }
        }
        index += 1;
    }
    (build_front_matter(lines), index)
}

fn build_front_matter(lines: Vec<String>) -> Option<ParsedFrontMatter> {
    if lines.is_empty() {
        return None;
    }
    Some(ParsedFrontMatter {
        kind: ParsedFrontMatterKind::Yaml,
        raw: lines.join("\n"),
    })
}

fn field_list_is_front_matter_candidate(field: &Field) -> bool {
    field.body.is_empty()
        || field.body.iter().all(|block| {
            matches!(
                block,
                RstBlock::Paragraph(inlines)
                    if inlines.iter().all(|inline| matches!(inline, RstInline::Text(_)))
            )
        })
}

fn field_list_key(field: &Field) -> String {
    if field.argument.is_empty() {
        field.name.clone()
    } else {
        format!("{} {}", field.name, field.argument)
    }
}

fn field_list_value_plain(field: &Field) -> String {
    field
        .body
        .iter()
        .filter_map(|block| match block {
            RstBlock::Paragraph(inlines) => Some(rst_inline_plain(inlines)),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn map_enhanced_blocks(
    blocks: &[EnhancedBlock],
    state: &mut RestState,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let mut out = Vec::new();
    for block in blocks {
        out.extend(map_enhanced_block(block, state)?);
    }
    Ok(out)
}

fn map_enhanced_block(
    block: &EnhancedBlock,
    state: &mut RestState,
) -> Result<Vec<ParsedBlock>, ParseError> {
    match block {
        EnhancedBlock::Plain(rst_block) => map_block(rst_block, state),
        EnhancedBlock::RichList(list) => Ok(vec![ParsedBlock::List(map_rich_list(list, state)?)]),
    }
}

fn map_rich_list(list: &RichList, state: &mut RestState) -> Result<ParsedList, ParseError> {
    Ok(ParsedList {
        ordered: list.ordered,
        items: list
            .items
            .iter()
            .map(|item| map_rich_list_item(item, state))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn map_rich_list_item(
    item: &RichListItem,
    state: &mut RestState,
) -> Result<ParsedListItem, ParseError> {
    let (inlines, checklist_id, checked) = parse_checklist_item_prefix(&item.inlines, state);
    let mut blocks = Vec::new();
    if !inlines.is_empty() {
        blocks.push(ParsedBlock::Paragraph(map_inlines(&inlines, state)));
    }
    for body in &item.body {
        blocks.extend(map_rich_body(body, state)?);
    }
    Ok(ParsedListItem {
        checklist_id,
        checked,
        content: blocks,
    })
}

fn map_rich_body(body: &RichBody, state: &mut RestState) -> Result<Vec<ParsedBlock>, ParseError> {
    match body {
        RichBody::Block(block) => map_block(block, state),
        RichBody::List(list) => Ok(vec![ParsedBlock::List(map_rich_list(list, state)?)]),
    }
}

fn map_blocks(blocks: &[RstBlock], state: &mut RestState) -> Result<Vec<ParsedBlock>, ParseError> {
    let mut out = Vec::new();
    for block in blocks {
        out.extend(map_block(block, state)?);
    }
    Ok(out)
}

fn map_block(block: &RstBlock, state: &mut RestState) -> Result<Vec<ParsedBlock>, ParseError> {
    Ok(match block {
        RstBlock::Heading { level, inlines } => {
            ParseError::ensure_heading_level(MarkupFormat::Rest, *level)?;
            vec![ParsedBlock::Heading(ParsedHeading {
                level: *level,
                content: map_inlines(inlines, state),
                anchor: Some(slugify_heading(&rst_inline_plain(inlines))),
            })]
        }
        RstBlock::Paragraph(inlines) if paragraph_is_horizontal_rule(inlines) => {
            vec![ParsedBlock::Rule]
        }
        RstBlock::Paragraph(inlines) => vec![ParsedBlock::Paragraph(map_inlines(inlines, state))],
        RstBlock::CodeBlock(content) | RstBlock::LiteralBlock(content) => {
            vec![ParsedBlock::CodeBlock(ParsedCodeBlock {
                language: None,
                content: content.clone(),
            })]
        }
        RstBlock::Quote(nested) => vec![ParsedBlock::BlockQuote(map_blocks(nested, state)?)],
        RstBlock::List { kind, items } => vec![ParsedBlock::List(ParsedList {
            ordered: matches!(kind, ListKind::Ordered),
            items: items
                .iter()
                .map(|item| {
                    let (inlines, checklist_id, checked) =
                        parse_checklist_item_prefix(item, state);
                    ParsedListItem {
                        checklist_id,
                        checked,
                        content: vec![ParsedBlock::Paragraph(map_inlines(&inlines, state))],
                    }
                })
                .collect(),
        })],
        RstBlock::Table { headers, rows } => {
            let column_count = headers
                .len()
                .max(rows.first().map(|row| row.len()).unwrap_or(0))
                .max(1);
            let meta = state.next_table_meta(column_count);
            vec![ParsedBlock::Table(ParsedTable {
                headers: headers
                    .iter()
                    .map(|cell| map_inlines(cell, state))
                    .collect(),
                rows: rows
                    .iter()
                    .filter(|row| {
                        let cells: Vec<String> =
                            row.iter().map(|cell| rst_inline_plain(cell)).collect();
                        !tables::is_alignment_separator_row(&cells)
                    })
                    .map(|row| {
                        row.iter()
                            .map(|cell| map_inlines(cell, state))
                            .collect()
                    })
                    .collect(),
                alignments: meta.alignments,
            })]
        }
        RstBlock::Directive {
            name,
            argument,
            content,
        } => map_directive(name, argument, content, state)?,
        RstBlock::Comment(nested) => map_comment(nested, state)?,
        RstBlock::FieldList { fields } => map_field_list(fields, state)?,
    })
}

fn map_comment(nested: &[RstBlock], state: &mut RestState) -> Result<Vec<ParsedBlock>, ParseError> {
    if let Some((label, body)) = extract_footnote_from_comment(nested, state)? {
        let footnote_id = state.footnote_id_for_label(&label);
        if let Some(def) = state.footnotes.get_mut(footnote_id) {
            def.blocks = body;
        }
        return Ok(Vec::new());
    }
    Ok(Vec::new())
}

fn extract_footnote_from_comment(
    nested: &[RstBlock],
    state: &mut RestState,
) -> Result<Option<(String, Vec<ParsedBlock>)>, ParseError> {
    let Some(first) = nested.first() else {
        return Ok(None);
    };
    let RstBlock::Paragraph(inlines) = first else {
        return Ok(None);
    };
    let plain = rst_inline_plain(inlines);
    let Some((label, rest)) = parse_footnote_label_prefix(&plain) else {
        return Ok(None);
    };
    let mut body = Vec::new();
    if !rest.trim().is_empty() {
        body.push(ParsedBlock::Paragraph(vec![ParsedInline::Text(rest)]));
    }
    body.extend(map_blocks(&nested[1..], state)?);
    Ok(Some((label, body)))
}

fn parse_footnote_label_prefix(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim_start();
    let rest = trimmed.strip_prefix('[')?;
    let (label, after) = rest.split_once(']')?;
    if label.is_empty() {
        return None;
    }
    Some((label.to_string(), after.trim_start().to_string()))
}

fn map_directive(
    name: &str,
    argument: &str,
    content: &[RstBlock],
    state: &mut RestState,
) -> Result<Vec<ParsedBlock>, ParseError> {
    if is_code_directive(name) {
        let language = if argument.is_empty() {
            None
        } else {
            Some(argument.to_string())
        };
        let body = collect_verbatim(content);
        return Ok(vec![ParsedBlock::CodeBlock(ParsedCodeBlock {
            language,
            content: body,
        })]);
    }
    if name.eq_ignore_ascii_case("mermaid") {
        let source = collect_verbatim(content);
        let (link_id, _url) = state.parts.push_mermaid(source.clone());
        let label = mermaid_link_label(&source);
        return Ok(vec![ParsedBlock::Paragraph(vec![ParsedInline::Link {
            link_id,
            children: vec![ParsedInline::Text(label)],
        }])]);
    }
    if name.eq_ignore_ascii_case("math") {
        let body = if argument.trim().is_empty() {
            collect_verbatim(content)
        } else {
            argument.trim().to_string()
        };
        if body.is_empty() {
            return Ok(Vec::new());
        }
        return Ok(vec![ParsedBlock::MathBlock(ParsedMathBlock { content: body })]);
    }
    if name.eq_ignore_ascii_case("image") {
        let url = argument.trim();
        if url.is_empty() {
            return Ok(Vec::new());
        }
        let alt = collect_verbatim(content);
        let label = if alt.is_empty() { url.to_string() } else { alt };
        let link_id = state.parts.push_link(ParsedLink::new(
            url.to_string(),
            None,
            ParsedLinkKind::Image,
        ));
        return Ok(vec![ParsedBlock::Paragraph(vec![ParsedInline::Link {
            link_id,
            children: vec![ParsedInline::Text(label)],
        }])]);
    }
    if is_admonition(name) {
        return map_admonition(name, content, state);
    }
    map_blocks(content, state)
}

fn is_code_directive(name: &str) -> bool {
    name.eq_ignore_ascii_case("code-block")
        || name.eq_ignore_ascii_case("sourcecode")
        || name.eq_ignore_ascii_case("code")
}

fn is_admonition(name: &str) -> bool {
    ADMONITIONS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(name))
}

fn map_admonition(
    name: &str,
    content: &[RstBlock],
    state: &mut RestState,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let mut inner = map_blocks(content, state)?;
    inner.insert(
        0,
        ParsedBlock::Paragraph(vec![ParsedInline::Strong(vec![ParsedInline::Text(
            format!("{}:", name.to_ascii_lowercase()),
        )])]),
    );
    Ok(vec![ParsedBlock::BlockQuote(inner)])
}

fn map_field_list(fields: &[Field], state: &mut RestState) -> Result<Vec<ParsedBlock>, ParseError> {
    let items = fields
        .iter()
        .map(|field| {
            let mut term = vec![ParsedInline::Text(field.name.clone())];
            if !field.argument.is_empty() {
                term.push(ParsedInline::Text(format!(" {}", field.argument)));
            }
            let body = map_blocks(&field.body, state)?;
            Ok(ParsedDefinitionItem {
                term,
                definitions: if body.is_empty() {
                    vec![Vec::new()]
                } else {
                    vec![body]
                },
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(vec![ParsedBlock::DefinitionList(ParsedDefinitionList { items })])
}

fn collect_verbatim(blocks: &[RstBlock]) -> String {
    let mut lines = Vec::new();
    for block in blocks {
        match block {
            RstBlock::Paragraph(inlines) => lines.push(rst_inline_plain(inlines)),
            RstBlock::CodeBlock(text) | RstBlock::LiteralBlock(text) => lines.push(text.clone()),
            RstBlock::Directive { content, .. } => lines.push(collect_verbatim(content)),
            _ => {}
        }
    }
    lines.join("\n")
}

fn map_inlines(inlines: &[RstInline], state: &mut RestState) -> Vec<ParsedInline> {
    let mapped = inlines
        .iter()
        .flat_map(|inline| map_inline(inline, state))
        .collect::<Vec<_>>();
    normalize_rst_inline_patterns(mapped)
}

fn map_inline(inline: &RstInline, state: &mut RestState) -> Vec<ParsedInline> {
    match inline {
        RstInline::Text(text) => expand_rst_text(text, state),
        RstInline::Em(children) => vec![ParsedInline::Emphasis(map_inlines(children, state))],
        RstInline::Strong(children) => vec![ParsedInline::Strong(map_inlines(children, state))],
        RstInline::Code(code) => vec![ParsedInline::Code(code.clone())],
        RstInline::Link { text, url } => {
            let (url, kind) = if url.starts_with('#') {
                (anchor_href(url), ParsedLinkKind::Anchor)
            } else {
                (url.clone(), ParsedLinkKind::classify_url(url))
            };
            let link_id = state.parts.push_link(ParsedLink::new(url, None, kind));
            vec![ParsedInline::Link {
                link_id,
                children: map_inlines(text, state),
            }]
        }
    }
}

fn expand_rst_text(text: &str, state: &mut RestState) -> Vec<ParsedInline> {
    let mut out = Vec::new();
    for (index, segment) in text.split("\\\n").enumerate() {
        if index > 0 {
            out.push(ParsedInline::HardBreak);
        }
        for inline in expand_footnote_refs(segment, state) {
            match inline {
                ParsedInline::Text(value) => out.extend(expand_strikethrough(&value)),
                other => out.push(other),
            }
        }
    }
    out
}

fn expand_strikethrough(text: &str) -> Vec<ParsedInline> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("~~") {
        if start > 0 {
            out.push(ParsedInline::Text(rest[..start].to_string()));
        }
        rest = &rest[start + 2..];
        let Some(end) = rest.find("~~") else {
            out.push(ParsedInline::Text(format!("~~{rest}")));
            return out;
        };
        out.push(ParsedInline::Strikethrough(vec![ParsedInline::Text(
            rest[..end].to_string(),
        )]));
        rest = &rest[end + 2..];
    }
    if !rest.is_empty() {
        out.push(ParsedInline::Text(rest.to_string()));
    }
    out
}

fn normalize_rst_inline_patterns(inlines: Vec<ParsedInline>) -> Vec<ParsedInline> {
    let mut out = Vec::new();
    let mut iter = inlines.into_iter();
    while let Some(inline) = iter.next() {
        let Some((before, role)) = parse_inline_role_prefix(&inline) else {
            out.push(inline);
            continue;
        };
        let Some(ParsedInline::Code(content)) = iter.next() else {
            out.push(inline);
            continue;
        };
        if !before.is_empty() {
            out.push(ParsedInline::Text(before));
        }
        out.push(match role {
            InlineRole::Math => ParsedInline::Math(content),
            InlineRole::Strike => ParsedInline::Strikethrough(vec![ParsedInline::Text(content)]),
        });
    }
    out
}

#[derive(Clone, Copy)]
enum InlineRole {
    Math,
    Strike,
}

fn parse_inline_role_prefix(inline: &ParsedInline) -> Option<(String, InlineRole)> {
    let ParsedInline::Text(prefix) = inline else {
        return None;
    };
    for (marker, role) in [(":math:", InlineRole::Math), (":m:", InlineRole::Math)] {
        if let Some(before) = prefix.strip_suffix(marker) {
            return Some((before.to_string(), role));
        }
    }
    if let Some(before) = prefix.strip_suffix(":strike:") {
        return Some((before.to_string(), InlineRole::Strike));
    }
    None
}

fn expand_footnote_refs(text: &str, state: &mut RestState) -> Vec<ParsedInline> {
    let mut out = Vec::new();
    expand_footnote_refs_impl(text, state, &mut out);
    if out.is_empty() && !text.is_empty() {
        out.push(ParsedInline::Text(text.to_string()));
    }
    out
}

fn expand_footnote_refs_impl(text: &str, state: &mut RestState, out: &mut Vec<ParsedInline>) {
    let mut start = 0;
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < text.len() {
        if bytes[index] != b'[' {
            index += 1;
            continue;
        }
        let Some((label, end)) = read_footnote_reference(text, index) else {
            index += 1;
            continue;
        };
        if start < index {
            out.push(ParsedInline::Text(text[start..index].to_string()));
        }
        let footnote_id = state.footnote_id_for_label(&label);
        let display = state.footnote_display_for(footnote_id);
        out.push(ParsedInline::FootnoteReference {
            footnote_id,
            display,
        });
        index = end;
        start = end;
    }
    if start < text.len() {
        out.push(ParsedInline::Text(text[start..].to_string()));
    }
}

fn read_footnote_reference(text: &str, open: usize) -> Option<(String, usize)> {
    let rest = text.get(open + 1..)?;
    let close = rest.find(']')?;
    let label = rest[..close].to_string();
    if label.is_empty() {
        return None;
    }
    let after = open + 1 + close + 1;
    text.as_bytes().get(after).copied().filter(|b| *b == b'_')?;
    Some((label, after + 1))
}

fn parse_checklist_item_prefix(
    inlines: &[RstInline],
    state: &mut RestState,
) -> (Vec<RstInline>, Option<u32>, bool) {
    let plain = rst_inline_plain(inlines);
    let Some((checked, rest)) = parse_checkbox_prefix(&plain) else {
        return (inlines.to_vec(), None, false);
    };
    let id = state.parts.next_checklist_id();
    let remaining = if inlines.len() == 1 && matches!(&inlines[0], RstInline::Text(_)) {
        if rest.is_empty() {
            Vec::new()
        } else {
            vec![RstInline::Text(rest)]
        }
    } else {
        vec![RstInline::Text(rest)]
    };
    (remaining, Some(id), checked)
}

fn parse_checkbox_prefix(text: &str) -> Option<(bool, String)> {
    let trimmed = text.trim_start();
    if let Some(rest) = trimmed.strip_prefix("[ ]") {
        return Some((false, rest.trim_start().to_string()));
    }
    for prefix in ["[x]", "[X]"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return Some((true, rest.trim_start().to_string()));
        }
    }
    None
}

fn paragraph_is_horizontal_rule(inlines: &[RstInline]) -> bool {
    if inlines.len() != 1 {
        return false;
    }
    let RstInline::Text(text) = &inlines[0] else {
        return false;
    };
    is_transition_marker(text.trim())
}

fn is_transition_marker(text: &str) -> bool {
    if text.len() < 4 {
        return false;
    }
    let Some(marker) = text.chars().next() else {
        return false;
    };
    if marker == '=' || !matches!(
        marker,
        '*' | '`' | ':' | '|' | '_' | '-' | '#' | '.' | '^' | '"' | '~' | '+' | '\''
    ) {
        return false;
    }
    text.chars().all(|ch| ch == marker)
}

fn rst_inline_plain(inlines: &[RstInline]) -> String {
    inlines
        .iter()
        .map(|inline| match inline {
            RstInline::Text(text) => text.clone(),
            RstInline::Code(code) => code.clone(),
            RstInline::Em(children) | RstInline::Strong(children) => rst_inline_plain(children),
            RstInline::Link { text, .. } => rst_inline_plain(text),
        })
        .collect()
}

fn mermaid_link_label(source: &str) -> String {
    let first_line = source.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        "[mermaid diagram]".to_string()
    } else {
        format!("[mermaid: {first_line}]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Alignment, Block, Inline, LinkKind};

    #[test]
    fn parse_rest_heading_and_emphasis() {
        let dto = parse("Title\n=====\n\nHello *world*.").unwrap();
        let doc = dto.into_domain().unwrap();
        assert!(matches!(doc.blocks[0], Block::Heading(_)));
        let Block::Paragraph(inlines) = &doc.blocks[1] else {
            panic!("expected paragraph");
        };
        assert!(matches!(inlines[1], Inline::Emphasis(_)));
    }

    #[test]
    fn parse_rest_mermaid_directive() {
        let dto = parse(".. mermaid::\n\n   graph TD; A-->B;\n").unwrap();
        let doc = dto.into_domain().unwrap();
        assert_eq!(doc.links.len(), 1);
        assert_eq!(doc.links[0].kind, LinkKind::Mermaid);
    }

    #[test]
    fn parse_rest_admonition_as_blockquote() {
        let dto = parse(".. note::\n\n   Remember this.\n").unwrap();
        assert!(matches!(dto.blocks[0], ParsedBlock::BlockQuote(_)));
    }

    #[test]
    fn rest_heading_carries_github_slug_anchor() {
        let dto = parse("Hello World\n===========\n").unwrap();
        let ParsedBlock::Heading(heading) = &dto.blocks[0] else {
            panic!("expected heading");
        };
        assert_eq!(heading.anchor.as_deref(), Some("hello-world"));
    }

    #[test]
    fn parse_rest_hash_link_becomes_anchor() {
        let dto = parse("Title\n=====\n\n`Jump <#hello-world>`_ now.\n").unwrap();
        let doc = dto.into_domain().unwrap();
        assert_eq!(doc.links.len(), 1);
        assert_eq!(doc.links[0].kind, LinkKind::Anchor);
        assert_eq!(doc.links[0].url.as_str(), "#hello-world");
    }

    #[test]
    fn parse_rest_field_list_maps_term_and_body() {
        let dto = parse("Title\n=====\n\n:Author: Jane Doe\n\nBody paragraph.\n").unwrap();
        let ParsedBlock::DefinitionList(list) = &dto.blocks[1] else {
            panic!("expected definition list, got {:?}", dto.blocks[1]);
        };
        assert_eq!(list.items.len(), 1);
        assert!(matches!(
            &list.items[0].term[0],
            ParsedInline::Text(t) if t == "Author"
        ));
        let ParsedBlock::Paragraph(inlines) = &list.items[0].definitions[0][0] else {
            panic!("expected definition paragraph");
        };
        assert!(matches!(&inlines[0], ParsedInline::Text(t) if t == "Jane Doe"));
    }

    #[test]
    fn parse_rest_comment_is_dropped() {
        let dto = parse("Visible.\n\n.. Hidden comment\n\nMore.\n").unwrap();
        assert_eq!(dto.blocks.len(), 2);
        let Block::Paragraph(inlines) = &dto.into_domain().unwrap().blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(matches!(&inlines[0], Inline::Text(t) if t == "Visible."));
    }

    #[test]
    fn parse_rest_image_directive() {
        let dto = parse(".. image:: /img/logo.png\n").unwrap();
        let doc = dto.into_domain().unwrap();
        assert_eq!(doc.links.len(), 1);
        assert_eq!(doc.links[0].kind, LinkKind::Image);
        assert_eq!(doc.links[0].url.as_str(), "/img/logo.png");
    }

    #[test]
    fn parse_rest_code_directive_preserves_language() {
        let doc = ".. code:: rust\n\n    fn main() {}\n";
        let dto = parse(doc).unwrap();
        let domain = dto.into_domain().unwrap();
        let Block::CodeBlock(code) = &domain.blocks[0] else {
            panic!("expected code block");
        };
        assert_eq!(code.language.as_deref(), Some("rust"));
        assert!(code.content.contains("fn main()"));
    }

    #[test]
    fn parse_rest_footnote_reference_and_definition() {
        let dto = parse("Text [#note]_.\n\n.. [#note] Footnote body.\n").unwrap();
        assert_eq!(dto.footnotes.len(), 1);
        assert_eq!(dto.footnote_order, vec![0]);
        assert_eq!(dto.footnotes[0].label, "#note");
        let doc = dto.into_domain().unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(matches!(
            &inlines[1],
            Inline::FootnoteReference(crate::domain::FootnoteId(0), 1)
        ));
    }

    #[test]
    fn parse_rest_leading_field_list_becomes_front_matter() {
        let dto = parse(":Author: Jane Doe\n:Date: 2025-01-01\n\nBody.\n").unwrap();
        let front_matter = dto.front_matter.expect("front matter");
        assert!(front_matter.raw.contains("Author: Jane Doe"));
        assert!(front_matter.raw.contains("Date: 2025-01-01"));
        assert_eq!(dto.blocks.len(), 1);
    }

    #[test]
    fn parse_rest_math_directive() {
        let dto = parse(".. math::\n\n    x^2 + y^2 = z^2\n").unwrap();
        let ParsedBlock::MathBlock(math) = &dto.blocks[0] else {
            panic!("expected math block, got {:?}", dto.blocks);
        };
        assert!(math.content.contains("x^2"));
    }

    #[test]
    fn parse_rest_field_list_as_definition_list() {
        let dto = parse("Intro.\n\n:Author: Jane Doe\n:License: MIT\n\nBody.\n").unwrap();
        let ParsedBlock::DefinitionList(list) = &dto.blocks[1] else {
            panic!("expected definition list, got {:?}", dto.blocks);
        };
        assert_eq!(list.items.len(), 2);
        assert!(matches!(
            &list.items[0].term[0],
            ParsedInline::Text(t) if t == "Author"
        ));
    }

    #[test]
    fn parse_rest_nested_list_preserves_structure() {
        let doc = parse("- One\n  - Nested\n- Two").unwrap().into_domain().unwrap();
        let Block::List(list) = &doc.blocks[0] else {
            panic!("expected list");
        };
        assert_eq!(list.items.len(), 2);
        let Block::List(nested) = &list.items[0].content[1] else {
            panic!("expected nested list, got {:?}", list.items[0].content);
        };
        assert_eq!(nested.items.len(), 1);
    }

    #[test]
    fn parse_rest_list_item_continuation() {
        let doc = parse("- One\n\n  Continuation.\n\n- Two").unwrap().into_domain().unwrap();
        let Block::List(list) = &doc.blocks[0] else {
            panic!("expected list");
        };
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].content.len(), 2);
        let Block::Paragraph(inlines) = &list.items[0].content[1] else {
            panic!("expected continuation paragraph");
        };
        assert!(matches!(&inlines[0], Inline::Text(t) if t == "Continuation."));
    }

    #[test]
    fn parse_rest_horizontal_rule() {
        let doc = parse("Paragraph.\n\n----\n\nMore.\n").unwrap().into_domain().unwrap();
        assert!(matches!(doc.blocks[0], Block::Paragraph(_)));
        assert!(matches!(doc.blocks[1], Block::Rule));
        assert!(matches!(doc.blocks[2], Block::Paragraph(_)));
    }

    #[test]
    fn parse_rest_inline_math_role() {
        let doc = parse("Text :math:`x^2` here.").unwrap().into_domain().unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(inlines.iter().any(|inline| matches!(inline, Inline::Math(s) if s == "x^2")));
    }

    #[test]
    fn parse_rest_hard_line_break() {
        let doc = parse("line one\\\nline two").unwrap().into_domain().unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(inlines.iter().any(|inline| matches!(inline, Inline::HardBreak)));
    }

    #[test]
    fn parse_rest_strikethrough_markup() {
        let doc = parse("~~deleted~~").unwrap().into_domain().unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(matches!(
            &inlines[0],
            Inline::Strikethrough(children) if children == &[Inline::Text("deleted".into())]
        ));
    }

    #[test]
    fn parse_rest_table_column_alignments() {
        let source = "=====  =====\nLeft   Right\n=====  =====\nA      B\n-----  ------:\nC       D\n=====  =====\n";
        let doc = parse(source).unwrap().into_domain().unwrap();
        let Block::Table(table) = &doc.blocks[0] else {
            panic!("expected table");
        };
        assert_eq!(table.alignments, vec![Alignment::Left, Alignment::Right]);
        assert_eq!(table.rows.len(), 2);
    }

    #[test]
    fn parse_rest_checklist_items() {
        let doc = parse("- [ ] Todo\n- [x] Done").unwrap().into_domain().unwrap();
        let Block::List(list) = &doc.blocks[0] else {
            panic!("expected list");
        };
        assert!(list.items[0].checklist_id.is_some());
        assert!(!list.items[0].checked);
        assert!(list.items[1].checklist_id.is_some());
        assert!(list.items[1].checked);
    }
}
