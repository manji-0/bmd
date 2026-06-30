//! View, scroll, and search state with typed transitions.

use super::link::LinkId;
use super::markdown::Document;

/// Terminal dimensions with the invariant that neither dimension is zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalSize {
    width: u16,
    height: u16,
}

impl TerminalSize {
    /// # Errors
    ///
    /// Returns `TerminalSizeError::ZeroDimension` if either dimension is zero.
    pub fn new(width: u16, height: u16) -> Result<Self, TerminalSizeError> {
        if width == 0 || height == 0 {
            return Err(TerminalSizeError::ZeroDimension);
        }
        Ok(Self { width, height })
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TerminalSizeError {
    #[error("terminal dimension cannot be zero")]
    ZeroDimension,
}

/// Scroll offset in logical lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scroll {
    offset: usize,
}

impl Default for Scroll {
    fn default() -> Self {
        Self::new()
    }
}

impl Scroll {
    pub const fn new() -> Self {
        Self { offset: 0 }
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }
}

/// Direction used when starting a search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchDirection {
    Forward,
    Backward,
}

/// A non-empty search query string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchQuery(String);

impl SearchQuery {
    /// # Errors
    ///
    /// Returns `SearchQueryError::Empty` if the value is empty.
    pub fn new(value: String) -> Result<Self, SearchQueryError> {
        if value.is_empty() {
            return Err(SearchQueryError::Empty);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SearchQueryError {
    #[error("search query cannot be empty")]
    Empty,
}

/// A search match expressed as a logical line offset in the rendered document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchMatch {
    pub line_offset: usize,
    pub match_index: usize,
}

impl SearchMatch {
    pub fn new(line_offset: usize, match_index: usize) -> Self {
        Self {
            line_offset,
            match_index,
        }
    }
}

/// Search state with typed transitions.
///
/// Invalid transitions are modelled out by the `ViewState` API: callers can only
/// mutate the query while in `Input`, and can only navigate matches while in
/// `Active`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchState {
    Inactive,
    Input {
        direction: SearchDirection,
        query: String,
    },
    Active {
        direction: SearchDirection,
        query: SearchQuery,
        matches: Vec<SearchMatch>,
        current_index: usize,
    },
}

impl SearchState {
    pub const fn inactive() -> Self {
        Self::Inactive
    }
}

/// View state with typed transitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewState {
    scroll: Scroll,
    selected_link: Option<LinkId>,
    terminal_size: TerminalSize,
    search_state: SearchState,
}

impl ViewState {
    pub fn new(terminal_size: TerminalSize) -> Self {
        Self {
            scroll: Scroll::new(),
            selected_link: None,
            terminal_size,
            search_state: SearchState::inactive(),
        }
    }

    /// Scroll down by `n` lines, clamped to `max_scroll`.
    pub fn scroll_down(self, n: usize, max_scroll: usize) -> Self {
        let offset = self.scroll.offset.saturating_add(n).min(max_scroll);
        Self {
            scroll: Scroll { offset },
            ..self
        }
    }

    /// Scroll up by `n` lines.
    pub fn scroll_up(self, n: usize) -> Self {
        let offset = self.scroll.offset.saturating_sub(n);
        Self {
            scroll: Scroll { offset },
            ..self
        }
    }

    pub fn half_page_down(self, max_scroll: usize) -> Self {
        let n = (self.terminal_size.height() / 2) as usize;
        self.scroll_down(n, max_scroll)
    }

    pub fn half_page_up(self) -> Self {
        let n = (self.terminal_size.height() / 2) as usize;
        self.scroll_up(n)
    }

    pub fn jump_to_top(self) -> Self {
        Self {
            scroll: Scroll { offset: 0 },
            ..self
        }
    }

    pub fn scroll_to(self, offset: usize) -> Self {
        Self {
            scroll: Scroll { offset },
            ..self
        }
    }

    pub fn jump_to_bottom(self, max_scroll: usize) -> Self {
        Self {
            scroll: Scroll { offset: max_scroll },
            ..self
        }
    }

    pub fn resize(self, terminal_size: TerminalSize) -> Self {
        Self {
            terminal_size,
            scroll: Scroll {
                offset: self.scroll.offset,
            },
            ..self
        }
    }

    /// Enter search input mode with the given direction.
    ///
    /// Any previously active search is discarded and the query input starts empty.
    pub fn start_search(self, direction: SearchDirection) -> Self {
        Self {
            search_state: SearchState::Input {
                direction,
                query: String::new(),
            },
            ..self
        }
    }

    /// Cancel search input or an active search and return to normal navigation.
    pub fn cancel_search(self) -> Self {
        Self {
            search_state: SearchState::Inactive,
            ..self
        }
    }

    /// Append a character to the query while in search input mode.
    ///
    /// If the view is not in search input mode, this is a no-op.
    pub fn append_search_input(self, c: char) -> Self {
        let search_state = match self.search_state {
            SearchState::Input { direction, query } => {
                let mut next = query;
                next.push(c);
                SearchState::Input {
                    direction,
                    query: next,
                }
            }
            other => other,
        };
        Self {
            search_state,
            ..self
        }
    }

