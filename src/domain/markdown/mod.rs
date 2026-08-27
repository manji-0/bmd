//! Markdown document and block model.

mod block;
mod inline;
mod table;

use std::collections::HashMap;

use super::front_matter::FrontMatter;
use super::link::{DocumentError, Link, LinkId, LinkKind};
use super::mermaid_render::mermaid_diagram_index;

pub use block::{
    Block, CodeBlock, DefinitionItem, DefinitionList, Heading, HeadingLevel, List, ListItem,
    MathBlock,
};
pub use inline::Inline;
pub use table::{Alignment, Table};

/// Opaque identifier for a footnote stored in `Document.footnotes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FootnoteId(pub usize);

impl std::fmt::Display for FootnoteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fn{}", self.0)
    }
}

/// A footnote definition body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FootnoteDefinition {
    pub label: String,
    pub content: Vec<Block>,
}

/// A parsed markdown document.
///
/// Fields are crate-private so validated construction via [`Document::new`] is
/// the only path available to downstream crates. In-crate mutation of links
/// (for example GitHub relative-link rewriting) goes through
/// [`Document::rewrite_document_links`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub(crate) blocks: Vec<Block>,
    pub(crate) links: Vec<Link>,
    pub(crate) mermaid_diagrams: Vec<MermaidDiagram>,
    pub(crate) footnotes: Vec<FootnoteDefinition>,
    /// Footnote ids in order of first inline reference.
    pub(crate) footnote_order: Vec<FootnoteId>,
    pub(crate) front_matter: Option<FrontMatter>,
}

