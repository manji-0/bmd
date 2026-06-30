//! Terminal layout helpers.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::domain::TerminalSize;
use crate::error::AppError;
use crate::keymap::KeymapMode;

pub(crate) fn terminal_size() -> Result<TerminalSize, AppError> {
    let (width, height) = crossterm::terminal::size()?;
    TerminalSize::new(width, height).map_err(AppError::TerminalSize)
}

/// Split the terminal area into the main content area and a one-line prompt area
/// when the application is in search input mode.
pub(crate) fn split_main_and_prompt(area: Rect, mode: KeymapMode) -> (Rect, Rect) {
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

pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
