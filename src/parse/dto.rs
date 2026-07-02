//! Format-agnostic parse DTOs produced by markup parsers.

use crate::domain::{LinkKind, is_remote_link_dest};

/// Parsed document before domain validation.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ParsedDocument {
    pub blocks: Vec<ParsedBlock>,
    pub links: Vec<ParsedLink>,
    pub mermaid_diagrams: Vec<ParsedMermaidDiagram>,
    pub footnotes: Vec<ParsedFootnoteDefinition>,
    /// Footnote ids in order of first inline reference (for bottom section ordering).
    pub footnote_order: Vec<usize>,
    pub front_matter: Option<ParsedFrontMatter>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParsedFrontMatterKind {
    Yaml,
    Toml,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedFrontMatter {
    pub kind: ParsedFrontMatterKind,
    pub raw: String,
}

impl ParsedDocument {
    pub fn new(
        blocks: Vec<ParsedBlock>,
        links: Vec<ParsedLink>,
        mermaid_diagrams: Vec<ParsedMermaidDiagram>,
        footnotes: Vec<ParsedFootnoteDefinition>,
        footnote_order: Vec<usize>,
        front_matter: Option<ParsedFrontMatter>,
    ) -> Self {
        Self {
            blocks,
            links,
            mermaid_diagrams,
            footnotes,
            footnote_order,
            front_matter,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedFootnoteDefinition {
    pub label: String,
    pub blocks: Vec<ParsedBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParsedBlock {
    Heading(ParsedHeading),
    Paragraph(Vec<ParsedInline>),
    CodeBlock(ParsedCodeBlock),
    MathBlock(ParsedMathBlock),
    BlockQuote(Vec<ParsedBlock>),
    List(ParsedList),
    DefinitionList(ParsedDefinitionList),
    Table(ParsedTable),
    Rule,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedDefinitionList {
    pub items: Vec<ParsedDefinitionItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedDefinitionItem {
    pub term: Vec<ParsedInline>,
    pub definitions: Vec<Vec<ParsedBlock>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedHeading {
    pub level: u8,
    pub content: Vec<ParsedInline>,
    /// Explicit anchor slug; when absent, derived from heading text at jump time.
    pub anchor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedCodeBlock {
    pub language: Option<String>,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedMathBlock {
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedList {
    pub ordered: bool,
    pub items: Vec<ParsedListItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedListItem {
    pub checklist_id: Option<u32>,
    pub checked: bool,
    pub content: Vec<ParsedBlock>,
}

impl ParsedListItem {
    pub fn plain(content: Vec<ParsedBlock>) -> Self {
        Self {
            checklist_id: None,
            checked: false,
            content,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParsedAlignment {
    None,
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedTable {
    pub headers: Vec<Vec<ParsedInline>>,
    pub rows: Vec<Vec<Vec<ParsedInline>>>,
    pub alignments: Vec<ParsedAlignment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedMermaidDiagram {
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedLink {
    pub url: String,
    pub title: Option<String>,
    pub kind: ParsedLinkKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParsedLinkKind {
    Web,
    Anchor,
    Document,
    Image,
    Mermaid,
}

impl ParsedLinkKind {
    pub fn classify_url(url: &str) -> Self {
        if url.starts_with('#') {
            Self::Anchor
        } else if is_remote_link_dest(url) {
            Self::Web
        } else {
            Self::Document
        }
    }

    pub fn to_domain(self) -> LinkKind {
        match self {
            Self::Web => LinkKind::Web,
            Self::Anchor => LinkKind::Anchor,
            Self::Document => LinkKind::Document,
            Self::Image => LinkKind::Image,
            Self::Mermaid => LinkKind::Mermaid,
        }
    }
}

impl ParsedLink {
    pub fn new(url: String, title: Option<String>, kind: ParsedLinkKind) -> Self {
        Self { url, title, kind }
    }

    pub fn from_url(url: String, title: Option<String>) -> Self {
        let kind = ParsedLinkKind::classify_url(&url);
        Self { url, title, kind }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParsedInline {
    Text(String),
    Strong(Vec<ParsedInline>),
    Emphasis(Vec<ParsedInline>),
    Strikethrough(Vec<ParsedInline>),
    Subscript(Vec<ParsedInline>),
    Superscript(Vec<ParsedInline>),
    Code(String),
    Link {
        link_id: usize,
        children: Vec<ParsedInline>,
    },
    FootnoteReference {
        footnote_id: usize,
        display: usize,
    },
    Math(String),
    HardBreak,
    SoftBreak,
}

impl ParsedInline {
    pub fn plain_text(inlines: &[ParsedInline]) -> String {
        let mut out = String::new();
        for (i, inline) in inlines.iter().enumerate() {
            match inline {
                ParsedInline::Text(t) | ParsedInline::Code(t) => out.push_str(t),
                ParsedInline::Strong(c)
                | ParsedInline::Emphasis(c)
                | ParsedInline::Strikethrough(c)
                | ParsedInline::Subscript(c)
                | ParsedInline::Superscript(c)
                | ParsedInline::Link { children: c, .. } => out.push_str(&Self::plain_text(c)),
                ParsedInline::FootnoteReference { .. } | ParsedInline::Math(_) => {}
                ParsedInline::HardBreak | ParsedInline::SoftBreak => {
                    if i > 0 {
                        out.push(' ');
                    }
                }
            }
        }
        out
    }
}

/// Mutable accumulator shared by format parsers when building links and diagrams.
#[derive(Clone, Debug, Default)]
pub struct ParsedDocumentParts {
    pub links: Vec<ParsedLink>,
    pub mermaid_diagrams: Vec<ParsedMermaidDiagram>,
    next_checklist_id: u32,
}

impl ParsedDocumentParts {
    pub fn push_link(&mut self, link: ParsedLink) -> usize {
        let id = self.links.len();
        self.links.push(link);
        id
    }

    pub fn push_mermaid(&mut self, source: String) -> (usize, String) {
        let diagram_idx = self.mermaid_diagrams.len();
        self.mermaid_diagrams.push(ParsedMermaidDiagram {
            source: source.clone(),
        });
        let url = format!("bmd:mermaid:{diagram_idx}");
        let link_id = self.push_link(ParsedLink::new(url.clone(), None, ParsedLinkKind::Mermaid));
        (link_id, url)
    }

    pub fn next_checklist_id(&mut self) -> u32 {
        let id = self.next_checklist_id;
        self.next_checklist_id += 1;
        id
    }
}
