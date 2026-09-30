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

pub(crate) fn render_math_block(content: &str, area: Rect, buf: &mut Buffer, ctx: &RenderContext) {
    if area.width == 0 {
        return;
    }
    let rendered = render_latex(content).center_in(area.width as usize);
    for (row_idx, row) in rendered
        .cells()
        .iter()
        .take(area.height as usize)
        .enumerate()
    {
        let text = rendered_row_text(row);
        buf.set_string(area.x, area.y + row_idx as u16, &text, ctx.theme.math);
    }
}
