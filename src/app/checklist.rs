//! Mouse interaction for task-list checkboxes and links.

use crossterm::event::{MouseButton, MouseEventKind};
use ratatui::layout::{Position, Rect};

use crate::error::AppError;
use crate::render::checklist::checklist_at_click;
use crate::render::{PREVIEW_POPUP_PERCENT, centered_rect, link_at_click};

use super::App;
use super::layout::split_main_and_prompt;

impl App {
    pub(crate) fn handle_mouse_event(
        &mut self,
        column: u16,
        row: u16,
        kind: MouseEventKind,
    ) -> Result<bool, AppError> {
        if !matches!(kind, MouseEventKind::Down(MouseButton::Left)) {
            return Ok(false);
        }

        let terminal = self.view_state.terminal_size();
        let full_area = Rect {
            x: 0,
            y: 0,
            width: terminal.width(),
            height: terminal.height(),
        };

        if self.view_state.mode().preview_link().is_some() {
            let popup = centered_rect(PREVIEW_POPUP_PERCENT, PREVIEW_POPUP_PERCENT, full_area);
            if !popup.contains(Position::new(column, row)) {
                self.close_preview();
                return Ok(true);
            }
            return Ok(false);
        }

        if !self.view_state.mode().is_normal() {
            return Ok(false);
        }

        let (main_area, _) = split_main_and_prompt(full_area, self.view_state.mode());

        if column < main_area.x
            || column >= main_area.x + main_area.width
            || row < main_area.y
            || row >= main_area.y + main_area.height
        {
            return Ok(false);
        }

        let local_col = (column - main_area.x) as usize;
        let local_row = (row - main_area.y) as usize;
        let logical_row = self.scroll_visual.floor() as usize + local_row;
        let ctx = self.render_context();
        let width = main_area.width;

        if let Some(item) = checklist_at_click(&self.document, width, &ctx, logical_row, local_col)
        {
            self.checklist_state.toggle(item);
            self.document_cache.invalidate();
            return Ok(true);
        }

        if let Some(link_id) = link_at_click(&self.document, width, &ctx, logical_row, local_col) {
            self.open_link_by_id(link_id);
            return Ok(true);
        }

        Ok(false)
    }

    pub(crate) fn toggle_checklist_at_viewport(&mut self) {
        if !self.view_state.mode().is_normal() {
            return;
        }
        if self.view_state.mode().preview_link().is_some() {
            return;
        }

        let logical_row = self.scroll_visual.floor() as usize;
        let ctx = self.render_context();
        let width = self.view_state.terminal_size().width();

        let Some(item) = checklist_at_click(&self.document, width, &ctx, logical_row, 0) else {
            return;
        };

        self.checklist_state.toggle(item);
        self.document_cache.invalidate();
    }
}
