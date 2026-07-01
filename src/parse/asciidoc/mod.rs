//! AsciiDoc parser: acdc-parser AST -> DTO.

use acdc_parser::{
    self, Admonition, Block as AdocBlock, BlockMetadata, DelimitedBlockType, HorizontalAlignment,
    InlineMacro, InlineNode, ListItemCheckedStatus, Options, Source, Table,
};

use crate::parse::dto::{
    ParsedAlignment, ParsedBlock, ParsedCodeBlock, ParsedDocument, ParsedDocumentParts,
    ParsedHeading, ParsedInline, ParsedLink, ParsedLinkKind, ParsedList, ParsedListItem,
    ParsedTable,
};
use crate::parse::error::ParseError;
use crate::parse::format::MarkupFormat;
use crate::parse::slug::{anchor_href, normalize_anchor_slug, slugify_heading};

/// Parse AsciiDoc into a [`ParsedDocument`].
pub fn parse(content: &str) -> Result<ParsedDocument, ParseError> {
    let options = Options::default();
    let parsed = acdc_parser::parse(content, &options)
        .map_err(|error| ParseError::syntax(MarkupFormat::AsciiDoc, error.to_string()))?;
    let mut parts = ParsedDocumentParts::default();
    let mut blocks = Vec::new();
    if let Some(header) = parsed.document().header.as_ref() {
        let title = acdc_parser::inlines_to_string(&header.title);
        blocks.push(ParsedBlock::Heading(ParsedHeading {
            level: 1,
            content: map_inlines(&header.title, &mut parts),
            anchor: Some(slugify_heading(&title)),
        }));
    }
    blocks.extend(map_blocks(&parsed.document().blocks, &mut parts)?);
    Ok(parts.into_document(blocks))
}

