//! View, scroll, and search state with typed transitions.

use super::link::LinkId;
use super::mode::{NormalSearch, UiMode};

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

/// View state with typed transitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewState {
    scroll: Scroll,
    selected_link: Option<LinkId>,
    terminal_size: TerminalSize,
    mode: UiMode,
    normal_search: NormalSearch,
}

impl ViewState {
    pub fn new(terminal_size: TerminalSize) -> Self {
        Self {
            scroll: Scroll::new(),
            selected_link: None,
            terminal_size,
            mode: UiMode::normal(),
            normal_search: NormalSearch::inactive(),
        }
    }

    /// Reset interactive state after a document reload, preserving scroll position.
    pub fn reset_for_reload(self, scroll_offset: usize, max_scroll: usize) -> Self {
        Self {
            scroll: Scroll {
                offset: scroll_offset.min(max_scroll),
            },
            selected_link: None,
            terminal_size: self.terminal_size,
            mode: UiMode::normal(),
            normal_search: NormalSearch::inactive(),
        }
    }

    pub fn mode(&self) -> &UiMode {
        &self.mode
    }

    pub fn normal_search(&self) -> &NormalSearch {
        &self.normal_search
    }

    pub fn is_search_active(&self) -> bool {
        self.normal_search.is_active()
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
            mode: UiMode::SearchInput {
                direction,
                query: String::new(),
            },
            normal_search: NormalSearch::inactive(),
            ..self
        }
    }

    /// Cancel search input or clear an active search overlay.
    pub fn cancel_search(self) -> Self {
        match self.mode {
            UiMode::SearchInput { .. } => Self {
                mode: UiMode::Normal,
                normal_search: NormalSearch::inactive(),
                ..self
            },
            UiMode::Normal => Self {
                normal_search: NormalSearch::inactive(),
                ..self
            },
            UiMode::Preview { .. } => self,
        }
    }

    /// Append a character to the query while in search input mode.
    pub fn append_search_input(self, c: char) -> Self {
        let mode = match self.mode {
            UiMode::SearchInput { direction, query } => {
                let mut next = query;
                next.push(c);
                UiMode::SearchInput {
                    direction,
                    query: next,
                }
            }
            other => other,
        };
        Self { mode, ..self }
    }

    /// Remove the last character from the query while in search input mode.
    pub fn backspace_search_input(self) -> Self {
        let mode = match self.mode {
            UiMode::SearchInput { direction, query } => {
                let mut next = query;
                next.pop();
                UiMode::SearchInput {
                    direction,
                    query: next,
                }
            }
            other => other,
        };
        Self { mode, ..self }
    }

    /// Confirm the current search query, build matches, and return to normal mode.
    ///
    /// `matches` must be sorted by ascending `line_offset`. The first match that
    /// is at or after the current scroll offset is selected for forward searches;
    /// for backward searches the last match at or before the offset is selected.
    ///
    /// # Errors
    ///
    /// Returns `SearchQueryError::Empty` if the trimmed query is empty.
    pub fn confirm_search(self, matches: Vec<SearchMatch>) -> Result<Self, SearchQueryError> {
        let UiMode::SearchInput { direction, query } = self.mode else {
            return Ok(self);
        };
        let query = SearchQuery::new(query.trim().to_string())?;
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
        Ok(Self {
            mode: UiMode::Normal,
            normal_search: NormalSearch::Active {
                direction,
                query,
                matches,
                current_index,
            },
            ..self
        })
    }

    /// Open a floating preview for the given preview link.
    pub fn open_preview(self, link_id: LinkId) -> Self {
        Self {
            mode: UiMode::Preview { link_id },
            ..self
        }
    }

    /// Close the floating preview and resume normal navigation.
    pub fn close_preview(self) -> Self {
        match self.mode {
            UiMode::Preview { .. } => Self {
                mode: UiMode::Normal,
                ..self
            },
            other => Self {
                mode: other,
                ..self
            },
        }
    }

    /// Move to the next search match and scroll to it.
    pub fn next_search_match(self, max_scroll: usize) -> Self {
        let (normal_search, line_offset) = match self.normal_search {
            NormalSearch::Active {
                direction,
                query,
                matches,
                current_index,
            } => {
                if matches.is_empty() {
                    (
                        NormalSearch::Active {
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
                        NormalSearch::Active {
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
            normal_search,
            scroll,
            ..self
        }
    }

    /// Move to the previous search match and scroll to it.
    pub fn prev_search_match(self, max_scroll: usize) -> Self {
        let (normal_search, line_offset) = match self.normal_search {
            NormalSearch::Active {
                direction,
                query,
                matches,
                current_index,
            } => {
                if matches.is_empty() {
                    (
                        NormalSearch::Active {
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
                        NormalSearch::Active {
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
            normal_search,
            scroll,
            ..self
        }
    }

    /// Select the next link within `visible`, wrapping at the ends.
    pub fn select_next_link_in(self, visible: &[LinkId]) -> Self {
        if visible.is_empty() {
            return self;
        }
        let next = match self.selected_link {
            None => visible[0],
            Some(current) => visible
                .iter()
                .position(|&id| id == current)
                .map(|idx| visible[(idx + 1) % visible.len()])
                .unwrap_or(visible[0]),
        };
        Self {
            selected_link: Some(next),
            ..self
        }
    }

    /// Select the previous link within `visible`, wrapping at the ends.
    pub fn select_prev_link_in(self, visible: &[LinkId]) -> Self {
        if visible.is_empty() {
            return self;
        }
        let prev = match self.selected_link {
            None => *visible.last().expect("visible is non-empty"),
            Some(current) => visible
                .iter()
                .position(|&id| id == current)
                .map(|idx| {
                    if idx == 0 {
                        *visible.last().expect("visible is non-empty")
                    } else {
                        visible[idx - 1]
                    }
                })
                .unwrap_or(*visible.last().expect("visible is non-empty")),
        };
        Self {
            selected_link: Some(prev),
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
