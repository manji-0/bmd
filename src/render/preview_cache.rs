//! Pre-rendered floating preview popup buffers.

use std::collections::HashMap;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Block, Clear, Widget},
};
use ratatui_image::protocol::Protocol;

use crate::domain::{LinkId, TerminalSize};

use super::image::{PREVIEW_POPUP_PERCENT, centered_rect, render_floating_image};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PreviewCacheKey {
    link_id: usize,
    width: u16,
    height: u16,
}

/// Cached bordered preview popups keyed by link and terminal size.
#[derive(Clone, Default)]
pub struct PreviewRenderCache {
    entries: HashMap<PreviewCacheKey, Buffer>,
}

impl PreviewRenderCache {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    #[cfg(test)]
    pub fn len_entries(&self) -> usize {
        self.entries.len()
    }

    pub fn popup_rect(terminal: TerminalSize) -> Rect {
        let full = Rect::new(0, 0, terminal.width(), terminal.height());
        centered_rect(PREVIEW_POPUP_PERCENT, PREVIEW_POPUP_PERCENT, full)
    }

    /// Build or return a cached popup buffer with border, title, and centered image.
    pub fn ensure(
        &mut self,
        link_id: LinkId,
        terminal: TerminalSize,
        title: &str,
        protocol: &Protocol,
    ) -> &Buffer {
        let key = PreviewCacheKey {
            link_id: link_id.0,
            width: terminal.width(),
            height: terminal.height(),
        };
        self.entries.entry(key).or_insert_with(|| {
            let layout = Self::popup_rect(terminal);
            let area = Rect::new(0, 0, layout.width, layout.height);
            let mut buffer = Buffer::empty(area);
            Clear.render(area, &mut buffer);
            let block = Block::bordered().title(title.to_string());
            let inner = block.inner(area);
            block.render(area, &mut buffer);
            render_floating_image(protocol, inner, &mut buffer);
            buffer
        })
    }

    /// Blit a cached preview into `dest_area` when available.
    pub fn blit(
        &self,
        link_id: LinkId,
        terminal: TerminalSize,
        dest_area: Rect,
        buf: &mut Buffer,
    ) -> bool {
        let key = PreviewCacheKey {
            link_id: link_id.0,
            width: terminal.width(),
            height: terminal.height(),
        };
        let Some(cached) = self.entries.get(&key) else {
            return false;
        };
        let screen_popup = centered_rect(PREVIEW_POPUP_PERCENT, PREVIEW_POPUP_PERCENT, dest_area);
        let width = screen_popup
            .width
            .min(cached.area().width)
            .min(dest_area.width);
        let height = screen_popup
            .height
            .min(cached.area().height)
            .min(dest_area.height);
        for y in 0..height {
            for x in 0..width {
                if let Some(cell) = cached.cell((x, y)) {
                    buf[(screen_popup.x + x, screen_popup.y + y)] = cell.clone();
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TerminalSize;
    use ratatui::layout::Size;
    use ratatui_image::picker::Picker;
    use ratatui_image::protocol::Protocol;

    fn dummy_protocol() -> Protocol {
        let img = image::DynamicImage::new_rgba8(4, 4);
        let picker = Picker::halfblocks();
        let target = Size::new(8, 4);
        picker
            .new_protocol(img, target, ratatui_image::Resize::Fit(None))
            .unwrap()
    }

    #[test]
    fn ensure_and_blit_round_trip() {
        let terminal = TerminalSize::new(80, 30).unwrap();
        let mut cache = PreviewRenderCache::default();
        let protocol = dummy_protocol();
        cache.ensure(LinkId(0), terminal, "title", &protocol);

        let mut screen = Buffer::empty(Rect::new(0, 0, 80, 30));
        assert!(cache.blit(LinkId(0), terminal, screen.area, &mut screen));
        assert!(!cache.blit(LinkId(1), terminal, screen.area, &mut screen));
    }

    #[test]
    fn clear_drops_entries() {
        let terminal = TerminalSize::new(80, 30).unwrap();
        let mut cache = PreviewRenderCache::default();
        cache.ensure(LinkId(0), terminal, "title", &dummy_protocol());
        cache.clear();
        let mut screen = Buffer::empty(Rect::new(0, 0, 80, 30));
        assert!(!cache.blit(LinkId(0), terminal, screen.area, &mut screen));
    }
}