impl Document {
    /// Build a document after validating that link references are in bounds.
    ///
    /// # Errors
    ///
    /// Returns `DocumentError::DanglingLink` if an inline references a link id that does not exist.
    /// Returns `DocumentError::InvalidMermaidLink` if a mermaid link URL does not map to a diagram.
    pub fn new(
        blocks: Vec<Block>,
        links: Vec<Link>,
        mermaid_diagrams: Vec<MermaidDiagram>,
        footnotes: Vec<FootnoteDefinition>,
        footnote_order: Vec<FootnoteId>,
        front_matter: Option<FrontMatter>,
    ) -> Result<Self, DocumentError> {
        let doc = Self {
            blocks,
            links,
            mermaid_diagrams,
            footnotes,
            footnote_order,
            front_matter,
        };
        doc.validate_links()?;
        doc.validate_mermaid_links()?;
        doc.validate_footnotes()?;
        Ok(doc)
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    pub fn links(&self) -> &[Link] {
        &self.links
    }

    pub fn mermaid_diagrams(&self) -> &[MermaidDiagram] {
        &self.mermaid_diagrams
    }

    pub fn footnotes(&self) -> &[FootnoteDefinition] {
        &self.footnotes
    }

    pub fn footnote_order(&self) -> &[FootnoteId] {
        &self.footnote_order
    }

    pub fn front_matter(&self) -> Option<&FrontMatter> {
        self.front_matter.as_ref()
    }

    /// Rewrite each document link through `f`, keeping only successful updates.
    ///
    /// Used at the GitHub adapter boundary so relative document links can become
    /// absolute web URLs without exposing raw field mutation to downstream crates.
    pub fn rewrite_document_links(&mut self, mut f: impl FnMut(&Link) -> Option<Link>) {
        for link in &mut self.links {
            if let Some(next) = f(link) {
                *link = next;
            }
        }
    }

    fn validate_links(&self) -> Result<(), DocumentError> {
        let count = self.links.len();
        for (block_idx, block) in self.blocks.iter().enumerate() {
            Self::validate_block_links(block, block_idx, count)?;
        }
        Ok(())
    }

    fn validate_block_links(
        block: &Block,
        block_idx: usize,
        link_count: usize,
    ) -> Result<(), DocumentError> {
        match block {
            Block::Paragraph(inlines)
            | Block::Heading(Heading {
                content: inlines, ..
            }) => {
                Self::validate_inlines_links(inlines, block_idx, link_count)?;
            }
            Block::CodeBlock(_) | Block::MathBlock(_) | Block::Rule => {}
            Block::BlockQuote(blocks) => {
                for child in blocks {
                    Self::validate_block_links(child, block_idx, link_count)?;
                }
            }
            Block::Callout(callout) => {
                for child in &callout.body {
                    Self::validate_block_links(child, block_idx, link_count)?;
                }
            }
            Block::List(list) => {
                for item in &list.items {
                    for child in &item.content {
                        Self::validate_block_links(child, block_idx, link_count)?;
                    }
                }
            }
            Block::DefinitionList(list) => {
                for item in &list.items {
                    Self::validate_inlines_links(&item.term, block_idx, link_count)?;
                    for definition in &item.definitions {
                        for child in definition {
                            Self::validate_block_links(child, block_idx, link_count)?;
                        }
                    }
                }
            }
            Block::Table(table) => {
                for cell in &table.headers {
                    Self::validate_inlines_links(cell, block_idx, link_count)?;
                }
                for row in &table.rows {
                    for cell in row {
                        Self::validate_inlines_links(cell, block_idx, link_count)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_inlines_links(
        inlines: &[Inline],
        block_idx: usize,
        link_count: usize,
    ) -> Result<(), DocumentError> {
        for inline in inlines {
            match inline {
                Inline::Link(id, children) => {
                    if id.0 >= link_count {
                        return Err(DocumentError::DanglingLink {
                            block_index: block_idx,
                            link_id: *id,
                        });
                    }
                    Self::validate_inlines_links(children, block_idx, link_count)?;
                }
                Inline::Strong(children)
                | Inline::Emphasis(children)
                | Inline::Strikethrough(children)
                | Inline::Subscript(children)
                | Inline::Superscript(children) => {
                    Self::validate_inlines_links(children, block_idx, link_count)?;
                }
                Inline::Text(_)
                | Inline::Code(_)
                | Inline::Math(_)
                | Inline::HardBreak
                | Inline::SoftBreak
                | Inline::FootnoteReference(_, _) => {}
            }
        }
        Ok(())
    }

    fn validate_footnotes(&self) -> Result<(), DocumentError> {
        let count = self.footnotes.len();
        let referenced: HashMap<FootnoteId, ()> = self
            .footnote_order
            .iter()
            .copied()
            .map(|id| (id, ()))
            .collect();
        for (block_idx, block) in self.blocks.iter().enumerate() {
            Self::validate_block_footnotes(block, block_idx, count, &referenced, &self.footnotes)?;
        }
        Ok(())
    }

    fn validate_block_footnotes(
        block: &Block,
        block_idx: usize,
        footnote_count: usize,
        referenced: &HashMap<FootnoteId, ()>,
        footnotes: &[FootnoteDefinition],
    ) -> Result<(), DocumentError> {
        match block {
            Block::Paragraph(inlines)
            | Block::Heading(Heading {
                content: inlines, ..
            }) => {
                Self::validate_inlines_footnotes(
                    inlines,
                    block_idx,
                    footnote_count,
                    referenced,
                    footnotes,
                )?;
            }
            Block::CodeBlock(_) | Block::MathBlock(_) | Block::Rule => {}
            Block::BlockQuote(blocks) => {
                for child in blocks {
                    Self::validate_block_footnotes(
                        child,
                        block_idx,
                        footnote_count,
                        referenced,
                        footnotes,
                    )?;
                }
            }
            Block::Callout(callout) => {
                for child in &callout.body {
                    Self::validate_block_footnotes(
                        child,
                        block_idx,
                        footnote_count,
                        referenced,
                        footnotes,
                    )?;
                }
            }
            Block::List(list) => {
                for item in &list.items {
                    for child in &item.content {
                        Self::validate_block_footnotes(
                            child,
                            block_idx,
                            footnote_count,
                            referenced,
                            footnotes,
                        )?;
                    }
                }
            }
            Block::DefinitionList(list) => {
                for item in &list.items {
                    Self::validate_inlines_footnotes(
                        &item.term,
                        block_idx,
                        footnote_count,
                        referenced,
                        footnotes,
                    )?;
                    for definition in &item.definitions {
                        for child in definition {
                            Self::validate_block_footnotes(
                                child,
                                block_idx,
                                footnote_count,
                                referenced,
                                footnotes,
                            )?;
                        }
                    }
                }
            }
            Block::Table(table) => {
                for cell in &table.headers {
                    Self::validate_inlines_footnotes(
                        cell,
                        block_idx,
                        footnote_count,
                        referenced,
                        footnotes,
                    )?;
                }
                for row in &table.rows {
                    for cell in row {
                        Self::validate_inlines_footnotes(
                            cell,
                            block_idx,
                            footnote_count,
                            referenced,
                            footnotes,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_inlines_footnotes(
        inlines: &[Inline],
        block_idx: usize,
        footnote_count: usize,
        referenced: &HashMap<FootnoteId, ()>,
        footnotes: &[FootnoteDefinition],
    ) -> Result<(), DocumentError> {
        for inline in inlines {
            match inline {
                Inline::FootnoteReference(id, _) => {
                    if id.0 >= footnote_count {
                        return Err(DocumentError::DanglingFootnote {
                            block_index: block_idx,
                            footnote_id: *id,
                        });
                    }
                    if referenced.contains_key(id) && footnotes[id.0].content.is_empty() {
                        return Err(DocumentError::UndefinedFootnote { footnote_id: *id });
                    }
                }
                Inline::Link(_, children)
                | Inline::Strong(children)
                | Inline::Emphasis(children)
                | Inline::Strikethrough(children)
                | Inline::Subscript(children)
                | Inline::Superscript(children) => {
                    Self::validate_inlines_footnotes(
                        children,
                        block_idx,
                        footnote_count,
                        referenced,
                        footnotes,
                    )?;
                }
                Inline::Text(_)
                | Inline::Code(_)
                | Inline::Math(_)
                | Inline::HardBreak
                | Inline::SoftBreak => {}
            }
        }
        Ok(())
    }

    fn validate_mermaid_links(&self) -> Result<(), DocumentError> {
        for (link_idx, link) in self.links.iter().enumerate() {
            if link.kind != LinkKind::Mermaid {
                continue;
            }
            let Some(diagram_idx) = mermaid_diagram_index(link.url.as_str()) else {
                return Err(DocumentError::InvalidMermaidLink {
                    link_id: LinkId(link_idx),
                });
            };
            if diagram_idx >= self.mermaid_diagrams.len() {
                return Err(DocumentError::InvalidMermaidLink {
                    link_id: LinkId(link_idx),
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MermaidDiagram {
    pub source: String,
}

impl MermaidDiagram {
    /// Estimate the rendered width of the diagram in terminal columns.
    ///
    /// This uses a simple heuristic based on the number of lines and
    /// average node length, clamped to a reasonable range.
    pub fn estimated_width(&self) -> u16 {
        let lines: Vec<&str> = self.source.lines().collect();
        let max_line_len = lines.iter().map(|l| l.len()).max().unwrap_or(0);
        let avg_node_len = if lines.is_empty() {
            0
        } else {
            self.source.len() / lines.len()
        };
        let estimate = max_line_len.max(avg_node_len).min(200);
        estimate.clamp(20, 160) as u16
    }
}
