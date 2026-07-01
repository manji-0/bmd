//! Terminal layout helpers.

use ratatui::layout::Rect;

use crate::domain::{TerminalSize, UiMode};
use crate::error::AppError;

pub(crate) const STATUS_BAR_HEIGHT: u16 = 1;

pub(crate) struct LayoutAreas {
    pub main: Rect,
    pub status: Rect,
    pub prompt: Rect,
}

pub(crate) fn terminal_size() -> Result<TerminalSize, AppError> {
    let (width, height) = crossterm::terminal::size()?;
    TerminalSize::new(width, height).map_err(AppError::TerminalSize)
}

/// Visible document height after reserving the status bar and optional search prompt.
pub(crate) fn content_height(terminal_height: u16, mode: &UiMode) -> u16 {
    let chrome = STATUS_BAR_HEIGHT + if mode.is_search_input() { 1 } else { 0 };
    terminal_height.saturating_sub(chrome).max(1)
}

/// Split the terminal into main content, a one-line status bar, and an optional prompt.
pub(crate) fn split_layout(area: Rect, mode: &UiMode) -> LayoutAreas {
    let prompt_rows = if mode.is_search_input() { 1 } else { 0 };
    let chrome = STATUS_BAR_HEIGHT + prompt_rows;
    let main_height = area.height.saturating_sub(chrome).max(1);

    let main = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
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
        main,
        status,
        prompt,
    }
}

/// Split the terminal area into the main content area and a one-line prompt area
/// when the application is in search input mode.
pub(crate) fn split_main_and_prompt(area: Rect, mode: &UiMode) -> (Rect, Rect) {
    let areas = split_layout(area, mode);
    (areas.main, areas.prompt)
}

pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    crate::render::centered_rect(percent_x, percent_y, r)
}
