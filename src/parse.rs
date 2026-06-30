//! Markdown parser adapter: pulldown-cmark events -> domain model.

use pulldown_cmark::{Alignment as CmarkAlignment, Event, Options, Parser, Tag, TagEnd};

use crate::domain::{
    Alignment, Block, CodeBlock, Document, Heading, HeadingLevel, Inline, Link, LinkId, LinkUrl,
    List, ListItem, MermaidDiagram, Table,
};
use crate::error::AppError;

/// Parsed inline HTML token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineHtmlToken {
    Br,
    A,
    B,
    Strong,
    I,
    Em,
    Code,
    Del,
    S,
    Unknown,
}

impl InlineHtmlToken {
    /// Parse a self-closing or opening tag like `<br>`, `<br/>`, `<a href="...">`.
    /// Returns the token, its kind, and, for opening `<a>`, the href attribute if present.
    fn parse_tag(html: &str) -> (Self, InlineHtmlKind, Option<String>) {
        let trimmed = html.trim();
        let after_open = trimmed.strip_prefix('<').unwrap_or(trimmed);
        let is_closing = after_open.starts_with('/');
        let tag_body = if is_closing {
            after_open.strip_prefix('/').unwrap_or(after_open)
        } else {
            after_open
        };
        let mut iter = tag_body.splitn(2, '>');
        let inner = iter.next().unwrap_or("").trim();
        if inner.is_empty() {
            return (
                Self::Unknown,
                if is_closing {
                    InlineHtmlKind::Close
                } else {
                    InlineHtmlKind::Open
                },
                None,
            );
        }
        let mut parts = inner.split_whitespace();
        let mut tag_name = parts.next().unwrap_or("");
        let rest: &str = inner[tag_name.len()..].trim();
        let is_self_closing = rest.ends_with('/') || (!rest.is_empty() && tag_name.ends_with('/'));
        if tag_name.ends_with('/') {
            tag_name = &tag_name[..tag_name.len() - 1];
        }
        let rest = if is_self_closing {
            rest[..rest.len().saturating_sub(1)].trim_end()
        } else {
            rest
        };
        let href = if tag_name.eq_ignore_ascii_case("a") && !is_closing {
            extract_href(rest).map(String::from)
        } else {
            None
        };
        let token = match tag_name.to_ascii_lowercase().as_str() {
            "br" => Self::Br,
            "a" => Self::A,
            "b" | "big" => Self::B,
            "strong" => Self::Strong,
            "i" | "cite" | "dfn" => Self::I,
            "em" => Self::Em,
            "code" => Self::Code,
            "del" => Self::Del,
            "s" | "strike" => Self::S,
            _ => Self::Unknown,
        };
        let kind = if is_closing {
            InlineHtmlKind::Close
        } else if is_self_closing || token == Self::Br {
            InlineHtmlKind::SelfClosing
        } else {
            InlineHtmlKind::Open
        };
        (token, kind, href)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineHtmlKind {
    Open,
    Close,
    SelfClosing,
}

fn extract_href(rest: &str) -> Option<&str> {
    // rest is the attribute substring, e.g. `href="https://x" target="_blank"`
    // We look for a `href` attribute that is not part of a longer attribute name.
    // Attribute names are matched case-insensitively.
    let lower = rest.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(pos) = lower[search_from..].find("href") {
        let abs = search_from + pos;
        let prefix = &rest[abs..];
        // Because `lower` matched "href" at `abs`, `prefix` must begin with
        // `href` or `HREF` in some ASCII case. Use the lower-cased prefix to
        // strip the attribute name without extra branching.
        let after = prefix
            .strip_prefix("href")
            .or_else(|| prefix.strip_prefix("HREF"))?;
        // Ensure `href` is a complete attribute name: preceding char must be
        // whitespace or start of string, and next non-space char must be `=`.
        let prev_ok = abs == 0 || rest[..abs].ends_with(|c: char| c.is_ascii_whitespace());
        let after_ws = after.trim_start_matches(|c: char| c.is_ascii_whitespace());
        if prev_ok && after_ws.starts_with('=') {
            let value = after_ws.strip_prefix('=').unwrap_or(after_ws).trim_start();
            let quote = value.chars().next()?;
            if quote != '\"' && quote != '\'' {
                return None;
            }
            let after_quote = &value[1..];
            return after_quote.find(quote).map(|end| &after_quote[..end]);
        }
        search_from = abs + 4;
    }
    None
}
/// Parse CommonMark (with tables) into a `Document`.
pub fn parse(markdown: &str) -> Result<Document, AppError> {
    let parser = Parser::new_ext(markdown, Options::all());
    let mut state = ParserState::new(parser);
    state.run()?;
    Document::new(state.blocks, state.links).map_err(AppError::Document)
}

#[derive(Debug)]
struct ParserState<'a> {
    iter: std::iter::Peekable<Parser<'a>>,
    blocks: Vec<Block>,
    links: Vec<Link>,
    stack: Vec<BlockFrame>,
}

#[derive(Debug)]
enum BlockFrame {
    BlockQuote(Vec<Block>),
    List {
        ordered: bool,
        items: Vec<ListItem>,
        current_item: Vec<Block>,
    },
    ListItem(Vec<Block>),
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

#[derive(Debug)]
struct InlineParser {
    output: Vec<Inline>,
    stack: Vec<InlineFrame>,
}

#[derive(Debug)]
enum InlineFrame {
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    Link(LinkId, Vec<Inline>),
    /// Transparent fallback for links with invalid URLs: children are flattened.
    Group(Vec<Inline>),
    /// Inline HTML code wrapper.
    Code(Vec<Inline>),
    /// Inline HTML del/s/strike wrapper rendered as plain text.
    Deleted(Vec<Inline>),
}

impl InlineParser {
    fn new() -> Self {
        Self {
            output: Vec::new(),
            stack: Vec::new(),
        }
    }

