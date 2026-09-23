//! Status bar text and help overlay content.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Wrap},
};

use crate::domain::{Document, NavTarget, NormalSearch, ViewState};

use super::layout::content_height;

/// Max characters for a selected link URL in the status bar.
const SELECTED_URL_MAX_CHARS: usize = 48;

const HELP_TEXT: &str = "\
bmd — Markdown viewer (press H or Esc to close)

Navigation    j/k ↓↑ scroll   d/u PgDn/PgUp half page   g/G top/bottom   wheel scroll
Headings      [/] prev/next section   #anchor links jump in-document
Outline       t toggle sidebar   tracks scroll   click entry to jump
Marks         ma set mark   'a jump to mark
Links         n/N next/prev (scrolls)   Tab/Shift-Tab   o/Enter open   click link
Back          Esc / O  one step: close preview or back one jump
              status: back → file.md  or  back → previous position   (repeat to go further)
Search        / forward   ? backward   live count + highlight while typing   Enter jump   n/N next/prev   Esc clear
Yank          y then l link / h heading / c code / y selection   (y alone copies active selection)
Preview       Ctrl+pinch or +/- zoom   0 reset zoom   Esc/o/O close   click outside to close
Tasks         click checkbox   x toggle at top line
Selection     drag to select (highlight only)   y copy when selected
Other         h help   H close help   q/Ctrl-c quit";

/// Inputs for the bottom status line.
pub(crate) struct StatusBarInput<'a> {
    pub source_label: Option<&'a str>,
    pub document: &'a Document,
    pub view_state: &'a ViewState,
    pub max_scroll: usize,
    pub doc_stack_depth: usize,
    pub status_message: Option<&'a str>,
    pub outline_visible: bool,
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
        parts.push("outline".to_string());
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

    if let Some(selected) = selected_nav_status(input.document, input.view_state) {
        parts.push(selected);
    }

    if input.doc_stack_depth > 0 {
        parts.push(format!("doc+{}", input.doc_stack_depth));
    }

    parts.join("  |  ")
}

/// Human-readable status text for the selected link or footnote.
fn selected_nav_status(document: &Document, view_state: &ViewState) -> Option<String> {
    match view_state.selected_nav()? {
        NavTarget::Link(id) => {
            let link = document.links().get(id.0)?;
            Some(truncate_status(link.url.as_str(), SELECTED_URL_MAX_CHARS))
        }
        NavTarget::Footnote(id) => {
            let footnote = document.footnotes().get(id.0)?;
            Some(format!("footnote [{}]", footnote.label))
        }
    }
}

fn truncate_status(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let truncated: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{truncated}…")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FootnoteId, LinkId, TerminalSize};
    use crate::parse::parse;

    fn status_text(document: &Document, view_state: &ViewState) -> String {
        format_status_bar(StatusBarInput {
            source_label: Some("doc.md"),
            document,
            view_state,
            max_scroll: 0,
            doc_stack_depth: 0,
            status_message: None,
            outline_visible: false,
            pending_prompt: None,
        })
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect()
    }

    #[test]
    fn status_shows_selected_link_url_not_opaque_id() {
        let document = parse("[docs](https://example.com/path)\n").unwrap();
        let view_state =
            ViewState::new(TerminalSize::new(80, 24).unwrap()).with_selected_link(LinkId(0));
        let text = status_text(&document, &view_state);
        assert!(
            text.contains("https://example.com/path"),
            "expected URL in status, got: {text}"
        );
        assert!(
            !text.contains("link #0"),
            "opaque link id should not appear: {text}"
        );
    }

    #[test]
    fn status_truncates_long_selected_link_url() {
        let long = format!("https://example.com/{}", "a".repeat(80));
        let document = parse(&format!("[x]({long})\n")).unwrap();
        let view_state =
            ViewState::new(TerminalSize::new(80, 24).unwrap()).with_selected_link(LinkId(0));
        let text = status_text(&document, &view_state);
        assert!(text.contains('…'), "expected truncated URL: {text}");
        assert!(
            !text.contains(&long),
            "full long URL should not appear: {text}"
        );
    }

    #[test]
    fn status_shows_footnote_label_not_opaque_id() {
        let document = parse("See note.[^note]\n\n[^note]: Footnote body.\n").unwrap();
        let view_state = ViewState::new(TerminalSize::new(80, 24).unwrap())
            .with_selected_footnote(FootnoteId(0));
        let text = status_text(&document, &view_state);
        assert!(
            text.contains("footnote [note]"),
            "expected footnote label in status, got: {text}"
        );
        assert!(
            !text.contains("footnote #0"),
            "opaque footnote id should not appear: {text}"
        );
    }
}
