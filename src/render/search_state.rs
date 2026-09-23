//! Search-state helpers for render context.

use crate::domain::{NormalSearch, ViewState};

pub(crate) fn active_search_query(normal_search: &NormalSearch) -> Option<String> {
    match normal_search {
        NormalSearch::Active(active) => Some(active.query().as_str().to_string()),
        _ => None,
    }
}

/// Query used for document highlighting: live search input, else confirmed search.
pub(crate) fn render_search_query(view_state: &ViewState) -> Option<String> {
    if let Some((_, query)) = view_state.mode().search_input_query() {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return None;
        }
        return Some(trimmed.to_string());
    }
    active_search_query(view_state.normal_search())
}

pub(crate) fn active_search_match_index(normal_search: &NormalSearch) -> Option<usize> {
    match normal_search {
        NormalSearch::Active(active) => active
            .matches()
            .get(active.current_index())
            .map(|m| m.match_index),
        _ => None,
    }
}

pub(crate) fn active_search_match_line_offset(normal_search: &NormalSearch) -> Option<usize> {
    match normal_search {
        NormalSearch::Active(active) => active
            .matches()
            .get(active.current_index())
            .map(|m| m.line_offset),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{SearchDirection, SearchMatch, SearchQuery, TerminalSize, UiMode};

    fn active_search() -> NormalSearch {
        NormalSearch::active(
            SearchDirection::Forward,
            SearchQuery::new("needle".to_string()).unwrap(),
            vec![SearchMatch::new(4, 7), SearchMatch::new(9, 2)],
            1,
        )
    }

    #[test]
    fn inactive_search_returns_none() {
        let inactive = NormalSearch::inactive();
        assert_eq!(active_search_query(&inactive), None);
        assert_eq!(active_search_match_index(&inactive), None);
        assert_eq!(active_search_match_line_offset(&inactive), None);
    }

    #[test]
    fn active_search_exposes_query_and_current_match() {
        let search = active_search();
        assert_eq!(active_search_query(&search), Some("needle".to_string()));
        assert_eq!(active_search_match_index(&search), Some(2));
        assert_eq!(active_search_match_line_offset(&search), Some(9));
    }

    #[test]
    fn render_search_query_uses_live_search_input() {
        let size = TerminalSize::new(80, 24).unwrap();
        let state = ViewState::new(size).start_search(SearchDirection::Forward);
        assert_eq!(render_search_query(&state), None);

        let state = state.append_search_input('a').unwrap();
        assert_eq!(render_search_query(&state), Some("a".to_string()));

        let state = state.append_search_input(' ').unwrap();
        // Trailing space is trimmed for matching/highlighting.
        assert_eq!(render_search_query(&state), Some("a".to_string()));
    }

    #[test]
    fn render_search_query_ignores_whitespace_only_input() {
        let size = TerminalSize::new(80, 24).unwrap();
        let state = ViewState::new(size)
            .start_search(SearchDirection::Forward)
            .append_search_input(' ')
            .unwrap();
        assert_eq!(render_search_query(&state), None);
        assert!(matches!(
            state.mode(),
            UiMode::SearchInput { query, .. } if query == " "
        ));
    }

    #[test]
    fn render_search_query_falls_back_to_active_search() {
        let size = TerminalSize::new(80, 24).unwrap();
        let state = ViewState::new(size)
            .start_search(SearchDirection::Forward)
            .append_search_input('x')
            .unwrap()
            .confirm_search(vec![SearchMatch::new(0, 0)])
            .unwrap();
        assert_eq!(render_search_query(&state), Some("x".to_string()));
    }
}
