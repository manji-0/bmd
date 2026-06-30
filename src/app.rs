//! Application loop and state.

use std::time::Duration;

use crossterm::event;
use ratatui::{
    Terminal,
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Clear, Paragraph},
};
use ratatui_image::picker::Picker;

use crate::browser::open_link;
use crate::domain::{Document, TerminalSize, ViewState};
use crate::error::AppError;
use crate::keymap::{Command, map_event};
use crate::render::{
    MarkdownWidget, RenderContext, RenderedDocument, SyntaxAssets, Theme, measure_document_height,
};

#[cfg(test)]
use crate::parse::parse;

pub struct App {
    document: Document,
    rendered: RenderedDocument,
    view_state: ViewState,
    syntax_assets: SyntaxAssets,
    theme: Theme,
    should_quit: bool,
    error_message: Option<String>,
}

impl App {
    pub fn new(document: Document, picker: Picker) -> Result<Self, AppError> {
        let size = terminal_size()?;
        let rendered = RenderedDocument::new(&document, &picker, size.width())?;
        let view_state = ViewState::new(size);
        Ok(Self {
            document,
            rendered,
            view_state,
            syntax_assets: SyntaxAssets::new(),
            theme: Theme::default(),
            should_quit: false,
            error_message: None,
        })
    }

    pub fn run<B: Backend>(mut self, terminal: &mut Terminal<B>) -> Result<(), AppError>
    where
        AppError: From<B::Error>,
    {
        // Poll with a short timeout so quit keys are handled promptly without busy-waiting.
        let poll_timeout = Duration::from_millis(1);

        while !self.should_quit {
            // Drain all available input before rendering. If a quit key is queued, handle
            // it immediately and skip the draw call.
            while event::poll(poll_timeout)? {
                let command = map_event(event::read()?);
                if self.is_quit(command) {
                    self.should_quit = true;
                    break;
                }
                self.handle_command(command)?;
            }

            if self.should_quit {
                break;
            }

            terminal.draw(|f| {
                let main_area = f.area();
                let ctx = RenderContext::new(
                    &self.theme,
                    &self.syntax_assets.syntax_set,
                    self.syntax_assets.theme(),
                    &self.rendered,
                    &self.view_state,
                );
                let widget = MarkdownWidget::new(&self.document, &ctx, &self.view_state);
                f.render_widget(widget, main_area);

                if let Some(ref msg) = self.error_message {
                    let popup = centered_rect(60, 20, main_area);
                    f.render_widget(Clear, popup);
                    let block = Block::bordered().title("Error");
                    let para = Paragraph::new(msg.clone()).block(block);
                    f.render_widget(para, popup);
                }
            })?;

            // Auto-clear transient error messages after one rendered frame.
            if self.error_message.is_some() {
                self.error_message = None;
            }
        }
        Ok(())
    }

    fn handle_command(&mut self, command: Command) -> Result<(), AppError> {
        if std::env::var("BMD_DEBUG").is_ok() {
            eprintln!("[bmd debug] command: {:?}", command);
        }
        match command {
            Command::None => {}
            Command::ScrollDown => self.scroll_down(1),
            Command::ScrollUp => self.scroll_up(1),
            Command::HalfPageDown => self.half_page_down(),
            Command::HalfPageUp => self.half_page_up(),
            Command::JumpToTop => self.jump_to_top(),
            Command::JumpToBottom => self.jump_to_bottom(),
            Command::NextLink => self.next_link(),
            Command::PrevLink => self.prev_link(),
            Command::OpenLink => self.open_current_link(),
            Command::Quit => self.should_quit = true,
        }
        Ok(())
    }

    fn is_quit(&self, command: Command) -> bool {
        matches!(command, Command::Quit)
    }

    fn scroll_down(&mut self, n: usize) {
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().scroll_down(n, max);
    }

    fn scroll_up(&mut self, n: usize) {
        self.view_state = self.view_state.clone().scroll_up(n);
    }

    fn half_page_down(&mut self) {
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().half_page_down(max);
    }

