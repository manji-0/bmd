//! Mouse interaction for task-list checkboxes.

use crossterm::event::{MouseButton, MouseEventKind};

use crate::error::AppError;
use crate::render::checklist::checklist_at_click;

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
        if !self.view_state.mode().is_normal() {
            return Ok(false);
        }
        if self.view_state.mode().preview_link().is_some() {
            return Ok(false);
        }

        let (main_area, _) = split_main_and_prompt(
            ratatui::layout::Rect {
                x: 0,
                y: 0,
                width: self.view_state.terminal_size().width(),
                height: self.view_state.terminal_size().height(),
            },
            self.view_state.mode(),
        );

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

        let Some(item) = checklist_at_click(&self.document, width, &ctx, logical_row, local_col)
        else {
            return Ok(false);
        };

        self.checklist_state.toggle(item);
        self.document_cache.invalidate();
        Ok(true)
    }
}
