//! Inline text conversion and search highlighting.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::domain::{HeadingLevel, Inline};

use super::context::RenderContext;
use super::math::render_latex;
use super::theme::Theme;

pub(crate) fn footnote_marker_style(ctx: &RenderContext) -> Style {
    ctx.theme.code_inline
}

pub(crate) fn heading_styles(level: HeadingLevel, theme: &Theme) -> (Style, Style) {
    match level {
        HeadingLevel::H1 => (theme.h1, theme.h1_prefix),
        HeadingLevel::H2 => (theme.h2, theme.h2_prefix),
        HeadingLevel::H3 => (theme.h3, theme.h3_prefix),
        HeadingLevel::H4 => (theme.h4, theme.h4_prefix),
        HeadingLevel::H5 => (theme.h5, theme.h5_prefix),
        HeadingLevel::H6 => (theme.h6, theme.h6_prefix),
    }
}
/// Highlight a pre-built `Line` if a search query is active.
pub(crate) fn highlight_line(
    line: Line<'static>,
    ctx: &RenderContext,
    line_offset: usize,
) -> Line<'static> {
    match ctx.search_query {
        None => line,
        Some(ref query) => Line::from(
            line.spans
                .into_iter()
                .flat_map(|span| {
                    highlight_span(
                        span,
                        &query.to_lowercase(),
                        ctx.theme.search_match,
                        ctx.theme.search_match_selected,
                        ctx.selected_match_line_offset == Some(line_offset),
                    )
                })
                .collect::<Vec<_>>(),
        ),
    }
}

pub(crate) fn syntect_span(
    style: syntect::highlighting::Style,
    text: &str,
    fallback: Style,
) -> Span<'static> {
    if text.is_empty() {
        return Span::styled(" ".to_string(), fallback);
    }
    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
    Span::styled(text.to_string(), Style::default().fg(fg))
}

/// Convert inline content to ratatui `Text`, respecting hard breaks and collapsing
/// consecutive whitespace (including SoftBreak) into single spaces.
#[cfg(test)]
pub(crate) fn inlines_to_text(
    inlines: &[Inline],
    ctx: &RenderContext,
    base_style: Style,
    line_offset: usize,
) -> Text<'static> {
    highlight_text(
        inlines_to_text_raw(inlines, ctx, base_style),
        ctx.search_query.as_deref(),
        ctx.theme.search_match,
        ctx.theme.search_match_selected,
        ctx.selected_match_line_offset,
        line_offset,
    )
}

/// Convert inline content to wrapped terminal rows with search highlighting.
pub(crate) fn inlines_to_wrapped_lines(
    inlines: &[Inline],
    ctx: &RenderContext,
    base_style: Style,
    start_line_offset: usize,
    width: usize,
) -> Vec<(usize, Line<'static>)> {
    let raw = inlines_to_text_raw(inlines, ctx, base_style);
    let mut out = Vec::new();
    let mut next_offset = start_line_offset;
    for line in raw.lines {
        let wrapped = wrap_styled_line(line, width, next_offset, ctx);
        if wrapped.is_empty() {
            out.push((next_offset, Line::from(" ")));
            next_offset += 1;
        } else {
            next_offset = wrapped
                .last()
                .map(|(offset, _)| offset + 1)
                .unwrap_or(next_offset);
            out.extend(wrapped);
        }
    }
    if out.is_empty() {
        out.push((start_line_offset, Line::from(" ")));
    }
    out
}

/// Word-wrap a styled line while preserving span styles and search highlights.
fn wrap_styled_line(
    line: Line<'static>,
    width: usize,
    line_offset: usize,
    ctx: &RenderContext,
) -> Vec<(usize, Line<'static>)> {
    if width == 0 {
        return vec![(line_offset, highlight_line(line, ctx, line_offset))];
    }

    let words = words_from_spans(&line.spans);
    if words.is_empty() {
        return vec![(line_offset, highlight_line(line, ctx, line_offset))];
    }

    let mut wrapped = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0usize;

    for (word, style) in words {
        if word.width() <= width {
            append_word_with_space(
                &mut current_spans,
                &mut current_width,
                &mut wrapped,
                &word,
                style,
                width,
            );
        } else {
            for grapheme in word.graphemes(true) {
                append_grapheme(
                    &mut current_spans,
                    &mut current_width,
                    &mut wrapped,
                    grapheme,
                    style,
                    width,
                );
            }
        }
    }

    if !current_spans.is_empty() {
        wrapped.push(Line::from(current_spans));
    }

    wrapped
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            let offset = line_offset + i;
            (offset, highlight_line(line, ctx, offset))
        })
        .collect()
}

