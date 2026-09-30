//! Status bar text and help overlay content.

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Clear, Padding, Paragraph},
};

use crate::domain::{Document, NavTarget, NormalSearch, ViewState};
use crate::render::Theme;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::layout::content_height;

/// Status priority of the section breadcrumb, which shrinks before it is dropped.
const SECTION_PRIORITY: u8 = 3;
/// Narrowest the breadcrumb gets before it is dropped instead.
const SECTION_MIN_WIDTH: usize = 16;

/// Max characters for a selected link URL in the status bar.
const SELECTED_URL_MAX_CHARS: usize = 48;

/// Help rows as (section, bindings). Each row fits in 76 columns so the
/// overlay reads without wrapping on an 80-column terminal.
pub(crate) const HELP_ROWS: &[(&str, &str)] = &[
    (
        "Scroll",
        "j/k ↓/↑ line   d/u PgDn/PgUp half page   g/G top/bottom",
    ),
    (
        "Sections",
        "[/] prev/next heading   t outline   click outline entry",
    ),
    ("Marks", "ma set mark (a-z)   'a jump to mark"),
    (
        "Links",
        "n/N Tab/S-Tab next/prev   o/Enter open   click a link",
    ),
    (
        "Back",
        "Esc/O close preview, or back one jump (repeat to go further)",
    ),
    (
        "Search",
        "/ forward   ? backward   Enter jump   n/N next/prev   Esc clear",
    ),
    ("Yank", "y then l link / h heading / c code / y selection"),
    ("Select", "drag to select   y copy selection"),
    (
        "Preview",
        "+/- or Ctrl+pinch zoom   0 reset   Esc/o/O or click outside close",
    ),
    (
        "Other",
        "click checkbox or footnote   h help   q/Ctrl-c quit",
    ),
];

/// Width of the section column in the help overlay.
const HELP_LABEL_WIDTH: usize = 10;

/// Inputs for the bottom status line.
pub(crate) struct StatusBarInput<'a> {
    pub source_label: Option<&'a str>,
    /// Breadcrumb of headings enclosing the scroll position.
    pub section: Option<&'a str>,
    pub document: &'a Document,
    pub view_state: &'a ViewState,
    pub max_scroll: usize,
    pub doc_stack_depth: usize,
    pub status_message: Option<&'a str>,
    pub outline_visible: bool,
    pub pending_prompt: Option<&'a str>,
    pub status_is_error: bool,
    pub theme: &'a Theme,
    /// Columns available for the whole status line.
    pub width: u16,
}

