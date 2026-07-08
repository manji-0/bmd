//! Frame drawing.

use ratatui::{
    Terminal,
    backend::Backend,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};

use crate::domain::{PreviewKind, PreviewLoadStatus, SearchDirection, UiMode};
use crate::error::AppError;
use crate::render::{
    CachedMarkdownView, RenderContext, footnote_preview_title, paint_selection_overlay,
    render_footnote_preview,
};

use super::App;
use super::layout::split_layout;
use super::preview::preview_failed_message;
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

            if let Some(selection) = self.text_selection.filter(|s| !s.is_empty()) {
                paint_selection_overlay(
                    f.buffer_mut(),
                    areas.main,
                    self.scroll_visual,
                    selection,
                    self.theme.text_selection,
                );
            }

            if let Some(kind) = self.view_state.mode().preview_kind() {
                match kind {
                    PreviewKind::Link(link_id) => {
                        self.draw_floating_preview(f, full_area, link_id);
                    }
                    PreviewKind::Footnote(footnote_id) => {
                        self.draw_footnote_preview(f, full_area, footnote_id, &ctx);
                    }
                }
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
        &mut self,
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

        if link.kind == crate::domain::LinkKind::Toc {
            self.draw_toc_preview(frame, area);
            return;
        }

        let title = link
            .title
            .as_deref()
            .unwrap_or(link.url.as_str())
            .to_string();

        if let Some(protocol) =
            self.rendered
                .preview_protocol(link_id.0, link.kind, link.url.as_str())
        {
            let terminal = self.view_state.terminal_size();
            let popup = crate::render::centered_rect(
                crate::render::PREVIEW_POPUP_PERCENT,
                crate::render::PREVIEW_POPUP_PERCENT,
                area,
            );

            if (self.preview_zoom - 1.0).abs() < f32::EPSILON {
                self.preview_render_cache
                    .ensure(link_id, terminal, &title, protocol);
                if self
                    .preview_render_cache
                    .blit(link_id, terminal, area, frame.buffer_mut())
                {
                    return;
                }
            }

            frame.render_widget(Clear, popup);
            let block = Block::bordered().title(title);
            let inner = block.inner(popup);
            frame.render_widget(block, popup);
            crate::render::render_floating_image(
                protocol,
                inner,
                frame.buffer_mut(),
                self.preview_zoom,
            );
            return;
        }

        if matches!(self.preview_load_status(link_id), PreviewLoadStatus::Failed) {
            let popup = crate::render::centered_rect(
                crate::render::PREVIEW_POPUP_PERCENT,
                crate::render::PREVIEW_POPUP_PERCENT,
                area,
            );
            frame.render_widget(Clear, popup);
            let block = Block::bordered().title(title);
            let inner = block.inner(popup);
            frame.render_widget(block, popup);
            let message = preview_failed_message(link.kind);
            frame.render_widget(Paragraph::new(message), inner);
        }
    }

    fn draw_toc_preview(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        let entries = self.collect_toc_entries();
        let popup = crate::render::centered_rect(
            crate::render::PREVIEW_POPUP_PERCENT,
            crate::render::PREVIEW_POPUP_PERCENT,
            area,
        );
        frame.render_widget(Clear, popup);
        let block = Block::bordered().title("Table of Contents");
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        if entries.is_empty() {
            frame.render_widget(Paragraph::new("(no headings)"), inner);
            return;
        }

        let selected = self.toc_selected_index;
        let normal_style = self.theme.text;
        let selected_style = self.theme.link_selected;
        let prefix_style = Style::default().add_modifier(Modifier::DIM);

        let lines: Vec<Line> = entries
            .iter()
            .enumerate()
            .map(|(i, (level, text, _slug))| {
                let indent = "  ".repeat(level.as_u8().saturating_sub(1) as usize);
                let prefix = level.prefix();
                if i == selected {
                    Line::from(vec![
                        Span::styled(format!("{indent}{prefix}"), selected_style),
                        Span::styled(text.as_str(), selected_style),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled(format!("{indent}{prefix}"), prefix_style),
                        Span::styled(text.as_str(), normal_style),
                    ])
                }
            })
            .collect();

        let visible_height = inner.height as usize;
        let scroll_y = if visible_height > 0 && selected >= visible_height {
            selected - visible_height + 1
        } else {
            0
        };
        let paragraph = Paragraph::new(lines).scroll((scroll_y as u16, 0));
        frame.render_widget(paragraph, inner);
    }

    fn draw_footnote_preview(
        &self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        footnote_id: crate::domain::FootnoteId,
        ctx: &RenderContext<'_>,
    ) {
        let popup = crate::render::centered_rect(
            crate::render::PREVIEW_POPUP_PERCENT,
            crate::render::PREVIEW_POPUP_PERCENT,
            area,
        );
        frame.render_widget(Clear, popup);
        let title = footnote_preview_title(&self.document, footnote_id);
        let block = Block::bordered().title(title);
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        if self.document.footnotes.get(footnote_id.0).is_none() {
            frame.render_widget(Paragraph::new("(footnote not found)"), inner);
            return;
        }

        let mut buffer = ratatui::buffer::Buffer::empty(inner);
        if !render_footnote_preview(&self.document, footnote_id, inner, &mut buffer, ctx) {
            frame.render_widget(Paragraph::new("(footnote not found)"), inner);
            return;
        }

        for y in 0..inner.height {
            for x in 0..inner.width {
                if let Some(cell) = buffer.cell((x, y)) {
                    frame.buffer_mut()[(inner.x + x, inner.y + y)] = cell.clone();
                }
            }
        }
    }
}
