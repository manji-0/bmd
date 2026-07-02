//! Block-level rendering.

use super::list_marker::{list_marker_label, list_marker_width_at};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Widget},
};
use syntect::{easy::HighlightLines, util::LinesWithEndings};
use unicode_width::UnicodeWidthStr;

use super::context::RenderContext;
use super::inline::{heading_styles, highlight_line, inlines_to_wrapped_lines, syntect_span};
use super::math::render_math_block;
use super::measure::measure_block_height;
use super::table::render_table;

use crate::domain::{Block, CodeBlock, DefinitionList, Heading, Inline, List, MathBlock};

pub(crate) fn render_block(
    block: &Block,
    _block_idx: usize,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    match block {
        Block::Heading(h) => render_heading(h, area, buf, skip_rows, ctx, line_offset),
        Block::Paragraph(inlines) => {
            render_paragraph(inlines, area, buf, skip_rows, ctx, line_offset)
        }
        Block::CodeBlock(cb) => render_code_block(cb, area, buf, skip_rows, ctx, line_offset),
        Block::MathBlock(math) => render_math_block_content(math, area, buf, skip_rows, ctx),
        Block::BlockQuote(blocks) => {
            render_blockquote(blocks, area, buf, skip_rows, ctx, line_offset)
        }
        Block::List(list) => render_list(list, area, buf, skip_rows, ctx, line_offset),
        Block::DefinitionList(list) => {
            render_definition_list(list, area, buf, skip_rows, ctx, line_offset)
        }
        Block::Table(table) => render_table(table, area, buf, skip_rows, ctx, line_offset),
        Block::Rule => render_rule(area, buf),
    }
}

fn render_heading(
    heading: &Heading,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    let (style, prefix_style) = heading_styles(heading.level, ctx.theme);
    let prefix = heading.level.prefix();
    let prefix_width = prefix.width();
    if area.width as usize > prefix_width + 1 {
        let content_width = (area.width as usize).saturating_sub(prefix_width).max(1);
        let rows =
            inlines_to_wrapped_lines(&heading.content, ctx, style, line_offset, content_width);
        render_prefixed_offset_lines(prefix, prefix_style, &rows, area, buf, skip_rows);
    } else {
        let rows = inlines_to_wrapped_lines(
            &heading.content,
            ctx,
            style,
            line_offset,
            area.width as usize,
        );
        render_offset_lines(&rows, area, buf, skip_rows);
    }
}

fn render_paragraph(
    inlines: &[Inline],
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    let rows = inlines_to_wrapped_lines(
        inlines,
        ctx,
        ctx.theme.text,
        line_offset,
        area.width as usize,
    );
    render_offset_lines(&rows, area, buf, skip_rows);
}

fn render_math_block_content(
    math: &MathBlock,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
) {
    render_math_block(&math.content, area, buf, skip_rows, ctx);
}

