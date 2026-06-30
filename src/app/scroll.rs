//! Scroll animation, bounds, and line-scroll key helpers.

use std::time::{Duration, Instant};

use crossterm::event::KeyCode;

use crate::domain::TerminalSize;
use crate::error::AppError;
use crate::keymap::Command;
use crate::render::subpixel::SUBPIXEL_SNAP;
use crate::render::{RenderContext, measure_document_height};

use super::App;

pub(crate) const IMAGE_REENABLE_DELAY: Duration = Duration::from_millis(100);
pub(crate) const SCROLL_REPEAT_DELAY: Duration = Duration::from_millis(180);
pub(crate) const SCROLL_REPEAT_INTERVAL: Duration = Duration::from_millis(33);
pub(crate) const ACTIVE_FRAME_INTERVAL: Duration = Duration::from_millis(16);
pub(crate) const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(50);
pub(crate) const SCROLL_ANIM_SPEED: f32 = 20.0;
/// Exponential smoothing rate for half-page scroll (higher = faster convergence).
pub(crate) const HALF_PAGE_SCROLL_ANIM_SPEED: f32 = 140.0;
pub(crate) const LINE_SCROLL_LINES: usize = 2;

impl App {
    pub(crate) fn max_scroll(&self) -> usize {
        let total_height = measure_document_height(
            &self.document,
            self.view_state.terminal_size().width(),
            &self.render_context(),
        );
        let view_height = self.view_state.terminal_size().height() as usize;
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
            self.show_terminal_images,
            &self.checklist_state,
        )
    }

    /// Hide terminal images while scroll position changes; show again after idle.
    ///
    /// Returns `true` when image visibility toggled and the frame should redraw.
    pub(crate) fn update_terminal_image_visibility(&mut self, now: Instant) -> bool {
        let scroll_pos = self.scroll_visual;
        let mut dirty = false;

        if (scroll_pos - self.tracked_scroll_position).abs() >= SUBPIXEL_SNAP {
            self.tracked_scroll_position = scroll_pos;
            self.images_reenable_at = None;
            if self.show_terminal_images {
                self.show_terminal_images = false;
                self.document_cache.invalidate();
                dirty = true;
            }
        } else if !self.show_terminal_images && self.images_reenable_at.is_none() {
            self.images_reenable_at = Some(now + IMAGE_REENABLE_DELAY);
        }

        if let Some(deadline) = self.images_reenable_at
            && now >= deadline
        {
            self.images_reenable_at = None;
            self.show_terminal_images = true;
            self.document_cache.invalidate();
            dirty = true;
        }

        dirty
    }

    pub(crate) fn snap_scroll_visual(&mut self) {
        self.scroll_visual = self.view_state.scroll().offset() as f32;
    }

    /// Advance the visual scroll toward the logical target with exponential ease-out.
    /// Returns `true` while the target has not been reached.
    pub(crate) fn tick_scroll_animation(&mut self, dt: Duration) -> bool {
        let target = self.view_state.scroll().offset() as f32;
        let delta = target - self.scroll_visual;
        if delta.abs() < SUBPIXEL_SNAP {
            self.scroll_visual = target;
            self.scroll_anim_speed = SCROLL_ANIM_SPEED;
            return false;
        }
        let t = dt.as_secs_f32().max(1.0 / 120.0);
        let factor = 1.0 - (-self.scroll_anim_speed * t).exp();
        self.scroll_visual += delta * factor;
        if (target - self.scroll_visual).abs() < SUBPIXEL_SNAP {
            self.scroll_visual = target;
            self.scroll_anim_speed = SCROLL_ANIM_SPEED;
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
        let clamped = self.view_state.scroll().offset().min(max) as f32;
        self.scroll_visual = clamped;
        Ok(true)
    }
}

pub(crate) fn is_line_scroll_key(code: &KeyCode) -> bool {
    matches!(
        code,
        KeyCode::Char('j') | KeyCode::Char('k') | KeyCode::Down | KeyCode::Up
    )
}

/// Keys that should fire once per physical press; OS repeat is ignored.
pub(crate) fn is_single_press_key(code: &KeyCode) -> bool {
    is_line_scroll_key(code)
        || matches!(
            code,
            KeyCode::Char('d') | KeyCode::Char('u') | KeyCode::PageDown | KeyCode::PageUp
        )
}

pub(crate) fn line_scroll_command(code: &KeyCode) -> Command {
    match code {
        KeyCode::Char('k') | KeyCode::Up => Command::ScrollUp,
        _ => Command::ScrollDown,
    }
}
