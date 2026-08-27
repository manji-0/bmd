//! Keyboard input and command dispatch.

use std::time::Instant;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind};

use crate::domain::{MarkName, SearchDirection};
use crate::error::AppError;
use crate::keymap::Command;

use super::App;
use super::pending::PendingInput;
use super::preview::PREVIEW_ZOOM_STEP;
use super::scroll::{LINE_SCROLL_LINES, SCROLL_REPEAT_DELAY, SCROLL_REPEAT_INTERVAL};

impl App {
    pub(crate) fn handle_crossterm_event(&mut self, event: Event) -> Result<bool, AppError> {
        if let Event::Mouse(mouse) = &event {
            if self.view_state.mode().preview_link().is_some()
                && !self.help_visible
                && mouse.modifiers.contains(KeyModifiers::CONTROL)
            {
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        self.adjust_preview_zoom(1.0 / PREVIEW_ZOOM_STEP);
                        return Ok(true);
                    }
                    MouseEventKind::ScrollUp => {
                        self.adjust_preview_zoom(PREVIEW_ZOOM_STEP);
                        return Ok(true);
                    }
                    _ => {}
                }
            }

            if self.view_state.mode().is_normal() && !self.help_visible {
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        self.handle_command(Command::ScrollDown)?;
                        return Ok(true);
                    }
                    MouseEventKind::ScrollUp => {
                        self.handle_command(Command::ScrollUp)?;
                        return Ok(true);
                    }
                    _ => {}
                }
            }
            return self.handle_mouse_event(mouse.column, mouse.row, mouse.kind);
        }

        if let Event::Key(key) = &event {
            if key.kind == KeyEventKind::Release {
                return Ok(false);
            }
            if key.kind == KeyEventKind::Repeat
                && self.view_state.mode().is_normal()
                && self.keymap.is_single_press_key(key)
                && !self.keymap.is_line_scroll_key(key)
            {
                return Ok(false);
            }

            if (key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat)
                && let Some(handled) = self.handle_pending_key(key)?
            {
                return Ok(handled);
            }

            if (key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat)
                && let Some(handled) = self.handle_outline_key(key)
            {
                return Ok(handled);
            }

            if (key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat)
                && let Some(handled) = self.handle_mark_prefix_key(key)
            {
                return Ok(handled);
            }

            if self.view_state.mode().is_normal() {
                if self.keymap.is_line_scroll_key(key) {
                    return self.handle_line_scroll_key(key);
                }
                if key.kind == KeyEventKind::Press {
                    self.scroll.key_down_at = None;
                }
            }
        }

        if let Event::Key(key) = &event
            && (key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat)
            && let Some(handled) = self.handle_toc_preview_key(key)
        {
            return Ok(handled);
        }

        let command = self.keymap.map_event(
            event,
            self.view_state.mode(),
            self.view_state.normal_search(),
        );
        if self.is_quit(&command) {
            self.should_quit = true;
            return Ok(true);
        }
        self.handle_command(command)?;
        Ok(true)
    }

    /// Handle j/k/arrow line scrolling with rate-limited OS key repeat.
    ///
    /// We do not track key hold state because most terminals do not deliver
    /// `KeyEventKind::Release` without keyboard enhancement flags.
    pub(crate) fn handle_line_scroll_key(&mut self, key: &KeyEvent) -> Result<bool, AppError> {
        let now = Instant::now();
        match key.kind {
            KeyEventKind::Press => {
                self.scroll.key_down_at = Some(now);
                self.handle_command(self.keymap.line_scroll_command(key))?;
                Ok(true)
            }
            KeyEventKind::Repeat => {
                let Some(pressed_at) = self.scroll.key_down_at else {
                    self.scroll.key_down_at = Some(now);
                    self.handle_command(self.keymap.line_scroll_command(key))?;
                    return Ok(true);
                };
                if now < pressed_at + SCROLL_REPEAT_DELAY {
                    return Ok(false);
                }
                if now < self.scroll.last_repeat + SCROLL_REPEAT_INTERVAL {
                    return Ok(false);
                }
                self.handle_command(self.keymap.line_scroll_command(key))?;
                self.scroll.last_repeat = now;
                Ok(true)
            }
            KeyEventKind::Release => {
                self.scroll.key_down_at = None;
                Ok(false)
            }
        }
    }

    fn handle_mark_prefix_key(&mut self, key: &KeyEvent) -> Option<bool> {
        if !self.view_state.mode().is_normal() || self.help_visible {
            return None;
        }
        if self.pending_input.is_active() {
            return None;
        }
        if !key.modifiers.is_empty() {
            return None;
        }
        match key.code {
            KeyCode::Char('m') => {
                self.begin_set_mark();
                Some(true)
            }
            KeyCode::Char('\'') => {
                self.begin_jump_mark();
                Some(true)
            }
            _ => None,
        }
    }

    fn handle_pending_key(&mut self, key: &KeyEvent) -> Result<Option<bool>, AppError> {
        if !self.pending_input.is_active() {
            return Ok(None);
        }
        if !self.view_state.mode().is_normal() {
            self.pending_input = PendingInput::None;
            return Ok(None);
        }

        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            self.pending_input = PendingInput::None;
            self.status_message = None;
            self.status_message_until = None;
            return Ok(Some(true));
        }

        let KeyCode::Char(c) = key.code else {
            self.pending_input = PendingInput::None;
            return Ok(Some(true));
        };

        match self.pending_input {
            PendingInput::None => Ok(None),
            PendingInput::SetMark => {
                match MarkName::new(c) {
                    Ok(name) => self.set_mark(name),
                    Err(_) => {
                        self.pending_input = PendingInput::None;
                        self.set_status_message("mark name must be a-z".into());
                    }
                }
                Ok(Some(true))
            }
            PendingInput::JumpMark => {
                match MarkName::new(c) {
                    Ok(name) => self.jump_to_mark(name),
                    Err(_) => {
                        self.pending_input = PendingInput::None;
                        self.set_status_message("mark name must be a-z".into());
                    }
                }
                Ok(Some(true))
            }
            PendingInput::Yank => {
                self.pending_input = PendingInput::None;
                match c {
                    'l' => self.yank_link_url()?,
                    'h' => self.yank_heading_slug()?,
                    'c' => self.yank_code_block()?,
                    'y' => self.copy_text_selection()?,
                    _ => self
                        .set_status_message("yank: l link  h heading  c code  y selection".into()),
                }
                Ok(Some(true))
            }
        }
    }

    pub(crate) fn handle_command(&mut self, command: Command) -> Result<(), AppError> {
        if std::env::var("BMD_DEBUG").is_ok() {
            eprintln!("[bmd debug] command: {:?}", command);
        }

        if self.help_visible && self.view_state.mode().is_normal() {
            match command {
                Command::CloseHelp | Command::SearchCancel | Command::NavReset => {
                    self.help_visible = false;
                }
                Command::Quit => self.should_quit = true,
                _ => {}
            }
            return Ok(());
        }

        match command {
            Command::None => {}
            Command::ScrollDown => self.scroll_down(LINE_SCROLL_LINES),
            Command::ScrollUp => self.scroll_up(LINE_SCROLL_LINES),
            Command::HalfPageDown => self.half_page_down(),
            Command::HalfPageUp => self.half_page_up(),
            Command::JumpToTop => self.jump_to_top(),
            Command::JumpToBottom => self.jump_to_bottom(),
            Command::NextLink => {
                if self.view_state.is_search_active() {
                    self.next_search_match();
                } else {
                    self.next_link();
                }
            }
            Command::PrevLink => {
                if self.view_state.is_search_active() {
                    self.prev_search_match();
                } else {
                    self.prev_link();
                }
            }
            Command::NextHeading => self.next_heading(),
            Command::PrevHeading => self.prev_heading(),
            Command::OpenLink => self.open_current_link(),
            Command::ClosePreview => self.close_preview(),
            Command::PreviewZoomIn => self.adjust_preview_zoom(PREVIEW_ZOOM_STEP),
            Command::PreviewZoomOut => self.adjust_preview_zoom(1.0 / PREVIEW_ZOOM_STEP),
            Command::PreviewZoomReset => self.reset_preview_zoom(),
            Command::NavBack => self.nav_back(),
            Command::NavReset => {
                self.clear_text_selection();
                if self.outline.focused {
                    self.unfocus_outline();
                } else {
                    self.nav_reset();
                }
            }
            Command::StartSearchForward => {
                self.pending_input = PendingInput::None;
                self.start_search(SearchDirection::Forward);
            }
            Command::StartSearchBackward => {
                self.pending_input = PendingInput::None;
                self.start_search(SearchDirection::Backward);
            }
            Command::SearchConfirm => self.confirm_search(),
            Command::SearchCancel => self.cancel_search(),
            Command::SearchInput(c) => self.append_search_input(c),
            Command::SearchBackspace => self.backspace_search_input(),
            Command::ToggleHelp => self.help_visible = true,
            Command::CloseHelp => self.help_visible = false,
            Command::ToggleChecklist => self.toggle_checklist_at_viewport(),
            Command::ToggleOutline => self.toggle_outline(),
            Command::CopySelection => self.copy_text_selection()?,
            Command::YankPrefix => self.begin_yank()?,
            Command::ClearSelection => self.clear_text_selection(),
            Command::Quit => self.should_quit = true,
        }
        Ok(())
    }

    fn begin_yank(&mut self) -> Result<(), AppError> {
        if self.text_selection.as_ref().is_some_and(|s| !s.is_empty()) {
            return self.copy_text_selection();
        }
        self.pending_input = PendingInput::Yank;
        self.set_status_message("yank: l link  h heading  c code  y selection".into());
        Ok(())
    }

    fn handle_toc_preview_key(&mut self, key: &KeyEvent) -> Option<bool> {
        let link_id = self.view_state.mode().preview_link()?;
        let link = self.document.links.get(link_id.0)?;
        if link.kind != crate::domain::LinkKind::Toc {
            return None;
        }
        match key.code {
            KeyCode::Char('n') | KeyCode::Tab | KeyCode::Down => {
                self.toc_select_next();
                Some(true)
            }
            KeyCode::Char('N') | KeyCode::Char('p') | KeyCode::BackTab | KeyCode::Up => {
                self.toc_select_prev();
                Some(true)
            }
            KeyCode::Char('o') | KeyCode::Enter => {
                self.jump_to_toc_heading();
                Some(true)
            }
            _ => None,
        }
    }

    pub(crate) fn is_quit(&self, command: &Command) -> bool {
        matches!(command, Command::Quit)
    }
}
