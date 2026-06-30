//! Render context.

use syntect::highlighting::Theme as SyntectTheme;
use syntect::parsing::SyntaxSet;

use crate::domain::{LinkId, ViewState};

use super::mermaid::RenderedDocument;
use super::search_state::{
    active_search_match_index, active_search_match_line_offset, active_search_query,
};
use super::theme::Theme;

/// Everything needed to render blocks.
pub struct RenderContext<'a> {
    pub theme: &'a Theme,
    pub syntax_set: &'a SyntaxSet,
    pub syntax_theme: &'a SyntectTheme,
    pub rendered: &'a RenderedDocument,
    pub selected_link: Option<LinkId>,
    pub search_query: Option<String>,
    pub selected_search_match: Option<usize>,
    pub selected_match_line_offset: Option<usize>,
}

impl<'a> RenderContext<'a> {
    pub fn new(
        theme: &'a Theme,
        syntax_set: &'a SyntaxSet,
        syntax_theme: &'a SyntectTheme,
        rendered: &'a RenderedDocument,
        view_state: &'a ViewState,
    ) -> Self {
        Self {
            theme,
            syntax_set,
            syntax_theme,
            rendered,
            selected_link: view_state.selected_link(),
            search_query: active_search_query(view_state.search_state()),
            selected_search_match: active_search_match_index(view_state.search_state()),
            selected_match_line_offset: active_search_match_line_offset(view_state.search_state()),
        }
    }
}
