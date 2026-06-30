//! Block-level markdown event handling.

use pulldown_cmark::{Alignment as CmarkAlignment, Event, Parser, Tag, TagEnd};

use crate::domain::{
    Alignment, Block, ChecklistId, CodeBlock, Heading, HeadingLevel, Inline, Link, LinkId,
    LinkKind, List, ListItem, MermaidDiagram, Table,
};
use crate::error::AppError;

use super::html::{InlineHtmlKind, InlineHtmlToken};
use super::inline::{InlineFrame, InlineParser};

#[derive(Debug)]
struct PendingStandaloneImage {
    src: String,
    title: Option<String>,
    alt: String,
}

#[derive(Debug)]
pub(crate) struct ParserState<'a> {
    iter: std::iter::Peekable<Parser<'a>>,
    blocks: Vec<Block>,
    links: Vec<Link>,
    mermaid_diagrams: Vec<MermaidDiagram>,
    stack: Vec<BlockFrame>,
    paragraph_standalone_image: Option<PendingStandaloneImage>,
    next_checklist_id: u32,
}

#[derive(Debug)]
enum BlockFrame {
    BlockQuote(Vec<Block>),
    List {
        ordered: bool,
        items: Vec<ListItem>,
        current_item: Vec<Block>,
    },
    ListItem {
        blocks: Vec<Block>,
        checked: bool,
        checklist_id: Option<ChecklistId>,
    },
    Heading(InlineParser),
    Paragraph(InlineParser),
    Table {
        alignments: Vec<Alignment>,
        headers: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    TableHead(Vec<Vec<Inline>>),
    TableRow(Vec<Vec<Inline>>),
    TableCell(InlineParser),
    CodeBlock {
        language: Option<String>,
        content: String,
        is_mermaid: bool,
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
        }
    }

