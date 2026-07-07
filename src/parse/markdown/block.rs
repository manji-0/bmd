//! Block-level markdown event handling.

use pulldown_cmark::{Alignment as CmarkAlignment, Event, MetadataBlockKind, Parser, Tag, TagEnd};

use std::collections::HashMap;

use crate::parse::dto::{
    ParsedAlignment, ParsedBlock, ParsedCodeBlock, ParsedDefinitionItem, ParsedDefinitionList,
    ParsedDocument, ParsedFootnoteDefinition, ParsedFrontMatter, ParsedFrontMatterKind,
    ParsedHeading, ParsedInline, ParsedLink, ParsedLinkKind, ParsedList, ParsedListItem,
    ParsedMathBlock, ParsedMermaidDiagram, ParsedTable,
};
use crate::parse::error::ParseError;

use super::callout;
use super::html::{InlineHtmlKind, InlineHtmlToken};
use super::inline::{InlineFrame, InlineParser};
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

    fn start_tag(&mut self, tag: Tag<'a>) -> Result<(), ParseError> {
        match tag {
            Tag::Paragraph => self.stack.push(BlockFrame::Paragraph(InlineParser::new())),
            Tag::Heading { id, .. } => {
                self.stack.push(BlockFrame::Heading {
                    parser: InlineParser::new(),
                    anchor: id.map(|s| s.into_string()).filter(|s| !s.trim().is_empty()),
                });
            }
            Tag::BlockQuote(kind) => self.stack.push(BlockFrame::BlockQuote {
                kind,
                blocks: Vec::new(),
            }),
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        if lang.is_empty() {
                            None
                        } else {
                            Some(lang.into_string())
                        }
                    }
                    pulldown_cmark::CodeBlockKind::Indented => None,
                };
                let is_mermaid = language
                    .as_ref()
                    .and_then(|l: &String| l.split_whitespace().next())
                    .map(|l| l.eq_ignore_ascii_case("mermaid"))
                    .unwrap_or(false);
                self.stack.push(BlockFrame::CodeBlock {
                    language,
                    content: String::new(),
                    is_mermaid,
                });
            }
            Tag::List(start_number) => self.stack.push(BlockFrame::List {
                ordered: start_number.is_some(),
                items: Vec::new(),
                current_item: Vec::new(),
            }),
            Tag::Item => self.stack.push(BlockFrame::ListItem {
                blocks: Vec::new(),
                checked: false,
                checklist_id: None,
            }),
            Tag::Table(alignments) => self.stack.push(BlockFrame::Table {
                alignments: alignments.into_iter().map(map_alignment).collect(),
                headers: Vec::new(),
                rows: Vec::new(),
            }),
            Tag::TableHead => self.stack.push(BlockFrame::TableHead(Vec::new())),
            Tag::TableRow => self.stack.push(BlockFrame::TableRow(Vec::new())),
            Tag::TableCell => self.stack.push(BlockFrame::TableCell(InlineParser::new())),
            Tag::Emphasis => self.with_inline_parser(|p| p.start_emphasis()),
            Tag::Strong => self.with_inline_parser(|p| p.start_strong()),
            Tag::Strikethrough => self.with_inline_parser(|p| p.start_strikethrough()),
            Tag::Subscript => self.with_inline_parser(|p| p.start_subscript()),
            Tag::Superscript => self.with_inline_parser(|p| p.start_superscript()),
            Tag::Link {
                dest_url, title, ..
            } => {
                let dest = dest_url.into_string();
                let title = title.into_string();
                if let Some(
                    BlockFrame::Paragraph(p)
                    | BlockFrame::Heading { parser: p, .. }
                    | BlockFrame::TableCell(p),
                ) = self.stack.last_mut()
                {
                    let kind = ParsedLinkKind::classify_url(&dest);
                    p.start_link(&mut self.links, dest, title, kind);
                }
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                let dest = dest_url.into_string();
                let title = if title.is_empty() {
                    None
                } else {
                    Some(title.into_string())
                };
                if let Some(BlockFrame::Paragraph(parser)) = self.stack.last() {
                    if parser.is_empty() {
                        self.paragraph_standalone_image = Some(PendingStandaloneImage {
                            src: dest,
                            title,
                            alt: String::new(),
                        });
                    } else if let Some(BlockFrame::Paragraph(p)) = self.stack.last_mut() {
                        p.start_link(
                            &mut self.links,
                            dest,
                            title.unwrap_or_default(),
                            ParsedLinkKind::Image,
                        );
                    }
                } else if let Some(
                    BlockFrame::Heading { parser: p, .. } | BlockFrame::TableCell(p),
                ) = self.stack.last_mut()
                {
                    p.start_link(
                        &mut self.links,
                        dest,
                        title.unwrap_or_default(),
                        ParsedLinkKind::Image,
                    );
                }
            }
            Tag::FootnoteDefinition(label) => {
                let label = label.into_string();
                let footnote_id = self.footnote_id_for_label(&label);
                self.stack.push(BlockFrame::FootnoteDefinition {
                    footnote_id,
                    blocks: Vec::new(),
                });
            }
            Tag::MetadataBlock(kind) => {
                self.stack.push(BlockFrame::MetadataBlock {
                    kind: map_metadata_kind(kind),
                    content: String::new(),
                });
            }
            Tag::DefinitionList => self.stack.push(BlockFrame::DefinitionList {
                items: Vec::new(),
                current_term: None,
            }),
            Tag::DefinitionListTitle => {
                self.stack.push(BlockFrame::Paragraph(InlineParser::new()));
            }
            Tag::DefinitionListDefinition => self
                .stack
                .push(BlockFrame::DefinitionListDefinition { blocks: Vec::new() }),
            Tag::HtmlBlock => {}
        }
        Ok(())
    }

    fn end_tag(&mut self, tag_end: TagEnd) -> Result<(), ParseError> {
        match tag_end {
            TagEnd::Paragraph => {
                let frame = self.pop_frame("paragraph")?;
                if let BlockFrame::Paragraph(parser) = frame {
                    if let Some(pending) = self.paragraph_standalone_image.take() {
                        let label = if pending.alt.is_empty() {
                            pending.src.clone()
                        } else {
                            pending.alt
                        };
                        if pending.src.trim().is_empty() {
                            self.finish_block(ParsedBlock::Paragraph(vec![ParsedInline::Text(
                                label,
                            )]));
                        } else {
                            let link_id = self.links.len();
                            self.links.push(ParsedLink {
                                url: pending.src,
                                title: pending.title,
                                kind: ParsedLinkKind::Image,
                            });
                            self.finish_block(ParsedBlock::Paragraph(vec![ParsedInline::Link {
                                link_id,
                                children: vec![ParsedInline::Text(label)],
                            }]));
                        }
                    } else {
                        let inlines = parser.into_inlines(&mut self.links);
                        if let Some(link_id) = toc_marker_link_id(&inlines, &self.links) {
                            self.links[link_id] =
                                ParsedLink::new("bmd:toc".into(), None, ParsedLinkKind::Toc);
                            self.finish_block(ParsedBlock::Paragraph(vec![ParsedInline::Link {
                                link_id,
                                children: vec![ParsedInline::Text("[table of contents]".into())],
                            }]));
                        } else {
                            self.finish_block(ParsedBlock::Paragraph(inlines));
                        }
                    }
                }
            }
            TagEnd::Heading(level) => {
                let frame = self.pop_frame("heading")?;
                if let BlockFrame::Heading { parser, anchor } = frame {
                    let content = parser.into_inlines(&mut self.links);
                    self.finish_block(ParsedBlock::Heading(ParsedHeading {
                        level: heading_level_to_u8(level),
                        content,
                        anchor,
                    }));
                }
            }
            TagEnd::BlockQuote(_) => {
                let frame = self.pop_frame("blockquote")?;
                if let BlockFrame::BlockQuote { kind, blocks } = frame {
                    self.finish_block(callout::normalize_blockquote(kind, blocks));
                }
            }
            TagEnd::CodeBlock => {
                let frame = self.pop_frame("code block")?;
                if let BlockFrame::CodeBlock {
                    language,
                    content,
                    is_mermaid,
                } = frame
                {
                    if is_mermaid {
                        let diagram_idx = self.mermaid_diagrams.len();
                        self.mermaid_diagrams
                            .push(ParsedMermaidDiagram { source: content });
                        let label = mermaid_link_label(&self.mermaid_diagrams[diagram_idx].source);
                        let link_id = self.links.len();
                        self.links.push(ParsedLink {
                            url: format!("bmd:mermaid:{diagram_idx}"),
                            title: None,
                            kind: ParsedLinkKind::Mermaid,
                        });
                        self.finish_block(ParsedBlock::Paragraph(vec![ParsedInline::Link {
                            link_id,
                            children: vec![ParsedInline::Text(label)],
                        }]));
                    } else {
                        self.finish_block(ParsedBlock::CodeBlock(ParsedCodeBlock {
                            language,
                            content,
                        }));
                    }
                }
            }
            TagEnd::List(_) => {
                let frame = self.pop_frame("list")?;
                if let BlockFrame::List {
                    ordered,
                    items,
                    current_item,
                } = frame
                {
                    if !current_item.is_empty() {
                        return Err(syntax_error("list ended with unclosed item"));
                    }
                    self.finish_block(ParsedBlock::List(ParsedList { ordered, items }));
                }
            }
            TagEnd::Item => {
                let frame = self.pop_frame("list item")?;
                if let BlockFrame::ListItem {
                    blocks,
                    checked,
                    checklist_id,
                } = frame
                {
                    if let Some(BlockFrame::List { items, .. }) = self.stack.last_mut() {
                        items.push(ParsedListItem {
                            checklist_id,
                            checked,
                            content: blocks,
                        });
                    } else {
                        return Err(syntax_error("list item without parent list"));
                    }
                }
            }
            TagEnd::Table => {
                let frame = self.pop_frame("table")?;
                if let BlockFrame::Table {
                    alignments,
                    headers,
                    rows,
                } = frame
                {
                    self.finish_block(ParsedBlock::Table(ParsedTable {
                        headers,
                        rows,
                        alignments,
                    }));
                }
            }
            TagEnd::TableHead => {
                let frame = self.pop_frame("table head")?;
                if let BlockFrame::TableHead(cells) = frame {
                    if let Some(BlockFrame::Table { headers, .. }) = self.stack.last_mut() {
                        *headers = cells;
                    } else {
                        return Err(syntax_error("table head without parent table"));
                    }
                }
            }
            TagEnd::TableRow => {
                let frame = self.pop_frame("table row")?;
                if let BlockFrame::TableRow(cells) = frame {
                    if let Some(BlockFrame::Table { rows, .. }) = self.stack.last_mut() {
                        rows.push(cells);
                    } else {
                        return Err(syntax_error("table row without parent table"));
                    }
                }
            }
            TagEnd::TableCell => {
                let frame = self.pop_frame("table cell")?;
                if let BlockFrame::TableCell(parser) = frame {
                    let cells = match self.stack.last_mut() {
                        Some(BlockFrame::TableRow(row)) => row,
                        Some(BlockFrame::TableHead(head_cells)) => head_cells,
                        _ => {
                            return Err(syntax_error("table cell without parent row or head"));
                        }
                    };
                    cells.push(parser.into_inlines(&mut self.links));
                }
            }
            TagEnd::Emphasis => self.with_inline_parser(|p| {
                p.end_emphasis().ok();
            }),
            TagEnd::Strong => self.with_inline_parser(|p| {
                p.end_strong().ok();
            }),
            TagEnd::Strikethrough => self.with_inline_parser(|p| {
                p.end_strikethrough().ok();
            }),
            TagEnd::Subscript => self.with_inline_parser(|p| {
                p.end_subscript().ok();
            }),
            TagEnd::Superscript => self.with_inline_parser(|p| {
                p.end_superscript().ok();
            }),
            TagEnd::Link => self.with_inline_parser(|p| {
                p.end_link().ok();
            }),
            TagEnd::Image => {
                if self.paragraph_standalone_image.is_none() {
                    self.with_inline_parser(|p| {
                        p.end_link().ok();
                    });
                }
            }
            TagEnd::FootnoteDefinition => {
                let frame = self.pop_frame("footnote definition")?;
                if let BlockFrame::FootnoteDefinition {
                    footnote_id,
                    blocks,
                } = frame
                    && let Some(def) = self.footnotes.get_mut(footnote_id)
                {
                    def.blocks = blocks;
                }
            }
            TagEnd::MetadataBlock(_) => {
                let frame = self.pop_frame("metadata block")?;
                if let BlockFrame::MetadataBlock { kind, content } = frame {
                    let raw = content.trim_end().to_string();
                    if !raw.is_empty() && self.front_matter.is_none() {
                        self.front_matter = Some(ParsedFrontMatter { kind, raw });
                    }
                }
            }
            TagEnd::DefinitionList => {
                let frame = self.pop_frame("definition list")?;
                if let BlockFrame::DefinitionList {
                    items,
                    current_term,
                } = frame
                {
                    let mut items = items;
                    if let Some(term) = current_term {
                        items.push(ParsedDefinitionItem {
                            term,
                            definitions: Vec::new(),
                        });
                    }
                    self.finish_block(ParsedBlock::DefinitionList(ParsedDefinitionList { items }));
                }
            }
            TagEnd::DefinitionListTitle => {
                let frame = self.pop_frame("definition list title")?;
                if let BlockFrame::Paragraph(parser) = frame
                    && let Some(BlockFrame::DefinitionList { current_term, .. }) =
                        self.stack.last_mut()
                {
                    *current_term = Some(parser.into_inlines(&mut self.links));
                }
            }
            TagEnd::DefinitionListDefinition => {
                let frame = self.pop_frame("definition list definition")?;
                if let BlockFrame::DefinitionListDefinition { blocks } = frame {
                    self.finish_definition_list_definition(blocks);
                }
            }
            TagEnd::HtmlBlock => {}
        }
        Ok(())
    }

    fn inline_html(&mut self, html: String) {
        let (token, kind, href) = InlineHtmlToken::parse_tag(&html);
        match kind {
            InlineHtmlKind::Close => self.inline_html_close(token),
            InlineHtmlKind::SelfClosing => self.inline_html_self_closing(token, href),
            InlineHtmlKind::Open => self.inline_html_open(token, href),
        }
    }

    fn inline_html_open(&mut self, token: InlineHtmlToken, href: Option<String>) {
        match token {
            InlineHtmlToken::A => {
                if let Some(dest) = href {
                    self.start_html_link(dest);
                } else {
                    self.with_inline_parser(|p| p.stack.push(InlineFrame::Group(Vec::new())));
                }
            }
            InlineHtmlToken::B | InlineHtmlToken::Strong => {
                self.with_inline_parser(|p| p.start_strong());
            }
            InlineHtmlToken::I | InlineHtmlToken::Em => {
                self.with_inline_parser(|p| p.start_emphasis());
            }
            InlineHtmlToken::Code => {
                self.with_inline_parser(|p| p.start_code());
            }
            InlineHtmlToken::Del | InlineHtmlToken::S => {
                self.with_inline_parser(|p| p.start_strikethrough());
            }
            InlineHtmlToken::Br | InlineHtmlToken::Unknown => {}
        }
    }

    fn inline_html_close(&mut self, token: InlineHtmlToken) {
        match token {
            InlineHtmlToken::A => self.with_inline_parser(|p| {
                p.end_link().ok();
            }),
            InlineHtmlToken::B | InlineHtmlToken::Strong => self.with_inline_parser(|p| {
                p.end_strong().ok();
            }),
            InlineHtmlToken::I | InlineHtmlToken::Em => self.with_inline_parser(|p| {
                p.end_emphasis().ok();
            }),
            InlineHtmlToken::Code => self.with_inline_parser(|p| {
                p.end_code().ok();
            }),
            InlineHtmlToken::Del | InlineHtmlToken::S => self.with_inline_parser(|p| {
                p.end_strikethrough().ok();
            }),
            InlineHtmlToken::Br | InlineHtmlToken::Unknown => {}
        }
    }

    fn inline_html_self_closing(&mut self, token: InlineHtmlToken, _href: Option<String>) {
        if token == InlineHtmlToken::Br {
            if let Some(parser) = self.inline_parser() {
                parser.push_break(true);
            } else {
                self.push_inline_to_list_item(ParsedInline::HardBreak);
            }
        }
    }

    fn start_html_link(&mut self, dest: String) {
        let mut stack = std::mem::take(&mut self.stack);
        if let Some(parser) = Self::inline_parser_from_stack(&mut stack) {
            let kind = ParsedLinkKind::classify_url(&dest);
            parser.start_link(&mut self.links, dest, String::new(), kind);
        }
        self.stack = stack;
    }

    fn text(&mut self, text: String) {
        if let Some(BlockFrame::MetadataBlock { content, .. }) = self.stack.last_mut() {
            content.push_str(&text);
            return;
        }
        if let Some(img) = &mut self.paragraph_standalone_image {
            img.alt.push_str(&text);
            return;
        }
        if let Some(BlockFrame::CodeBlock { content, .. }) = self.stack.last_mut() {
            content.push_str(&text);
            return;
        }
        if let Some(parser) = self.inline_parser() {
            parser.push_text(text);
        } else {
            for inline in crate::parse::autolink::split_text_autolinks(&text, &mut self.links) {
                self.push_inline_to_list_item(inline);
            }
        }
    }

    fn code(&mut self, code: String) {
        if let Some(BlockFrame::CodeBlock { content, .. }) = self.stack.last_mut() {
            content.push_str(&code);
            return;
        }
        if let Some(parser) = self.inline_parser() {
            parser.push_code(code);
        } else {
            self.push_inline_to_list_item(ParsedInline::Code(code));
        }
    }

    fn soft_break(&mut self) {
        if let Some(BlockFrame::CodeBlock { content, .. }) = self.stack.last_mut() {
            content.push('\n');
            return;
        }
        if let Some(parser) = self.inline_parser() {
            parser.push_break(false);
        } else {
            self.push_inline_to_list_item(ParsedInline::SoftBreak);
        }
    }

    fn hard_break(&mut self) {
        if let Some(BlockFrame::CodeBlock { content, .. }) = self.stack.last_mut() {
            content.push('\n');
            return;
        }
        if let Some(parser) = self.inline_parser() {
            parser.push_break(true);
        } else {
            self.push_inline_to_list_item(ParsedInline::HardBreak);
        }
    }

    fn push_inline_to_list_item(&mut self, inline: ParsedInline) {
        if let Some(BlockFrame::DefinitionListDefinition { blocks }) = self.stack.last_mut() {
            if let Some(ParsedBlock::Paragraph(inlines)) = blocks.last_mut() {
                inlines.push(inline);
            } else {
                blocks.push(ParsedBlock::Paragraph(vec![inline]));
            }
            return;
        }
        if let Some(BlockFrame::ListItem { blocks, .. }) = self.stack.last_mut() {
            if let Some(ParsedBlock::Paragraph(inlines)) = blocks.last_mut() {
                inlines.push(inline);
            } else {
                blocks.push(ParsedBlock::Paragraph(vec![inline]));
            }
        }
    }

    fn task_list_marker(&mut self, checked: bool) {
        let id = self.next_checklist_id;
        self.next_checklist_id += 1;
        if let Some(BlockFrame::ListItem {
            checked: item_checked,
            checklist_id,
            ..
        }) = self.stack.last_mut()
        {
            *item_checked = checked;
            *checklist_id = Some(id);
        }
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
            _ => None,
        }
    }

    fn finish_block(&mut self, block: ParsedBlock) {
        if let Some(parent) = self.stack.last_mut() {
            match parent {
                BlockFrame::BlockQuote { blocks, .. } => blocks.push(block),
                BlockFrame::ListItem { blocks, .. } => blocks.push(block),
                BlockFrame::List { current_item, .. } => current_item.push(block),
                BlockFrame::FootnoteDefinition { blocks, .. } => blocks.push(block),
                BlockFrame::DefinitionListDefinition { blocks } => blocks.push(block),
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
