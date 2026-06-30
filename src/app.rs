//! Application loop and state.

use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    Terminal,
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Clear, Paragraph},
};
use ratatui_image::picker::Picker;

use crate::browser::open_link;
use crate::domain::{Document, SearchDirection, SearchState, TerminalSize, ViewState};
use crate::error::AppError;
use crate::keymap::{Command, KeymapMode, map_event};
use crate::render::{
    CachedMarkdownView, DocumentRenderCache, RenderContext, RenderedDocument, SyntaxAssets, Theme,
    find_search_matches, measure_document_height,
};

/// Delay before line-scroll auto-repeat begins after the initial key press.
const SCROLL_REPEAT_DELAY: Duration = Duration::from_millis(180);
/// Fixed interval between repeated line scrolls while j/k is held.
const SCROLL_REPEAT_INTERVAL: Duration = Duration::from_millis(33);
/// Target frame interval while scrolling or animating (~60 fps).
const ACTIVE_FRAME_INTERVAL: Duration = Duration::from_millis(16);
/// Poll interval when idle (no scroll animation or held keys).
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(50);
/// Line-scroll animation speed for half-page, jump, and search navigation.
const SCROLL_ANIM_SPEED: f32 = 20.0;

#[cfg(test)]
use crate::parse::parse;

pub struct App {
    document: Document,
    rendered: RenderedDocument,
    view_state: ViewState,
    document_cache: DocumentRenderCache,
    /// Animated scroll position; lerps toward `view_state.scroll().offset()`.
    scroll_visual: f32,
    /// When the current j/k hold sequence started (`Press`); cleared on `Release`.
    scroll_key_down_at: Option<Instant>,
    /// Timestamp of the last line scroll triggered by key repeat.
    last_scroll_repeat: Instant,
    last_tick: Instant,
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
        let scroll_visual = view_state.scroll().offset() as f32;
        let now = Instant::now();
        Ok(Self {
            document,
            rendered,
            view_state,
            document_cache: DocumentRenderCache::default(),
            scroll_visual,
            scroll_key_down_at: None,
            last_scroll_repeat: now,
            last_tick: now,
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
        self.last_tick = Instant::now();
        let mut last_draw = Instant::now();
        self.draw_frame(terminal)?;

        while !self.should_quit {
            let now = Instant::now();
            let dt = now.saturating_duration_since(self.last_tick);
            self.last_tick = now;
            let mut dirty = false;

            while event::poll(Duration::ZERO)? {
                if self.handle_crossterm_event(event::read()?)? {
                    dirty = true;
                }
                if self.should_quit {
                    break;
                }
            }

            if self.should_quit {
                break;
            }

            if self.poll_terminal_resize()? {
                dirty = true;
            }

            let animating = self.tick_scroll_animation(dt);

            if dirty || animating {
                self.draw_frame(terminal)?;
                last_draw = now;
                if self.error_message.is_some() {
                    self.error_message = None;
                }
            }

            let frame_budget = if animating {
                ACTIVE_FRAME_INTERVAL
            } else {
                IDLE_POLL_INTERVAL
            };
            let wait = frame_budget.saturating_sub(last_draw.elapsed());
            if event::poll(wait)? {
                continue;
            }
            if animating {
                continue;
            }
        }
        Ok(())
    }

    fn draw_frame<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), AppError>
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
            );
            let width = self.view_state.terminal_size().width();
            self.document_cache
                .ensure(&self.document, &ctx, &self.view_state, width);
            let scroll = self.display_scroll_offset();
            let widget = CachedMarkdownView {
                cache: &self.document_cache,
                scroll,
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

    /// Handle a crossterm event. Returns `true` when the frame should be redrawn.
    fn handle_crossterm_event(&mut self, event: Event) -> Result<bool, AppError> {
        if let Event::Key(key) = &event {
            if self.keymap_mode() == KeymapMode::Normal {
                if is_line_scroll_key(&key.code) {
                    return self.handle_line_scroll_key(key);
                }
                if key.kind == KeyEventKind::Press {
                    self.scroll_key_down_at = None;
                }
            }
            if key.kind == KeyEventKind::Release {
                return Ok(false);
            }
            if key.kind == KeyEventKind::Repeat
                && self.keymap_mode() == KeymapMode::Normal
                && is_single_press_key(&key.code)
                && !is_line_scroll_key(&key.code)
            {
                return Ok(false);
            }
        }

        let mode = self.keymap_mode();
        let command = map_event(event, mode, self.view_state.is_search_active());
        if self.is_quit(&command) {
            self.should_quit = true;
            return Ok(true);
        }
        self.handle_command(command)?;
        Ok(true)
    }

    /// Handle j/k/arrow line scrolling with rate-limited OS key repeat.
    ///
    /// We do not track key hold state because most terminals do not deliver
    /// `KeyEventKind::Release` without keyboard enhancement flags.
    fn handle_line_scroll_key(&mut self, key: &KeyEvent) -> Result<bool, AppError> {
        let now = Instant::now();
        match key.kind {
            KeyEventKind::Press => {
                self.scroll_key_down_at = Some(now);
                self.handle_command(line_scroll_command(&key.code))?;
                Ok(true)
            }
            KeyEventKind::Repeat => {
                let Some(pressed_at) = self.scroll_key_down_at else {
                    self.scroll_key_down_at = Some(now);
                    self.handle_command(line_scroll_command(&key.code))?;
                    return Ok(true);
                };
                if now < pressed_at + SCROLL_REPEAT_DELAY {
                    return Ok(false);
                }
                if now < self.last_scroll_repeat + SCROLL_REPEAT_INTERVAL {
                    return Ok(false);
                }
                self.handle_command(line_scroll_command(&key.code))?;
                self.last_scroll_repeat = now;
                Ok(true)
            }
            KeyEventKind::Release => {
                self.scroll_key_down_at = None;
                Ok(false)
            }
        }
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
            Command::NextLink => {
                if self.view_state.is_search_active() {
                    self.next_search_match();
                } else {
                    self.next_link();
                }
            }
            Command::PrevLink => {
                if self.view_state.is_search_active() {
                    self.prev_search_match();
                } else {
                    self.prev_link();
                }
            }
            Command::OpenLink => self.open_current_link(),
            Command::StartSearchForward => self.start_search(SearchDirection::Forward),
            Command::StartSearchBackward => self.start_search(SearchDirection::Backward),
            Command::SearchConfirm => self.confirm_search(),
            Command::SearchCancel => self.cancel_search(),
            Command::SearchInput(c) => self.append_search_input(c),
            Command::SearchBackspace => self.backspace_search_input(),
            Command::Quit => self.should_quit = true,
        }
        Ok(())
    }

    fn is_quit(&self, command: &Command) -> bool {
        matches!(command, Command::Quit)
    }

    fn keymap_mode(&self) -> KeymapMode {
        match self.view_state.search_state() {
            SearchState::Input { .. } => KeymapMode::Search,
            _ => KeymapMode::Normal,
        }
    }

    fn scroll_down(&mut self, n: usize) {
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().scroll_down(n, max);
        self.snap_scroll_visual();
    }

    fn scroll_up(&mut self, n: usize) {
        self.view_state = self.view_state.clone().scroll_up(n);
        self.snap_scroll_visual();
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

    fn start_search(&mut self, direction: SearchDirection) {
        self.view_state = self.view_state.clone().start_search(direction);
    }

    fn cancel_search(&mut self) {
        self.view_state = self.view_state.clone().cancel_search();
    }

    fn append_search_input(&mut self, c: char) {
        self.view_state = self.view_state.clone().append_search_input(c);
    }

    fn backspace_search_input(&mut self) {
        self.view_state = self.view_state.clone().backspace_search_input();
    }

    fn confirm_search(&mut self) {
        let (direction, query) = match self.view_state.search_state() {
            SearchState::Input { direction, query } => (*direction, query.clone()),
            _ => return,
        };

        let trimmed = query.trim().to_string();
        if trimmed.is_empty() {
            self.view_state = self.view_state.clone().cancel_search();
            return;
        }

        let ctx = self.render_context();
        let matches = find_search_matches(
            &self.document,
            self.view_state.terminal_size().width(),
            &trimmed,
            &ctx,
        );

        match self
            .view_state
            .clone()
            .confirm_search(trimmed, direction, matches)
        {
            Ok(state) => {
                self.view_state = state;
                // If matches were found, jump directly to the selected match line.
                if let SearchState::Active {
                    matches,
                    current_index,
                    ..
                } = self.view_state.search_state()
                {
                    if let Some(m) = matches.get(*current_index) {
                        let max = self.max_scroll();
                        let target = m.line_offset.min(max);
                        self.view_state = self.view_state.clone().scroll_to(target);
                    } else {
                        self.error_message = Some("no matches found".to_string());
                    }
                }
            }
            Err(e) => {
                self.error_message = Some(e.to_string());
                self.view_state = self.view_state.clone().cancel_search();
            }
        }
    }

    fn next_search_match(&mut self) {
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().next_search_match(max);
    }

    fn prev_search_match(&mut self) {
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().prev_search_match(max);
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

    fn display_scroll_offset(&self) -> usize {
        self.scroll_visual.round() as usize
    }

    fn snap_scroll_visual(&mut self) {
        self.scroll_visual = self.view_state.scroll().offset() as f32;
    }

    /// Advance the visual scroll toward the logical target at a fixed line rate.
    /// Returns `true` while the target has not been reached.
    fn tick_scroll_animation(&mut self, dt: Duration) -> bool {
        let target = self.view_state.scroll().offset() as f32;
        let delta = target - self.scroll_visual;
        if delta.abs() < 0.5 {
            self.scroll_visual = target;
            return false;
        }
        let step = SCROLL_ANIM_SPEED * dt.as_secs_f32().max(1.0 / 120.0);
        if step <= f32::EPSILON {
            return true;
        }
        if delta.abs() <= step {
            self.scroll_visual = target;
            return false;
        }
        self.scroll_visual += step * delta.signum();
        true
    }

    /// Returns `true` when the terminal size changed.
    fn poll_terminal_resize(&mut self) -> Result<bool, AppError> {
        let (width, height) = crossterm::terminal::size()?;
        let size = TerminalSize::new(width, height).map_err(AppError::TerminalSize)?;
        if size == self.view_state.terminal_size() {
            return Ok(false);
        }
        self.view_state = self.view_state.clone().resize(size);
        let max = self.max_scroll();
        let clamped = self.view_state.scroll().offset().min(max) as f32;
        self.scroll_visual = clamped;
        Ok(true)
    }
}

fn is_line_scroll_key(code: &KeyCode) -> bool {
    matches!(
        code,
        KeyCode::Char('j') | KeyCode::Char('k') | KeyCode::Down | KeyCode::Up
    )
}

/// Keys that should fire once per physical press; OS repeat is ignored.
fn is_single_press_key(code: &KeyCode) -> bool {
    is_line_scroll_key(code)
        || matches!(
            code,
            KeyCode::Char('d') | KeyCode::Char('u') | KeyCode::PageDown | KeyCode::PageUp
        )
}

fn line_scroll_command(code: &KeyCode) -> Command {
    match code {
        KeyCode::Char('k') | KeyCode::Up => Command::ScrollUp,
        _ => Command::ScrollDown,
    }
}

fn terminal_size() -> Result<TerminalSize, AppError> {
    let (width, height) = crossterm::terminal::size()?;
    TerminalSize::new(width, height).map_err(AppError::TerminalSize)
}

/// Split the terminal area into the main content area and a one-line prompt area
/// when the application is in search input mode.
fn split_main_and_prompt(area: Rect, mode: KeymapMode) -> (Rect, Rect) {
    match mode {
        KeymapMode::Search => {
            let main_height = area.height.saturating_sub(1).max(1);
            let main = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: main_height,
            };
            let prompt = Rect {
                x: area.x,
                y: area.y + main_height,
                width: area.width,
                height: 1,
            };
            (main, prompt)
        }
        KeymapMode::Normal => (
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: area.height,
            },
            Rect {
                x: area.x,
                y: area.y + area.height,
                width: area.width,
                height: 0,
            },
        ),
    }
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
    use crate::domain::{
        Block, Heading, HeadingLevel, Inline, Link, LinkUrl, SearchDirection, SearchState,
    };

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
    fn line_scroll_key_helpers() {
        assert!(is_line_scroll_key(&KeyCode::Char('j')));
        assert!(is_line_scroll_key(&KeyCode::Up));
        assert!(!is_line_scroll_key(&KeyCode::Char('d')));
        assert_eq!(
            line_scroll_command(&KeyCode::Char('j')),
            Command::ScrollDown
        );
        assert_eq!(line_scroll_command(&KeyCode::Char('k')), Command::ScrollUp);
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
                let width = app.view_state.terminal_size().width();
                let mut cache = DocumentRenderCache::default();
                cache.ensure(&app.document, &ctx, &app.view_state, width);
                let widget = CachedMarkdownView {
                    cache: &cache,
                    scroll: app.view_state.scroll().offset(),
                };
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

    #[test]
    fn search_command_flow_scrolls_to_match() {
        let mut input = String::from("# Alpha\n\n");
        for i in 0..100 {
            input.push_str(&format!("paragraph {}\n\n", i));
        }
        input.push_str("target line\n");
        let doc = parse(&input).unwrap();
        let picker = Picker::halfblocks();
        let mut app = App::new(doc, picker).unwrap();

        app.start_search(SearchDirection::Forward);
        assert!(matches!(
            app.view_state.search_state(),
            SearchState::Input { .. }
        ));

        for c in "target".chars() {
            app.append_search_input(c);
        }
        app.confirm_search();

        assert!(app.view_state.is_search_active());
        // The document is long enough that the target line is below the first screen.
        let max_scroll = app.max_scroll();
        assert!(max_scroll > 0);
        assert!(app.view_state.scroll().offset() > 0);

        let before = app.view_state.scroll().offset();
        app.next_search_match();
        // With a single match, cycling wraps back to the same position.
        assert_eq!(app.view_state.scroll().offset(), before);
    }
}
