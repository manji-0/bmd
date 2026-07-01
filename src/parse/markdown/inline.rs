//! Inline content accumulation.

use crate::parse::dto::{ParsedInline, ParsedLink, ParsedLinkKind};

use super::syntax_error;
use crate::parse::error::ParseError;

#[derive(Debug)]
pub(crate) struct InlineParser {
    output: Vec<ParsedInline>,
    pub(crate) stack: Vec<InlineFrame>,
}

#[derive(Debug)]
pub(crate) enum InlineFrame {
    Strong(Vec<ParsedInline>),
    Emphasis(Vec<ParsedInline>),
    Link(usize, Vec<ParsedInline>),
    /// Transparent fallback for links with invalid URLs: children are flattened.
    Group(Vec<ParsedInline>),
    /// Inline HTML code wrapper.
    Code(Vec<ParsedInline>),
    /// Inline HTML del/s/strike wrapper rendered as plain text.
    Deleted(Vec<ParsedInline>),
}

impl InlineParser {
    pub(crate) fn new() -> Self {
        Self {
            output: Vec::new(),
            stack: Vec::new(),
        }
    }

    pub(crate) fn current_target(&mut self) -> &mut Vec<ParsedInline> {
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
        self.current_target().push(ParsedInline::Text(text));
    }

    pub(crate) fn push_code(&mut self, code: String) {
        self.current_target().push(ParsedInline::Code(code));
    }

    pub(crate) fn push_break(&mut self, hard: bool) {
        self.current_target().push(if hard {
            ParsedInline::HardBreak
        } else {
            ParsedInline::SoftBreak
        });
    }

    pub(crate) fn start_strong(&mut self) {
        self.stack.push(InlineFrame::Strong(Vec::new()));
    }

    pub(crate) fn end_strong(&mut self) -> Result<(), ParseError> {
        let frame = self
            .stack
            .pop()
            .ok_or_else(|| syntax_error("unmatched strong end"))?;
        match frame {
            InlineFrame::Strong(children) => {
                self.current_target().push(ParsedInline::Strong(children))
            }
            _ => return Err(syntax_error("unmatched strong end")),
        }
        Ok(())
    }

    pub(crate) fn start_emphasis(&mut self) {
        self.stack.push(InlineFrame::Emphasis(Vec::new()));
    }

    pub(crate) fn end_emphasis(&mut self) -> Result<(), ParseError> {
        let frame = self
            .stack
            .pop()
            .ok_or_else(|| syntax_error("unmatched emphasis end"))?;
        match frame {
            InlineFrame::Emphasis(children) => {
                self.current_target().push(ParsedInline::Emphasis(children))
            }
            _ => return Err(syntax_error("unmatched emphasis end")),
        }
        Ok(())
    }

    pub(crate) fn start_code(&mut self) {
        self.stack.push(InlineFrame::Code(Vec::new()));
    }

    pub(crate) fn start_deleted(&mut self) {
        self.stack.push(InlineFrame::Deleted(Vec::new()));
    }

    pub(crate) fn end_deleted(&mut self) -> Result<(), ParseError> {
        let frame = self
            .stack
            .pop()
            .ok_or_else(|| syntax_error("unmatched deleted end"))?;
        match frame {
            InlineFrame::Deleted(children) => {
                self.current_target().extend(children);
            }
            _ => return Err(syntax_error("unmatched deleted end")),
        }
        Ok(())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.output.is_empty() && self.stack.is_empty()
    }

    pub(crate) fn start_link(
        &mut self,
        links: &mut Vec<ParsedLink>,
        dest_url: String,
        title: String,
        kind: ParsedLinkKind,
    ) {
        if dest_url.trim().is_empty() {
            self.stack.push(InlineFrame::Group(Vec::new()));
            return;
        }
        let link_id = links.len();
        links.push(ParsedLink {
            url: dest_url,
            title: if title.is_empty() { None } else { Some(title) },
            kind,
        });
        self.stack.push(InlineFrame::Link(link_id, Vec::new()));
    }

    pub(crate) fn end_link(&mut self) -> Result<(), ParseError> {
        let frame = self
            .stack
            .pop()
            .ok_or_else(|| syntax_error("unmatched link end"))?;
        match frame {
            InlineFrame::Link(link_id, children) => {
                self.current_target()
                    .push(ParsedInline::Link { link_id, children });
            }
            InlineFrame::Group(children) => {
                self.current_target().extend(children);
            }
            _ => return Err(syntax_error("unmatched link end")),
        }
        Ok(())
    }

    pub(crate) fn end_code(&mut self) -> Result<(), ParseError> {
        let frame = self
            .stack
            .pop()
            .ok_or_else(|| syntax_error("unmatched code end"))?;
        match frame {
            InlineFrame::Code(children) => self
                .current_target()
                .push(ParsedInline::Code(ParsedInline::plain_text(&children))),
            _ => return Err(syntax_error("unmatched code end")),
        }
        Ok(())
    }

    pub(crate) fn into_inlines(mut self) -> Vec<ParsedInline> {
        while let Some(frame) = self.stack.pop() {
            let children = match frame {
                InlineFrame::Strong(c) => vec![ParsedInline::Strong(c)],
                InlineFrame::Emphasis(c) => vec![ParsedInline::Emphasis(c)],
                InlineFrame::Link(link_id, c) => vec![ParsedInline::Link {
                    link_id,
                    children: c,
                }],
                InlineFrame::Group(c) => c,
                InlineFrame::Code(c) => {
                    vec![ParsedInline::Code(ParsedInline::plain_text(&c))]
                }
                InlineFrame::Deleted(c) => c,
            };
            self.current_target().extend(children);
        }
        self.output
    }
}
