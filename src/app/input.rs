//! Keyboard input and command dispatch.

use std::time::Instant;

use crossterm::event::{Event, KeyEvent, KeyEventKind, MouseEventKind};

use crate::domain::SearchDirection;
use crate::error::AppError;
use crate::keymap::{Command, map_event};

use super::App;
use super::scroll::{
    LINE_SCROLL_LINES, SCROLL_REPEAT_DELAY, SCROLL_REPEAT_INTERVAL, is_line_scroll_key,
    is_single_press_key, line_scroll_command,
};

impl App {
    pub(crate) fn handle_crossterm_event(&mut self, event: Event) -> Result<bool, AppError> {
        if let Event::Mouse(mouse) = &event {
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
            if self.view_state.mode().is_normal() {
                if is_line_scroll_key(&key.code) {
                    return self.handle_line_scroll_key(key);
                }
                if key.kind == KeyEventKind::Press {
                    self.scroll_key_down_at = None;
                }
            }
            if key.kind == KeyEventKind::Release {
                return Ok(false);
            }
            if key.kind == KeyEventKind::Repeat
                && self.view_state.mode().is_normal()
                && is_single_press_key(&key.code)
                && !is_line_scroll_key(&key.code)
            {
                return Ok(false);
            }
        }

        let command = map_event(
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
                self.scroll_key_down_at = Some(now);
                self.handle_command(line_scroll_command(&key.code))?;
                Ok(true)
            }
            KeyEventKind::Repeat => {
                let Some(pressed_at) = self.scroll_key_down_at else {
                    self.scroll_key_down_at = Some(now);
                    self.handle_command(line_scroll_command(&key.code))?;
                    return Ok(true);
                };
                if now < pressed_at + SCROLL_REPEAT_DELAY {
                    return Ok(false);
                }
                if now < self.last_scroll_repeat + SCROLL_REPEAT_INTERVAL {
                    return Ok(false);
                }
                self.handle_command(line_scroll_command(&key.code))?;
                self.last_scroll_repeat = now;
                Ok(true)
            }
            KeyEventKind::Release => {
                self.scroll_key_down_at = None;
                Ok(false)
            }
        }
    }

    pub(crate) fn handle_command(&mut self, command: Command) -> Result<(), AppError> {
        if std::env::var("BMD_DEBUG").is_ok() {
            eprintln!("[bmd debug] command: {:?}", command);
        }

        if self.help_visible && self.view_state.mode().is_normal() {
            match command {
                Command::ToggleHelp | Command::SearchCancel => {
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
            Command::OpenLink => self.open_current_link(),
            Command::ClosePreview => self.close_preview(),
            Command::StartSearchForward => self.start_search(SearchDirection::Forward),
            Command::StartSearchBackward => self.start_search(SearchDirection::Backward),
            Command::SearchConfirm => self.confirm_search(),
            Command::SearchCancel => self.cancel_search(),
            Command::SearchInput(c) => self.append_search_input(c),
            Command::SearchBackspace => self.backspace_search_input(),
            Command::ToggleHelp => self.help_visible = true,
            Command::ToggleChecklist => self.toggle_checklist_at_viewport(),
            Command::Quit => self.should_quit = true,
        }
        Ok(())
    }

    pub(crate) fn is_quit(&self, command: &Command) -> bool {
        matches!(command, Command::Quit)
    }
}
