//! DTO -> domain conversion with validation at the boundary.

use crate::domain::{
    Alignment, Block, ChecklistId, CodeBlock, Document, Heading, HeadingLevel, Inline, Link,
    LinkId, LinkUrl, List, ListItem, MermaidDiagram, Table,
};

use super::dto::{
    ParsedAlignment, ParsedBlock, ParsedCodeBlock, ParsedDocument, ParsedHeading, ParsedInline,
    ParsedLink, ParsedList, ParsedListItem, ParsedTable,
};

#[derive(Debug, thiserror::Error)]
pub enum IntoDomainError {
    #[error("invalid document: {0}")]
    Document(#[from] crate::domain::DocumentError),

    #[error("invalid link URL: {0}")]
    LinkUrl(#[from] crate::domain::LinkUrlError),

    #[error("invalid heading level {level}")]
    InvalidHeadingLevel { level: u8 },
}

impl ParsedDocument {
    /// Validate DTO fields and build the domain [`Document`].
    pub fn into_domain(self) -> Result<Document, IntoDomainError> {
        let links = self
            .links
            .into_iter()
            .map(convert_link)
            .collect::<Result<Vec<_>, _>>()?;
        let blocks = self
            .blocks
            .into_iter()
            .map(convert_block)
            .collect::<Result<Vec<_>, _>>()?;
        let mermaid_diagrams = self
            .mermaid_diagrams
            .into_iter()
            .map(|diagram| MermaidDiagram {
                source: diagram.source,
            })
            .collect();
        Document::new(blocks, links, mermaid_diagrams).map_err(IntoDomainError::Document)
    }
}

fn convert_link(link: ParsedLink) -> Result<Link, IntoDomainError> {
    Ok(Link {
        url: LinkUrl::new(link.url)?,
        title: link.title,
        kind: link.kind.to_domain(),
    })
}

fn convert_block(owned: ParsedBlock) -> Result<Block, IntoDomainError> {
    match owned {
        ParsedBlock::Heading(heading) => Ok(Block::Heading(convert_heading(heading)?)),
        ParsedBlock::Paragraph(inlines) => Ok(Block::Paragraph(convert_inlines(inlines)?)),
        ParsedBlock::CodeBlock(code) => Ok(Block::CodeBlock(convert_code_block(code))),
        ParsedBlock::BlockQuote(blocks) => Ok(Block::BlockQuote(convert_blocks(blocks)?)),
        ParsedBlock::List(list) => Ok(Block::List(convert_list(list)?)),
        ParsedBlock::Table(table) => Ok(Block::Table(convert_table(table)?)),
        ParsedBlock::Rule => Ok(Block::Rule),
    }
}

fn convert_blocks(blocks: Vec<ParsedBlock>) -> Result<Vec<Block>, IntoDomainError> {
    blocks.into_iter().map(convert_block).collect()
}

fn convert_heading(heading: ParsedHeading) -> Result<Heading, IntoDomainError> {
    Ok(Heading {
        level: HeadingLevel::from_u8(heading.level).ok_or(
            IntoDomainError::InvalidHeadingLevel {
                level: heading.level,
            },
        )?,
        content: convert_inlines(heading.content)?,
        anchor: heading.anchor,
    })
}

fn convert_code_block(code: ParsedCodeBlock) -> CodeBlock {
    CodeBlock {
        language: code.language,
        content: code.content,
    }
}

fn convert_list(list: ParsedList) -> Result<List, IntoDomainError> {
    Ok(List {
        ordered: list.ordered,
        items: list
            .items
            .into_iter()
            .map(convert_list_item)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn convert_list_item(item: ParsedListItem) -> Result<ListItem, IntoDomainError> {
    Ok(ListItem {
        checklist_id: item.checklist_id.map(ChecklistId),
        checked: item.checked,
        content: convert_blocks(item.content)?,
    })
}

fn convert_table(table: ParsedTable) -> Result<Table, IntoDomainError> {
    Ok(Table {
        headers: table
            .headers
            .into_iter()
            .map(convert_inlines)
            .collect::<Result<Vec<_>, _>>()?,
        rows: table
            .rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(convert_inlines)
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?,
        alignments: table
            .alignments
            .into_iter()
            .map(convert_alignment)
            .collect(),
    })
}

fn convert_alignment(alignment: ParsedAlignment) -> Alignment {
    match alignment {
        ParsedAlignment::None => Alignment::None,
        ParsedAlignment::Left => Alignment::Left,
        ParsedAlignment::Center => Alignment::Center,
        ParsedAlignment::Right => Alignment::Right,
    }
}

fn convert_inlines(inlines: Vec<ParsedInline>) -> Result<Vec<Inline>, IntoDomainError> {
    inlines
        .into_iter()
        .map(convert_inline)
        .collect::<Result<Vec<_>, _>>()
}

fn convert_inline(inline: ParsedInline) -> Result<Inline, IntoDomainError> {
    Ok(match inline {
        ParsedInline::Text(text) => Inline::Text(text),
        ParsedInline::Strong(children) => Inline::Strong(convert_inlines(children)?),
        ParsedInline::Emphasis(children) => Inline::Emphasis(convert_inlines(children)?),
        ParsedInline::Code(code) => Inline::Code(code),
        ParsedInline::Link { link_id, children } => {
            Inline::Link(LinkId(link_id), convert_inlines(children)?)
        }
        ParsedInline::HardBreak => Inline::HardBreak,
        ParsedInline::SoftBreak => Inline::SoftBreak,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::LinkKind;

    #[test]
    fn into_domain_validates_links_and_headings() {
        let dto = ParsedDocument::new(
            vec![ParsedBlock::Heading(ParsedHeading {
                level: 1,
                content: vec![ParsedInline::Text("Title".into())],
                anchor: None,
            })],
            vec![ParsedLink::from_url("https://example.com".into(), None)],
            vec![],
        );
        let doc = dto.into_domain().unwrap();
        assert_eq!(doc.links[0].kind, LinkKind::Web);
    }

    #[test]
    fn into_domain_rejects_invalid_heading_level() {
        let dto = ParsedDocument::new(
            vec![ParsedBlock::Heading(ParsedHeading {
                level: 9,
                content: vec![ParsedInline::Text("Bad".into())],
                anchor: None,
            })],
            vec![],
            vec![],
        );
        assert!(matches!(
            dto.into_domain(),
            Err(IntoDomainError::InvalidHeadingLevel { level: 9 })
        ));
    }
}
