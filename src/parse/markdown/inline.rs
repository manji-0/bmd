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
    Strikethrough(Vec<ParsedInline>),
    Subscript(Vec<ParsedInline>),
    Superscript(Vec<ParsedInline>),
    Link(usize, Vec<ParsedInline>),
    /// Transparent fallback for links with invalid URLs: children are flattened.
    Group(Vec<ParsedInline>),
    /// Inline HTML code wrapper.
    Code(Vec<ParsedInline>),
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
            | Some(InlineFrame::Strikethrough(v))
            | Some(InlineFrame::Subscript(v))
            | Some(InlineFrame::Superscript(v))
            | Some(InlineFrame::Link(_, v))
            | Some(InlineFrame::Group(v))
            | Some(InlineFrame::Code(v)) => v,
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

    pub(crate) fn start_strikethrough(&mut self) {
        self.stack.push(InlineFrame::Strikethrough(Vec::new()));
    }

    pub(crate) fn end_strikethrough(&mut self) -> Result<(), ParseError> {
        let frame = self
            .stack
            .pop()
            .ok_or_else(|| syntax_error("unmatched strikethrough end"))?;
        match frame {
            InlineFrame::Strikethrough(children) => self
                .current_target()
                .push(ParsedInline::Strikethrough(children)),
            _ => return Err(syntax_error("unmatched strikethrough end")),
        }
        Ok(())
    }

    pub(crate) fn start_subscript(&mut self) {
        self.stack.push(InlineFrame::Subscript(Vec::new()));
    }

    pub(crate) fn end_subscript(&mut self) -> Result<(), ParseError> {
        let frame = self
            .stack
            .pop()
            .ok_or_else(|| syntax_error("unmatched subscript end"))?;
        match frame {
            InlineFrame::Subscript(children) => {
                self.current_target().push(ParsedInline::Subscript(children))
            }
            _ => return Err(syntax_error("unmatched subscript end")),
        }
        Ok(())
    }

    pub(crate) fn start_superscript(&mut self) {
        self.stack.push(InlineFrame::Superscript(Vec::new()));
    }

    pub(crate) fn end_superscript(&mut self) -> Result<(), ParseError> {
        let frame = self
            .stack
            .pop()
            .ok_or_else(|| syntax_error("unmatched superscript end"))?;
        match frame {
            InlineFrame::Superscript(children) => self
                .current_target()
                .push(ParsedInline::Superscript(children)),
            _ => return Err(syntax_error("unmatched superscript end")),
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
                InlineFrame::Strikethrough(c) => vec![ParsedInline::Strikethrough(c)],
                InlineFrame::Subscript(c) => vec![ParsedInline::Subscript(c)],
                InlineFrame::Superscript(c) => vec![ParsedInline::Superscript(c)],
                InlineFrame::Link(link_id, c) => vec![ParsedInline::Link {
                    link_id,
                    children: c,
                }],
                InlineFrame::Group(c) => c,
                InlineFrame::Code(c) => {
                    vec![ParsedInline::Code(ParsedInline::plain_text(&c))]
                }
            };
            self.current_target().extend(children);
        }
        normalize_inlines(self.output)
    }
}

fn normalize_inlines(inlines: Vec<ParsedInline>) -> Vec<ParsedInline> {
    inlines
        .into_iter()
        .flat_map(normalize_inline)
        .collect()
}

fn normalize_inline(inline: ParsedInline) -> Vec<ParsedInline> {
    match inline {
        ParsedInline::Text(text) => expand_tight_sub_sup_text(&text),
        ParsedInline::Strong(children) => vec![ParsedInline::Strong(normalize_inlines(children))],
        ParsedInline::Emphasis(children) => vec![ParsedInline::Emphasis(normalize_inlines(children))],
        ParsedInline::Strikethrough(children) => {
            vec![ParsedInline::Strikethrough(normalize_inlines(children))]
        }
        ParsedInline::Subscript(children) => vec![ParsedInline::Subscript(normalize_inlines(children))],
        ParsedInline::Superscript(children) => {
            vec![ParsedInline::Superscript(normalize_inlines(children))]
        }
        ParsedInline::Link { link_id, children } => vec![ParsedInline::Link {
            link_id,
            children: normalize_inlines(children),
        }],
        other => vec![other],
    }
}

fn expand_tight_sub_sup_text(text: &str) -> Vec<ParsedInline> {
    let with_sub = expand_delimited(text, '~', ParsedInline::Subscript);
    with_sub
        .into_iter()
        .flat_map(|inline| match inline {
            ParsedInline::Text(value) => expand_delimited(&value, '^', ParsedInline::Superscript),
            other => vec![other],
        })
        .collect()
}

fn expand_delimited(
    text: &str,
    marker: char,
    wrap: fn(Vec<ParsedInline>) -> ParsedInline,
) -> Vec<ParsedInline> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(marker) {
        if start > 0 {
            out.push(ParsedInline::Text(rest[..start].to_string()));
        }
        rest = &rest[start + marker.len_utf8()..];
        let Some(end) = rest.find(marker) else {
            out.push(ParsedInline::Text(format!("{marker}{rest}")));
            return out;
        };
        if end == 0 {
            out.push(ParsedInline::Text(format!("{marker}{marker}")));
            rest = &rest[marker.len_utf8()..];
            continue;
        }
        out.push(wrap(vec![ParsedInline::Text(rest[..end].to_string())]));
        rest = &rest[end + marker.len_utf8()..];
    }
    if !rest.is_empty() {
        out.push(ParsedInline::Text(rest.to_string()));
    }
    out
}
