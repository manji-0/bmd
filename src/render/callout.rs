//! Boxed GFM alert callout rendering.

use ratatui::{buffer::Buffer, layout::Rect, style::Style};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::domain::Callout;

use super::blocks::render_block;
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
        .map(|block| measure_block_height(block, usize::MAX, inner_width, ctx))
        .sum();
    body_height + 2
}

pub(crate) fn render_callout(
    callout: &Callout,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    if area.width < 3 || area.height == 0 {
        return;
    }

    let styles = ctx.theme.callout_styles(callout.kind);
    let width = callout_frame_width(callout, area.width) as usize;
    let total_height = measure_callout_height(callout, area.width, ctx);
    if skip_rows >= total_height {
        return;
    }

    let inner_width = width.saturating_sub(2).max(1) as u16;
    let mut logical_row = 0usize;
    let mut y = area.y;

    if logical_row >= skip_rows && y < area.y + area.height {
        draw_top_border(buf, area.x, y, width, &callout.header_label(), styles);
        y += 1;
    }
    logical_row += 1;

    let mut callout_theme = ctx.theme.clone();
    callout_theme.text = styles.body;
    let callout_ctx = RenderContext {
        theme: &callout_theme,
        syntax_set: ctx.syntax_set,
        syntax_theme: ctx.syntax_theme,
        rendered: ctx.rendered,
        links: ctx.links,
        selected_link: ctx.selected_link,
        selected_footnote: ctx.selected_footnote,
        search_query: ctx.search_query.clone(),
        selected_search_match: ctx.selected_search_match,
        selected_match_line_offset: ctx.selected_match_line_offset,
        checklist_state: ctx.checklist_state,
        show_terminal_images: ctx.show_terminal_images,
    };

    let mut body_line_offset = line_offset + 1;
    for block in &callout.body {
        let block_height = measure_block_height(block, usize::MAX, inner_width, ctx);
        if logical_row + block_height <= skip_rows {
            logical_row += block_height;
            body_line_offset += block_height;
            continue;
        }
        let block_skip = skip_rows.saturating_sub(logical_row);
        let remaining = (area.y + area.height).saturating_sub(y) as usize;
        let render_height = block_height.saturating_sub(block_skip).min(remaining);
        if render_height == 0 {
            break;
        }
        for row in 0..render_height {
            let row_y = y + row as u16;
            if row_y < area.y + area.height {
                paint_body_row(buf, area.x, row_y, width, styles.border);
            }
        }
        let block_area = Rect {
            x: area.x + 1,
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
            &callout_ctx,
            body_line_offset,
        );
        y += render_height as u16;
        logical_row += block_height;
        body_line_offset += block_height;
        if y >= area.y + area.height {
            break;
        }
    }

    if logical_row >= skip_rows && y < area.y + area.height {
        draw_bottom_border(buf, area.x, y, width, styles.border);
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
