//! Boxed GFM alert callout rendering.

use ratatui::{buffer::Buffer, layout::Rect, style::Style};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::domain::Callout;

use super::blocks::{render_stacked, rows_below};
use super::context::RenderContext;
use super::measure::measure_block_height;
use super::theme::CalloutStyles;

pub(crate) fn callout_inner_width(callout: &Callout, total_width: u16) -> u16 {
    callout.allocate_inner_width(total_width as usize).max(1) as u16
}

pub(crate) fn callout_frame_width(callout: &Callout, total_width: u16) -> u16 {
    callout.frame_width(total_width as usize) as u16
}

pub(crate) fn measure_callout_height(callout: &Callout, width: u16, ctx: &RenderContext) -> usize {
    if width < 3 {
        return 0;
    }
    let inner_width = callout_inner_width(callout, width);
    let body_height: usize = callout
        .body
        .iter()
        .map(|block| measure_block_height(block, inner_width, ctx))
        .sum();
    body_height + 2
}

pub(crate) fn render_callout(
    callout: &Callout,
    area: Rect,
    buf: &mut Buffer,
    ctx: &RenderContext,
    line_offset: usize,
) {
    if area.width < 3 || area.height == 0 {
        return;
    }
    let styles = ctx.theme.callout_styles(callout.kind);
    let width = callout_frame_width(callout, area.width) as usize;
    draw_top_border(buf, area.x, area.y, width, &callout.header_label(), styles);

    let mut callout_theme = ctx.theme.clone();
    callout_theme.text = styles.body;
    let callout_ctx = RenderContext {
        theme: &callout_theme,
        ..ctx.clone()
    };
    let body = rows_below(area, 1);
    let body_rows =
        (measure_callout_height(callout, area.width, ctx) - 2).min(body.height as usize);
    for row in 0..body_rows {
        paint_body_row(buf, area.x, body.y + row as u16, width, styles.border);
    }
    let body = Rect {
        x: area.x + 1,
        width: width.saturating_sub(2).max(1) as u16,
        ..body
    };
    render_stacked(&callout.body, body, buf, &callout_ctx, line_offset + 1, 0);
    if body_rows < body.height as usize {
        draw_bottom_border(buf, area.x, body.y + body_rows as u16, width, styles.border);
    }
}

fn draw_top_border(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: usize,
    title: &str,
    styles: CalloutStyles,
) {
    fill_row(buf, x, y, width, styles.border);
    let label = format!(" {title} ");
    let label_width = label.width();
    let mut col = x;
    set_char(buf, col, y, '╭', styles.border);
    col += 1;

    if width <= 2 {
        return;
    }

    if label_width + 2 >= width {
        for ch in label.chars().take(width.saturating_sub(2)) {
            set_char(buf, col, y, ch, styles.title);
            col += ch.width().unwrap_or(1) as u16;
        }
        if (x as usize + width).saturating_sub(1) > col as usize {
            set_char(buf, x + width as u16 - 1, y, '╮', styles.border);
        }
        return;
    }

    for ch in label.chars() {
        set_char(buf, col, y, ch, styles.title);
        col += ch.width().unwrap_or(1) as u16;
    }
    let end = x + width as u16 - 1;
    while col < end {
        set_char(buf, col, y, '─', styles.border);
        col += 1;
    }
    set_char(buf, end, y, '╮', styles.border);
}

fn draw_bottom_border(buf: &mut Buffer, x: u16, y: u16, width: usize, border: Style) {
    fill_row(buf, x, y, width, border);
    let mut col = x;
    set_char(buf, col, y, '╰', border);
    col += 1;
    let end = x + width as u16 - 1;
    while col < end {
        set_char(buf, col, y, '─', border);
        col += 1;
    }
    set_char(buf, end, y, '╯', border);
}

fn paint_body_row(buf: &mut Buffer, x: u16, y: u16, width: usize, border: Style) {
    fill_row(buf, x, y, width, border);
    set_char(buf, x, y, '│', border);
    if width > 1 {
        set_char(buf, x + width as u16 - 1, y, '│', border);
    }
}

fn fill_row(buf: &mut Buffer, x: u16, y: u16, width: usize, style: Style) {
    for col in x..x + width as u16 {
        buf[(col, y)].set_symbol(" ").set_style(style);
    }
}

fn set_char(buf: &mut Buffer, x: u16, y: u16, ch: char, style: Style) {
    let width = ch.width().unwrap_or(1);
    buf.set_stringn(x, y, ch.to_string(), width, style);
}
