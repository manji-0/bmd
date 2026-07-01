//! Frame drawing.

use ratatui::{
    Terminal,
    backend::Backend,
    widgets::{Block, Clear, Paragraph},
};

use crate::domain::{SearchDirection, UiMode};
use crate::error::AppError;
use crate::render::{
    CachedMarkdownView, PREVIEW_POPUP_PERCENT, RenderContext, render_floating_image,
};

use super::App;
use super::layout::{centered_rect, split_layout};
use super::status::{draw_help_overlay, draw_status_bar, format_status_bar};

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
            let areas = split_layout(full_area, self.view_state.mode());

            let ctx = RenderContext::new(
                &self.theme,
                &self.syntax_assets,
                &self.rendered,
                &self.document.links,
                &self.view_state,
                self.show_terminal_images,
                &self.checklist_state,
            );
            let width = self.view_state.terminal_size().width();
            self.document_cache
                .ensure(&self.document, &ctx, &self.view_state, width);
            let widget = CachedMarkdownView {
                cache: &self.document_cache,
                scroll: self.scroll_visual,
            };
            f.render_widget(widget, areas.main);

            if let Some(link_id) = self.view_state.mode().preview_link() {
                self.draw_floating_preview(f, full_area, link_id);
            }

            if self.help_visible {
                draw_help_overlay(f, areas.main);
            }

            let status = format_status_bar(
                self.source_label.as_deref(),
                &self.view_state,
                self.max_scroll(),
                self.doc_stack.len_frames(),
                self.status_message.as_deref(),
            );
            draw_status_bar(f, areas.status, status);

            if let UiMode::SearchInput { direction, query } = self.view_state.mode() {
                let prefix = match direction {
                    SearchDirection::Forward => "/",
                    SearchDirection::Backward => "?",
                };
                let prompt = format!("{}{}", prefix, query);
                let para = Paragraph::new(prompt);
                f.render_widget(para, areas.prompt);
            }
        })?;
        Ok(())
    }

    fn draw_floating_preview(
        &self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        link_id: crate::domain::LinkId,
    ) {
        let Some(link) = self.document.links.get(link_id.0) else {
            return;
        };
        if !link.kind.is_preview() {
            return;
        }

        let popup = centered_rect(PREVIEW_POPUP_PERCENT, PREVIEW_POPUP_PERCENT, area);
        frame.render_widget(Clear, popup);
        let title = link
            .title
            .as_deref()
            .unwrap_or(link.url.as_str())
            .to_string();
        let block = Block::bordered().title(title);
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        if let Some(protocol) =
            self.rendered
                .preview_protocol(link_id.0, link.kind, link.url.as_str())
        {
            render_floating_image(protocol, inner, frame.buffer_mut());
        } else {
            let para = Paragraph::new(format!("[failed to load preview: {}]", link.url.as_str()));
            frame.render_widget(para, inner);
        }
    }
}
