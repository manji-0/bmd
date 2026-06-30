//! Inline content accumulation.

use crate::domain::{Inline, Link, LinkId, LinkUrl};
use crate::error::AppError;

#[derive(Debug)]
pub(crate) struct InlineParser {
    output: Vec<Inline>,
    pub(crate) stack: Vec<InlineFrame>,
}

#[derive(Debug)]
pub(crate) enum InlineFrame {
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
    pub(crate) fn new() -> Self {
        Self {
            output: Vec::new(),
            stack: Vec::new(),
        }
    }

    pub(crate) fn current_target(&mut self) -> &mut Vec<Inline> {
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

    pub(crate) fn push_text(&mut self, text: String) {
        self.current_target().push(Inline::Text(text));
    }

    pub(crate) fn push_code(&mut self, code: String) {
        self.current_target().push(Inline::Code(code));
    }

    pub(crate) fn push_break(&mut self, hard: bool) {
        self.current_target().push(if hard {
            Inline::HardBreak
        } else {
            Inline::SoftBreak
        });
    }

    pub(crate) fn start_strong(&mut self) {
        self.stack.push(InlineFrame::Strong(Vec::new()));
    }

    pub(crate) fn end_strong(&mut self) -> Result<(), AppError> {
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

    pub(crate) fn start_emphasis(&mut self) {
        self.stack.push(InlineFrame::Emphasis(Vec::new()));
    }

    pub(crate) fn end_emphasis(&mut self) -> Result<(), AppError> {
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

    pub(crate) fn start_code(&mut self) {
        self.stack.push(InlineFrame::Code(Vec::new()));
    }

    pub(crate) fn start_deleted(&mut self) {
        self.stack.push(InlineFrame::Deleted(Vec::new()));
    }

    pub(crate) fn end_deleted(&mut self) -> Result<(), AppError> {
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

    pub(crate) fn is_empty(&self) -> bool {
        self.output.is_empty() && self.stack.is_empty()
    }

    pub(crate) fn start_link(&mut self, links: &mut Vec<Link>, dest_url: String, title: String) {
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

    pub(crate) fn end_link(&mut self) -> Result<(), AppError> {
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

    pub(crate) fn end_code(&mut self) -> Result<(), AppError> {
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

    pub(crate) fn into_inlines(mut self) -> Vec<Inline> {
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