    fn current_target(&mut self) -> &mut Vec<Inline> {
        match self.stack.last_mut() {
            Some(InlineFrame::Strong(v))
            | Some(InlineFrame::Emphasis(v))
            | Some(InlineFrame::Link(_, v))
            | Some(InlineFrame::Group(v))
            | Some(InlineFrame::Code(v))
            | Some(InlineFrame::Deleted(v)) => v,
            None => &mut self.output,
        }
    }

    fn push_text(&mut self, text: String) {
        self.current_target().push(Inline::Text(text));
    }

    fn push_code(&mut self, code: String) {
        self.current_target().push(Inline::Code(code));
    }

    fn push_break(&mut self, hard: bool) {
        self.current_target().push(if hard {
            Inline::HardBreak
        } else {
            Inline::SoftBreak
        });
    }

    fn start_strong(&mut self) {
        self.stack.push(InlineFrame::Strong(Vec::new()));
    }

    fn end_strong(&mut self) -> Result<(), AppError> {
        let frame = self
            .stack
            .pop()
            .ok_or_else(|| AppError::MarkdownParse("unmatched strong end".into()))?;
        match frame {
            InlineFrame::Strong(children) => self.current_target().push(Inline::Strong(children)),
            _ => return Err(AppError::MarkdownParse("unmatched strong end".into())),
        }
        Ok(())
    }

    fn start_emphasis(&mut self) {
        self.stack.push(InlineFrame::Emphasis(Vec::new()));
    }

    fn end_emphasis(&mut self) -> Result<(), AppError> {
        let frame = self
            .stack
            .pop()
            .ok_or_else(|| AppError::MarkdownParse("unmatched emphasis end".into()))?;
        match frame {
            InlineFrame::Emphasis(children) => {
                self.current_target().push(Inline::Emphasis(children))
            }
            _ => return Err(AppError::MarkdownParse("unmatched emphasis end".into())),
        }
        Ok(())
    }

    fn start_code(&mut self) {
        self.stack.push(InlineFrame::Code(Vec::new()));
    }

    fn start_deleted(&mut self) {
        self.stack.push(InlineFrame::Deleted(Vec::new()));
    }

    fn end_deleted(&mut self) -> Result<(), AppError> {
        let frame = self
            .stack
            .pop()
            .ok_or_else(|| AppError::MarkdownParse("unmatched deleted end".into()))?;
        match frame {
            // Deleted text is intentionally flattened to plain text; terminal does
            // not have a reliable strikethrough glyph across fonts, so we keep it simple.
            InlineFrame::Deleted(children) => {
                self.current_target().extend(children);
            }
            _ => return Err(AppError::MarkdownParse("unmatched deleted end".into())),
        }
        Ok(())
    }

    fn start_link(&mut self, links: &mut Vec<Link>, dest_url: String, title: String) {
        match LinkUrl::new(dest_url) {
            Ok(url) => {
                let id = LinkId(links.len());
                links.push(Link {
                    url,
                    title: if title.is_empty() { None } else { Some(title) },
                });
                self.stack.push(InlineFrame::Link(id, Vec::new()));
            }
            Err(_) => {
                self.stack.push(InlineFrame::Group(Vec::new()));
            }
        }
    }