fn words_from_spans(spans: &[Span<'_>]) -> Vec<(String, Style)> {
    spans
        .iter()
        .flat_map(|span| {
            span.content
                .split_whitespace()
                .map(|word| (word.to_string(), span.style))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn append_word_with_space(
    current_spans: &mut Vec<Span<'static>>,
    current_width: &mut usize,
    wrapped: &mut Vec<Line<'static>>,
    word: &str,
    style: Style,
    width: usize,
) {
    let word_width = word.width();
    let gap = usize::from(!current_spans.is_empty());
    if !current_spans.is_empty() && *current_width + gap + word_width > width {
        wrapped.push(Line::from(std::mem::take(current_spans)));
        *current_width = 0;
    }
    if !current_spans.is_empty() {
        let space_style = current_spans.last().map(|s| s.style).unwrap_or(style);
        append_span_text(current_spans, " ", space_style);
        *current_width += 1;
    }
    append_span_text(current_spans, word, style);
    *current_width += word_width;
}

fn append_grapheme(
    current_spans: &mut Vec<Span<'static>>,
    current_width: &mut usize,
    wrapped: &mut Vec<Line<'static>>,
    grapheme: &str,
    style: Style,
    width: usize,
) {
    let grapheme_width = grapheme.width();
    if !current_spans.is_empty() && *current_width + grapheme_width > width {
        wrapped.push(Line::from(std::mem::take(current_spans)));
        *current_width = 0;
    }
    append_span_text(current_spans, grapheme, style);
    *current_width += grapheme_width;
}

fn append_span_text(spans: &mut Vec<Span<'static>>, text: &str, style: Style) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut()
        && last.style == style
    {
        let mut merged = last.content.to_string();
        merged.push_str(text);
        last.content = merged.into();
        return;
    }
    spans.push(Span::styled(text.to_string(), style));
}

#[cfg(test)]
pub(crate) fn highlight_text(
    text: Text<'static>,
    query: Option<&str>,
    match_style: Style,
    selected_match_style: Style,
    selected_match_line_offset: Option<usize>,
    current_line_offset: usize,
) -> Text<'static> {
    let Some(query) = query else {
        return text;
    };
    if query.is_empty() {
        return text;
    }
    let query_lower = query.to_lowercase();
    let mut line_offset = current_line_offset;
    Text::from(
        text.lines
            .into_iter()
            .map(|line| {
                let is_selected_line = selected_match_line_offset == Some(line_offset);
                let highlighted = Line::from(
                    line.spans
                        .into_iter()
                        .flat_map(|span| {
                            highlight_span(
                                span,
                                &query_lower,
                                match_style,
                                selected_match_style,
                                is_selected_line,
                            )
                        })
                        .collect::<Vec<_>>(),
                );
                line_offset += 1;
                highlighted
            })
            .collect::<Vec<_>>(),
    )
}

pub(crate) fn highlight_span(
    span: Span<'static>,
    query_lower: &str,
    match_style: Style,
    selected_match_style: Style,
    is_selected_line: bool,
) -> Vec<Span<'static>> {
    let text = &span.content;
    let mut out = Vec::new();
    let mut last = 0usize;
    for (start, matched) in find_case_insensitive_matches(text, query_lower) {
        if last < start {
            out.push(Span::styled(text[last..start].to_string(), span.style));
        }
        out.push(Span::styled(
            text[start..]
                .chars()
                .take(matched.chars().count())
                .collect::<String>(),
            if is_selected_line {
                selected_match_style
            } else {
                match_style
            },
        ));
        last = start + matched.len();
    }
    if last < text.len() {
        out.push(Span::styled(text[last..].to_string(), span.style));
    }
    if out.is_empty() {
        out.push(span);
    }
    out
}

/// Find case-insensitive matches of `query` in `text` using Unicode-aware
/// grapheme iteration. Returns byte offsets and the matched substring from the
/// original `text` so that slicing is always safe even when `to_lowercase`
/// changes byte length.
fn find_case_insensitive_matches<'a>(text: &'a str, query_lower: &str) -> Vec<(usize, &'a str)> {
    if query_lower.is_empty() {
        return Vec::new();
    }
    let mut matches = Vec::new();
    let mut chars = text.char_indices().peekable();

    while let Some(&(start, _)) = chars.peek() {
        let query_chars = query_lower.chars();
        let mut text_iter = chars.clone();
        let mut matched = true;

        for query_char in query_chars {
            match text_iter.next() {
                Some((_, c)) if c.to_lowercase().to_string() == query_char.to_string() => {}
                _ => {
                    matched = false;
                    break;
                }
            }
        }

        if matched {
            let end = if let Some(&(last_idx, _)) = text_iter.peek() {
                last_idx
            } else {
                text.len()
            };
            matches.push((start, &text[start..end]));
            // Advance past the match to avoid overlapping highlights.
            for _ in 0..query_lower.chars().count() {
                chars.next();
            }
        } else {
            chars.next();
        }
    }

    matches
}

fn inlines_to_text_raw(
    inlines: &[Inline],
    ctx: &RenderContext,
    base_style: Style,
) -> Text<'static> {
    let mut segments = Vec::new();
    inlines_to_segments(inlines, ctx, base_style, &mut segments);
    let mut lines = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut pending_whitespace = false;

    for seg in segments {
        if seg.force_break_after {
            // Finish the current line, trimming trailing spaces.
            lines.push(Line::from(std::mem::take(&mut spans)));
            pending_whitespace = false;
            continue;
        }

        if seg.text.is_empty() {
            continue;
        }

        // Normalise whitespace within the segment: split on whitespace runs and join with a
        // single space. This keeps styled spans contiguous while preserving word boundaries.
        let words: Vec<&str> = seg.text.split_whitespace().collect();
        if words.is_empty() {
            pending_whitespace = true;
            continue;
        }

        if pending_whitespace && !spans.is_empty() {
            spans.push(Span::styled(" ".to_string(), seg.style));
        }

        // If the segment originally started with whitespace, prefix a single space before the
        // first word, but only if there is already preceding content.
        let starts_with_space = seg
            .text
            .chars()
            .next()
            .map(|c| c.is_whitespace())
            .unwrap_or(false);
        if starts_with_space && !spans.is_empty() && !pending_whitespace {
            spans.push(Span::styled(" ".to_string(), seg.style));
        }

        for (i, word) in words.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" ".to_string(), seg.style));
            }
            spans.push(Span::styled((*word).to_string(), seg.style));
        }

        pending_whitespace = seg
            .text
            .chars()
            .last()
            .map(|c| c.is_whitespace())
            .unwrap_or(false);
    }

    if pending_whitespace && !spans.is_empty() {
        spans.push(Span::styled(
            " ".to_string(),
            spans.last().map(|s| s.style).unwrap_or(base_style),
        ));
    }
    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }

    Text::from(lines)
}

