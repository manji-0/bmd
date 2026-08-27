//! Inline flattening and word-level link / footnote hit collection.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::domain::{FootnoteId, Inline, LinkId};

use super::{FootnoteHit, LinkHit};

struct WordHitCursor<'a> {
    width: usize,
    base_x: usize,
    line: &'a mut usize,
    x: &'a mut usize,
    target_line: Option<usize>,
    hits: &'a mut Vec<LinkHit>,
}

#[derive(Clone, Debug)]
enum FlatPiece {
    Word {
        text: String,
        link_id: Option<LinkId>,
        footnote_id: Option<FootnoteId>,
    },
    Space {
        link_id: Option<LinkId>,
    },
    Break,
}

pub(super) fn collect_inline_link_hits(
    inlines: &[Inline],
    width: usize,
    base_x: usize,
    start_line: usize,
    hits: &mut Vec<LinkHit>,
) {
    collect_inline_link_hits_filtered(inlines, width, base_x, start_line, None, hits);
}

pub(super) fn collect_inline_link_hits_filtered(
    inlines: &[Inline],
    width: usize,
    base_x: usize,
    start_line: usize,
    target_line: Option<usize>,
    hits: &mut Vec<LinkHit>,
) {
    if width == 0 {
        return;
    }
    let pieces = flatten_inline_pieces(inlines, None);
    let mut line = start_line;
    let mut x = 0usize;

    for piece in pieces {
        match piece {
            FlatPiece::Break => {
                line += 1;
                x = 0;
            }
            FlatPiece::Space { link_id } => {
                if x == 0 {
                    continue;
                }
                if x + 1 > width {
                    line += 1;
                    x = 0;
                    continue;
                }
                if let Some(id) = link_id
                    && target_line.is_none_or(|target| target == line)
                {
                    push_link_hit(hits, id, line, base_x + x, 1);
                }
                x += 1;
            }
            FlatPiece::Word {
                text,
                link_id,
                footnote_id: _,
            } => {
                append_word_hits_filtered(
                    &text,
                    link_id,
                    &mut WordHitCursor {
                        width,
                        base_x,
                        line: &mut line,
                        x: &mut x,
                        target_line,
                        hits,
                    },
                );
            }
        }
    }
}

fn append_word_hits_filtered(word: &str, link_id: Option<LinkId>, cursor: &mut WordHitCursor<'_>) {
    let word_width = word.width();
    if word_width <= cursor.width {
        append_fitting_word_filtered(word, link_id, cursor);
        return;
    }
    for grapheme in word.graphemes(true) {
        let grapheme_width = grapheme.width();
        if *cursor.x > 0 && *cursor.x + grapheme_width > cursor.width {
            *cursor.line += 1;
            *cursor.x = 0;
        }
        if let Some(id) = link_id
            && cursor
                .target_line
                .is_none_or(|target| target == *cursor.line)
        {
            push_link_hit(
                cursor.hits,
                id,
                *cursor.line,
                cursor.base_x + *cursor.x,
                grapheme_width,
            );
        }
        *cursor.x += grapheme_width;
    }
}

fn append_fitting_word_filtered(
    word: &str,
    link_id: Option<LinkId>,
    cursor: &mut WordHitCursor<'_>,
) {
    let word_width = word.width();
    let gap = usize::from(*cursor.x > 0);
    if *cursor.x > 0 && *cursor.x + gap + word_width > cursor.width {
        *cursor.line += 1;
        *cursor.x = 0;
    }
    if *cursor.x > 0 {
        *cursor.x += 1;
    }
    if let Some(id) = link_id
        && cursor
            .target_line
            .is_none_or(|target| target == *cursor.line)
    {
        push_link_hit(
            cursor.hits,
            id,
            *cursor.line,
            cursor.base_x + *cursor.x,
            word_width,
        );
    }
    *cursor.x += word_width;
}

fn push_link_hit(hits: &mut Vec<LinkHit>, id: LinkId, line: usize, x: usize, width: usize) {
    if width == 0 {
        return;
    }
    hits.push(LinkHit { id, line, x, width });
}

struct FootnoteWordHitCursor<'a> {
    width: usize,
    base_x: usize,
    line: &'a mut usize,
    x: &'a mut usize,
    target_line: Option<usize>,
    hits: &'a mut Vec<FootnoteHit>,
}

pub(super) fn collect_inline_footnote_hits(
    inlines: &[Inline],
    width: usize,
    base_x: usize,
    start_line: usize,
    hits: &mut Vec<FootnoteHit>,
) {
    collect_inline_footnote_hits_filtered(inlines, width, base_x, start_line, None, hits);
}

