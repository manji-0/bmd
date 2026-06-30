//! Search-state helpers for render context.

use crate::domain::NormalSearch;

pub(crate) fn active_search_query(normal_search: &NormalSearch) -> Option<String> {
    match normal_search {
        NormalSearch::Active { query, .. } => Some(query.as_str().to_string()),
        _ => None,
    }
}

pub(crate) fn active_search_match_index(normal_search: &NormalSearch) -> Option<usize> {
    match normal_search {
        NormalSearch::Active {
            matches,
            current_index,
            ..
        } => matches.get(*current_index).map(|m| m.match_index),
        _ => None,
    }
}

pub(crate) fn active_search_match_line_offset(normal_search: &NormalSearch) -> Option<usize> {
    match normal_search {
        NormalSearch::Active {
            matches,
            current_index,
            ..
        } => matches.get(*current_index).map(|m| m.line_offset),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{SearchDirection, SearchMatch, SearchQuery};

    fn active_search() -> NormalSearch {
        NormalSearch::Active {
            direction: SearchDirection::Forward,
            query: SearchQuery::new("needle".to_string()).unwrap(),
            matches: vec![SearchMatch::new(4, 7), SearchMatch::new(9, 2)],
            current_index: 1,
        }
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
}