pub(crate) fn format_status_bar(input: StatusBarInput<'_>) -> Line<'static> {
    let theme = input.theme;
    // A pending key prompt or a message leads the line so it survives clipping
    // on narrow terminals; the trailing position summary is clipped first.
    let lead = match (input.pending_prompt, input.status_message) {
        (Some(prompt), _) => Some(Span::styled(prompt.to_string(), theme.status_info)),
        (None, Some(msg)) => {
            let style = if input.status_is_error {
                theme.status_error
            } else {
                theme.status_info
            };
            Some(Span::styled(msg.to_string(), style))
        }
        (None, None) => None,
    };
    let lead_width = lead
        .as_ref()
        .map_or(0, |span| span.width() + STATUS_SEPARATOR_GAP);
    let budget = usize::from(input.width).saturating_sub(lead_width);
    let trailing = Span::raw(trailing_status(&input, budget));
    match lead {
        Some(lead) => Line::from(vec![lead, Span::raw("  "), trailing]),
        None => Line::from(trailing),
    }
}

/// Separator between status segments.
const STATUS_SEPARATOR: &str = "  |  ";
/// Gap between a leading message and the position summary.
const STATUS_SEPARATOR_GAP: usize = 2;

/// Position summary, dropping low-priority segments until it fits `budget` columns.
fn trailing_status(input: &StatusBarInput<'_>, budget: usize) -> String {
    // (priority, text): higher priority survives longer on narrow terminals.
    let mut parts: Vec<(u8, String)> = Vec::new();
    let source = input.source_label.unwrap_or("(stdin)");
    parts.push((6, source.to_string()));

    if let Some(section) = input.section {
        parts.push((SECTION_PRIORITY, section.to_string()));
    }

    let offset = input.view_state.scroll().offset().min(input.max_scroll);
    let pct = if input.max_scroll == 0 {
        100
    } else {
        ((offset as f64 / input.max_scroll as f64) * 100.0).round() as u32
    };
    parts.push((7, format!("{pct}%")));

    if input.outline_visible {
        parts.push((1, "outline".to_string()));
    }

    if let NormalSearch::Active(active) = input.view_state.normal_search() {
        let total = active.matches().len();
        let current = if total == 0 {
            0
        } else {
            active.current_index() + 1
        };
        parts.push((
            5,
            format!("{}/{} '{}'", current, total, active.query().as_str()),
        ));
    }

    if let Some(selected) = selected_nav_status(input.document, input.view_state) {
        parts.push((4, selected));
    }

    if input.doc_stack_depth > 0 {
        parts.push((2, format!("doc+{}", input.doc_stack_depth)));
    }

    let joined_width = |parts: &[(u8, String)]| {
        let text: usize = parts.iter().map(|(_, text)| text.width()).sum();
        text + STATUS_SEPARATOR.len() * parts.len().saturating_sub(1)
    };
    while joined_width(&parts) > budget && parts.len() > 2 {
        let lowest = parts
            .iter()
            .enumerate()
            .min_by_key(|(_, (priority, _))| *priority)
            .map(|(index, _)| index)
            .expect("parts is non-empty");
        let (priority, text) = &parts[lowest];
        let overflow = joined_width(&parts) - budget;
        let shrunk = text.width().saturating_sub(overflow);
        if *priority == SECTION_PRIORITY && shrunk >= SECTION_MIN_WIDTH {
            // Keep the innermost heading; it is the one that changes as you read.
            parts[lowest].1 = truncate_start_to_width(text, shrunk);
            continue;
        }
        parts.remove(lowest);
    }
    let overflow = joined_width(&parts).saturating_sub(budget);
    if overflow > 0 {
        // Keep the end of the path: the file name tells documents apart.
        let keep = source.width().saturating_sub(overflow);
        parts[0].1 = truncate_start_to_width(source, keep);
    }

    parts
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>()
        .join(STATUS_SEPARATOR)
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

/// Shorten `text` to at most `max_chars` characters, ending with `…` when cut.
pub(crate) fn truncate_status(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let truncated: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{truncated}…")
}

/// Shorten `text` to at most `max_width` columns, ending with `…` when cut.
pub(crate) fn truncate_to_width(text: &str, max_width: usize) -> String {
    if text.width() <= max_width {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = c.width().unwrap_or(0);
        if used + w + 1 > max_width {
            break;
        }
        out.push(c);
        used += w;
    }
    if max_width > 0 {
        out.push('…');
    }
    out
}

/// Shorten `text` to at most `max_width` columns, starting with `…` when cut.
fn truncate_start_to_width(text: &str, max_width: usize) -> String {
    if text.width() <= max_width {
        return text.to_string();
    }
    let mut tail = Vec::new();
    let mut used = 0;
    for c in text.chars().rev() {
        let w = c.width().unwrap_or(0);
        if used + w + 1 > max_width {
            break;
        }
        tail.push(c);
        used += w;
    }
    let mut out = String::from(if max_width > 0 { "…" } else { "" });
    out.extend(tail.into_iter().rev());
    out
}

pub(crate) fn draw_status_bar(frame: &mut Frame, area: Rect, line: Line<'_>, theme: &Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let para = Paragraph::new(line).style(theme.status_bar);
    frame.render_widget(para, area);
}

/// Draw the help overlay starting at row `scroll` of its wrapped content.
/// Returns `scroll` clamped to the last page so key presses past the end
/// do not accumulate.
pub(crate) fn draw_help_overlay(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    scroll: usize,
) -> usize {
    let content_width = HELP_ROWS
        .iter()
        .map(|(_, body)| HELP_LABEL_WIDTH + body.width())
        .max()
        .unwrap_or(0);
    let width = (content_width as u16).saturating_add(4).min(area.width);
    let inner_width = usize::from(width.saturating_sub(4)).max(1);
    let lines: Vec<Line> = HELP_ROWS
        .iter()
        .flat_map(|(label, body)| help_row_lines(label, body, inner_width, theme))
        .collect();
    let wrapped_rows = lines.len();
    let height = (wrapped_rows as u16).saturating_add(2).min(area.height);
    let visible_rows = usize::from(height.saturating_sub(2));
    let max_scroll = wrapped_rows.saturating_sub(visible_rows);
    let scroll = scroll.min(max_scroll);
    let popup = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, popup);
    let mut block = Block::bordered()
        .border_style(theme.popup_border)
        .title(Span::styled(" bmd help — Esc to close ", theme.status_info))
        .padding(Padding::horizontal(1));
    if max_scroll > 0 {
        let hint = match (scroll > 0, scroll < max_scroll) {
            (true, true) => " ↑↓ j/k ",
            (true, false) => " ↑ k ",
            _ => " ↓ j ",
        };
        block = block.title_bottom(Line::styled(hint, theme.status_info).right_aligned());
    }
    let para = Paragraph::new(lines)
        .block(block)
        .scroll((scroll as u16, 0));
    frame.render_widget(para, popup);
    scroll
}

/// Word-wrap one help row to `width` columns. Continuation rows hang under
/// the bindings column when there is room for it.
fn help_row_lines(label: &str, body: &str, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let hang = if width > HELP_LABEL_WIDTH * 2 {
        HELP_LABEL_WIDTH
    } else {
        0
    };
    let mut rows: Vec<String> = vec![String::new()];
    let mut used = HELP_LABEL_WIDTH;
    for word in body.split(' ') {
        let row = rows.last_mut().expect("rows is non-empty");
        let sep = usize::from(!row.is_empty());
        if used + sep + word.width() > width && !row.is_empty() {
            rows.push(word.to_string());
            used = hang + word.width();
        } else {
            if sep == 1 {
                row.push(' ');
            }
            row.push_str(word);
            used += sep + word.width();
        }
    }
    rows.into_iter()
        .enumerate()
        .map(|(i, row)| {
            let lead = if i == 0 {
                Span::styled(format!("{label:<HELP_LABEL_WIDTH$}"), theme.status_info)
            } else {
                Span::raw(" ".repeat(hang))
            };
            Line::from(vec![lead, Span::styled(row, theme.text)])
        })
        .collect()
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
            section: None,
            document,
            view_state,
            max_scroll: 0,
            doc_stack_depth: 0,
            status_message: None,
            outline_visible: false,
            pending_prompt: None,
            status_is_error: false,
            theme: &Theme::default(),
            width: 200,
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

    #[test]
    fn narrow_status_drops_low_priority_segments_first() {
        let document = parse("[docs](https://example.com/path)\n").unwrap();
        let view_state =
            ViewState::new(TerminalSize::new(80, 24).unwrap()).with_selected_link(LinkId(0));
        let line = format_status_bar(StatusBarInput {
            source_label: Some("notes/readme.md"),
            section: Some("Guide › Install"),
            document: &document,
            view_state: &view_state,
            max_scroll: 0,
            doc_stack_depth: 2,
            status_message: None,
            outline_visible: true,
            pending_prompt: None,
            status_is_error: false,
            theme: &Theme::default(),
            width: 30,
        });
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.width() <= 30, "status overflows: {text}");
        assert!(
            text.contains("notes/readme.md") && text.contains("100%"),
            "{text}"
        );
        assert!(!text.contains("outline"), "lowest priority kept: {text}");
        assert!(
            !text.contains("Install"),
            "section outranks the file name: {text}"
        );
    }

    #[test]
    fn long_source_label_keeps_its_file_name() {
        let document = parse("text\n").unwrap();
        let view_state = ViewState::new(TerminalSize::new(80, 24).unwrap());
        let line = format_status_bar(StatusBarInput {
            source_label: Some("/very/long/path/to/some/deeply/nested/docs/readme.md"),
            section: None,
            document: &document,
            view_state: &view_state,
            max_scroll: 0,
            doc_stack_depth: 0,
            status_message: None,
            outline_visible: false,
            pending_prompt: None,
            status_is_error: false,
            theme: &Theme::default(),
            width: 24,
        });
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.width(), 24, "{text}");
        assert!(
            text.starts_with('…') && text.contains("readme.md"),
            "{text}"
        );
    }

    #[test]
    fn truncation_respects_display_width() {
        assert_eq!(truncate_to_width("abcdef", 6), "abcdef");
        assert_eq!(truncate_to_width("abcdef", 4), "abc…");
        assert_eq!(truncate_to_width("見出し", 4), "見…");
        assert_eq!(truncate_start_to_width("a/b/file.md", 8), "…file.md");
    }

    #[test]
    fn long_section_shrinks_to_its_innermost_heading() {
        let document = parse("text\n").unwrap();
        let view_state = ViewState::new(TerminalSize::new(80, 24).unwrap());
        let line = format_status_bar(StatusBarInput {
            source_label: Some("doc.md"),
            section: Some("Getting started › Installing on every platform › From source"),
            document: &document,
            view_state: &view_state,
            max_scroll: 0,
            doc_stack_depth: 0,
            status_message: None,
            outline_visible: false,
            pending_prompt: None,
            status_is_error: false,
            theme: &Theme::default(),
            width: 50,
        });
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.width() <= 50, "{text}");
        assert!(
            text.contains("…") && text.contains("› From source"),
            "{text}"
        );
    }
}
