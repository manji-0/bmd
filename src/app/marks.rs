//! Marks: set and jump (`ma` / `'a`).

use crate::domain::MarkName;

use super::App;
use super::pending::PendingInput;

impl App {
    pub(crate) fn begin_set_mark(&mut self) {
        self.pending_input = PendingInput::SetMark;
        self.set_status_message("m — mark a-z".into());
    }

    pub(crate) fn begin_jump_mark(&mut self) {
        self.pending_input = PendingInput::JumpMark;
        self.set_status_message("' — jump a-z".into());
    }

    pub(crate) fn set_mark(&mut self, name: MarkName) {
        let offset = self.view_state.scroll().offset();
        self.marks = self.marks.clone().set(name, offset);
        self.pending_input = PendingInput::None;
        self.set_status_message(format!("mark {} set", name.as_char()));
    }

    pub(crate) fn jump_to_mark(&mut self, name: MarkName) {
        self.pending_input = PendingInput::None;
        let Some(offset) = self.marks.get(name) else {
            self.set_status_message(format!("mark {} not set", name.as_char()));
            return;
        };
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().scroll_to(offset, max);
        self.snap_scroll_visual();
        self.set_status_message(format!("jumped to mark {}", name.as_char()));
    }

    pub(crate) fn clear_marks(&mut self) {
        self.marks = self.marks.clone().clear();
    }
}