    /// Remove the last character from the query while in search input mode.
    ///
    /// If the view is not in search input mode, this is a no-op.
    pub fn backspace_search_input(self) -> Self {
        let search_state = match self.search_state {
            SearchState::Input { direction, query } => {
                let mut next = query;
                next.pop();
                SearchState::Input {
                    direction,
                    query: next,
                }
            }
            other => other,
        };
        Self {
            search_state,
            ..self
        }
    }

    /// Confirm the current search query, build matches, and activate search.
    ///
    /// `matches` must be sorted by ascending `line_offset`. The first match that
    /// is at or after the current scroll offset is selected for forward searches;
    /// for backward searches the last match at or before the offset is selected.
    ///
    /// # Errors
    ///
    /// Returns `SearchQueryError::Empty` if the trimmed query is empty.
    pub fn confirm_search(
        self,
        query: String,
        direction: SearchDirection,
        matches: Vec<SearchMatch>,
    ) -> Result<Self, SearchQueryError> {
        let query = SearchQuery::new(query)?;
        let current_index = if matches.is_empty() {
            0
        } else {
            match direction {
                SearchDirection::Forward => matches
                    .iter()
                    .position(|m| m.line_offset >= self.scroll.offset)
                    .unwrap_or(0),
                SearchDirection::Backward => matches
                    .iter()
                    .rposition(|m| m.line_offset <= self.scroll.offset)
                    .unwrap_or(matches.len() - 1),
            }
        };
        let search_state = SearchState::Active {
            direction,
            query,
            matches,
            current_index,
        };
        Ok(Self {
            search_state,
            ..self
        })
    }

    /// Move to the next search match and scroll to it.
    ///
    /// If no search is active, this is a no-op.
    pub fn next_search_match(self, max_scroll: usize) -> Self {
        let (search_state, line_offset) = match self.search_state {
            SearchState::Active {
                direction,
                query,
                matches,
                current_index,
            } => {
                if matches.is_empty() {
                    (
                        SearchState::Active {
                            direction,
                            query,
                            matches,
                            current_index,
                        },
                        None,
                    )
                } else {
                    let next_index = (current_index + 1) % matches.len();
                    let line_offset = Some(matches[next_index].line_offset);
                    (
                        SearchState::Active {
                            direction,
                            query,
                            matches,
                            current_index: next_index,
                        },
                        line_offset,
                    )
                }
            }
            other => (other, None),
        };
        let scroll = match line_offset {
            Some(offset) => Scroll {
                offset: offset.min(max_scroll),
            },
            None => self.scroll,
        };
        Self {
            search_state,
            scroll,
            ..self
        }
    }

    /// Move to the previous search match and scroll to it.
    ///
    /// If no search is active, this is a no-op.
    pub fn prev_search_match(self, max_scroll: usize) -> Self {
        let (search_state, line_offset) = match self.search_state {
            SearchState::Active {
                direction,
                query,
                matches,
                current_index,
            } => {
                if matches.is_empty() {
                    (
                        SearchState::Active {
                            direction,
                            query,
                            matches,
                            current_index,
                        },
                        None,
                    )
                } else {
                    let prev_index = if current_index == 0 {
                        matches.len() - 1
                    } else {
                        current_index - 1
                    };
                    let line_offset = Some(matches[prev_index].line_offset);
                    (
                        SearchState::Active {
                            direction,
                            query,
                            matches,
                            current_index: prev_index,
                        },
                        line_offset,
                    )
                }
            }
            other => (other, None),
        };
        let scroll = match line_offset {
            Some(offset) => Scroll {
                offset: offset.min(max_scroll),
            },
            None => self.scroll,
        };
        Self {
            search_state,
            scroll,
            ..self
        }
    }

    pub fn search_state(&self) -> &SearchState {
        &self.search_state
    }

    pub fn is_search_active(&self) -> bool {
        matches!(self.search_state, SearchState::Active { .. })
    }

    pub fn select_next_link(self, document: &Document) -> Self {
        if document.links.is_empty() {
            return self;
        }
        let next = match self.selected_link {
            None => Some(LinkId(0)),
            Some(LinkId(i)) => Some(LinkId((i + 1) % document.links.len())),
        };
        Self {
            selected_link: next,
            ..self
        }
    }

    pub fn select_prev_link(self, document: &Document) -> Self {
        if document.links.is_empty() {
            return self;
        }
        let prev = match self.selected_link {
            None => Some(LinkId(document.links.len() - 1)),
            Some(LinkId(i)) => {
                if i == 0 {
                    Some(LinkId(document.links.len() - 1))
                } else {
                    Some(LinkId(i - 1))
                }
            }
        };
        Self {
            selected_link: prev,
            ..self
        }
    }

    pub fn clear_link_selection(self) -> Self {
        Self {
            selected_link: None,
            ..self
        }
    }

    pub fn scroll(&self) -> Scroll {
        self.scroll
    }

    pub fn selected_link(&self) -> Option<LinkId> {
        self.selected_link
    }

    pub fn terminal_size(&self) -> TerminalSize {
        self.terminal_size
    }
}
