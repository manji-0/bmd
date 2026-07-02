//! DTO -> domain conversion with validation at the boundary.

use crate::domain::{
    Alignment, Block, ChecklistId, CodeBlock, Document, FootnoteDefinition, FootnoteId,
    FrontMatter, FrontMatterKind, Heading, HeadingLevel, Inline, Link, LinkId, LinkUrl, List,
    ListItem, MathBlock, MermaidDiagram, Table,
};

use super::dto::{
    ParsedAlignment, ParsedBlock, ParsedCodeBlock, ParsedDocument, ParsedFootnoteDefinition,
    ParsedFrontMatter, ParsedFrontMatterKind, ParsedHeading, ParsedInline, ParsedLink, ParsedList,
    ParsedListItem, ParsedMathBlock, ParsedTable,
};
use crate::parse::normalize_anchor_slug;

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
        let footnotes = self
            .footnotes
            .into_iter()
            .map(convert_footnote)
            .collect::<Result<Vec<_>, _>>()?;
        let footnote_order = self.footnote_order.into_iter().map(FootnoteId).collect();
        Document::new(
            blocks,
            links,
            mermaid_diagrams,
            footnotes,
            footnote_order,
            self.front_matter.map(convert_front_matter),
        )
        .map_err(IntoDomainError::Document)
    }
}

fn convert_front_matter(front_matter: ParsedFrontMatter) -> FrontMatter {
    FrontMatter {
        kind: match front_matter.kind {
            ParsedFrontMatterKind::Yaml => FrontMatterKind::Yaml,
            ParsedFrontMatterKind::Toml => FrontMatterKind::Toml,
        },
        raw: front_matter.raw,
    }
}

fn convert_footnote(
    footnote: ParsedFootnoteDefinition,
) -> Result<FootnoteDefinition, IntoDomainError> {
    Ok(FootnoteDefinition {
        label: footnote.label,
        content: convert_blocks(footnote.blocks)?,
    })
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
        ParsedBlock::MathBlock(math) => Ok(Block::MathBlock(convert_math_block(math))),
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
        anchor: heading.anchor.and_then(|anchor| {
            let normalized = normalize_anchor_slug(&anchor);
            if normalized.is_empty() {
                None
            } else {
                Some(normalized)
            }
        }),
    })
}

fn convert_code_block(code: ParsedCodeBlock) -> CodeBlock {
    CodeBlock {
        language: code.language,
        content: code.content,
    }
}

fn convert_math_block(math: ParsedMathBlock) -> MathBlock {
    MathBlock {
        content: math.content,
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
        ParsedInline::Strikethrough(children) => Inline::Strikethrough(convert_inlines(children)?),
        ParsedInline::Subscript(children) => Inline::Subscript(convert_inlines(children)?),
        ParsedInline::Superscript(children) => Inline::Superscript(convert_inlines(children)?),
        ParsedInline::Code(code) => Inline::Code(code),
        ParsedInline::Link { link_id, children } => {
            Inline::Link(LinkId(link_id), convert_inlines(children)?)
        }
        ParsedInline::FootnoteReference {
            footnote_id,
            display,
        } => Inline::FootnoteReference(FootnoteId(footnote_id), display),
        ParsedInline::Math(content) => Inline::Math(content),
        ParsedInline::HardBreak => Inline::HardBreak,
        ParsedInline::SoftBreak => Inline::SoftBreak,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Block, LinkKind};

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
            vec![],
            vec![],
            None,
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
            vec![],
            vec![],
            None,
        );
        assert!(matches!(
            dto.into_domain(),
            Err(IntoDomainError::InvalidHeadingLevel { level: 9 })
        ));
    }

    #[test]
    fn into_domain_normalizes_heading_anchor() {
        let dto = ParsedDocument::new(
            vec![ParsedBlock::Heading(ParsedHeading {
                level: 1,
                content: vec![ParsedInline::Text("Title".into())],
                anchor: Some("_Hello_World".into()),
            })],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        );
        let doc = dto.into_domain().unwrap();
        let Block::Heading(heading) = &doc.blocks[0] else {
            panic!("expected heading");
        };
        assert_eq!(heading.anchor.as_deref(), Some("hello-world"));
    }
}
