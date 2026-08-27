//! AsciiDoc parser: acdc-parser AST -> DTO.

mod inline;
mod table;

use acdc_parser::{
    self, Admonition, AttributeValue, Author, Block as AdocBlock, BlockMetadata,
    DelimitedBlockType, Footnote as AdocFootnote, ListItemCheckedStatus, Options, StemContent,
    TocEntry,
};

use crate::domain::{anchor_href, normalize_anchor_slug, slugify_heading};
use crate::parse::dto::{
    ParsedBlock, ParsedCodeBlock, ParsedDefinitionItem, ParsedDefinitionList, ParsedDocument,
    ParsedDocumentParts, ParsedFootnoteDefinition, ParsedFrontMatter, ParsedFrontMatterKind,
    ParsedHeading, ParsedInline, ParsedLink, ParsedLinkKind, ParsedList, ParsedListItem,
    ParsedMathBlock,
};
use crate::parse::error::ParseError;
use crate::parse::format::MarkupFormat;

use inline::{
    map_inline_without_state, map_inlines, map_verbatim_inlines, mermaid_link_label,
    source_to_string, verbatim_content,
};
use table::map_table;

struct AsciiDocState<'a> {
    parts: ParsedDocumentParts,
    footnotes: Vec<ParsedFootnoteDefinition>,
    footnote_order: Vec<usize>,
    front_matter: Option<ParsedFrontMatter>,
    toc_entries: &'a [TocEntry<'a>],
}

impl<'a> AsciiDocState<'a> {
    fn new(
        footnotes: Vec<ParsedFootnoteDefinition>,
        front_matter: Option<ParsedFrontMatter>,
        toc_entries: &'a [TocEntry<'a>],
    ) -> Self {
        Self {
            parts: ParsedDocumentParts::default(),
            footnotes,
            footnote_order: Vec::new(),
            front_matter,
            toc_entries,
        }
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

    fn footnote_display_for(&mut self, footnote_id: usize) -> usize {
        if let Some(pos) = self.footnote_order.iter().position(|&id| id == footnote_id) {
            pos + 1
        } else {
            self.footnote_order.push(footnote_id);
            self.footnote_order.len()
        }
    }
}

/// Parse AsciiDoc into a [`ParsedDocument`].
pub fn parse(content: &str) -> Result<ParsedDocument, ParseError> {
    let options = Options::default();
    let parsed = acdc_parser::parse(content, &options)
        .map_err(|error| ParseError::syntax(MarkupFormat::AsciiDoc, error.to_string()))?;
    let doc = parsed.document();
    let footnotes = convert_document_footnotes(&doc.footnotes);
    let front_matter = build_front_matter(&doc.attributes, doc.header.as_ref());
    let mut state = AsciiDocState::new(footnotes, front_matter, &doc.toc_entries);
    let mut blocks = Vec::new();
    if let Some(header) = doc.header.as_ref() {
        let title = acdc_parser::inlines_to_string(&header.title);
        blocks.push(ParsedBlock::Heading(ParsedHeading {
            level: 1,
            content: map_inlines(&header.title, &mut state),
            anchor: section_anchor(&header.metadata).or_else(|| Some(slugify_heading(&title))),
        }));
        if let Some(subtitle) = &header.subtitle {
            blocks.push(ParsedBlock::Paragraph(map_inlines(subtitle, &mut state)));
        }
        for author in &header.authors {
            blocks.extend(author_blocks(author));
        }
    }
    blocks.extend(map_blocks(&doc.blocks, &mut state)?);
    Ok(state.into_document(blocks))
}