fn render_offset_lines(
    rows: &[(usize, Line<'static>)],
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
) {
    let max_y = area.y + area.height;
    for (row_idx, (_, line)) in rows.iter().enumerate() {
        if row_idx < skip_rows {
            continue;
        }
        let screen_y = area.y + (row_idx - skip_rows) as u16;
        if screen_y >= max_y {
            break;
        }
        buf.set_line(area.x, screen_y, line, area.width);
    }
}

/// Render a block-level prefix (e.g. heading marker "##") in front of wrapped
/// content rows, reducing the available width for the text.
fn render_prefixed_offset_lines(
    prefix: &str,
    prefix_style: Style,
    rows: &[(usize, Line<'static>)],
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
) {
    let prefix_width = prefix.width();
    let total_width = area.width as usize;
    if prefix_width >= total_width {
        render_offset_lines(rows, area, buf, skip_rows);
        return;
    }

    let text_width = area.width.saturating_sub(prefix_width as u16);
    let max_y = area.y + area.height;
    for (row_idx, (_, line)) in rows.iter().enumerate() {
        if row_idx < skip_rows {
            continue;
        }
        let screen_y = area.y + (row_idx - skip_rows) as u16;
        if screen_y >= max_y {
            break;
        }
        buf.set_stringn(area.x, screen_y, prefix, prefix_width, prefix_style);
        buf.set_line(area.x + prefix_width as u16, screen_y, line, text_width);
    }
}

/// Returns the text style and prefix style for a heading level.
pub(crate) fn render_code_block(
    cb: &CodeBlock,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    let mut lines: Vec<Line> = Vec::new();

    // Language label line.
    let label = cb
        .language
        .as_ref()
        .map(|l| format!(" {l} "))
        .unwrap_or_else(|| " code ".to_string());
    lines.push(Line::styled(label, ctx.theme.code_block_language));

    // Syntax highlighted content.
    let syntax = cb
        .language
        .as_ref()
        .and_then(|lang| ctx.syntax_set.find_syntax_by_token(lang));
    let mut highlighter = syntax
        .map(|s| HighlightLines::new(s, ctx.syntax_theme))
        .unwrap_or_else(|| {
            HighlightLines::new(ctx.syntax_set.find_syntax_plain_text(), ctx.syntax_theme)
        });

    for (i, line) in LinesWithEndings::from(&cb.content).enumerate() {
        let line_without_nl = line.strip_suffix('\n').unwrap_or(line);
        let highlighted = highlighter
            .highlight_line(line_without_nl, ctx.syntax_set)
            .unwrap_or_default();
        let spans: Vec<Span> = highlighted
            .into_iter()
            .map(|(style, text)| syntect_span(style, text, ctx.theme.code_block))
            .collect();
        let styled_line = Line::from(spans);
        lines.push(highlight_line(styled_line, ctx, line_offset + 1 + i));
    }

    // Scroll and render.  Do not add a synthetic trailing line; the inter-block gap
    // already provides visual separation.
    let text = Text::from(lines);
    let para = Paragraph::new(text)
        .style(ctx.theme.code_block)
        .scroll((skip_rows as u16, 0));
    para.render(area, buf);
}

fn render_blockquote(
    blocks: &[Block],
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    mut line_offset: usize,
) {
    if area.width < 3 {
        return;
    }
    let inner_area = Rect {
        x: area.x + 2,
        y: area.y,
        width: area.width - 2,
        height: area.height,
    };
    // Draw left border.
    for y in area.y..area.y + area.height {
        buf.set_stringn(area.x, y, "▌", area.width as usize, ctx.theme.blockquote);
    }

    let mut y = inner_area.y;
    let mut line_offset_inner: usize = 0;
    let scroll = skip_rows;
    let max_y = inner_area.y + inner_area.height;

    for block in blocks {
        let height = measure_block_height(block, usize::MAX, inner_area.width, ctx);
        if line_offset_inner.saturating_add(height) <= scroll {
            line_offset_inner += height;
            continue;
        }
        let height = measure_block_height(block, usize::MAX, inner_area.width, ctx);
        if line_offset.saturating_add(height) <= scroll {
            line_offset += height;
            continue;
        }
        let block_skip = scroll.saturating_sub(line_offset);
        let remaining = (max_y - y) as usize;
        let render_height = height.saturating_sub(block_skip).min(remaining);
        if render_height == 0 {
            break;
        }
        let block_area = Rect {
            x: inner_area.x,
            y,
            width: inner_area.width,
            height: render_height as u16,
        };
        render_block(
            block,
            usize::MAX, // block index not used for nested blocks
            block_area,
            buf,
            block_skip,
            ctx,
            line_offset + 1 + line_offset_inner,
        );
        y += render_height as u16;
        line_offset_inner += height;
        if y >= max_y {
            break;
        }
    }
}

fn render_definition_list(
    list: &DefinitionList,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    const INDENT: u16 = 2;
    let inner_width = (area.width as usize).saturating_sub(INDENT as usize).max(1) as u16;
    let mut y = area.y;
    let mut line_offset_inner = 0usize;
    let scroll = skip_rows;
    let max_y = area.y + area.height;
    let term_style = ctx.theme.text.add_modifier(Modifier::BOLD);

    for item in &list.items {
        if !item.term.is_empty() {
            let rows = inlines_to_wrapped_lines(
                &item.term,
                ctx,
                term_style,
                line_offset + line_offset_inner,
                area.width as usize,
            );
            let term_height = rows.len().max(1);
            if line_offset_inner + term_height > scroll {
                let term_skip = scroll.saturating_sub(line_offset_inner);
                let term_area = Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: area.height.saturating_sub(y.saturating_sub(area.y)),
                };
                render_offset_lines(&rows, term_area, buf, term_skip);
                let rendered = term_height
                    .saturating_sub(term_skip)
                    .min((max_y.saturating_sub(y)) as usize);
                y += rendered as u16;
            }
            line_offset_inner += term_height;
            if y >= max_y {
                return;
            }
        }

        for definition in &item.definitions {
            for block in definition {
                let height = measure_block_height(block, usize::MAX, inner_width, ctx);
                if line_offset_inner + height <= scroll {
                    line_offset_inner += height;
                    continue;
                }
                let block_skip = scroll.saturating_sub(line_offset_inner);
                let remaining = (max_y - y) as usize;
                let render_height = height.saturating_sub(block_skip).min(remaining);
                if render_height == 0 {
                    return;
                }
                let block_area = Rect {
                    x: area.x + INDENT,
                    y,
                    width: inner_width,
                    height: render_height as u16,
                };
                render_block(
                    block,
                    usize::MAX,
                    block_area,
                    buf,
                    block_skip,
                    ctx,
                    line_offset + line_offset_inner,
                );
                y += render_height as u16;
                line_offset_inner += height;
                if y >= max_y {
                    return;
                }
            }
        }
    }
}

fn render_list(
    list: &List,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    let mut y = area.y;
    let mut line_offset_inner: usize = 0;
    let scroll = skip_rows;
    let max_y = area.y + area.height;

    for (idx, item) in list.items.iter().enumerate() {
        let marker = list_marker_label(list, idx, item, ctx.checklist_state);
        let marker_width = list_marker_width_at(list, idx, item, ctx.checklist_state);
        let inner_width = (area.width as usize).saturating_sub(marker_width).max(1) as u16;

        if item.content.is_empty() {
            if line_offset_inner >= scroll && y < max_y {
                buf.set_stringn(area.x, y, &marker, marker_width, ctx.theme.list_marker);
                y += 1;
            }
            line_offset_inner += 1;
            if y >= max_y {
                break;
            }
            continue;
        }

        let mut item_y = y;
        let mut item_line_offset = line_offset_inner;
        let mut drew_marker = false;
        for block in &item.content {
            let height = measure_block_height(block, usize::MAX, inner_width, ctx);
            if item_line_offset.saturating_add(height) <= scroll {
                item_line_offset += height;
                continue;
            }
            let block_skip = scroll.saturating_sub(item_line_offset);
            let block_line_offset = line_offset + item_line_offset;
            let remaining = (max_y - item_y) as usize;
            let render_height = height.saturating_sub(block_skip).min(remaining);
            if render_height == 0 {
                break;
            }

            // Draw the marker on the first visible row of the first content block.
            if !drew_marker && item_line_offset + height > scroll {
                let marker_y = item_y + block_skip as u16;
                if marker_y < max_y {
                    buf.set_stringn(
                        area.x,
                        marker_y,
                        &marker,
                        marker_width,
                        ctx.theme.list_marker,
                    );
                }
                drew_marker = true;
            }

            let block_area = Rect {
                x: area.x + marker_width as u16,
                y: item_y,
                width: inner_width,
                height: render_height as u16,
            };
            render_block(
                block,
                usize::MAX,
                block_area,
                buf,
                block_skip,
                ctx,
                block_line_offset,
            );
            item_y += render_height as u16;
            item_line_offset += height;
            if item_y >= max_y {
                break;
            }
        }

        let item_height = item_line_offset - line_offset_inner;
        y = item_y;
        line_offset_inner += item_height;

        if y >= max_y {
            break;
        }
    }
}

fn render_rule(area: Rect, buf: &mut Buffer) {
    let line = "─".repeat(area.width as usize);
    let para = Paragraph::new(Text::from(vec![Line::styled(
        line,
        Style::default().fg(Color::DarkGray),
    )]));
    para.render(area, buf);
}
