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

use super::callout::render_callout;
use super::context::RenderContext;
use super::inline::{heading_styles, highlight_line, inlines_to_wrapped_lines, syntect_span};
use super::math::render_math_block;
use super::measure::measure_block_height;
use super::table::render_table;

use crate::domain::{Block, CodeBlock, DefinitionList, Heading, List};

pub(crate) fn render_block(
    block: &Block,
    area: Rect,
    buf: &mut Buffer,
    ctx: &RenderContext,
    line_offset: usize,
) {
    match block {
        Block::Heading(h) => render_heading(h, area, buf, ctx, line_offset),
        Block::Paragraph(inlines) => {
            let rows = inlines_to_wrapped_lines(
                inlines,
                ctx,
                ctx.theme.text,
                line_offset,
                area.width as usize,
            );
            render_offset_lines(&rows, area, buf);
        }
        Block::CodeBlock(cb) => render_code_block(cb, area, buf, ctx, line_offset),
        Block::MathBlock(math) => render_math_block(&math.content, area, buf, ctx),
        Block::Quote(blocks) => render_blockquote(blocks, area, buf, ctx, line_offset),
        Block::Callout(callout) => render_callout(callout, area, buf, ctx, line_offset),
        Block::List(list) => render_list(list, area, buf, ctx, line_offset),
        Block::DefinitionList(list) => render_definition_list(list, area, buf, ctx, line_offset),
        Block::Table(table) => render_table(table, area, buf, ctx, line_offset),
        Block::Rule => render_rule(area, buf),
    }
}

/// Render `blocks` top to bottom with `gap` blank rows between them, each at its
/// measured height. Returns the number of rows consumed (clipped to `area`).
pub(crate) fn render_stacked(
    blocks: &[Block],
    area: Rect,
    buf: &mut Buffer,
    ctx: &RenderContext,
    line_offset: usize,
    gap: usize,
) -> usize {
    let mut row = 0usize;
    for (idx, block) in blocks.iter().enumerate() {
        if idx > 0 {
            row += gap;
        }
        let height = measure_block_height(block, area.width, ctx);
        let visible = height.min((area.height as usize).saturating_sub(row));
        if visible == 0 {
            break;
        }
        let block_area = Rect {
            y: area.y + row as u16,
            height: visible as u16,
            ..area
        };
        render_block(block, block_area, buf, ctx, line_offset + row);
        row += height;
    }
    row.min(area.height as usize)
}

fn render_heading(
    heading: &Heading,
    area: Rect,
    buf: &mut Buffer,
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
        render_prefixed_offset_lines(prefix, prefix_style, &rows, area, buf);
    } else {
        let rows = inlines_to_wrapped_lines(
            &heading.content,
            ctx,
            style,
            line_offset,
            area.width as usize,
        );
        render_offset_lines(&rows, area, buf);
    }
}

fn render_offset_lines(rows: &[(usize, Line<'static>)], area: Rect, buf: &mut Buffer) {
    for (row_idx, (_, line)) in rows.iter().take(area.height as usize).enumerate() {
        buf.set_line(area.x, area.y + row_idx as u16, line, area.width);
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
) {
    let prefix_width = prefix.width();
    if prefix_width >= area.width as usize {
        render_offset_lines(rows, area, buf);
        return;
    }
    let text_width = area.width.saturating_sub(prefix_width as u16);
    for (row_idx, (_, line)) in rows.iter().take(area.height as usize).enumerate() {
        let y = area.y + row_idx as u16;
        buf.set_stringn(area.x, y, prefix, prefix_width, prefix_style);
        buf.set_line(area.x + prefix_width as u16, y, line, text_width);
    }
}

pub(crate) fn render_code_block(
    cb: &CodeBlock,
    area: Rect,
    buf: &mut Buffer,
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

    // Do not add a synthetic trailing line; the inter-block gap already provides
    // visual separation.
    Paragraph::new(Text::from(lines))
        .style(ctx.theme.code_block)
        .render(area, buf);
}

fn render_blockquote(
    blocks: &[Block],
    area: Rect,
    buf: &mut Buffer,
    ctx: &RenderContext,
    line_offset: usize,
) {
    if area.width < 3 {
        return;
    }
    for y in area.y..area.y + area.height {
        buf.set_stringn(area.x, y, "▌", area.width as usize, ctx.theme.blockquote);
    }
    let inner = Rect {
        x: area.x + 2,
        width: area.width - 2,
        ..area
    };
    render_stacked(blocks, inner, buf, ctx, line_offset, 0);
}

fn render_definition_list(
    list: &DefinitionList,
    area: Rect,
    buf: &mut Buffer,
    ctx: &RenderContext,
    line_offset: usize,
) {
    const INDENT: u16 = 2;
    let term_style = ctx.theme.text.add_modifier(Modifier::BOLD);
    let mut row = 0usize;
    for item in &list.items {
        if !item.term.is_empty() {
            let rows = inlines_to_wrapped_lines(
                &item.term,
                ctx,
                term_style,
                line_offset + row,
                area.width as usize,
            );
            render_offset_lines(&rows, rows_below(area, row), buf);
            row += rows.len().max(1);
        }
        for definition in &item.definitions {
            let body = rows_below(area, row);
            let body = Rect {
                x: body.x + INDENT,
                width: body.width.saturating_sub(INDENT).max(1),
                ..body
            };
            row += render_stacked(definition, body, buf, ctx, line_offset + row, 0);
        }
    }
}

fn render_list(list: &List, area: Rect, buf: &mut Buffer, ctx: &RenderContext, line_offset: usize) {
    let mut row = 0usize;
    for (idx, item) in list.items.iter().enumerate() {
        let item_area = rows_below(area, row);
        if item_area.height == 0 {
            break;
        }
        let marker = list_marker_label(list, idx, item, ctx.checklist_state);
        let marker_width = list_marker_width_at(list, idx, item, ctx.checklist_state);
        buf.set_stringn(
            area.x,
            item_area.y,
            &marker,
            marker_width,
            ctx.theme.list_marker,
        );
        if item.content.is_empty() {
            row += 1;
            continue;
        }
        let body = Rect {
            x: item_area.x + marker_width as u16,
            width: (area.width as usize).saturating_sub(marker_width).max(1) as u16,
            ..item_area
        };
        row += render_stacked(&item.content, body, buf, ctx, line_offset + row, 0);
    }
}

/// The part of `area` starting `row` rows below its top.
pub(crate) fn rows_below(area: Rect, row: usize) -> Rect {
    let row = row.min(area.height as usize) as u16;
    Rect {
        y: area.y + row,
        height: area.height - row,
        ..area
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