    fn end_link(&mut self) -> Result<(), AppError> {
        let frame = self
            .stack
            .pop()
            .ok_or_else(|| AppError::MarkdownParse("unmatched link end".into()))?;
        match frame {
            InlineFrame::Link(id, children) => {
                self.current_target().push(Inline::Link(id, children));
            }
            InlineFrame::Group(children) => {
                self.current_target().extend(children);
            }
            _ => return Err(AppError::MarkdownParse("unmatched link end".into())),
        }
        Ok(())
    }

    fn end_code(&mut self) -> Result<(), AppError> {
        let frame = self
            .stack
            .pop()
            .ok_or_else(|| AppError::MarkdownParse("unmatched code end".into()))?;
        match frame {
            InlineFrame::Code(children) => {
                self.current_target()
                    .push(Inline::Code(Inline::plain_text(&children)));
            }
            _ => return Err(AppError::MarkdownParse("unmatched code end".into())),
        }
        Ok(())
    }

    fn into_inlines(mut self) -> Vec<Inline> {
        // Defensive: flatten any unclosed inline frames so the UI never panics.
        while let Some(frame) = self.stack.pop() {
            let children = match frame {
                InlineFrame::Strong(c) => vec![Inline::Strong(c)],
                InlineFrame::Emphasis(c) => vec![Inline::Emphasis(c)],
                InlineFrame::Link(id, c) => vec![Inline::Link(id, c)],
                InlineFrame::Group(c) => c,
                InlineFrame::Code(c) => vec![Inline::Code(Inline::plain_text(&c))],
                InlineFrame::Deleted(c) => c,
            };
            self.current_target().extend(children);
        }
        self.output
    }
}

impl<'a> ParserState<'a> {
    fn new(parser: Parser<'a>) -> Self {
        Self {
            iter: parser.peekable(),
            blocks: Vec::new(),
            links: Vec::new(),
            stack: Vec::new(),
        }
    }