    pub(crate) fn run(&mut self) -> Result<(), AppError> {
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
                Event::Rule => self.blocks.push(Block::Rule),
                Event::FootnoteReference(_) | Event::InlineMath(_) | Event::DisplayMath(_) => {}
                Event::TaskListMarker(checked) => self.task_list_marker(checked),
            }
        }
        Ok(())
    }

    fn start_tag(&mut self, tag: Tag<'a>) -> Result<(), AppError> {
        match tag {
            Tag::Paragraph => self.stack.push(BlockFrame::Paragraph(InlineParser::new())),
            Tag::Heading { .. } => {
                self.stack.push(BlockFrame::Heading(InlineParser::new()));
            }
            Tag::BlockQuote(_) => self.stack.push(BlockFrame::BlockQuote(Vec::new())),
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
            Tag::Strikethrough => self.with_inline_parser(|p| p.start_deleted()),
            Tag::Link {
                dest_url, title, ..
            } => {
                let dest = dest_url.into_string();
                let title = title.into_string();
                if let Some(
                    BlockFrame::Paragraph(p) | BlockFrame::Heading(p) | BlockFrame::TableCell(p),
                ) = self.stack.last_mut()
                {
                    p.start_link(&mut self.links, dest, title, LinkKind::Web);
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
                            LinkKind::Image,
                        );
                    }
                } else if let Some(BlockFrame::Heading(p) | BlockFrame::TableCell(p)) =
                    self.stack.last_mut()
                {
                    p.start_link(
                        &mut self.links,
                        dest,
                        title.unwrap_or_default(),
                        LinkKind::Image,
                    );
                }
            }
            Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::HtmlBlock
            | Tag::Superscript
            | Tag::Subscript
            | Tag::MetadataBlock(_) => {}
        }
        Ok(())
    }

    fn end_tag(&mut self, tag_end: TagEnd) -> Result<(), AppError> {
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
                        let id = LinkId(self.links.len());
                        if let Ok(url) = crate::domain::LinkUrl::new(pending.src) {
                            self.links.push(Link {
                                url,
                                title: pending.title,
                                kind: LinkKind::Image,
                            });
                            self.finish_block(Block::Paragraph(vec![Inline::Link(
                                id,
                                vec![Inline::Text(label)],
                            )]));
                        } else {
                            self.finish_block(Block::Paragraph(vec![Inline::Text(label)]));
                        }
                    } else {
                        self.finish_block(Block::Paragraph(parser.into_inlines()));
                    }
                }
            }
            TagEnd::Heading(level) => {
                let frame = self.pop_frame("heading")?;
                if let BlockFrame::Heading(parser) = frame {
                    let level =
                        HeadingLevel::from_u8(heading_level_to_u8(level)).ok_or_else(|| {
                            AppError::MarkdownParse(format!("invalid heading level {level:?}"))
                        })?;
                    self.finish_block(Block::Heading(Heading {
                        level,
                        content: parser.into_inlines(),
                    }));
                }
            }
            TagEnd::BlockQuote(_) => {
                let frame = self.pop_frame("blockquote")?;
                if let BlockFrame::BlockQuote(blocks) = frame {
                    self.finish_block(Block::BlockQuote(blocks));
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
                            .push(MermaidDiagram { source: content });
                        let label = mermaid_link_label(&self.mermaid_diagrams[diagram_idx].source);
                        let id = LinkId(self.links.len());
                        if let Ok(url) =
                            crate::domain::LinkUrl::new(format!("bmd:mermaid:{diagram_idx}"))
                        {
                            self.links.push(Link {
                                url,
                                title: None,
                                kind: LinkKind::Mermaid,
                            });
                            self.finish_block(Block::Paragraph(vec![Inline::Link(
                                id,
                                vec![Inline::Text(label)],
                            )]));
                        } else {
                            self.finish_block(Block::Paragraph(vec![Inline::Text(label)]));
                        }
                    } else {
                        self.finish_block(Block::CodeBlock(CodeBlock { language, content }));
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
                        return Err(AppError::MarkdownParse(
                            "list ended with unclosed item".into(),
                        ));
                    }
                    self.finish_block(Block::List(List { ordered, items }));
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
                        items.push(ListItem {
                            checklist_id,
                            checked,
                            content: blocks,
                        });
                    } else {
                        return Err(AppError::MarkdownParse(
                            "list item without parent list".into(),
                        ));
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
                    self.finish_block(Block::Table(Table {
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
                        return Err(AppError::MarkdownParse(
                            "table head without parent table".into(),
                        ));
                    }
                }
            }
            TagEnd::TableRow => {
                let frame = self.pop_frame("table row")?;
                if let BlockFrame::TableRow(cells) = frame {
                    if let Some(BlockFrame::Table { rows, .. }) = self.stack.last_mut() {
                        rows.push(cells);
                    } else {
                        return Err(AppError::MarkdownParse(
                            "table row without parent table".into(),
                        ));
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
                            return Err(AppError::MarkdownParse(
                                "table cell without parent row or head".into(),
                            ));
                        }
                    };
                    cells.push(parser.into_inlines());
                }
            }
            TagEnd::Emphasis => self.with_inline_parser(|p| {
                p.end_emphasis().ok();
            }),
            TagEnd::Strong => self.with_inline_parser(|p| {
                p.end_strong().ok();
            }),
            TagEnd::Strikethrough => self.with_inline_parser(|p| {
                p.end_deleted().ok();
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
            TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::HtmlBlock
            | TagEnd::Superscript
            | TagEnd::Subscript
            | TagEnd::MetadataBlock(_) => {}
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
                    // Push a transparent group so that a matching </a> does not
                    // accidentally close an outer HTML formatting frame.
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
                self.with_inline_parser(|p| p.start_deleted());
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
                p.end_deleted().ok();
            }),
            InlineHtmlToken::Br | InlineHtmlToken::Unknown => {}
        }
    }

    fn inline_html_self_closing(&mut self, token: InlineHtmlToken, _href: Option<String>) {
        if token == InlineHtmlToken::Br {
            if let Some(parser) = self.inline_parser() {
                parser.push_break(true);
            } else {
                self.push_inline_to_list_item(Inline::HardBreak);
            }
        }
    }

    fn start_html_link(&mut self, dest: String) {
        // We need to call start_link which takes `&mut self.links`. inline_parser()
        // borrows from `self.stack`, so we can't hold both borrows at once.
        // Temporarily take the stack, start the link, then restore it.
        let mut stack = std::mem::take(&mut self.stack);
        if let Some(parser) = Self::inline_parser_from_stack(&mut stack) {
            parser.start_link(&mut self.links, dest, String::new(), LinkKind::Web);
        }
        self.stack = stack;
    }

    fn text(&mut self, text: String) {
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
            self.push_inline_to_list_item(Inline::Text(text));
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
            self.push_inline_to_list_item(Inline::Code(code));
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
            self.push_inline_to_list_item(Inline::SoftBreak);
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
            self.push_inline_to_list_item(Inline::HardBreak);
        }
    }

    /// Append inline content directly inside a list item when no paragraph frame is active.
    fn push_inline_to_list_item(&mut self, inline: Inline) {
        if let Some(BlockFrame::ListItem { blocks, .. }) = self.stack.last_mut() {
            if let Some(Block::Paragraph(inlines)) = blocks.last_mut() {
                inlines.push(inline);
            } else {
                blocks.push(Block::Paragraph(vec![inline]));
            }
        }
    }

    fn task_list_marker(&mut self, checked: bool) {
        let id = ChecklistId(self.next_checklist_id);
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
            BlockFrame::Paragraph(p) | BlockFrame::Heading(p) | BlockFrame::TableCell(p) => Some(p),
            _ => None,
        }
    }

    fn finish_block(&mut self, block: Block) {
        if let Some(parent) = self.stack.last_mut() {
            match parent {
                BlockFrame::BlockQuote(blocks) => blocks.push(block),
                BlockFrame::ListItem { blocks, .. } => blocks.push(block),
                BlockFrame::List { current_item, .. } => current_item.push(block),
                _ => self.blocks.push(block),
            }
        } else {
            self.blocks.push(block);
        }
    }

    fn pop_frame(&mut self, expected: &str) -> Result<BlockFrame, AppError> {
        self.stack
            .pop()
            .ok_or_else(|| AppError::MarkdownParse(format!("unexpected end tag for {expected}")))
    }

    pub(crate) fn into_parts(self) -> (Vec<Block>, Vec<Link>, Vec<MermaidDiagram>) {
        (self.blocks, self.links, self.mermaid_diagrams)
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

fn map_alignment(a: CmarkAlignment) -> Alignment {
    match a {
        CmarkAlignment::None => Alignment::None,
        CmarkAlignment::Left => Alignment::Left,
        CmarkAlignment::Center => Alignment::Center,
        CmarkAlignment::Right => Alignment::Right,
    }
}
