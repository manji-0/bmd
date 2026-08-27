//! Inline markdown nodes and width helpers.

use unicode_width::UnicodeWidthStr;

use super::super::link::LinkId;
use super::FootnoteId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Subscript(Vec<Inline>),
    Superscript(Vec<Inline>),
    Code(String),
    Link(LinkId, Vec<Inline>),
    FootnoteReference(FootnoteId, usize),
    Math(String),
    HardBreak,
    SoftBreak,
}

impl Inline {
    /// Width of the inline content in terminal columns.
    pub fn text_width(inlines: &[Inline]) -> usize {
        inlines
            .iter()
            .map(|i| match i {
                Inline::Text(t) | Inline::Code(t) | Inline::Math(t) => t.width(),
                Inline::Strong(c)
                | Inline::Emphasis(c)
                | Inline::Strikethrough(c)
                | Inline::Subscript(c)
                | Inline::Superscript(c)
                | Inline::Link(_, c) => Self::text_width(c),
                Inline::FootnoteReference(_, display) => footnote_marker_width(*display),
                Inline::HardBreak | Inline::SoftBreak => 1,
            })
            .sum()
    }

    /// Maximum width of any single whitespace-separated word in the inlines.
    pub fn min_word_width(inlines: &[Inline]) -> usize {
        inlines
            .iter()
            .map(|i| match i {
                Inline::Text(t) | Inline::Code(t) | Inline::Math(t) => {
                    t.split_whitespace().map(|w| w.width()).max().unwrap_or(0)
                }
                Inline::Strong(c)
                | Inline::Emphasis(c)
                | Inline::Strikethrough(c)
                | Inline::Subscript(c)
                | Inline::Superscript(c)
                | Inline::Link(_, c) => Self::min_word_width(c),
                Inline::FootnoteReference(_, display) => footnote_marker_width(*display),
                Inline::HardBreak | Inline::SoftBreak => 0,
            })
            .max()
            .unwrap_or(0)
    }

    /// Extract plain text from inline children, preserving a single space for breaks.
    pub(crate) fn plain_text(inlines: &[Inline]) -> String {
        let mut out = String::new();
        for (i, inline) in inlines.iter().enumerate() {
            match inline {
                Inline::Text(t) | Inline::Code(t) | Inline::Math(t) => out.push_str(t),
                Inline::Strong(c)
                | Inline::Emphasis(c)
                | Inline::Strikethrough(c)
                | Inline::Subscript(c)
                | Inline::Superscript(c)
                | Inline::Link(_, c) => {
                    out.push_str(&Self::plain_text(c));
                }
                Inline::FootnoteReference(_, _) => {}
                Inline::HardBreak | Inline::SoftBreak => {
                    if i > 0 {
                        out.push(' ');
                    }
                }
            }
        }
        out
    }
}

fn footnote_marker_width(display: usize) -> usize {
    format!("[{display}]").width()
}
