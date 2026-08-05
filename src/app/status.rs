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
bmd — Markdown viewer (press H or Esc to close)

Navigation    j/k ↓↑ scroll   d/u PgDn/PgUp half page   g/G top/bottom   wheel scroll
Headings      [/] prev/next section   #anchor links jump in-document
Outline       t toggle sidebar   j/k when focused   Enter/o jump   Esc unfocus   click entry
Marks         ma set mark   'a jump to mark
Links         n/p/N next/prev in viewport   o/Enter open   click link   O step back or close preview   Esc reset stack
Open/close    o opens links and previews   O closes what o opened (preview overlay) or steps back one navigation level
Search        / forward   ? backward   n/p/N next/prev match   Esc clear
Yank          y then l link / h heading / c code / y selection   (y alone copies active selection)
Preview       Ctrl+pinch or +/- zoom   0 reset zoom   o/Esc close   click outside to close
Tasks         click checkbox   x toggle at top line
Selection     drag to select (auto-copy on release)   y copy again when selected
Other         h help   H close help   q/Ctrl-c quit";

/// Inputs for the bottom status line.
pub(crate) struct StatusBarInput<'a> {
    pub source_label: Option<&'a str>,
    pub view_state: &'a ViewState,
    pub max_scroll: usize,
    pub doc_stack_depth: usize,
    pub status_message: Option<&'a str>,
    pub outline_visible: bool,
    pub outline_focused: bool,
    pub pending_prompt: Option<&'a str>,
}

pub(crate) fn format_status_bar(input: StatusBarInput<'_>) -> Line<'static> {
    if let Some(msg) = input.status_message {
        return Line::from(vec![
            Span::styled(
                msg.to_string(),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(trailing_status(&input), dim_style()),
        ]);
    }

    Line::from(vec![Span::styled(trailing_status(&input), dim_style())])
}

fn trailing_status(input: &StatusBarInput<'_>) -> String {
    let mut parts = Vec::new();
    parts.push(
        input
            .source_label
            .map(ToString::to_string)
            .unwrap_or_else(|| "(stdin)".to_string()),
    );

    let offset = input.view_state.scroll().offset().min(input.max_scroll);
    let pct = if input.max_scroll == 0 {
        100
    } else {
        ((offset as f64 / input.max_scroll as f64) * 100.0).round() as u32
    };
    parts.push(format!("{pct}%"));

    if let Some(prompt) = input.pending_prompt {
        parts.push(prompt.to_string());
    }

    if input.outline_visible {
        parts.push(if input.outline_focused {
            "outline*".to_string()
        } else {
            "outline".to_string()
        });
    }

    if let NormalSearch::Active(active) = input.view_state.normal_search() {
        let total = active.matches().len();
        let current = if total == 0 {
            0
        } else {
            active.current_index() + 1
        };
        parts.push(format!(
            "{}/{} '{}'",
            current,
            total,
            active.query().as_str()
        ));
    }

    if let Some(id) = input.view_state.selected_link() {
        parts.push(format!("link #{}", id.0));
    }

    if let Some(id) = input.view_state.selected_footnote() {
        parts.push(format!("footnote #{}", id.0));
    }

    if input.doc_stack_depth > 0 {
        parts.push(format!("doc+{}", input.doc_stack_depth));
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
