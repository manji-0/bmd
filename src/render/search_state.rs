//! Search-state helpers for render context.

use crate::domain::SearchState;

pub(crate) fn active_search_query(search_state: &SearchState) -> Option<String> {
    match search_state {
        SearchState::Active { query, .. } => Some(query.as_str().to_string()),
        _ => None,
    }
}

pub(crate) fn active_search_match_index(search_state: &SearchState) -> Option<usize> {
    match search_state {
        SearchState::Active {
            matches,
            current_index,
            ..
        } => matches.get(*current_index).map(|m| m.match_index),
        _ => None,
    }
}

pub(crate) fn active_search_match_line_offset(search_state: &SearchState) -> Option<usize> {
    match search_state {
        SearchState::Active {
            matches,
            current_index,
            ..
        } => matches.get(*current_index).map(|m| m.line_offset),
        _ => None,
    }
}