fn convert_document_footnotes(footnotes: &[AdocFootnote<'_>]) -> Vec<ParsedFootnoteDefinition> {
    footnotes
        .iter()
        .map(|footnote| ParsedFootnoteDefinition {
            label: footnote
                .id
                .map(str::to_string)
                .unwrap_or_else(|| footnote.number.to_string()),
            blocks: vec![ParsedBlock::Paragraph(
                footnote
                    .content
                    .iter()
                    .flat_map(|inline| map_inline_without_state(inline))
                    .collect(),
            )],
        })
        .collect()
}

fn build_front_matter(
    attributes: &acdc_parser::DocumentAttributes<'_>,
    header: Option<&acdc_parser::Header<'_>>,
) -> Option<ParsedFrontMatter> {
    let mut lines = Vec::new();
    for (name, value) in attributes.iter() {
        lines.push(format!("{name}: {}", attribute_value_yaml(value)));
    }
    if let Some(header) = header {
        if let Some(subtitle) = &header.subtitle {
            let text = acdc_parser::inlines_to_string(subtitle);
            if !text.is_empty() {
                lines.push(format!("subtitle: {text}"));
            }
        }
        for author in &header.authors {
            lines.push(format!("author: {}", author_display(author)));
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(ParsedFrontMatter {
        kind: ParsedFrontMatterKind::Yaml,
        raw: lines.join("\n"),
    })
}

fn attribute_value_yaml(value: &AttributeValue<'_>) -> String {
    match value {
        AttributeValue::String(text) => text.to_string(),
        AttributeValue::Bool(true) => "true".to_string(),
        AttributeValue::Bool(false) => "false".to_string(),
        AttributeValue::None => "null".to_string(),
        _ => String::new(),
    }
}

fn author_display(author: &Author<'_>) -> String {
    let mut name = author.first_name.to_string();
    if let Some(middle) = author.middle_name {
        name.push(' ');
        name.push_str(middle);
    }
    name.push(' ');
    name.push_str(author.last_name);
    if let Some(email) = author.email {
        name.push_str(" <");
        name.push_str(email);
        name.push('>');
    }
    name
}

fn author_blocks(author: &Author<'_>) -> Vec<ParsedBlock> {
    vec![ParsedBlock::Paragraph(vec![ParsedInline::Text(
        author_display(author),
    )])]
}

fn map_blocks(
    blocks: &[AdocBlock<'_>],
    state: &mut AsciiDocState<'_>,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let mut out = Vec::new();
    for block in blocks {
        out.extend(map_block(block, state)?);
    }
    Ok(out)
}

fn map_block(
    block: &AdocBlock<'_>,
    state: &mut AsciiDocState<'_>,
) -> Result<Vec<ParsedBlock>, ParseError> {
    Ok(match block {
        AdocBlock::Section(section) => {
            let title = acdc_parser::inlines_to_string(&section.title);
            let level = section_heading_level(section.level)?;
            let mut mapped = vec![ParsedBlock::Heading(ParsedHeading {
                level,
                content: map_inlines(&section.title, state),
                anchor: section_anchor(&section.metadata).or_else(|| Some(slugify_heading(&title))),
            })];
            mapped.extend(map_blocks(&section.content, state)?);
            mapped
        }
        AdocBlock::Paragraph(paragraph) => {
            vec![ParsedBlock::Paragraph(map_inlines(
                &paragraph.content,
                state,
            ))]
        }
        AdocBlock::DelimitedBlock(delimited) => map_delimited_block(delimited, state)?,
        AdocBlock::UnorderedList(list) => vec![ParsedBlock::List(ParsedList {
            ordered: false,
            items: list
                .items
                .iter()
                .map(|item| map_list_item(item, state))
                .collect::<Result<Vec<_>, _>>()?,
        })],
        AdocBlock::OrderedList(list) => vec![ParsedBlock::List(ParsedList {
            ordered: true,
            items: list
                .items
                .iter()
                .map(|item| map_list_item(item, state))
                .collect::<Result<Vec<_>, _>>()?,
        })],
        AdocBlock::ThematicBreak(_) => vec![ParsedBlock::Rule],
        AdocBlock::Image(image) => {
            let alt = acdc_parser::inlines_to_string(&image.title);
            let url = source_to_string(&image.source);
            let link_id = state
                .parts
                .push_link(ParsedLink::new(url, None, ParsedLinkKind::Image));
            vec![ParsedBlock::Paragraph(vec![ParsedInline::Link {
                link_id,
                children: vec![ParsedInline::Text(alt)],
            }])]
        }
        AdocBlock::Admonition(admonition) => vec![map_admonition(admonition, state)?],
        AdocBlock::DescriptionList(list) => map_description_list(list, state)?,
        AdocBlock::DiscreteHeader(header) => {
            let title = acdc_parser::inlines_to_string(&header.title);
            let level = section_heading_level(header.level)?;
            vec![ParsedBlock::Heading(ParsedHeading {
                level,
                content: map_inlines(&header.title, state),
                anchor: section_anchor(&header.metadata).or_else(|| Some(slugify_heading(&title))),
            })]
        }
        AdocBlock::CalloutList(list) => map_callout_list(list, state)?,
        AdocBlock::TableOfContents(_) => map_table_of_contents(state),
        AdocBlock::PageBreak(_) => vec![ParsedBlock::Rule],
        AdocBlock::Audio(audio) => vec![media_block(
            source_to_string(&audio.source),
            acdc_parser::inlines_to_string(&audio.title),
            state,
        )],
        AdocBlock::Video(video) => {
            let url = video
                .sources
                .first()
                .map(source_to_string)
                .unwrap_or_default();
            vec![media_block(
                url,
                acdc_parser::inlines_to_string(&video.title),
                state,
            )]
        }
        AdocBlock::Comment(_) | AdocBlock::DocumentAttribute(_) | _ => Vec::new(),
    })
}

fn media_block(url: String, title: String, state: &mut AsciiDocState<'_>) -> ParsedBlock {
    let link_id = state
        .parts
        .push_link(ParsedLink::new(url, None, ParsedLinkKind::Web));
    let label = if title.is_empty() {
        "[media]".into()
    } else {
        title
    };
    ParsedBlock::Paragraph(vec![ParsedInline::Link {
        link_id,
        children: vec![ParsedInline::Text(label)],
    }])
}

fn map_table_of_contents(state: &mut AsciiDocState<'_>) -> Vec<ParsedBlock> {
    let entries: Vec<(String, usize, String)> = state
        .toc_entries
        .iter()
        .map(|entry| {
            (
                entry.id.to_string(),
                entry.level as usize,
                acdc_parser::inlines_to_string(&entry.title),
            )
        })
        .collect();
    if entries.is_empty() {
        return vec![ParsedBlock::Paragraph(vec![ParsedInline::Text(
            "[table of contents]".into(),
        )])];
    }
    let items = entries
        .into_iter()
        .map(|(id, level, title)| {
            let indent = "  ".repeat(level.saturating_sub(1));
            let link_id = state.parts.push_link(ParsedLink::new(
                anchor_href(&id),
                None,
                ParsedLinkKind::Anchor,
            ));
            ParsedListItem::plain(vec![ParsedBlock::Paragraph(vec![ParsedInline::Link {
                link_id,
                children: vec![ParsedInline::Text(format!("{indent}{title}"))],
            }])])
        })
        .collect();
    vec![ParsedBlock::List(ParsedList {
        ordered: false,
        items,
    })]
}

fn map_admonition(
    admonition: &Admonition<'_>,
    state: &mut AsciiDocState<'_>,
) -> Result<ParsedBlock, ParseError> {
    let mut inner = map_blocks(&admonition.blocks, state)?;
    inner.insert(
        0,
        ParsedBlock::Paragraph(vec![ParsedInline::Strong(vec![ParsedInline::Text(
            format!("{}:", admonition.variant),
        )])]),
    );
    Ok(ParsedBlock::BlockQuote(inner))
}

fn map_callout_list(
    list: &acdc_parser::CalloutList<'_>,
    state: &mut AsciiDocState<'_>,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let items = list
        .items
        .iter()
        .map(|item| {
            let mut inlines = vec![ParsedInline::Text(format!("<{}> ", item.callout.number))];
            inlines.extend(map_inlines(&item.principal, state));
            let mut content = vec![ParsedBlock::Paragraph(inlines)];
            content.extend(map_blocks(&item.blocks, state).unwrap_or_default());
            ParsedListItem::plain(content)
        })
        .collect();
    Ok(vec![ParsedBlock::List(ParsedList {
        ordered: true,
        items,
    })])
}

fn map_description_list(
    list: &acdc_parser::DescriptionList<'_>,
    state: &mut AsciiDocState<'_>,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let items = list
        .items
        .iter()
        .map(|item| {
            let mut definition = Vec::new();
            let principal = map_inlines(&item.principal_text, state);
            if !principal.is_empty() {
                definition.push(ParsedBlock::Paragraph(principal));
            }
            definition.extend(map_blocks(&item.description, state)?);
            Ok(ParsedDefinitionItem {
                term: map_inlines(&item.term, state),
                definitions: vec![definition],
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(vec![ParsedBlock::DefinitionList(ParsedDefinitionList {
        items,
    })])
}

fn map_delimited_block(
    delimited: &acdc_parser::DelimitedBlock<'_>,
    state: &mut AsciiDocState<'_>,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let style = delimited.metadata.style.unwrap_or("");
    if style.eq_ignore_ascii_case("mermaid") {
        let source = verbatim_content(&delimited.inner);
        let (link_id, _url) = state.parts.push_mermaid(source.clone());
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
        DelimitedBlockType::DelimitedPass(inlines) => {
            Ok(vec![ParsedBlock::CodeBlock(ParsedCodeBlock {
                language: None,
                content: acdc_parser::inlines_to_string(inlines),
            })])
        }
        DelimitedBlockType::DelimitedVerse(inlines) => Ok(vec![ParsedBlock::Paragraph(
            map_verbatim_inlines(inlines, state),
        )]),
        DelimitedBlockType::DelimitedStem(StemContent { content, .. }) => {
            Ok(vec![ParsedBlock::MathBlock(ParsedMathBlock {
                content: (*content).to_string(),
            })])
        }
        DelimitedBlockType::DelimitedQuote(blocks) => {
            Ok(vec![ParsedBlock::BlockQuote(map_blocks(blocks, state)?)])
        }
        DelimitedBlockType::DelimitedExample(blocks)
        | DelimitedBlockType::DelimitedOpen(blocks)
        | DelimitedBlockType::DelimitedSidebar(blocks) => map_blocks(blocks, state),
        DelimitedBlockType::DelimitedTable(table) => Ok(vec![map_table(table, state)?]),
        DelimitedBlockType::DelimitedComment(_) | _ => Ok(Vec::new()),
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
    state: &mut AsciiDocState<'_>,
) -> Result<ParsedListItem, ParseError> {
    let mut content = Vec::new();
    if !item.principal.is_empty() {
        content.push(ParsedBlock::Paragraph(map_inlines(&item.principal, state)));
    }
    content.extend(map_blocks(&item.blocks, state)?);
    let (checklist_id, checked) = match item.checked {
        Some(ListItemCheckedStatus::Checked) => (Some(state.parts.next_checklist_id()), true),
        Some(ListItemCheckedStatus::Unchecked) => (Some(state.parts.next_checklist_id()), false),
        None | Some(_) => (None, false),
    };
    Ok(ParsedListItem {
        checklist_id,
        checked,
        content,
    })
}

#[cfg(test)]
mod tests;
