//! Terminal layout helpers.

use ratatui::layout::Rect;

use crate::domain::{TerminalSize, UiMode};
use crate::error::AppError;

pub(crate) const STATUS_BAR_HEIGHT: u16 = 1;
/// Preferred outline sidebar width in columns.
pub(crate) const OUTLINE_WIDTH: u16 = 28;
/// Gap between outline and main content.
pub(crate) const OUTLINE_GAP: u16 = 1;
/// Minimum terminal width required to show the outline panel.
pub(crate) const OUTLINE_MIN_TERMINAL_WIDTH: u16 = 40;

pub(crate) struct LayoutAreas {
    pub outline: Rect,
    pub main: Rect,
    pub status: Rect,
    pub prompt: Rect,
}

pub(crate) fn terminal_size() -> Result<TerminalSize, AppError> {
    let (width, height) = crossterm::terminal::size()?;
    TerminalSize::new(width, height).map_err(AppError::TerminalSize)
}

/// Width of the outline panel alone (excluding gap). Zero when the terminal is too narrow.
pub(crate) fn outline_panel_width(terminal_width: u16) -> u16 {
    if terminal_width < OUTLINE_MIN_TERMINAL_WIDTH {
        return 0;
    }
    OUTLINE_WIDTH
        .min(terminal_width / 3)
        .max(16)
        .min(terminal_width.saturating_sub(20))
}

/// Visible document height after reserving the status bar and optional search prompt.
pub(crate) fn content_height(terminal_height: u16, mode: &UiMode) -> u16 {
    let chrome = STATUS_BAR_HEIGHT + if mode.is_search_input() { 1 } else { 0 };
    terminal_height.saturating_sub(chrome).max(1)
}

/// Split the terminal into optional outline, main content, status bar, and prompt.
///
/// With `max_width`, a wider main column is narrowed to it and centered in the
/// space beside the outline. Every width-dependent path (wrapping, hit tests,
/// selection) reads the main rect from here so they stay in agreement.
pub(crate) fn split_layout(
    area: Rect,
    mode: &UiMode,
    outline_visible: bool,
    max_width: Option<u16>,
) -> LayoutAreas {
    let prompt_rows = if mode.is_search_input() { 1 } else { 0 };
    let chrome = STATUS_BAR_HEIGHT + prompt_rows;
    let main_height = area.height.saturating_sub(chrome).max(1);

    let panel_w = if outline_visible {
        outline_panel_width(area.width)
    } else {
        0
    };
    let (outline, main_x, main_width) = if panel_w > 0 {
        let gap = OUTLINE_GAP.min(area.width.saturating_sub(panel_w));
        let outline = Rect {
            x: area.x,
            y: area.y,
            width: panel_w,
            height: main_height,
        };
        let main_x = area.x + panel_w + gap;
        let main_width = area.width.saturating_sub(panel_w + gap).max(1);
        (outline, main_x, main_width)
    } else {
        (
            Rect {
                x: area.x,
                y: area.y,
                width: 0,
                height: main_height,
            },
            area.x,
            area.width,
        )
    };

    let (main_x, main_width) = match max_width {
        Some(max) if main_width > max => (main_x + (main_width - max) / 2, max),
        _ => (main_x, main_width),
    };
    let main = Rect {
        x: main_x,
        y: area.y,
        width: main_width,
        height: main_height,
    };

    let mut y = area.y + main_height;
    let prompt = if prompt_rows > 0 {
        let r = Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        };
        y += 1;
        r
    } else {
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: 0,
        }
    };

    let status = Rect {
        x: area.x,
        y,
        width: area.width,
        height: STATUS_BAR_HEIGHT.min(area.height),
    };

    LayoutAreas {
        outline,
        main,
        status,
        prompt,
    }
}
