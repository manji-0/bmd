//! In-document search commands.

use crate::domain::{SearchDirection, SearchState};
use crate::render::find_search_matches;

use super::App;

impl App {
    pub(crate) fn start_search(&mut self, direction: SearchDirection) {
        self.view_state = self.view_state.clone().start_search(direction);
    }

    pub(crate) fn cancel_search(&mut self) {
        self.view_state = self.view_state.clone().cancel_search();
    }

    pub(crate) fn append_search_input(&mut self, c: char) {
        self.view_state = self.view_state.clone().append_search_input(c);
    }

    pub(crate) fn backspace_search_input(&mut self) {
        self.view_state = self.view_state.clone().backspace_search_input();
    }

    pub(crate) fn confirm_search(&mut self) {
        let (direction, query) = match self.view_state.search_state() {
            SearchState::Input { direction, query } => (*direction, query.clone()),
            _ => return,
        };

        let trimmed = query.trim().to_string();
        if trimmed.is_empty() {
            self.view_state = self.view_state.clone().cancel_search();
            return;
        }

        let ctx = self.render_context();
        let matches = find_search_matches(
            &self.document,
            self.view_state.terminal_size().width(),
            &trimmed,
            &ctx,
        );

        match self
            .view_state
            .clone()
            .confirm_search(trimmed, direction, matches)
        {
            Ok(state) => {
                self.view_state = state;
                // If matches were found, jump directly to the selected match line.
                if let SearchState::Active {
                    matches,
                    current_index,
                    ..
                } = self.view_state.search_state()
                {
                    if let Some(m) = matches.get(*current_index) {
                        let max = self.max_scroll();
                        let target = m.line_offset.min(max);
                        self.view_state = self.view_state.clone().scroll_to(target);
                    } else {
                        self.error_message = Some("no matches found".to_string());
                    }
                }
            }
            Err(e) => {
                self.error_message = Some(e.to_string());
                self.view_state = self.view_state.clone().cancel_search();
            }
        }
    }

    pub(crate) fn next_search_match(&mut self) {
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().next_search_match(max);
    }

    pub(crate) fn prev_search_match(&mut self) {
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().prev_search_match(max);
    }
}
