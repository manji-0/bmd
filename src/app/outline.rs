//! Sticky heading outline sidebar.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};

use super::App;
use super::layout::{OUTLINE_GAP, outline_panel_width};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct OutlineUi {
    pub visible: bool,
    pub selected: usize,
}

impl App {
    pub(crate) fn toggle_outline(&mut self) {
        if self.outline.visible {
            self.outline.visible = false;
            self.clamp_after_outline_change();
            return;
        }
        self.outline.visible = true;
        let index = self.heading_index_at_scroll(self.view_state.scroll().offset());
        if let Some(index) = index {
            self.outline.selected = index;
        }
        self.clamp_after_outline_change();
    }

    fn clamp_after_outline_change(&mut self) {
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().clamp_scroll(max);
        self.scroll.visual = self.view_state.scroll().offset() as f32;
        self.document_cache.invalidate();
        self.invalidate_prefetch_viewport();
    }

    pub(crate) fn sync_outline_selection_from_scroll(&mut self) {
        let Some(index) = self.heading_index_at_scroll(self.view_state.scroll().offset()) else {
            return;
        };
        self.outline.selected = index;
    }

    pub(crate) fn heading_index_at_scroll(&mut self, scroll: usize) -> Option<usize> {
        self.refresh_heading_catalog();
        let headings = self.heading_cache.entries();
        headings
            .iter()
            .rposition(|heading| heading.line_offset <= scroll)
            .or_else(|| headings.first().map(|_| 0))
    }

    pub(crate) fn jump_to_outline_heading(&mut self) {
        self.refresh_heading_catalog();
        let slug = self
            .heading_cache
            .entries()
            .get(self.outline.selected)
            .map(|entry| entry.slug.clone());
        let Some(slug) = slug else {
            return;
        };
        self.follow_anchor(&slug);
    }

    pub(crate) fn outline_hit_index(
        &mut self,
        column: u16,
        row: u16,
        outline_area: Rect,
    ) -> Option<usize> {
        if outline_area.width == 0 {
            return None;
        }
        if column < outline_area.x
            || column >= outline_area.x + outline_area.width
            || row < outline_area.y
            || row >= outline_area.y + outline_area.height
        {
            return None;
        }
        self.refresh_heading_catalog();
        let count = self.heading_cache.entries().len();
        if count == 0 {
            return None;
        }
        let inner_y = outline_area.y.saturating_add(1);
        let inner_height = outline_area.height.saturating_sub(2);
        if row < inner_y || row >= inner_y + inner_height {
            return None;
        }
        let local_row = (row - inner_y) as usize;
        let selected = self.outline.selected.min(count - 1);
        let scroll_y = if inner_height > 0 && selected >= inner_height as usize {
            selected - inner_height as usize + 1
        } else {
            0
        };
        let index = scroll_y + local_row;
        if index < count { Some(index) } else { None }
    }

    pub(crate) fn draw_outline_sidebar(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.sync_outline_selection_from_scroll();

        let current_scroll = self.view_state.scroll().offset();
        let current_idx = self.heading_index_at_scroll(current_scroll);
        let selected_raw = self.outline.selected;
        let normal_style = self.theme.text;
        let selected_style = Style::default().add_modifier(Modifier::BOLD);
        let prefix_style = Style::default().add_modifier(Modifier::DIM);
        let entries = self.heading_cache.entries();

        frame.render_widget(Clear, area);
        let block = Block::bordered().title("Outline");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if entries.is_empty() {
            frame.render_widget(Paragraph::new("(no headings)"), inner);
            return;
        }

        let selected = selected_raw.min(entries.len() - 1);

        let lines: Vec<Line> = entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let indent = "  ".repeat(entry.level.as_u8().saturating_sub(1) as usize);
                let prefix = entry.level.prefix();
                let is_selected = i == selected;
                let is_current = current_idx == Some(i);
                let style = if is_selected {
                    selected_style
                } else if is_current {
                    Style::default().add_modifier(Modifier::UNDERLINED)
                } else {
                    normal_style
                };
                let marker_style = if is_selected {
                    selected_style
                } else {
                    prefix_style
                };
                Line::from(vec![
                    Span::styled(format!("{indent}{prefix}"), marker_style),
                    Span::styled(entry.text.as_str(), style),
                ])
            })
            .collect();

        let visible_height = inner.height as usize;
        let scroll_y = if visible_height > 0 && selected >= visible_height {
            selected - visible_height + 1
        } else {
            0
        };
        let paragraph = Paragraph::new(lines).scroll((scroll_y as u16, 0));
        frame.render_widget(paragraph, inner);
    }
}

/// Width reserved for the outline column including the gap before main content.
pub(crate) fn outline_reserve_width(terminal_width: u16, outline_visible: bool) -> u16 {
    if !outline_visible {
        return 0;
    }
    let panel = outline_panel_width(terminal_width);
    if panel == 0 {
        0
    } else {
        panel.saturating_add(OUTLINE_GAP)
    }
}
