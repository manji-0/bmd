//! Marks: set and jump (`ma` / `'a`).

use crate::domain::MarkName;

use super::App;
use super::pending::PendingInput;

impl App {
    pub(crate) fn begin_set_mark(&mut self) {
        self.pending_input = PendingInput::SetMark;
    }

    pub(crate) fn begin_jump_mark(&mut self) {
        self.pending_input = PendingInput::JumpMark;
    }

    pub(crate) fn set_mark(&mut self, name: MarkName) {
        let anchor = self.scroll_anchor();
        self.marks = self.marks.clone().set(name, anchor);
        self.pending_input = PendingInput::None;
        self.set_status_message(format!("mark {} set", name.as_char()));
    }

    pub(crate) fn jump_to_mark(&mut self, name: MarkName) {
        self.pending_input = PendingInput::None;
        let Some(anchor) = self.marks.get(name) else {
            self.set_status_message(format!("mark {} not set", name.as_char()));
            return;
        };
        self.scroll_to_anchor(anchor);
        self.set_status_message(format!("jumped to mark {}", name.as_char()));
    }

    pub(crate) fn clear_marks(&mut self) {
        self.marks = self.marks.clone().clear();
    }
}
