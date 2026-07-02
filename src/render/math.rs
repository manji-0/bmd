//! LaTeX math rendering via term-maths.

use ratatui::{buffer::Buffer, layout::Rect};
use term_maths::{RenderedBlock, render};

use super::context::RenderContext;

pub(crate) fn render_latex(latex: &str) -> RenderedBlock {
    render(latex)
}

pub(crate) fn rendered_row_text(row: &[String]) -> String {
    row.iter().map(|cell| cell.as_str()).collect()
}

pub(crate) fn measure_math_height(content: &str, width: u16) -> usize {
    if width == 0 {
        return 1;
    }
    render_latex(content)
        .center_in(width as usize)
        .height()
        .max(1)
}

pub(crate) fn render_math_block(
    content: &str,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let rendered = render_latex(content).center_in(area.width as usize);
    let style = ctx.theme.math;
    for (row_idx, row) in rendered.cells().iter().enumerate() {
        if row_idx < skip_rows {
            continue;
        }
        let y = area.y + (row_idx - skip_rows) as u16;
        if y >= area.y + area.height {
            break;
        }
        let text = rendered_row_text(row);
        buf.set_string(area.x, y, &text, style);
    }
}
