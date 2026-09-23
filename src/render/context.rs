//! Render context.

use syntect::highlighting::Theme as SyntectTheme;
use syntect::parsing::SyntaxSet;

use crate::domain::{ChecklistState, FootnoteId, LinkId, ViewState};

use super::mermaid::RenderedDocument;
use super::search_state::{
    active_search_match_index, active_search_match_line_offset, render_search_query,
};
use super::syntax::SyntaxAssets;
use super::theme::Theme;

use crate::domain::Link;

/// Everything needed to render blocks.
pub struct RenderContext<'a> {
    pub theme: &'a Theme,
    pub syntax_set: &'a SyntaxSet,
    pub syntax_theme: &'a SyntectTheme,
    pub rendered: &'a RenderedDocument,
    pub links: &'a [Link],
    pub selected_link: Option<LinkId>,
    pub selected_footnote: Option<FootnoteId>,
    pub search_query: Option<String>,
    pub selected_search_match: Option<usize>,
    pub selected_match_line_offset: Option<usize>,
    pub checklist_state: &'a ChecklistState,
}

impl<'a> RenderContext<'a> {
    pub fn new(
        theme: &'a Theme,
        syntax_assets: &'a SyntaxAssets,
        rendered: &'a RenderedDocument,
        links: &'a [Link],
        view_state: &'a ViewState,
        checklist_state: &'a ChecklistState,
    ) -> Self {
        Self {
            theme,
            syntax_set: &syntax_assets.syntax_set,
            syntax_theme: syntax_assets.theme(),
            rendered,
            links,
            selected_link: view_state.selected_link(),
            selected_footnote: view_state.selected_footnote(),
            // Live `/`/`?` input highlights matches while typing; selected emphasis
            // only applies after Enter confirms into NormalSearch::Active.
            search_query: render_search_query(view_state),
            selected_search_match: if view_state.mode().is_search_input() {
                None
            } else {
                active_search_match_index(view_state.normal_search())
            },
            selected_match_line_offset: if view_state.mode().is_search_input() {
                None
            } else {
                active_search_match_line_offset(view_state.normal_search())
            },
            checklist_state,
        }
    }
}
