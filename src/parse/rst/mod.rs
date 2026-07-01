//! reStructuredText parser: parserst AST -> DTO.

use parserst::{Block as RstBlock, Field, Inline as RstInline, ListKind};

use crate::parse::dto::{
    ParsedBlock, ParsedCodeBlock, ParsedDocument, ParsedDocumentParts, ParsedHeading, ParsedInline,
    ParsedLink, ParsedLinkKind, ParsedList, ParsedListItem, ParsedTable,
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

/// Parse reStructuredText into a [`ParsedDocument`].
pub fn parse(content: &str) -> Result<ParsedDocument, ParseError> {
    let blocks = parserst::parse(content)
        .map_err(|error| ParseError::syntax(MarkupFormat::Rest, error.to_string()))?;
    let mut parts = ParsedDocumentParts::default();
    let parsed_blocks = map_blocks(&blocks, &mut parts)?;
    Ok(parts.into_document(parsed_blocks))
}

fn map_blocks(
    blocks: &[RstBlock],
    parts: &mut ParsedDocumentParts,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let mut out = Vec::new();
    for block in blocks {
        out.extend(map_block(block, parts)?);
    }
    Ok(out)
}

fn map_block(
    block: &RstBlock,
    parts: &mut ParsedDocumentParts,
) -> Result<Vec<ParsedBlock>, ParseError> {
    Ok(match block {
        RstBlock::Heading { level, inlines } => {
            ParseError::ensure_heading_level(MarkupFormat::Rest, *level)?;
            vec![ParsedBlock::Heading(ParsedHeading {
                level: *level,
                content: map_inlines(inlines, parts),
                anchor: Some(slugify_heading(&rst_inline_plain(inlines))),
            })]
        }
        RstBlock::Paragraph(inlines) => vec![ParsedBlock::Paragraph(map_inlines(inlines, parts))],
        RstBlock::CodeBlock(content) | RstBlock::LiteralBlock(content) => {
            vec![ParsedBlock::CodeBlock(ParsedCodeBlock {
                language: None,
                content: content.clone(),
            })]
        }
        RstBlock::Quote(nested) => vec![ParsedBlock::BlockQuote(map_blocks(nested, parts)?)],
        RstBlock::List { kind, items } => vec![ParsedBlock::List(ParsedList {
            ordered: matches!(kind, ListKind::Ordered),
            items: items
                .iter()
                .map(|item| {
                    ParsedListItem::plain(vec![ParsedBlock::Paragraph(map_inlines(item, parts))])
                })
                .collect(),
        })],
        RstBlock::Table { headers, rows } => vec![ParsedBlock::Table(ParsedTable {
            headers: headers
                .iter()
                .map(|cell| map_inlines(cell, parts))
                .collect(),
            rows: rows
                .iter()
                .map(|row| row.iter().map(|cell| map_inlines(cell, parts)).collect())
                .collect(),
            alignments: vec![],
        })],
        RstBlock::Directive {
            name,
            argument,
            content,
        } => map_directive(name, argument, content, parts)?,
        RstBlock::Comment(nested) => map_blocks(nested, parts)?,
        RstBlock::FieldList { fields } => map_field_list(fields, parts)?,
    })
}

fn map_directive(
    name: &str,
    argument: &str,
    content: &[RstBlock],
    parts: &mut ParsedDocumentParts,
) -> Result<Vec<ParsedBlock>, ParseError> {
    if name.eq_ignore_ascii_case("code-block") || name.eq_ignore_ascii_case("sourcecode") {
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
        let (link_id, _url) = parts.push_mermaid(source.clone());
        let label = mermaid_link_label(&source);
        return Ok(vec![ParsedBlock::Paragraph(vec![ParsedInline::Link {
            link_id,
            children: vec![ParsedInline::Text(label)],
        }])]);
    }
    if is_admonition(name) {
        return map_admonition(name, content, parts);
    }
    map_blocks(content, parts)
}

fn is_admonition(name: &str) -> bool {
    ADMONITIONS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(name))
}

fn map_admonition(
    name: &str,
    content: &[RstBlock],
    parts: &mut ParsedDocumentParts,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let mut inner = map_blocks(content, parts)?;
    inner.insert(
        0,
        ParsedBlock::Paragraph(vec![ParsedInline::Strong(vec![ParsedInline::Text(
            format!("{}:", name.to_ascii_lowercase()),
        )])]),
    );
    Ok(vec![ParsedBlock::BlockQuote(inner)])
}

fn map_field_list(
    fields: &[Field],
    parts: &mut ParsedDocumentParts,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let mut out = Vec::new();
    for field in fields {
        let mut inlines = vec![
            ParsedInline::Strong(vec![ParsedInline::Text(field.name.clone())]),
            ParsedInline::Text(": ".into()),
        ];
        if !field.argument.is_empty() {
            inlines.push(ParsedInline::Text(field.argument.clone()));
        }
        out.push(ParsedBlock::Paragraph(inlines));
        out.extend(map_blocks(&field.body, parts)?);
    }
    Ok(out)
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

fn map_inlines(inlines: &[RstInline], parts: &mut ParsedDocumentParts) -> Vec<ParsedInline> {
    inlines
        .iter()
        .map(|inline| map_inline(inline, parts))
        .collect()
}

fn map_inline(inline: &RstInline, parts: &mut ParsedDocumentParts) -> ParsedInline {
    match inline {
        RstInline::Text(text) => ParsedInline::Text(text.clone()),
        RstInline::Em(children) => ParsedInline::Emphasis(map_inlines(children, parts)),
        RstInline::Strong(children) => ParsedInline::Strong(map_inlines(children, parts)),
        RstInline::Code(code) => ParsedInline::Code(code.clone()),
        RstInline::Link { text, url } => {
            let (url, kind) = if url.starts_with('#') {
                (anchor_href(url), ParsedLinkKind::Anchor)
            } else {
                (url.clone(), ParsedLinkKind::classify_url(url))
            };
            let link_id = parts.push_link(ParsedLink::new(url, None, kind));
            ParsedInline::Link {
                link_id,
                children: map_inlines(text, parts),
            }
        }
    }
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
    use crate::domain::LinkKind;

    #[test]
    fn parse_rest_heading_and_emphasis() {
        let dto = parse("Title\n=====\n\nHello *world*.").unwrap();
        let doc = dto.into_domain().unwrap();
        assert!(matches!(doc.blocks[0], crate::domain::Block::Heading(_)));
        let crate::domain::Block::Paragraph(inlines) = &doc.blocks[1] else {
            panic!("expected paragraph");
        };
        assert!(matches!(inlines[1], crate::domain::Inline::Emphasis(_)));
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
        let dto = parse(":Author: Jane Doe\n\nBody paragraph.\n").unwrap();
        assert!(matches!(dto.blocks[0], ParsedBlock::Paragraph(_)));
    }
}
