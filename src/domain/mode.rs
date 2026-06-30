//! UI interaction mode and typed transitions.

use super::link::LinkId;
use super::view::{SearchDirection, SearchMatch, SearchQuery};

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
    /// Floating preview of an image or mermaid link.
    Preview { link_id: LinkId },
}

/// In-document search state while in [`UiMode::Normal`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NormalSearch {
    Inactive,
    Active {
        direction: SearchDirection,
        query: SearchQuery,
        matches: Vec<SearchMatch>,
        current_index: usize,
    },
}

impl NormalSearch {
    pub const fn inactive() -> Self {
        Self::Inactive
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active { .. })
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
            Self::Preview { link_id } => Some(*link_id),
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
