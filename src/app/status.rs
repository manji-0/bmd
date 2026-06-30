//! Status bar text and help overlay content.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Wrap},
};

use crate::domain::{NormalSearch, ViewState};

use super::layout::content_height;

const HELP_TEXT: &str = "\
bmd — Markdown viewer (press h or Esc to close)

Navigation    j/k ↓↑ scroll   d/u PgDn/PgUp half page   g/G top/bottom   wheel scroll
Links         n/N next/prev   o/Enter open   (auto-scrolls into view)
Search        / forward   ? backward   n/N next/prev match   Esc clear
Tasks         click checkbox   x toggle at top line
Other         h help   q/Esc/Ctrl-c quit";

pub(crate) fn format_status_bar(
    source_label: Option<&str>,
    view_state: &ViewState,
    max_scroll: usize,
    status_message: Option<&str>,
) -> Line<'static> {
    if let Some(msg) = status_message {
        return Line::from(vec![
            Span::styled(
                msg.to_string(),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                trailing_status(source_label, view_state, max_scroll),
                dim_style(),
            ),
        ]);
    }

    Line::from(vec![Span::styled(
        trailing_status(source_label, view_state, max_scroll),
        dim_style(),
    )])
}

fn trailing_status(
    source_label: Option<&str>,
    view_state: &ViewState,
    max_scroll: usize,
) -> String {
    let mut parts = Vec::new();
    parts.push(
        source_label
            .map(ToString::to_string)
            .unwrap_or_else(|| "(stdin)".to_string()),
    );

    let offset = view_state.scroll().offset();
    let pct = if max_scroll == 0 {
        100
    } else {
        ((offset as f64 / max_scroll as f64) * 100.0).round() as u32
    };
    parts.push(format!("{pct}%"));

    if let NormalSearch::Active {
        matches,
        current_index,
        query,
        ..
    } = view_state.normal_search()
    {
        let total = matches.len();
        let current = if total == 0 { 0 } else { current_index + 1 };
        parts.push(format!("{current}/{total} '{}'", query.as_str()));
    }

    if let Some(id) = view_state.selected_link() {
        parts.push(format!("link #{}", id.0));
    }

    parts.join("  |  ")
}

fn dim_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub(crate) fn draw_status_bar(frame: &mut Frame, area: Rect, line: Line<'_>) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let block = Block::default().style(Style::default().bg(Color::Black));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let para = Paragraph::new(line).wrap(Wrap { trim: true });
    frame.render_widget(para, inner);
}

pub(crate) fn draw_help_overlay(frame: &mut Frame, area: Rect) {
    let popup = super::layout::centered_rect(70, 70, area);
    frame.render_widget(Clear, popup);
    let block = Block::bordered().title("Help");
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let para = Paragraph::new(HELP_TEXT);
    frame.render_widget(para, inner);
}

pub(crate) fn scroll_link_target(
    line_offset: usize,
    max_scroll: usize,
    view_state: &ViewState,
) -> usize {
    let visible = content_height(view_state.terminal_size().height(), view_state.mode()) as usize;
    let margin = visible / 4;
    line_offset.saturating_sub(margin).min(max_scroll)
}
