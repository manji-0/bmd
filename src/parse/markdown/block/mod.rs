//! Block-level markdown event handling.

mod events;
mod tags;

use pulldown_cmark::{Alignment as CmarkAlignment, Event, MetadataBlockKind, Parser};

use std::collections::HashMap;

use crate::parse::dto::{
    ParsedAlignment, ParsedBlock, ParsedDefinitionItem, ParsedDocument, ParsedFootnoteDefinition,
    ParsedFrontMatter, ParsedFrontMatterKind, ParsedInline, ParsedLink, ParsedListItem,
    ParsedMathBlock, ParsedMermaidDiagram,
};
use crate::parse::error::ParseError;

use super::inline::InlineParser;
use super::syntax_error;

#[derive(Debug)]
struct PendingStandaloneImage {
    src: String,
    title: Option<String>,
    alt: String,
}

#[derive(Debug)]
pub(crate) struct ParserState<'a> {
    iter: std::iter::Peekable<Parser<'a>>,
    blocks: Vec<ParsedBlock>,
    links: Vec<ParsedLink>,
    mermaid_diagrams: Vec<ParsedMermaidDiagram>,
    stack: Vec<BlockFrame>,
    paragraph_standalone_image: Option<PendingStandaloneImage>,
    next_checklist_id: u32,
    footnotes: Vec<ParsedFootnoteDefinition>,
    footnote_label_to_id: HashMap<String, usize>,
    footnote_order: Vec<usize>,
    front_matter: Option<ParsedFrontMatter>,
}

#[derive(Debug)]
enum BlockFrame {
    BlockQuote {
        kind: Option<pulldown_cmark::BlockQuoteKind>,
        blocks: Vec<ParsedBlock>,
    },
    List {
        ordered: bool,
        items: Vec<ParsedListItem>,
        current_item: Vec<ParsedBlock>,
    },
    ListItem {
        blocks: Vec<ParsedBlock>,
        checked: bool,
        checklist_id: Option<u32>,
        /// Accumulates inline content for tight list items, where pulldown-cmark
        /// emits inline events directly under the item without a `Paragraph` tag.
        pending_inline: Option<InlineParser>,
    },
    Heading {
        parser: InlineParser,
        anchor: Option<String>,
    },
    Paragraph(InlineParser),
    Table {
        alignments: Vec<ParsedAlignment>,
        headers: Vec<Vec<ParsedInline>>,
        rows: Vec<Vec<Vec<ParsedInline>>>,
    },
    TableHead(Vec<Vec<ParsedInline>>),
    TableRow(Vec<Vec<ParsedInline>>),
    TableCell(InlineParser),
    CodeBlock {
        language: Option<String>,
        content: String,
        is_mermaid: bool,
    },
    FootnoteDefinition {
        footnote_id: usize,
        blocks: Vec<ParsedBlock>,
    },
    MetadataBlock {
        kind: ParsedFrontMatterKind,
        content: String,
    },
    DefinitionList {
        items: Vec<ParsedDefinitionItem>,
        current_term: Option<Vec<ParsedInline>>,
    },
    DefinitionListDefinition {
        blocks: Vec<ParsedBlock>,
        /// See `ListItem::pending_inline`.
        pending_inline: Option<InlineParser>,
    },
}

impl<'a> ParserState<'a> {
    pub(crate) fn new(parser: Parser<'a>) -> Self {
        Self {
            iter: parser.peekable(),
            blocks: Vec::new(),
            links: Vec::new(),
            mermaid_diagrams: Vec::new(),
            stack: Vec::new(),
            paragraph_standalone_image: None,
            next_checklist_id: 0,
            footnotes: Vec::new(),
            footnote_label_to_id: HashMap::new(),
            footnote_order: Vec::new(),
            front_matter: None,
        }
    }

    pub(crate) fn run(&mut self) -> Result<(), ParseError> {
        while let Some(event) = self.iter.next() {
            match event {
                Event::Start(tag) => self.start_tag(tag)?,
                Event::End(tag_end) => self.end_tag(tag_end)?,
                Event::Text(text) => self.text(text.into_string()),
                Event::Code(code) => self.code(code.into_string()),
                Event::Html(html) => self.text(html.into_string()),
                Event::InlineHtml(html) => self.inline_html(html.into_string()),
                Event::SoftBreak => self.soft_break(),
                Event::HardBreak => self.hard_break(),
                Event::Rule => self.blocks.push(ParsedBlock::Rule),
                Event::FootnoteReference(label) => self.footnote_reference(label.into_string()),
                Event::InlineMath(math) => self.inline_math(math.into_string()),
                Event::DisplayMath(math) => self.display_math(math.into_string()),
                Event::TaskListMarker(checked) => self.task_list_marker(checked),
            }
        }
        Ok(())
    }

    fn with_inline_parser<F>(&mut self, f: F)
    where
        F: FnOnce(&mut InlineParser),
    {
        if let Some(parser) = self.inline_parser() {
            f(parser);
        }
    }

    fn inline_parser(&mut self) -> Option<&mut InlineParser> {
        Self::inline_parser_from_stack(&mut self.stack)
    }

    fn inline_parser_from_stack(stack: &mut [BlockFrame]) -> Option<&mut InlineParser> {
        match stack.last_mut()? {
            BlockFrame::Paragraph(p)
            | BlockFrame::Heading { parser: p, .. }
            | BlockFrame::TableCell(p) => Some(p),
            BlockFrame::ListItem { pending_inline, .. }
            | BlockFrame::DefinitionListDefinition { pending_inline, .. } => {
                Some(pending_inline.get_or_insert_with(InlineParser::new))
            }
            _ => None,
        }
    }

