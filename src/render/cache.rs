//! Document render cache.

use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::domain::{Document, LinkId, ViewState};

use super::context::RenderContext;
use super::measure::measure_document_height;
use super::subpixel::{SUBPIXEL_SNAP, compose_cells_vertical};
use super::widget::MarkdownWidget;

/// Key for invalidating a pre-rendered document buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RenderCacheKey {
    width: u16,
    search_query: Option<String>,
    selected_link: Option<LinkId>,
    selected_match_line_offset: Option<usize>,
    show_terminal_images: bool,
}

impl RenderCacheKey {
    fn from_context(ctx: &RenderContext<'_>, width: u16) -> Self {
        Self {
            width,
            search_query: ctx.search_query.clone(),
            selected_link: ctx.selected_link,
            selected_match_line_offset: ctx.selected_match_line_offset,
            show_terminal_images: ctx.show_terminal_images,
        }
    }
}

/// Full-document render cache. Rebuilds when width or highlight state changes;
/// scrolling only blits a viewport slice from the cached buffer.
pub struct DocumentRenderCache {
    key: Option<RenderCacheKey>,
    buffer: Buffer,
    total_height: usize,
}

impl Default for DocumentRenderCache {
    fn default() -> Self {
        Self {
            key: None,
            buffer: Buffer::empty(Rect::default()),
            total_height: 0,
        }
    }
}

impl DocumentRenderCache {
    /// Rebuild the cache when `ctx` or `width` no longer match the stored key.
    pub fn ensure(
        &mut self,
        document: &Document,
        ctx: &RenderContext<'_>,
        view_state: &ViewState,
        width: u16,
    ) {
        let key = RenderCacheKey::from_context(ctx, width);
        if self.key.as_ref() == Some(&key) && self.total_height > 0 {
            return;
        }
        self.rebuild(document, ctx, view_state, width, key);
    }

    fn rebuild(
        &mut self,
        document: &Document,
        ctx: &RenderContext<'_>,
        view_state: &ViewState,
        width: u16,
        key: RenderCacheKey,
    ) {
        let total_height = measure_document_height(document, width, ctx).max(1);
        let height = total_height.min(u16::MAX as usize) as u16;
        let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
        let top_view = view_state.clone().scroll_to(0);
        let widget = MarkdownWidget::new(document, ctx, &top_view);
        widget.render(Rect::new(0, 0, width, height), &mut buffer);
        self.key = Some(key);
        self.buffer = buffer;
        self.total_height = total_height;
    }

    /// Drop the cached buffer so the next `ensure` rebuilds it.
    pub(crate) fn invalidate(&mut self) {
        self.key = None;
    }

    pub fn total_height(&self) -> usize {
        self.total_height
    }

    /// Copy the visible viewport starting at fractional `scroll` into `buf`.
    pub fn blit(&self, scroll: f32, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let scroll = scroll.max(0.0);
        let cache_height = self.buffer.area().height as usize;
        let width = area.width as usize;

        for row in 0..area.height as usize {
            let src = scroll + row as f32;
            let src_i = src.floor() as usize;
            let frac = src - src_i as f32;
            if src_i >= cache_height {
                break;
            }

            for col in 0..width {
                let dest = (area.x + col as u16, area.y + row as u16);
                let top = self.buffer.cell((col as u16, src_i as u16));

                if frac < SUBPIXEL_SNAP {
                    if let Some(cell) = top {
                        buf[dest] = cell.clone();
                    }
                    continue;
                }

                if src_i + 1 >= cache_height {
                    if let Some(cell) = top {
                        buf[dest] = cell.clone();
                    }
                    continue;
                }

                let bottom = self.buffer.cell((col as u16, (src_i + 1) as u16));
                match (top, bottom) {
                    (Some(t), Some(b)) => buf[dest] = compose_cells_vertical(t, b, frac),
                    (Some(t), None) => buf[dest] = t.clone(),
                    (None, Some(b)) => buf[dest] = b.clone(),
                    (None, None) => {}
                }
            }
        }
    }
}

/// Widget that blits a pre-rendered [`DocumentRenderCache`] viewport.
pub struct CachedMarkdownView<'a> {
    pub cache: &'a DocumentRenderCache,
    pub scroll: f32,
}

impl Widget for CachedMarkdownView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.cache.blit(self.scroll, area, buf);
    }
}
