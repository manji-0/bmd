//! UI interaction mode and typed transitions.

use super::link::LinkId;
use super::markdown::FootnoteId;
use super::view::{SearchDirection, SearchMatch, SearchQuery};

/// What a floating preview is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewKind {
    Link(LinkId),
    Footnote(FootnoteId),
}

/// Top-level UI mode that selects key bindings and layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiMode {
    /// Normal navigation; may carry an active in-document search overlay.
    Normal,
    /// Typing a search query (`/` or `?`).
    SearchInput {
        direction: SearchDirection,
        query: String,
    },
    /// Floating preview of a link or footnote.
    Preview { kind: PreviewKind },
}

/// Validated in-document search overlay while in [`UiMode::Normal`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveSearch {
    direction: SearchDirection,
    query: SearchQuery,
    matches: Vec<SearchMatch>,
    current_index: usize,
}

impl ActiveSearch {
    /// Build an active search, clamping `current_index` into range.
    ///
    /// Empty `matches` always uses index `0`.
    pub fn new(
        direction: SearchDirection,
        query: SearchQuery,
        matches: Vec<SearchMatch>,
        current_index: usize,
    ) -> Self {
        let current_index = if matches.is_empty() {
            0
        } else {
            current_index.min(matches.len() - 1)
        };
        Self {
            direction,
            query,
            matches,
            current_index,
        }
    }

    pub fn direction(&self) -> SearchDirection {
        self.direction
    }

    pub fn query(&self) -> &SearchQuery {
        &self.query
    }

    pub fn matches(&self) -> &[SearchMatch] {
        &self.matches
    }

    pub fn current_index(&self) -> usize {
        self.current_index
    }

    pub(crate) fn with_index(self, current_index: usize) -> Self {
        Self::new(self.direction, self.query, self.matches, current_index)
    }
}

/// In-document search state while in [`UiMode::Normal`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NormalSearch {
    Inactive,
    Active(ActiveSearch),
}

impl NormalSearch {
    pub const fn inactive() -> Self {
        Self::Inactive
    }

    pub fn active(
        direction: SearchDirection,
        query: SearchQuery,
        matches: Vec<SearchMatch>,
        current_index: usize,
    ) -> Self {
        Self::Active(ActiveSearch::new(direction, query, matches, current_index))
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active(_))
    }
}

impl UiMode {
    pub const fn normal() -> Self {
        Self::Normal
    }

    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }

    pub fn is_search_input(&self) -> bool {
        matches!(self, Self::SearchInput { .. })
    }

    pub fn is_preview(&self) -> bool {
        matches!(self, Self::Preview { .. })
    }

    pub fn preview_link(&self) -> Option<LinkId> {
        match self {
            Self::Preview {
                kind: PreviewKind::Link(link_id),
            } => Some(*link_id),
            _ => None,
        }
    }

    pub fn preview_footnote(&self) -> Option<FootnoteId> {
        match self {
            Self::Preview {
                kind: PreviewKind::Footnote(footnote_id),
            } => Some(*footnote_id),
            _ => None,
        }
    }

    pub fn preview_kind(&self) -> Option<PreviewKind> {
        match self {
            Self::Preview { kind } => Some(*kind),
            _ => None,
        }
    }

    pub fn search_input_query(&self) -> Option<(&SearchDirection, &str)> {
        match self {
            Self::SearchInput { direction, query } => Some((direction, query.as_str())),
            _ => None,
        }
    }
}