    /// Flush a tight list item's/definition's accumulated inline run into a
    /// `Paragraph` block, so it precedes any sibling block-level content in
    /// document order.
    fn flush_pending_inline(
        pending_inline: &mut Option<InlineParser>,
        blocks: &mut Vec<ParsedBlock>,
        links: &mut Vec<ParsedLink>,
    ) {
        if let Some(parser) = pending_inline.take() {
            let inlines = parser.into_inlines(links);
            if !inlines.is_empty() {
                blocks.push(ParsedBlock::Paragraph(inlines));
            }
        }
    }

    fn finish_block(&mut self, block: ParsedBlock) {
        let links = &mut self.links;
        if let Some(parent) = self.stack.last_mut() {
            match parent {
                BlockFrame::BlockQuote { blocks, .. } => blocks.push(block),
                BlockFrame::ListItem {
                    blocks,
                    pending_inline,
                    ..
                } => {
                    Self::flush_pending_inline(pending_inline, blocks, links);
                    blocks.push(block);
                }
                BlockFrame::List { current_item, .. } => current_item.push(block),
                BlockFrame::FootnoteDefinition { blocks, .. } => blocks.push(block),
                BlockFrame::DefinitionListDefinition {
                    blocks,
                    pending_inline,
                } => {
                    Self::flush_pending_inline(pending_inline, blocks, links);
                    blocks.push(block);
                }
                _ => self.blocks.push(block),
            }
        } else {
            self.blocks.push(block);
        }
    }

    fn pop_frame(&mut self, expected: &str) -> Result<BlockFrame, ParseError> {
        self.stack
            .pop()
            .ok_or_else(|| syntax_error(format!("unexpected end tag for {expected}")))
    }

    pub(crate) fn into_document(self) -> ParsedDocument {
        ParsedDocument::new(
            self.blocks,
            self.links,
            self.mermaid_diagrams,
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

    fn footnote_reference(&mut self, label: String) {
        let footnote_id = self.footnote_id_for_label(&label);
        let display = self.footnote_display_for(footnote_id);
        let inline = ParsedInline::FootnoteReference {
            footnote_id,
            display,
        };
        if let Some(parser) = self.inline_parser() {
            parser.current_target().push(inline);
        } else {
            self.push_inline_to_list_item(inline);
        }
    }

    fn inline_math(&mut self, content: String) {
        let inline = ParsedInline::Math(content);
        if let Some(parser) = self.inline_parser() {
            parser.current_target().push(inline);
        } else {
            self.push_inline_to_list_item(inline);
        }
    }

    fn display_math(&mut self, content: String) {
        self.blocks
            .push(ParsedBlock::MathBlock(ParsedMathBlock { content }));
    }

    fn finish_definition_list_definition(&mut self, blocks: Vec<ParsedBlock>) {
        let Some(BlockFrame::DefinitionList {
            items,
            current_term,
        }) = self.stack.last_mut()
        else {
            return;
        };
        if let Some(term) = current_term.take() {
            items.push(ParsedDefinitionItem {
                term,
                definitions: vec![blocks],
            });
        } else if let Some(last) = items.last_mut() {
            last.definitions.push(blocks);
        } else {
            items.push(ParsedDefinitionItem {
                term: Vec::new(),
                definitions: vec![blocks],
            });
        }
    }
}

fn map_metadata_kind(kind: MetadataBlockKind) -> ParsedFrontMatterKind {
    match kind {
        MetadataBlockKind::YamlStyle => ParsedFrontMatterKind::Yaml,
        MetadataBlockKind::PlusesStyle => ParsedFrontMatterKind::Toml,
    }
}

fn mermaid_link_label(source: &str) -> String {
    let first_line = source.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        "[mermaid diagram]".to_string()
    } else {
        format!("[mermaid: {first_line}]")
    }
}

fn heading_level_to_u8(level: pulldown_cmark::HeadingLevel) -> u8 {
    match level {
        pulldown_cmark::HeadingLevel::H1 => 1,
        pulldown_cmark::HeadingLevel::H2 => 2,
        pulldown_cmark::HeadingLevel::H3 => 3,
        pulldown_cmark::HeadingLevel::H4 => 4,
        pulldown_cmark::HeadingLevel::H5 => 5,
        pulldown_cmark::HeadingLevel::H6 => 6,
    }
}

fn map_alignment(a: CmarkAlignment) -> ParsedAlignment {
    match a {
        CmarkAlignment::None => ParsedAlignment::None,
        CmarkAlignment::Left => ParsedAlignment::Left,
        CmarkAlignment::Center => ParsedAlignment::Center,
        CmarkAlignment::Right => ParsedAlignment::Right,
    }
}

fn toc_marker_link_id(inlines: &[ParsedInline], links: &[ParsedLink]) -> Option<usize> {
    if inlines.len() != 1 {
        return None;
    }
    let ParsedInline::Link { link_id, children } = &inlines[0] else {
        return None;
    };
    if children.len() != 1 {
        return None;
    }
    let ParsedInline::Text(t) = &children[0] else {
        return None;
    };
    if !t.trim().eq_ignore_ascii_case("toc") {
        return None;
    }
    let link = links.get(*link_id)?;
    if link.url.eq_ignore_ascii_case(t.trim()) {
        Some(*link_id)
    } else {
        None
    }
}
