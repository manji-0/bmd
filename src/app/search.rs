//! In-document search commands.

use crate::domain::{NormalSearch, SearchDirection, UiMode};
use crate::render::find_search_matches;

use super::App;

impl App {
    pub(crate) fn start_search(&mut self, direction: SearchDirection) {
        self.view_state = self.view_state.clone().start_search(direction);
        self.refresh_live_search_feedback();
    }

    pub(crate) fn cancel_search(&mut self) {
        self.view_state = self.view_state.clone().cancel_search();
        self.live_search_match_count = None;
    }

    pub(crate) fn append_search_input(&mut self, c: char) {
        if let Ok(state) = self.view_state.clone().append_search_input(c) {
            self.view_state = state;
            self.refresh_live_search_feedback();
        }
    }

    pub(crate) fn backspace_search_input(&mut self) {
        if let Ok(state) = self.view_state.clone().backspace_search_input() {
            self.view_state = state;
            self.refresh_live_search_feedback();
        }
    }

    pub(crate) fn confirm_search(&mut self) {
        let UiMode::SearchInput { query, .. } = self.view_state.mode().clone() else {
            return;
        };

        let trimmed = query.trim().to_string();
        if trimmed.is_empty() {
            self.view_state = self.view_state.clone().cancel_search();
            self.live_search_match_count = None;
            return;
        }

        let ctx = self.render_context();
        let matches = find_search_matches(&self.document, self.document_width(), &trimmed, &ctx);

        match self.view_state.clone().confirm_search(matches) {
            Ok(state) => {
                self.view_state = state;
                self.live_search_match_count = None;
                if let NormalSearch::Active(active) = self.view_state.normal_search() {
                    if let Some(m) = active.matches().get(active.current_index()) {
                        let max = self.max_scroll();
                        let target = m.line_offset.min(max);
                        self.view_state = self.view_state.clone().scroll_to(target, max);
                    } else {
                        self.set_status_message("no matches found".into());
                    }
                }
            }
            Err(e) => {
                self.set_status_message(e.to_string());
                self.view_state = self.view_state.clone().cancel_search();
                self.live_search_match_count = None;
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

    /// Recompute live match count while typing `/` or `?` (no scroll/jump).
    fn refresh_live_search_feedback(&mut self) {
        let Some((_, query)) = self.view_state.mode().search_input_query() else {
            self.live_search_match_count = None;
            return;
        };
        let trimmed = query.trim();
        if trimmed.is_empty() {
            self.live_search_match_count = None;
            return;
        }
        let ctx = self.render_context();
        let matches = find_search_matches(&self.document, self.document_width(), trimmed, &ctx);
        self.live_search_match_count = Some(matches.len());
    }
}

/// Format the bottom search prompt, optionally with a live match count.
pub(crate) fn format_search_prompt(
    direction: SearchDirection,
    query: &str,
    match_count: Option<usize>,
) -> String {
    let prefix = match direction {
        SearchDirection::Forward => "/",
        SearchDirection::Backward => "?",
    };
    match match_count {
        None => format!("{prefix}{query}"),
        Some(0) => format!("{prefix}{query}  0 matches"),
        Some(1) => format!("{prefix}{query}  1 match"),
        Some(n) => format!("{prefix}{query}  {n} matches"),
    }
}

#[cfg(test)]
mod prompt_tests {
    use super::*;

    #[test]
    fn format_search_prompt_without_count() {
        assert_eq!(
            format_search_prompt(SearchDirection::Forward, "foo", None),
            "/foo"
        );
        assert_eq!(
            format_search_prompt(SearchDirection::Backward, "bar", None),
            "?bar"
        );
    }

    #[test]
    fn format_search_prompt_with_counts() {
        assert_eq!(
            format_search_prompt(SearchDirection::Forward, "x", Some(0)),
            "/x  0 matches"
        );
        assert_eq!(
            format_search_prompt(SearchDirection::Forward, "x", Some(1)),
            "/x  1 match"
        );
        assert_eq!(
            format_search_prompt(SearchDirection::Backward, "x", Some(3)),
            "?x  3 matches"
        );
    }
}
