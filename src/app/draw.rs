//! Frame drawing.

use ratatui::{
    Terminal,
    backend::Backend,
    widgets::{Block, Clear, Paragraph},
};

use crate::domain::{SearchDirection, SearchState};
use crate::error::AppError;
use crate::render::{CachedMarkdownView, RenderContext};

use super::App;
use super::layout::{centered_rect, split_main_and_prompt};

impl App {
    pub(crate) fn draw_frame<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<(), AppError>
    where
        AppError: From<B::Error>,
    {
        terminal.draw(|f| {
            let full_area = f.area();
            let (main_area, prompt_area) = split_main_and_prompt(full_area, self.keymap_mode());

            let ctx = RenderContext::new(
                &self.theme,
                &self.syntax_assets.syntax_set,
                self.syntax_assets.theme(),
                &self.rendered,
                &self.view_state,
                self.show_terminal_images,
            );
            let width = self.view_state.terminal_size().width();
            self.document_cache
                .ensure(&self.document, &ctx, &self.view_state, width);
            let widget = CachedMarkdownView {
                cache: &self.document_cache,
                scroll: self.scroll_visual,
            };
            f.render_widget(widget, main_area);

            if let Some(ref msg) = self.error_message {
                let popup = centered_rect(60, 20, main_area);
                f.render_widget(Clear, popup);
                let block = Block::bordered().title("Error");
                let para = Paragraph::new(msg.clone()).block(block);
                f.render_widget(para, popup);
            }

            if let SearchState::Input { direction, query } = self.view_state.search_state() {
                let prefix = match direction {
                    SearchDirection::Forward => "/",
                    SearchDirection::Backward => "?",
                };
                let prompt = format!("{}{}", prefix, query);
                let para = Paragraph::new(prompt);
                f.render_widget(para, prompt_area);
            }
        })?;
        Ok(())
    }
}