pub(super) fn collect_inline_footnote_hits_filtered(
    inlines: &[Inline],
    width: usize,
    base_x: usize,
    start_line: usize,
    target_line: Option<usize>,
    hits: &mut Vec<FootnoteHit>,
) {
    if width == 0 {
        return;
    }
    let pieces = flatten_inline_pieces(inlines, None);
    let mut line = start_line;
    let mut x = 0usize;

    for piece in pieces {
        match piece {
            FlatPiece::Break => {
                line += 1;
                x = 0;
            }
            FlatPiece::Space { .. } => {
                if x == 0 {
                    continue;
                }
                if x + 1 > width {
                    line += 1;
                    x = 0;
                } else {
                    x += 1;
                }
            }
            FlatPiece::Word {
                text,
                link_id: _,
                footnote_id,
            } => {
                append_footnote_word_hits_filtered(
                    &text,
                    footnote_id,
                    &mut FootnoteWordHitCursor {
                        width,
                        base_x,
                        line: &mut line,
                        x: &mut x,
                        target_line,
                        hits,
                    },
                );
            }
        }
    }
}

fn append_footnote_word_hits_filtered(
    word: &str,
    footnote_id: Option<FootnoteId>,
    cursor: &mut FootnoteWordHitCursor<'_>,
) {
    let word_width = word.width();
    if word_width <= cursor.width {
        append_fitting_footnote_word_filtered(word, footnote_id, cursor);
        return;
    }
    for grapheme in word.graphemes(true) {
        let grapheme_width = grapheme.width();
        if *cursor.x > 0 && *cursor.x + grapheme_width > cursor.width {
            *cursor.line += 1;
            *cursor.x = 0;
        }
        if let Some(id) = footnote_id
            && cursor
                .target_line
                .is_none_or(|target| target == *cursor.line)
        {
            push_footnote_hit(
                cursor.hits,
                id,
                *cursor.line,
                cursor.base_x + *cursor.x,
                grapheme_width,
            );
        }
        *cursor.x += grapheme_width;
    }
}

fn append_fitting_footnote_word_filtered(
    word: &str,
    footnote_id: Option<FootnoteId>,
    cursor: &mut FootnoteWordHitCursor<'_>,
) {
    let word_width = word.width();
    let gap = usize::from(*cursor.x > 0);
    if *cursor.x > 0 && *cursor.x + gap + word_width > cursor.width {
        *cursor.line += 1;
        *cursor.x = 0;
    }
    if *cursor.x > 0 {
        *cursor.x += 1;
    }
    if let Some(id) = footnote_id
        && cursor
            .target_line
            .is_none_or(|target| target == *cursor.line)
    {
        push_footnote_hit(
            cursor.hits,
            id,
            *cursor.line,
            cursor.base_x + *cursor.x,
            word_width,
        );
    }
    *cursor.x += word_width;
}

fn push_footnote_hit(
    hits: &mut Vec<FootnoteHit>,
    id: FootnoteId,
    line: usize,
    x: usize,
    width: usize,
) {
    if width == 0 {
        return;
    }
    hits.push(FootnoteHit { id, line, x, width });
}

fn flatten_inline_pieces(inlines: &[Inline], active_link: Option<LinkId>) -> Vec<FlatPiece> {
    let mut out = Vec::new();
    flatten_inline_pieces_inner(inlines, active_link, &mut out);
    out
}

fn flatten_inline_pieces_inner(
    inlines: &[Inline],
    active_link: Option<LinkId>,
    out: &mut Vec<FlatPiece>,
) {
    for inline in inlines {
        match inline {
            Inline::Text(text) => flatten_text_pieces(text, active_link, out),
            Inline::Code(code) => {
                let mut first = true;
                for word in code.split_whitespace() {
                    if !first {
                        out.push(FlatPiece::Space { link_id: None });
                    }
                    out.push(FlatPiece::Word {
                        text: word.to_string(),
                        link_id: None,
                        footnote_id: None,
                    });
                    first = false;
                }
            }
            Inline::Strong(children)
            | Inline::Emphasis(children)
            | Inline::Strikethrough(children)
            | Inline::Subscript(children)
            | Inline::Superscript(children) => {
                flatten_inline_pieces_inner(children, active_link, out);
            }
            Inline::Link(id, children) => {
                flatten_inline_pieces_inner(children, Some(*id), out);
            }
            Inline::FootnoteReference(id, display) => {
                out.push(FlatPiece::Word {
                    text: format!("[{display}]"),
                    link_id: None,
                    footnote_id: Some(*id),
                });
            }
            Inline::Math(latex) => flatten_text_pieces(latex, active_link, out),
            Inline::SoftBreak => out.push(FlatPiece::Space {
                link_id: active_link,
            }),
            Inline::HardBreak => out.push(FlatPiece::Break),
        }
    }
}

fn flatten_text_pieces(text: &str, link_id: Option<LinkId>, out: &mut Vec<FlatPiece>) {
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() {
            out.push(FlatPiece::Space { link_id });
            while chars.peek().is_some_and(|next| next.is_whitespace()) {
                chars.next();
            }
        } else {
            let mut word = String::from(ch);
            while let Some(&next) = chars.peek() {
                if next.is_whitespace() {
                    break;
                }
                word.push(next);
                chars.next();
            }
            out.push(FlatPiece::Word {
                text: word,
                link_id,
                footnote_id: None,
            });
        }
    }
}