#[derive(Debug)]
struct Segment {
    text: String,
    style: Style,
    force_break_after: bool,
}

fn inlines_to_segments(
    inlines: &[Inline],
    ctx: &RenderContext,
    base_style: Style,
    out: &mut Vec<Segment>,
) {
    for inline in inlines {
        match inline {
            Inline::Text(t) => out.push(Segment {
                text: t.clone(),
                style: base_style,
                force_break_after: false,
            }),
            Inline::Code(code) => out.push(Segment {
                text: code.clone(),
                style: ctx.theme.code_inline,
                force_break_after: false,
            }),
            Inline::Strong(children) => {
                inlines_to_segments(children, ctx, base_style.add_modifier(Modifier::BOLD), out);
            }
            Inline::Emphasis(children) => {
                inlines_to_segments(
                    children,
                    ctx,
                    base_style.add_modifier(Modifier::ITALIC),
                    out,
                );
            }
            Inline::Strikethrough(children) => {
                inlines_to_segments(
                    children,
                    ctx,
                    base_style.add_modifier(Modifier::CROSSED_OUT),
                    out,
                );
            }
            Inline::Subscript(children) => {
                inlines_to_segments(children, ctx, base_style.add_modifier(Modifier::DIM), out);
            }
            Inline::Superscript(children) => {
                inlines_to_segments(
                    children,
                    ctx,
                    base_style.add_modifier(Modifier::ITALIC),
                    out,
                );
            }
            Inline::Link(id, children) => {
                let style = match ctx.links.get(id.0) {
                    Some(link) if link.kind.is_preview() => {
                        if ctx.selected_link == Some(*id) {
                            ctx.theme.image_link_selected
                        } else {
                            ctx.theme.image_link
                        }
                    }
                    _ => {
                        if ctx.selected_link == Some(*id) {
                            ctx.theme.link_selected
                        } else {
                            ctx.theme.link
                        }
                    }
                };
                inlines_to_segments(children, ctx, style, out);
            }
            Inline::FootnoteReference(_, display) => out.push(Segment {
                text: format!("[{display}]"),
                style: footnote_marker_style(ctx),
                force_break_after: false,
            }),
            Inline::Math(latex) => {
                let rendered = render_latex(latex);
                if rendered.is_empty() {
                    out.push(Segment {
                        text: latex.clone(),
                        style: ctx.theme.math,
                        force_break_after: false,
                    });
                } else {
                    for (row_idx, row) in rendered.cells().iter().enumerate() {
                        out.push(Segment {
                            text: row.iter().map(|cell| cell.as_str()).collect(),
                            style: ctx.theme.math,
                            force_break_after: false,
                        });
                        if row_idx + 1 < rendered.height() {
                            out.push(Segment {
                                text: String::new(),
                                style: ctx.theme.math,
                                force_break_after: true,
                            });
                        }
                    }
                }
            }
            Inline::SoftBreak => out.push(Segment {
                text: " ".to_string(),
                style: base_style,
                force_break_after: false,
            }),
            Inline::HardBreak => {
                if let Some(last) = out.last_mut() {
                    last.force_break_after = true;
                }
            }
        }
    }
}
