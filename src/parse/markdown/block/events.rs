//! Markdown leaf events: text, HTML, math, footnotes, and tight lists.

use crate::parse::dto::{ParsedBlock, ParsedInline, ParsedLinkKind};

use super::super::html::{InlineHtmlKind, InlineHtmlToken};
use super::super::inline::InlineFrame;
use super::{BlockFrame, ParserState};

impl<'a> ParserState<'a> {
    pub(super) fn inline_html(&mut self, html: String) {
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

    pub(super) fn text(&mut self, text: String) {
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

    pub(super) fn code(&mut self, code: String) {
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

    pub(super) fn soft_break(&mut self) {
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

    pub(super) fn hard_break(&mut self) {
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

    pub(super) fn push_inline_to_list_item(&mut self, inline: ParsedInline) {
        if let Some(BlockFrame::DefinitionListDefinition { blocks, .. }) = self.stack.last_mut() {
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

    pub(super) fn task_list_marker(&mut self, checked: bool) {
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
}
