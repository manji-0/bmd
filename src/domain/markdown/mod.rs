//! Markdown document and block model.

mod block;
mod inline;
mod table;

use super::checklist::ChecklistId;
use super::front_matter::FrontMatter;
use super::link::{DocumentError, Link, LinkId, LinkKind};
use super::preview_load::mermaid_diagram_index;

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

    /// The task-list item with `id`, searched through nested containers.
    pub fn checklist_item(&self, id: ChecklistId) -> Option<&ListItem> {
        fn in_blocks(blocks: &[Block], id: ChecklistId) -> Option<&ListItem> {
            blocks.iter().find_map(|block| match block {
                Block::List(list) => list.items.iter().find_map(|item| {
                    (item.checklist_id == Some(id))
                        .then_some(item)
                        .or_else(|| in_blocks(&item.content, id))
                }),
                Block::Quote(blocks) => in_blocks(blocks, id),
                Block::Callout(callout) => in_blocks(&callout.body, id),
                Block::DefinitionList(list) => list
                    .items
                    .iter()
                    .flat_map(|item| &item.definitions)
                    .find_map(|definition| in_blocks(definition, id)),
                _ => None,
            })
        }
        in_blocks(&self.blocks, id)
    }

    /// Visit every inline of the body depth first, with its top-level block index.
    pub fn visit_inlines(&self, mut f: impl FnMut(usize, &Inline)) {
        for (block_index, block) in self.blocks.iter().enumerate() {
            visit_block_inlines(block, &mut |inline| f(block_index, inline));
        }
    }

    fn validate_links(&self) -> Result<(), DocumentError> {
        let mut result = Ok(());
        self.visit_inlines(|block_index, inline| {
            if let Inline::Link(link_id, _) = inline
                && link_id.0 >= self.links.len()
                && result.is_ok()
            {
                result = Err(DocumentError::DanglingLink {
                    block_index,
                    link_id: *link_id,
                });
            }
        });
        result
    }

    fn validate_footnotes(&self) -> Result<(), DocumentError> {
        let mut result = Ok(());
        self.visit_inlines(|block_index, inline| {
            let Inline::FootnoteReference(footnote_id, _) = inline else {
                return;
            };
            if result.is_err() {
                return;
            }
            match self.footnotes.get(footnote_id.0) {
                None => {
                    result = Err(DocumentError::DanglingFootnote {
                        block_index,
                        footnote_id: *footnote_id,
                    });
                }
                Some(definition)
                    if definition.content.is_empty()
                        && self.footnote_order.contains(footnote_id) =>
                {
                    result = Err(DocumentError::UndefinedFootnote {
                        footnote_id: *footnote_id,
                    });
                }
                Some(_) => {}
            }
        });
        result
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

fn visit_block_inlines(block: &Block, f: &mut dyn FnMut(&Inline)) {
    let visit_all = |blocks: &[Block], f: &mut dyn FnMut(&Inline)| {
        for child in blocks {
            visit_block_inlines(child, f);
        }
    };
    match block {
        Block::Paragraph(inlines)
        | Block::Heading(Heading {
            content: inlines, ..
        }) => {
            visit_inlines(inlines, f);
        }
        Block::CodeBlock(_) | Block::MathBlock(_) | Block::Rule => {}
        Block::Quote(blocks) => visit_all(blocks, f),
        Block::Callout(callout) => visit_all(&callout.body, f),
        Block::List(list) => {
            for item in &list.items {
                visit_all(&item.content, f);
            }
        }
        Block::DefinitionList(list) => {
            for item in &list.items {
                visit_inlines(&item.term, f);
                for definition in &item.definitions {
                    visit_all(definition, f);
                }
            }
        }
        Block::Table(table) => {
            for cell in table.headers.iter().chain(table.rows.iter().flatten()) {
                visit_inlines(cell, f);
            }
        }
    }
}

fn visit_inlines(inlines: &[Inline], f: &mut dyn FnMut(&Inline)) {
    for inline in inlines {
        f(inline);
        match inline {
            Inline::Link(_, children)
            | Inline::Strong(children)
            | Inline::Emphasis(children)
            | Inline::Strikethrough(children)
            | Inline::Subscript(children)
            | Inline::Superscript(children) => visit_inlines(children, f),
            Inline::Text(_)
            | Inline::Code(_)
            | Inline::Math(_)
            | Inline::HardBreak
            | Inline::SoftBreak
            | Inline::FootnoteReference(_, _) => {}
        }
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