fn map_blocks(
    blocks: &[AdocBlock<'_>],
    parts: &mut ParsedDocumentParts,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let mut out = Vec::new();
    for block in blocks {
        out.extend(map_block(block, parts)?);
    }
    Ok(out)
}

fn map_block(
    block: &AdocBlock<'_>,
    parts: &mut ParsedDocumentParts,
) -> Result<Vec<ParsedBlock>, ParseError> {
    Ok(match block {
        AdocBlock::Section(section) => {
            let title = acdc_parser::inlines_to_string(&section.title);
            let level = section_heading_level(section.level)?;
            let mut mapped = vec![ParsedBlock::Heading(ParsedHeading {
                level,
                content: map_inlines(&section.title, parts),
                anchor: section_anchor(&section.metadata).or_else(|| Some(slugify_heading(&title))),
            })];
            mapped.extend(map_blocks(&section.content, parts)?);
            mapped
        }
        AdocBlock::Paragraph(paragraph) => {
            vec![ParsedBlock::Paragraph(map_inlines(
                &paragraph.content,
                parts,
            ))]
        }
        AdocBlock::DelimitedBlock(delimited) => map_delimited_block(delimited, parts)?,
        AdocBlock::UnorderedList(list) => vec![ParsedBlock::List(ParsedList {
            ordered: false,
            items: list
                .items
                .iter()
                .map(|item| map_list_item(item, parts))
                .collect::<Result<Vec<_>, _>>()?,
        })],
        AdocBlock::OrderedList(list) => vec![ParsedBlock::List(ParsedList {
            ordered: true,
            items: list
                .items
                .iter()
                .map(|item| map_list_item(item, parts))
                .collect::<Result<Vec<_>, _>>()?,
        })],
        AdocBlock::ThematicBreak(_) => vec![ParsedBlock::Rule],
        AdocBlock::Image(image) => {
            let alt = acdc_parser::inlines_to_string(&image.title);
            let url = source_to_string(&image.source);
            let link_id = parts.push_link(ParsedLink::new(url, None, ParsedLinkKind::Image));
            vec![ParsedBlock::Paragraph(vec![ParsedInline::Link {
                link_id,
                children: vec![ParsedInline::Text(alt)],
            }])]
        }
        AdocBlock::Admonition(admonition) => vec![map_admonition(admonition, parts)?],
        AdocBlock::DescriptionList(list) => map_description_list(list, parts)?,
        AdocBlock::Comment(_)
        | AdocBlock::DocumentAttribute(_)
        | AdocBlock::DiscreteHeader(_)
        | AdocBlock::PageBreak(_)
        | AdocBlock::TableOfContents(_)
        | AdocBlock::CalloutList(_)
        | AdocBlock::Audio(_)
        | AdocBlock::Video(_)
        | _ => Vec::new(),
    })
}

fn map_admonition(
    admonition: &Admonition<'_>,
    parts: &mut ParsedDocumentParts,
) -> Result<ParsedBlock, ParseError> {
    let mut inner = map_blocks(&admonition.blocks, parts)?;
    inner.insert(
        0,
        ParsedBlock::Paragraph(vec![ParsedInline::Strong(vec![ParsedInline::Text(
            format!("{}:", admonition.variant),
        )])]),
    );
    Ok(ParsedBlock::BlockQuote(inner))
}

fn map_description_list(
    list: &acdc_parser::DescriptionList<'_>,
    parts: &mut ParsedDocumentParts,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let mut out = Vec::new();
    for item in &list.items {
        let mut inlines = map_inlines(&item.term, parts);
        if !inlines.is_empty() {
            inlines.push(ParsedInline::Text(": ".into()));
        }
        inlines.extend(map_inlines(&item.principal_text, parts));
        if !inlines.is_empty() {
            out.push(ParsedBlock::Paragraph(inlines));
        }
        out.extend(map_blocks(&item.description, parts)?);
    }
    Ok(out)
}

fn map_delimited_block(
    delimited: &acdc_parser::DelimitedBlock<'_>,
    parts: &mut ParsedDocumentParts,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let style = delimited.metadata.style.unwrap_or("");
    if style.eq_ignore_ascii_case("mermaid") {
        let source = verbatim_content(&delimited.inner);
        let (link_id, _url) = parts.push_mermaid(source.clone());
        return Ok(vec![ParsedBlock::Paragraph(vec![ParsedInline::Link {
            link_id,
            children: vec![ParsedInline::Text(mermaid_link_label(&source))],
        }])]);
    }

    match &delimited.inner {
        DelimitedBlockType::DelimitedListing(inlines)
        | DelimitedBlockType::DelimitedLiteral(inlines) => {
            let language = delimited
                .metadata
                .attributes
                .iter()
                .next()
                .map(|(name, _)| name.to_string());
            Ok(vec![ParsedBlock::CodeBlock(ParsedCodeBlock {
                language: if style == "source" { language } else { None },
                content: acdc_parser::inlines_to_string(inlines),
            })])
        }
        DelimitedBlockType::DelimitedQuote(blocks) => {
            Ok(vec![ParsedBlock::BlockQuote(map_blocks(blocks, parts)?)])
        }
        DelimitedBlockType::DelimitedExample(blocks)
        | DelimitedBlockType::DelimitedOpen(blocks)
        | DelimitedBlockType::DelimitedSidebar(blocks) => map_blocks(blocks, parts),
        DelimitedBlockType::DelimitedTable(table) => Ok(vec![map_table(table, parts)?]),
        DelimitedBlockType::DelimitedComment(_)
        | DelimitedBlockType::DelimitedPass(_)
        | DelimitedBlockType::DelimitedVerse(_)
        | DelimitedBlockType::DelimitedStem(_)
        | _ => Ok(Vec::new()),
    }
}

fn map_table(
    table: &Table<'_>,
    parts: &mut ParsedDocumentParts,
) -> Result<ParsedBlock, ParseError> {
    let headers = if let Some(header) = &table.header {
        header
            .columns
            .iter()
            .map(|column| cell_to_inlines(&column.content, parts))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let rows = table
        .rows
        .iter()
        .map(|row| {
            row.columns
                .iter()
                .map(|column| cell_to_inlines(&column.content, parts))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
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

fn cell_to_inlines(
    blocks: &[AdocBlock<'_>],
    parts: &mut ParsedDocumentParts,
) -> Result<Vec<ParsedInline>, ParseError> {
    let mut paragraphs = Vec::new();
    for block in blocks {
        match block {
            AdocBlock::Paragraph(paragraph) => {
                paragraphs.push(map_inlines(&paragraph.content, parts));
            }
            other => {
                for mapped in map_block(other, parts)? {
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

fn section_heading_level(section_level: u8) -> Result<u8, ParseError> {
    let level = section_level.saturating_add(1);
    ParseError::ensure_heading_level(MarkupFormat::AsciiDoc, level)?;
    Ok(level)
}

fn section_anchor(metadata: &BlockMetadata<'_>) -> Option<String> {
    metadata
        .id
        .as_ref()
        .map(|anchor| normalize_anchor_slug(anchor.id))
        .or_else(|| {
            metadata
                .anchors
                .first()
                .map(|anchor| normalize_anchor_slug(anchor.id))
        })
}

fn map_list_item(
    item: &acdc_parser::ListItem<'_>,
    parts: &mut ParsedDocumentParts,
) -> Result<ParsedListItem, ParseError> {
    let mut content = Vec::new();
    if !item.principal.is_empty() {
        content.push(ParsedBlock::Paragraph(map_inlines(&item.principal, parts)));
    }
    content.extend(map_blocks(&item.blocks, parts)?);
    let (checklist_id, checked) = match item.checked {
        Some(ListItemCheckedStatus::Checked) => (Some(parts.next_checklist_id()), true),
        Some(ListItemCheckedStatus::Unchecked) => (Some(parts.next_checklist_id()), false),
        None | Some(_) => (None, false),
    };
    Ok(ParsedListItem {
        checklist_id,
        checked,
        content,
    })
}

fn map_inlines(inlines: &[InlineNode<'_>], parts: &mut ParsedDocumentParts) -> Vec<ParsedInline> {
    inlines
        .iter()
        .flat_map(|inline| map_inline(inline, parts))
        .collect()
}

fn map_inline(inline: &InlineNode<'_>, parts: &mut ParsedDocumentParts) -> Vec<ParsedInline> {
    match inline {
        InlineNode::PlainText(plain) => vec![ParsedInline::Text(plain.content.to_string())],
        InlineNode::RawText(raw) => vec![ParsedInline::Text(raw.content.to_string())],
        InlineNode::VerbatimText(verbatim) => {
            vec![ParsedInline::Code(verbatim.content.to_string())]
        }
        InlineNode::BoldText(bold) => vec![ParsedInline::Strong(map_inlines(&bold.content, parts))],
        InlineNode::ItalicText(italic) => {
            vec![ParsedInline::Emphasis(map_inlines(&italic.content, parts))]
        }
        InlineNode::MonospaceText(mono) => {
            vec![ParsedInline::Code(acdc_parser::inlines_to_string(
                &mono.content,
            ))]
        }
        InlineNode::LineBreak(_) => vec![ParsedInline::HardBreak],
        InlineNode::HighlightText(node) => map_inlines(&node.content, parts),
        InlineNode::SubscriptText(node) => map_inlines(&node.content, parts),
        InlineNode::SuperscriptText(node) => map_inlines(&node.content, parts),
        InlineNode::CurvedQuotationText(node) => map_inlines(&node.content, parts),
        InlineNode::CurvedApostropheText(node) => map_inlines(&node.content, parts),
        InlineNode::StandaloneCurvedApostrophe(_) => vec![ParsedInline::Text("'".into())],
        InlineNode::Macro(macro_node) => map_inline_macro(macro_node, parts),
        InlineNode::InlineAnchor(_) | InlineNode::CalloutRef(_) => Vec::new(),
        _ => Vec::new(),
    }
}

fn map_inline_macro(
    macro_node: &InlineMacro<'_>,
    parts: &mut ParsedDocumentParts,
) -> Vec<ParsedInline> {
    match macro_node {
        InlineMacro::Link(link) => {
            let parsed = classify_link_target(source_to_string(&link.target));
            let link_id = parts.push_link(parsed);
            vec![ParsedInline::Link {
                link_id,
                children: map_inlines(&link.text, parts),
            }]
        }
        InlineMacro::Url(url) => {
            let link = classify_link_target(source_to_string(&url.target));
            let display = link.url.clone();
            let link_id = parts.push_link(link);
            vec![ParsedInline::Link {
                link_id,
                children: vec![ParsedInline::Text(display)],
            }]
        }
        InlineMacro::Mailto(mailto) => {
            let target =
                ParsedLink::from_url(format!("mailto:{}", source_to_string(&mailto.target)), None);
            let link_id = parts.push_link(target);
            vec![ParsedInline::Link {
                link_id,
                children: map_inlines(&mailto.text, parts),
            }]
        }
        InlineMacro::Autolink(autolink) => {
            let url = source_to_string(&autolink.url);
            let link_id = parts.push_link(ParsedLink::from_url(url.clone(), None));
            vec![ParsedInline::Link {
                link_id,
                children: vec![ParsedInline::Text(url)],
            }]
        }
        InlineMacro::CrossReference(xref) => {
            let link_id = parts.push_link(ParsedLink::new(
                anchor_href(xref.target),
                None,
                ParsedLinkKind::Anchor,
            ));
            vec![ParsedInline::Link {
                link_id,
                children: map_inlines(&xref.text, parts),
            }]
        }
        InlineMacro::Image(image) => {
            let url = source_to_string(&image.source);
            let link_id = parts.push_link(ParsedLink::new(url, None, ParsedLinkKind::Image));
            vec![ParsedInline::Link {
                link_id,
                children: map_inlines(&image.title, parts),
            }]
        }
        InlineMacro::Footnote(_)
        | InlineMacro::Icon(_)
        | InlineMacro::Keyboard(_)
        | InlineMacro::Button(_)
        | InlineMacro::Menu(_)
        | InlineMacro::Pass(_)
        | InlineMacro::Stem(_)
        | InlineMacro::IndexTerm(_)
        | _ => Vec::new(),
    }
}

fn classify_link_target(url: String) -> ParsedLink {
    if url.starts_with('#') {
        ParsedLink::new(anchor_href(&url), None, ParsedLinkKind::Anchor)
    } else {
        ParsedLink::from_url(url, None)
    }
}

fn verbatim_content(inner: &DelimitedBlockType<'_>) -> String {
    match inner {
        DelimitedBlockType::DelimitedListing(inlines)
        | DelimitedBlockType::DelimitedLiteral(inlines)
        | DelimitedBlockType::DelimitedPass(inlines)
        | DelimitedBlockType::DelimitedVerse(inlines) => acdc_parser::inlines_to_string(inlines),
        _ => String::new(),
    }
}

fn source_to_string(source: &Source<'_>) -> String {
    source.to_string()
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
    use crate::domain::{Block, LinkKind};
    use crate::parse::error::ParseError;
    use crate::parse::format::MarkupFormat;

    #[test]
    fn parse_asciidoc_heading_and_emphasis() {
        let dto = parse("= Title\n\nHello *world*.\n").unwrap();
        let doc = dto.into_domain().unwrap();
        assert!(matches!(doc.blocks[0], Block::Heading(_)));
        assert!(matches!(doc.blocks[1], Block::Paragraph(_)));
    }

    #[test]
    fn parse_asciidoc_mermaid_block() {
        let dto = parse("[mermaid]\n....\ngraph TD; A-->B;\n....\n").unwrap();
        let doc = dto.into_domain().unwrap();
        assert_eq!(doc.links[0].kind, LinkKind::Mermaid);
    }

    #[test]
    fn parse_asciidoc_admonition_as_blockquote() {
        let dto = parse("NOTE: Remember this.\n").unwrap();
        assert!(matches!(dto.blocks[0], ParsedBlock::BlockQuote(_)));
    }

    #[test]
    fn parse_asciidoc_xref_uses_github_slug() {
        let dto = parse("= Doc\n\n== Hello World\n\nxref:hello-world[Jump]\n").unwrap();
        let doc = dto.into_domain().unwrap();
        assert_eq!(doc.links[0].url.as_str(), "#hello-world");
        assert_eq!(doc.links[0].kind, LinkKind::Anchor);
    }

    #[test]
    fn parse_asciidoc_rejects_heading_level_beyond_h6() {
        assert!(matches!(
            section_heading_level(6),
            Err(ParseError::InvalidHeadingLevel {
                format: MarkupFormat::AsciiDoc,
                level: 7,
            })
        ));
    }

    #[test]
    fn parse_asciidoc_description_list_orders_term_before_colon() {
        let dto = parse("= Doc\n\nname:: value\n").unwrap();
        let ParsedBlock::Paragraph(inlines) = &dto.blocks[1] else {
            panic!("expected description paragraph, got {:?}", dto.blocks);
        };
        assert!(matches!(&inlines[0], ParsedInline::Text(t) if t == "name"));
        assert!(matches!(&inlines[1], ParsedInline::Text(t) if t == ": "));
        assert!(matches!(&inlines[2], ParsedInline::Text(t) if t == "value"));
    }

    #[test]
    fn parse_asciidoc_delimited_table() {
        let dto = parse("|===\n|Name |Value\n\n|alpha |1\n|beta |2\n|===\n").unwrap();
        let doc = dto.into_domain().unwrap();
        let Block::Table(table) = &doc.blocks[0] else {
            panic!("expected table");
        };
        assert_eq!(table.headers.len(), 2);
        assert_eq!(table.rows.len(), 2);
    }
}
