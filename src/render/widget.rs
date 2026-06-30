//! Markdown scroll widget.

use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::domain::{Document, ViewState};

use super::blocks::render_block;
use super::context::RenderContext;
use super::measure::measure_block_height;

/// Stateful widget that renders the document with scroll.
pub struct MarkdownWidget<'a> {
    document: &'a Document,
    ctx: &'a RenderContext<'a>,
    view_state: &'a ViewState,
}

impl<'a> MarkdownWidget<'a> {
    pub fn new(
        document: &'a Document,
        ctx: &'a RenderContext<'a>,
        view_state: &'a ViewState,
    ) -> Self {
        Self {
            document,
            ctx,
            view_state,
        }
    }
}

impl Widget for MarkdownWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let ctx = self.ctx;
        let scroll = self.view_state.scroll().offset();
        let max_y = area.y + area.height;

        let mut y = area.y;
        let mut line_offset: usize = 0;

        for (block_idx, block) in self.document.blocks.iter().enumerate() {
            let gap = if block_idx == 0 { 0 } else { 1 };
            let block_height = measure_block_height(block, area.width, ctx);
            let total_height = block_height + gap;

            // Fully above the visible region?
            if line_offset.saturating_add(total_height) <= scroll {
                line_offset += total_height;
                continue;
            }

            // How many rows of this block+gap are above the scroll offset?
            let rows_above = scroll.saturating_sub(line_offset);
            debug_assert!(rows_above < total_height);

            let visible_height = total_height
                .saturating_sub(rows_above)
                .min((max_y - y) as usize);
            if visible_height == 0 {
                break;
            }

            // Render only the content rows that are visible. Gap rows are blank and the
            // buffer is already cleared, so they require no drawing.
            if rows_above < block_height {
                let skip_rows = rows_above;
                let content_visible = visible_height.min(block_height.saturating_sub(skip_rows));
                let block_area = Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: content_visible as u16,
                };
                render_block(
                    block,
                    block_idx,
                    block_area,
                    buf,
                    skip_rows,
                    ctx,
                    line_offset,
                );
                y += content_visible as u16;
                // Any remaining visible rows are part of the inter-block gap.
                y += (visible_height.saturating_sub(content_visible)) as u16;
            } else {
                // Scroll offset is inside the gap; all visible rows are blank.
                y += visible_height as u16;
            }

            line_offset += total_height;
            if y >= max_y {
                break;
            }
        }
    }
}
