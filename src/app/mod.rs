//! Application loop and state.

mod checklist;
mod draw;
mod input;
mod layout;
mod navigation;
mod reload;
mod scroll;
mod search;
pub mod status;

#[cfg(test)]
mod tests;

use std::time::{Duration, Instant};

use crossterm::event;

use ratatui::{Terminal, backend::Backend};
use ratatui_image::picker::Picker;

use crate::domain::{ChecklistState, ChecklistStyle, Document, TerminalSize, ViewState};
use crate::error::AppError;
use crate::render::{DocumentRenderCache, RenderedDocument, SyntaxAssets, Theme};

use layout::terminal_size;
use reload::FileWatch;
use scroll::{
    ACTIVE_FRAME_INTERVAL, IDLE_POLL_INTERVAL, SCROLL_ANIM_SPEED, STATUS_MESSAGE_DURATION,
};

pub struct App {
    document: Document,
    rendered: RenderedDocument,
    view_state: ViewState,
    document_cache: DocumentRenderCache,
    /// Animated scroll position; lerps toward `view_state.scroll().offset()`.
    scroll_visual: f32,
    /// Exponential smoothing rate for the current scroll animation.
    scroll_anim_speed: f32,
    /// When the current j/k hold sequence started (`Press`); cleared on `Release`.
    scroll_key_down_at: Option<Instant>,
    /// Timestamp of the last line scroll triggered by key repeat.
    last_scroll_repeat: Instant,
    last_tick: Instant,
    /// Fractional scroll position used to detect motion for image deferral.
    tracked_scroll_position: f32,
    /// When to turn terminal images back on after scrolling stops.
    images_reenable_at: Option<Instant>,
    /// Whether mermaid/markdown images are drawn in the current cache.
    pub(crate) show_terminal_images: bool,
    syntax_assets: SyntaxAssets,
    theme: Theme,
    checklist_state: ChecklistState,
    source_label: Option<String>,
    help_visible: bool,
    status_message: Option<String>,
    status_message_until: Option<Instant>,
    picker: Picker,
    pub(crate) file_watch: Option<FileWatch>,
    next_reload_poll: Instant,
    should_quit: bool,
}

impl App {
    pub fn new(
        document: Document,
        picker: Picker,
        base_path: Option<std::path::PathBuf>,
        source_label: Option<String>,
    ) -> Result<Self, AppError> {
        Self::new_with_terminal_size(document, picker, base_path, source_label, terminal_size()?)
    }

    pub fn new_with_terminal_size(
        document: Document,
        picker: Picker,
        base_path: Option<std::path::PathBuf>,
        source_label: Option<String>,
        terminal_size: TerminalSize,
    ) -> Result<Self, AppError> {
        let rendered =
            RenderedDocument::new(&document, &picker, terminal_size, base_path.as_deref())?;
        let view_state = ViewState::new(terminal_size);
        let scroll_visual = view_state.scroll().offset() as f32;
        let now = Instant::now();
        let file_watch = base_path
            .as_ref()
            .and_then(|path| FileWatch::new(path.clone()).ok());
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
            tracked_scroll_position: scroll_visual,
            images_reenable_at: None,
            show_terminal_images: true,
            syntax_assets: SyntaxAssets::new(),
            theme: Theme::default(),
            checklist_state: ChecklistState::new(ChecklistStyle::from_env()),
            source_label,
            help_visible: false,
            status_message: None,
            status_message_until: None,
            picker,
            file_watch,
            next_reload_poll: now,
            should_quit: false,
        })
    }

    pub(crate) fn set_status_message(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.status_message_until = Some(Instant::now() + STATUS_MESSAGE_DURATION);
    }

    pub(crate) fn tick_status_message(&mut self, now: Instant) {
        if let Some(until) = self.status_message_until
            && now >= until
        {
            self.status_message = None;
            self.status_message_until = None;
        }
    }

    pub(crate) fn content_height(&self) -> u16 {
        layout::content_height(
            self.view_state.terminal_size().height(),
            self.view_state.mode(),
        )
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

            if self.poll_file_reload(now)? {
                dirty = true;
            }

            let animating = self.tick_scroll_animation(dt);
            let image_dirty = self.update_terminal_image_visibility(now);
            let awaiting_images = self.images_reenable_at.is_some();
            self.tick_status_message(now);

            if dirty || animating || image_dirty {
                self.draw_frame(terminal)?;
                last_draw = now;
            }

            let frame_budget = if animating || awaiting_images {
                ACTIVE_FRAME_INTERVAL
            } else {
                IDLE_POLL_INTERVAL
            };
            let wait = frame_budget.saturating_sub(last_draw.elapsed());
            if event::poll(wait)? {
                continue;
            }
            if animating || awaiting_images {
                continue;
            }
        }
        Ok(())
    }
}