    fn half_page_up(&mut self) {
        self.view_state = self.view_state.clone().half_page_up();
    }

    fn jump_to_top(&mut self) {
        self.view_state = self.view_state.clone().jump_to_top();
    }

    fn jump_to_bottom(&mut self) {
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().jump_to_bottom(max);
    }

    fn next_link(&mut self) {
        self.view_state = self.view_state.clone().select_next_link(&self.document);
        self.scroll_to_selected_link();
    }

    fn prev_link(&mut self) {
        self.view_state = self.view_state.clone().select_prev_link(&self.document);
        self.scroll_to_selected_link();
    }

    fn open_current_link(&mut self) {
        match self.view_state.selected_link() {
            Some(id) => {
                if let Some(link) = self.document.links.get(id.0) {
                    if let Err(e) = open_link(&link.url) {
                        self.error_message = Some(e.to_string());
                    }
                } else {
                    self.error_message = Some(format!("dangling link {}", id));
                }
            }
            None => self.error_message = Some("no link selected".to_string()),
        }
    }

    fn scroll_to_selected_link(&mut self) {
        // Keep the selected link visible on screen. For now, rely on the user to scroll.
        // A future improvement would compute the Y position of each link occurrence.
    }

    fn max_scroll(&self) -> usize {
        let total_height = measure_document_height(
            &self.document,
            self.view_state.terminal_size().width(),
            &self.render_context(),
        );
        let view_height = self.view_state.terminal_size().height() as usize;
        // If the whole document fits on screen, do not allow scrolling.
        if total_height <= view_height {
            return 0;
        }
        total_height.saturating_sub(view_height)
    }

    fn render_context(&self) -> RenderContext<'_> {
        RenderContext::new(
            &self.theme,
            &self.syntax_assets.syntax_set,
            self.syntax_assets.theme(),
            &self.rendered,
            &self.view_state,
        )
    }
}

fn terminal_size() -> Result<TerminalSize, AppError> {
    let (width, height) = crossterm::terminal::size()?;
    TerminalSize::new(width, height).map_err(AppError::TerminalSize)
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Block, Heading, HeadingLevel, Inline, Link, LinkUrl};

    fn dummy_document() -> Document {
        Document {
            blocks: vec![Block::Heading(Heading {
                level: HeadingLevel::H1,
                content: vec![Inline::Text("Hello".to_string())],
            })],
            links: vec![Link {
                url: LinkUrl::new("https://example.com".to_string()).unwrap(),
                title: None,
            }],
        }
    }

    #[test]
    fn open_link_without_selection_records_error() {
        // This test exercises the command path without a real terminal.
        let doc = dummy_document();
        let picker = Picker::halfblocks();
        let mut app = App::new(doc, picker).unwrap();
        app.open_current_link();
        assert!(app.error_message.is_some());
    }

    #[test]
    fn renders_document_to_test_backend() {
        let input = "# Title\n\nA paragraph with **bold** and [a link](https://example.com).\n\n| Name | Value |\n|------|-------|\n| A    | 1     |\n";
        let doc = parse(input).unwrap();
        let picker = Picker::halfblocks();
        let app = App::new(doc, picker).unwrap();

        let backend = ratatui::backend::TestBackend::new(80, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let ctx = RenderContext::new(
                    &app.theme,
                    &app.syntax_assets.syntax_set,
                    app.syntax_assets.theme(),
                    &app.rendered,
                    &app.view_state,
                );
                let widget = MarkdownWidget::new(&app.document, &ctx, &app.view_state);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        assert!(!terminal.backend().buffer().content().is_empty());
    }

    #[test]
    fn short_document_cannot_scroll() {
        let input = "# Title\n\nA paragraph.\n";
        let doc = parse(input).unwrap();
        let picker = Picker::halfblocks();
        let app = App::new(doc, picker).unwrap();

        // On a 30-row terminal the content fits, so max_scroll should be 0.
        assert_eq!(app.max_scroll(), 0);
    }
}
