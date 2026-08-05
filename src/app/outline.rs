//! Sticky heading outline sidebar.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};

use super::App;
use super::layout::{OUTLINE_GAP, outline_panel_width};

impl App {
    pub(crate) fn toggle_outline(&mut self) {
        if self.outline_visible {
            self.outline_visible = false;
            self.outline_focused = false;
            self.clamp_after_outline_change();
            return;
        }
        self.outline_visible = true;
        self.outline_focused = true;
        let index = self.heading_index_at_scroll(self.view_state.scroll().offset());
        if let Some(index) = index {
            self.outline_selected_index = index;
        }
        self.clamp_after_outline_change();
    }

    pub(crate) fn unfocus_outline(&mut self) {
        self.outline_focused = false;
    }

    fn clamp_after_outline_change(&mut self) {
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().clamp_scroll(max);
        self.scroll_visual = self.view_state.scroll().offset() as f32;
        self.document_cache.invalidate();
        self.invalidate_prefetch_viewport();
    }

    pub(crate) fn sync_outline_selection_from_scroll(&mut self) {
        if self.outline_focused {
            return;
        }
        let Some(index) = self.heading_index_at_scroll(self.view_state.scroll().offset()) else {
            return;
        };
        self.outline_selected_index = index;
    }

    pub(crate) fn heading_index_at_scroll(&mut self, scroll: usize) -> Option<usize> {
        let headings = self.heading_offsets();
        headings
            .iter()
            .rposition(|(offset, _)| *offset <= scroll)
            .or_else(|| headings.first().map(|_| 0))
    }

    pub(crate) fn outline_select_next(&mut self) {
        let count = self.collect_toc_entries().len();
        if count == 0 {
            return;
        }
        self.outline_selected_index = (self.outline_selected_index + 1) % count;
        self.outline_focused = true;
    }

    pub(crate) fn outline_select_prev(&mut self) {
        let count = self.collect_toc_entries().len();
        if count == 0 {
            return;
        }
        self.outline_selected_index = if self.outline_selected_index == 0 {
            count - 1
        } else {
            self.outline_selected_index - 1
        };
        self.outline_focused = true;
    }

    pub(crate) fn jump_to_outline_heading(&mut self) {
        let entries = self.collect_toc_entries();
        let Some((_, _, slug)) = entries.get(self.outline_selected_index) else {
            return;
        };
        let slug = slug.clone();
        self.follow_anchor(&slug);
        self.outline_focused = false;
    }

    /// Handle a key while the outline sidebar has focus.
    ///
    /// Returns `Some(handled)` when the key was consumed by the outline.
    pub(crate) fn handle_outline_key(&mut self, key: &crossterm::event::KeyEvent) -> Option<bool> {
        if !self.outline_visible || !self.outline_focused {
            return None;
        }
        if !self.view_state.mode().is_normal() || self.help_visible {
            return None;
        }
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Char('n') | KeyCode::Tab => {
                self.outline_select_next();
                Some(true)
            }
            KeyCode::Char('k')
            | KeyCode::Up
            | KeyCode::Char('N')
            | KeyCode::Char('p')
            | KeyCode::BackTab => {
                self.outline_select_prev();
                Some(true)
            }
            KeyCode::Char('o') | KeyCode::Enter => {
                self.jump_to_outline_heading();
                Some(true)
            }
            KeyCode::Char('t') => {
                self.toggle_outline();
                Some(true)
            }
            KeyCode::Esc => {
                self.unfocus_outline();
                Some(true)
            }
            _ => None,
        }
    }

    pub(crate) fn outline_hit_index(
        &self,
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
        let entries = self.collect_toc_entries();
        if entries.is_empty() {
            return None;
        }
        let inner_y = outline_area.y.saturating_add(1);
        let inner_height = outline_area.height.saturating_sub(2);
        if row < inner_y || row >= inner_y + inner_height {
            return None;
        }
        let local_row = (row - inner_y) as usize;
        let selected = self.outline_selected_index.min(entries.len() - 1);
        let scroll_y = if inner_height > 0 && selected >= inner_height as usize {
            selected - inner_height as usize + 1
        } else {
            0
        };
        let index = scroll_y + local_row;
        if index < entries.len() {
            Some(index)
        } else {
            None
        }
    }

    pub(crate) fn draw_outline_sidebar(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.sync_outline_selection_from_scroll();

        let current_scroll = self.view_state.scroll().offset();
        let current_idx = self.heading_index_at_scroll(current_scroll);
        let entries = self.collect_toc_entries();

        frame.render_widget(Clear, area);
        let title = if self.outline_focused {
            "Outline (focused)"
        } else {
            "Outline"
        };
        let block = Block::bordered().title(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if entries.is_empty() {
            frame.render_widget(Paragraph::new("(no headings)"), inner);
            return;
        }

        let selected = self.outline_selected_index.min(entries.len() - 1);
        let normal_style = self.theme.text;
        let selected_style = if self.outline_focused {
            self.theme.link_selected
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };
        let prefix_style = Style::default().add_modifier(Modifier::DIM);

        let lines: Vec<Line> = entries
            .iter()
            .enumerate()
            .map(|(i, (level, text, _slug))| {
                let indent = "  ".repeat(level.as_u8().saturating_sub(1) as usize);
                let prefix = level.prefix();
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
                    Span::styled(text.as_str(), style),
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
