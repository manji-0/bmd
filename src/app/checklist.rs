//! Mouse interaction for text selection, task-list checkboxes, and links.

use crossterm::event::{MouseButton, MouseEventKind};
use ratatui::layout::{Position, Rect};

use crate::clipboard::copy_to_clipboard;
use crate::domain::{TextPoint, TextSelection};
use crate::error::AppError;
use crate::render::checklist::checklist_at_click;
use crate::render::{PREVIEW_POPUP_PERCENT, centered_rect, extract_selected_text, link_at_click};

use super::App;
use super::layout::split_main_and_prompt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SelectionDrag {
    anchor: TextPoint,
    cursor: TextPoint,
    dragged: bool,
}

impl App {
    pub(crate) fn handle_mouse_event(
        &mut self,
        column: u16,
        row: u16,
        kind: MouseEventKind,
    ) -> Result<bool, AppError> {
        match kind {
            MouseEventKind::Down(MouseButton::Left) => self.handle_mouse_down(column, row),
            MouseEventKind::Drag(MouseButton::Left) => self.handle_mouse_drag(column, row),
            MouseEventKind::Up(MouseButton::Left) => self.handle_mouse_up(column, row),
            _ => Ok(false),
        }
    }

    fn handle_mouse_down(&mut self, column: u16, row: u16) -> Result<bool, AppError> {
        let Some(point) = self.main_area_text_point(column, row) else {
            self.selection_drag = None;
            return Ok(false);
        };
        self.selection_drag = Some(SelectionDrag {
            anchor: point,
            cursor: point,
            dragged: false,
        });
        self.text_selection = Some(TextSelection::new(point, point));
        Ok(true)
    }

    fn handle_mouse_drag(&mut self, column: u16, row: u16) -> Result<bool, AppError> {
        let Some(point) = self.main_area_text_point(column, row) else {
            return Ok(false);
        };
        let Some(drag) = self.selection_drag.as_mut() else {
            return Ok(false);
        };
        drag.cursor = point;
        drag.dragged = true;
        self.text_selection = Some(TextSelection::new(drag.anchor, drag.cursor));
        Ok(true)
    }

    fn handle_mouse_up(&mut self, column: u16, row: u16) -> Result<bool, AppError> {
        let Some(drag) = self.selection_drag.take() else {
            return Ok(false);
        };

        if drag.dragged {
            if let Some(point) = self.main_area_text_point(column, row) {
                self.text_selection = Some(TextSelection::new(drag.anchor, point));
            } else {
                self.text_selection = Some(TextSelection::new(drag.anchor, drag.cursor));
            }
            self.copy_text_selection()?;
            return Ok(true);
        }

        self.text_selection = None;
        self.handle_mouse_click(column, row)
    }

    fn handle_mouse_click(&mut self, column: u16, row: u16) -> Result<bool, AppError> {
        let terminal = self.view_state.terminal_size();
        let full_area = Rect {
            x: 0,
            y: 0,
            width: terminal.width(),
            height: terminal.height(),
        };

        if self.view_state.mode().is_preview() {
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

    fn main_area_text_point(&self, column: u16, row: u16) -> Option<TextPoint> {
        if self.help_visible || !self.view_state.mode().is_normal() {
            return None;
        }
        if self.view_state.mode().is_preview() {
            return None;
        }

        let terminal = self.view_state.terminal_size();
        let full_area = Rect {
            x: 0,
            y: 0,
            width: terminal.width(),
            height: terminal.height(),
        };
        let (main_area, _) = split_main_and_prompt(full_area, self.view_state.mode());
        if column < main_area.x
            || column >= main_area.x + main_area.width
            || row < main_area.y
            || row >= main_area.y + main_area.height
        {
            return None;
        }

        let local_col = (column - main_area.x) as usize;
        let local_row = (row - main_area.y) as usize;
        let logical_row = self.scroll_visual.floor() as usize + local_row;
        Some(TextPoint::new(logical_row, local_col))
    }

    pub(crate) fn copy_text_selection(&mut self) -> Result<(), AppError> {
        let Some(selection) = self.text_selection.filter(|s| !s.is_empty()) else {
            self.set_status_message("no text selected".into());
            return Ok(());
        };
        let text = extract_selected_text(&self.document_cache, selection);
        if text.is_empty() {
            self.set_status_message("no text selected".into());
            return Ok(());
        }
        copy_to_clipboard(&text)?;
        self.set_status_message(format!("copied {} characters", text.chars().count()));
        Ok(())
    }

    pub(crate) fn clear_text_selection(&mut self) {
        self.text_selection = None;
        self.selection_drag = None;
    }

    pub(crate) fn toggle_checklist_at_viewport(&mut self) {
        if !self.view_state.mode().is_normal() {
            return;
        }
        if self.view_state.mode().is_preview() {
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
