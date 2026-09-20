//! Scroll animation, bounds, and line-scroll key helpers.

use std::time::{Duration, Instant};

use crate::domain::TerminalSize;
use crate::error::AppError;
use crate::render::subpixel::SUBPIXEL_SNAP;
use crate::render::{RenderContext, measure_document_height};

use super::App;

pub(crate) const SCROLL_REPEAT_DELAY: Duration = Duration::from_millis(180);
pub(crate) const SCROLL_REPEAT_INTERVAL: Duration = Duration::from_millis(33);
pub(crate) const ACTIVE_FRAME_INTERVAL: Duration = Duration::from_millis(16);
pub(crate) const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(50);
pub(crate) const SCROLL_ANIM_SPEED: f32 = 20.0;
/// Exponential smoothing rate for half-page scroll (higher = faster convergence).
pub(crate) const HALF_PAGE_SCROLL_ANIM_SPEED: f32 = 140.0;
pub(crate) const LINE_SCROLL_LINES: usize = 2;
pub(crate) const STATUS_MESSAGE_DURATION: Duration = Duration::from_secs(3);

pub(crate) struct ScrollUi {
    pub visual: f32,
    pub anim_speed: f32,
    pub key_down_at: Option<Instant>,
    pub last_repeat: Instant,
}

impl ScrollUi {
    pub(crate) fn new(visual: f32, now: Instant) -> Self {
        Self {
            visual,
            anim_speed: SCROLL_ANIM_SPEED,
            key_down_at: None,
            last_repeat: now,
        }
    }

    pub(crate) fn reset_to_top(&mut self) {
        self.visual = 0.0;
        self.anim_speed = SCROLL_ANIM_SPEED;
        self.key_down_at = None;
    }
}

impl App {
    pub(crate) fn max_scroll(&self) -> usize {
        let total_height = measure_document_height(
            &self.document,
            self.document_width(),
            &self.render_context(),
        );
        let view_height = self.content_height() as usize;
        if total_height <= view_height {
            return 0;
        }
        total_height.saturating_sub(view_height)
    }

    pub(crate) fn render_context(&self) -> RenderContext<'_> {
        RenderContext::new(
            &self.theme,
            &self.syntax_assets,
            &self.rendered,
            &self.document.links,
            &self.view_state,
            &self.checklist_state,
        )
    }

    pub(crate) fn snap_scroll_visual(&mut self) {
        self.scroll.visual = self.view_state.scroll().offset() as f32;
    }

    /// Advance the visual scroll toward the logical target with exponential ease-out.
    /// Returns `true` while the target has not been reached.
    pub(crate) fn tick_scroll_animation(&mut self, dt: Duration) -> bool {
        let target = self.view_state.scroll().offset() as f32;
        let delta = target - self.scroll.visual;
        if delta.abs() < SUBPIXEL_SNAP {
            self.scroll.visual = target;
            self.scroll.anim_speed = SCROLL_ANIM_SPEED;
            return false;
        }
        let t = dt.as_secs_f32().max(1.0 / 120.0);
        let factor = 1.0 - (-self.scroll.anim_speed * t).exp();
        self.scroll.visual += delta * factor;
        if (target - self.scroll.visual).abs() < SUBPIXEL_SNAP {
            self.scroll.visual = target;
            self.scroll.anim_speed = SCROLL_ANIM_SPEED;
            return false;
        }
        true
    }

    /// Returns `true` when the terminal size changed.
    pub(crate) fn poll_terminal_resize(&mut self) -> Result<bool, AppError> {
        let (width, height) = crossterm::terminal::size()?;
        let size = TerminalSize::new(width, height).map_err(AppError::TerminalSize)?;
        if size == self.view_state.terminal_size() {
            return Ok(false);
        }
        self.view_state = self.view_state.clone().resize(size);
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().clamp_scroll(max);
        self.scroll.visual = self.view_state.scroll().offset() as f32;
        self.invalidate_preview_caches();
        Ok(true)
    }
}