    fn run(&mut self) -> Result<(), AppError> {
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
                Event::FootnoteReference(_)
                | Event::TaskListMarker(_)
                | Event::InlineMath(_)
                | Event::DisplayMath(_) => {}
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
            Tag::Item => self.stack.push(BlockFrame::ListItem(Vec::new())),
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
                    p.start_link(&mut self.links, dest, title);
                }
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                // Images are not inline-rendered; show them as a link placeholder.
                let dest = dest_url.into_string();
                let title = title.into_string();
                if let Some(
                    BlockFrame::Paragraph(p) | BlockFrame::Heading(p) | BlockFrame::TableCell(p),
                ) = self.stack.last_mut()
                {
                    p.start_link(&mut self.links, dest, title);
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
                    self.finish_block(Block::Paragraph(parser.into_inlines()));
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
                        self.finish_block(Block::Mermaid(MermaidDiagram { source: content }));
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
                if let BlockFrame::ListItem(mut blocks) = frame {
                    if let Some(BlockFrame::List { items, .. }) = self.stack.last_mut() {
                        items.push(ListItem {
                            content: std::mem::take(&mut blocks),
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
            TagEnd::Image => self.with_inline_parser(|p| {
                p.end_link().ok();
            }),
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
            parser.start_link(&mut self.links, dest, String::new());
        }
        self.stack = stack;
    }

    fn text(&mut self, text: String) {
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
        if let Some(BlockFrame::ListItem(blocks)) = self.stack.last_mut() {
            if let Some(Block::Paragraph(inlines)) = blocks.last_mut() {
                inlines.push(inline);
            } else {
                blocks.push(Block::Paragraph(vec![inline]));
            }
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
                BlockFrame::ListItem(blocks) => blocks.push(block),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_paragraph() {
        let doc = parse("Hello **world**!").unwrap();
        assert_eq!(doc.blocks.len(), 1);
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert_eq!(inlines.len(), 3);
    }

    #[test]
    fn parse_mermaid_block() {
        let doc = parse("```mermaid\ngraph TD; A-->B;\n```").unwrap();
        assert_eq!(doc.blocks.len(), 1);
        assert!(matches!(doc.blocks[0], Block::Mermaid(_)));
    }

    #[test]
    fn parse_table() {
        let doc = parse("| a | b |\n|---|---|\n| 1 | 2 |").unwrap();
        assert_eq!(doc.blocks.len(), 1);
        let Block::Table(table) = &doc.blocks[0] else {
            panic!("expected table");
        };
        assert_eq!(table.headers.len(), 2);
        assert_eq!(table.rows.len(), 1);
    }

    #[test]
    fn parse_link_collects_url() {
        let doc = parse("[text](https://example.com)").unwrap();
        assert_eq!(doc.links.len(), 1);
        assert_eq!(doc.links[0].url.as_str(), "https://example.com");
    }

    #[test]
    fn parse_headings_all_levels() {
        let doc = parse("# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6").unwrap();
        assert_eq!(doc.blocks.len(), 6);
        for (i, block) in doc.blocks.iter().enumerate() {
            let Block::Heading(heading) = block else {
                panic!("expected heading at {i}");
            };
            assert_eq!(heading.level.as_u8(), i as u8 + 1);
        }
    }

    #[test]
    fn parse_blockquote() {
        let doc = parse("> quoted").unwrap();
        assert_eq!(doc.blocks.len(), 1);
        let Block::BlockQuote(children) = &doc.blocks[0] else {
            panic!("expected blockquote");
        };
        assert_eq!(children.len(), 1);
        assert!(matches!(children[0], Block::Paragraph(_)));
    }

    #[test]
    fn parse_unordered_list() {
        let doc = parse("- alpha\n- beta").unwrap();
        assert_eq!(doc.blocks.len(), 1);
        let Block::List(list) = &doc.blocks[0] else {
            panic!("expected list");
        };
        assert!(!list.ordered);
        assert_eq!(list.items.len(), 2);
    }

    #[test]
    fn parse_ordered_list() {
        let doc = parse("1. first\n2. second").unwrap();
        assert_eq!(doc.blocks.len(), 1);
        let Block::List(list) = &doc.blocks[0] else {
            panic!("expected list");
        };
        assert!(list.ordered);
        assert_eq!(list.items.len(), 2);
    }

    #[test]
    fn parse_nested_list() {
        let doc = parse("- outer\n  - inner").unwrap();
        let Block::List(outer) = &doc.blocks[0] else {
            panic!("expected outer list");
        };
        assert_eq!(outer.items.len(), 1);
        let nested = outer.items[0]
            .content
            .iter()
            .find(|b| matches!(b, Block::List(_)))
            .expect("expected a nested list");
        let Block::List(inner) = nested else {
            unreachable!();
        };
        assert_eq!(inner.items.len(), 1);
    }

    #[test]
    fn parse_fenced_code_block_with_language() {
        let doc = parse("```rust\nfn main() {}\n```").unwrap();
        assert_eq!(doc.blocks.len(), 1);
        let Block::CodeBlock(cb) = &doc.blocks[0] else {
            panic!("expected code block");
        };
        assert_eq!(cb.language.as_deref(), Some("rust"));
        assert!(cb.content.contains("fn main"));
    }

    #[test]
    fn parse_fenced_code_block_language_is_case_insensitive() {
        let doc = parse("```MERMAID\ngraph TD;\n```").unwrap();
        assert!(matches!(doc.blocks[0], Block::Mermaid(_)));
    }

    #[test]
    fn parse_indented_code_block() {
        let doc = parse("    line one\n    line two").unwrap();
        let Block::CodeBlock(cb) = &doc.blocks[0] else {
            panic!("expected code block");
        };
        assert!(cb.language.is_none());
        assert_eq!(cb.content.lines().count(), 2);
    }

    #[test]
    fn parse_image_is_represented_as_link_placeholder() {
        let doc = parse("![alt text](diagram.png)").unwrap();
        assert_eq!(doc.links.len(), 1);
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(matches!(inlines[0], Inline::Link(_, _)));
    }

    #[test]
    fn parse_emphasis_and_strong() {
        let doc = parse("*emphasis* **strong**").unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(matches!(inlines[0], Inline::Emphasis(_)));
        assert!(matches!(inlines[1], Inline::Text(_)));
        assert!(matches!(inlines[2], Inline::Strong(_)));
    }

    #[test]
    fn parse_inline_code() {
        let doc = parse("`code`").unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(matches!(inlines[0], Inline::Code(_)));
    }

    #[test]
    fn parse_hard_line_break() {
        let doc = parse("line  \\\nnext").unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(inlines.iter().any(|i| matches!(i, Inline::HardBreak)));
    }

    #[test]
    fn parse_horizontal_rule() {
        let doc = parse("---").unwrap();
        assert_eq!(doc.blocks.len(), 1);
        assert!(matches!(doc.blocks[0], Block::Rule));
    }

    #[test]
    fn parse_strikethrough_is_treated_as_plain_text() {
        let doc = parse("~~deleted~~").unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        // Strikethrough wrapper is ignored; only the text remains.
        assert!(matches!(&inlines[0], Inline::Text(t) if t == "deleted"));
    }

    #[test]
    fn parse_empty_link_url_is_rejected() {
        let doc = parse("[text](  )").unwrap();
        assert!(doc.links.is_empty());
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(matches!(&inlines[0], Inline::Text(t) if t == "text"));
    }

    #[test]
    fn parse_inline_html_br_becomes_hard_break() {
        let doc = parse("hello<br>world").unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(matches!(inlines[1], Inline::HardBreak));
    }

    #[test]
    fn parse_inline_html_br_with_slash_becomes_hard_break() {
        let doc = parse("hello <br/> world").unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(inlines.iter().any(|i| matches!(i, Inline::HardBreak)));
    }

    #[test]
    fn parse_inline_html_link_collected() {
        let doc = parse(r#"<a href="https://example.com">text</a>"#).unwrap();
        assert_eq!(doc.links.len(), 1);
        assert_eq!(doc.links[0].url.as_str(), "https://example.com");
    }

    #[test]
    fn parse_inline_html_link_closes_before_trailing_text() {
        let doc = parse(r#"<a href="https://example.com">link</a> after"#).unwrap();
        assert_eq!(doc.links.len(), 1);
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        let Inline::Link(_, link_children) = &inlines[0] else {
            panic!("expected link, got {:?}", inlines[0]);
        };
        assert!(
            link_children
                .iter()
                .any(|i| matches!(i, Inline::Text(t) if t == "link"))
        );
        assert!(
            inlines
                .iter()
                .any(|i| matches!(i, Inline::Text(t) if t == " after"))
        );
    }

    #[test]
    fn parse_inline_html_link_without_href_is_ignored() {
        let doc = parse("<a>text</a>").unwrap();
        assert!(doc.links.is_empty());
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(matches!(&inlines[0], Inline::Text(t) if t == "text"));
    }

    #[test]
    fn parse_inline_html_b_becomes_strong_and_closes() {
        let doc = parse("<b>bold</b> normal").unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        let Inline::Strong(strong_children) = &inlines[0] else {
            panic!("expected strong");
        };
        assert!(
            strong_children
                .iter()
                .any(|i| matches!(i, Inline::Text(t) if t == "bold"))
        );
        assert!(
            inlines
                .iter()
                .any(|i| matches!(i, Inline::Text(t) if t == " normal"))
        );
    }

    #[test]
    fn parse_inline_html_i_and_em_become_emphasis() {
        let doc = parse("<i>i</i> <em>em</em>").unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        let emphasis_count = inlines
            .iter()
            .filter(|i| matches!(i, Inline::Emphasis(_)))
            .count();
        assert_eq!(emphasis_count, 2);
    }

    #[test]
    fn parse_inline_html_code_becomes_inline_code() {
        let doc = parse("<code>x + y</code>").unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(matches!(&inlines[0], Inline::Code(t) if t == "x + y"));
    }

    #[test]
    fn parse_inline_html_nested_em_in_strong() {
        let doc = parse("<strong>bold <em>italic</em></strong>").unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        let Inline::Strong(strong_children) = &inlines[0] else {
            panic!("expected strong");
        };
        assert!(
            strong_children
                .iter()
                .any(|i| matches!(i, Inline::Emphasis(_)))
        );
    }

    #[test]
    fn parse_inline_html_unknown_tag_is_ignored() {
        let doc = parse("foo <span>bar</span> baz").unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        let texts: Vec<&str> = inlines
            .iter()
            .filter_map(|i| match i {
                Inline::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, vec!["foo ", "bar", " baz"]);
    }

    #[test]
    fn parse_inline_html_del_is_flattened() {
        let doc = parse("<del>removed</del>").unwrap();
        let Block::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(matches!(&inlines[0], Inline::Text(t) if t == "removed"));
    }

    #[test]
    fn parse_inline_html_href_ignores_prefixed_attribute() {
        let doc = parse(r#"<a data-href="wrong" href="https://right.example">x</a>"#).unwrap();
        assert_eq!(doc.links.len(), 1);
        assert_eq!(doc.links[0].url.as_str(), "https://right.example");
    }

    #[test]
    fn parse_inline_html_in_heading() {
        let doc = parse("# hello <br> world").unwrap();
        let Block::Heading(heading) = &doc.blocks[0] else {
            panic!("expected heading");
        };
        assert!(
            heading
                .content
                .iter()
                .any(|i| matches!(i, Inline::HardBreak))
        );
    }
}
