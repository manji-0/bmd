//! Application loop and state.

mod draw;
mod input;
mod layout;
mod navigation;
mod scroll;
mod search;

#[cfg(test)]
mod tests;

use std::time::{Duration, Instant};

use crossterm::event;

use ratatui::{Terminal, backend::Backend};
use ratatui_image::picker::Picker;

use crate::domain::{Document, ViewState};
use crate::error::AppError;
use crate::render::{DocumentRenderCache, RenderedDocument, SyntaxAssets, Theme};

use layout::terminal_size;
use scroll::{ACTIVE_FRAME_INTERVAL, IDLE_POLL_INTERVAL, SCROLL_ANIM_SPEED};

pub struct App {
    document: Document,
    rendered: RenderedDocument,
    view_state: ViewState,
    document_cache: DocumentRenderCache,
    /// Animated scroll position; lerps toward `view_state.scroll().offset()`.
    scroll_visual: f32,
    /// Lines per second for the current scroll animation.
    scroll_anim_speed: f32,
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
    pub fn new(
        document: Document,
        picker: Picker,
        base_path: Option<std::path::PathBuf>,
    ) -> Result<Self, AppError> {
        let size = terminal_size()?;
        let rendered =
            RenderedDocument::new(&document, &picker, size.width(), base_path.as_deref())?;
        let view_state = ViewState::new(size);
        let scroll_visual = view_state.scroll().offset() as f32;
        let now = Instant::now();
        Ok(Self {
            document,
            rendered,
            view_state,
            document_cache: DocumentRenderCache::default(),
            scroll_visual,
            scroll_anim_speed: SCROLL_ANIM_SPEED,
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
}
