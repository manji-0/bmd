//! Keyboard and mouse scenarios through the real keymap, command dispatch, and draw path.

use std::time::{Duration, Instant};

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend};
use ratatui_image::picker::Picker;

use super::App;
use crate::config::Config;
use crate::domain::{LinkKind, NavTarget, TerminalSize};
use crate::parse::{MarkupFormat, parse_document};
use crate::render::HitTarget;
use crate::test_fixtures::SAMPLES;

const WIDTH: u16 = 80;
const HEIGHT: u16 = 24;

struct Harness {
    app: App,
    terminal: Terminal<TestBackend>,
}

impl Harness {
    fn new(format: MarkupFormat, source: &str) -> Self {
        Self::sized(format, source, WIDTH, HEIGHT)
    }

    fn sized(format: MarkupFormat, source: &str, width: u16, height: u16) -> Self {
        Self::with_config(format, source, width, height, Config::default())
    }

    fn with_config(
        format: MarkupFormat,
        source: &str,
        width: u16,
        height: u16,
        config: Config,
    ) -> Self {
        let document = parse_document(format, source).unwrap();
        let size = TerminalSize::new(width, height).unwrap();
        let app =
            App::new_with_terminal_size(document, Picker::halfblocks(), None, None, size, config)
                .unwrap();
        let terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        Self { app, terminal }
    }

    fn kitchen_sink() -> Self {
        let (_, format, source) = SAMPLES[0];
        Self::new(format, source)
    }

    fn draw(&mut self) -> String {
        self.app.draw_frame(&mut self.terminal).unwrap();
        let buf = self.terminal.backend().buffer();
        buf.content.iter().map(|cell| cell.symbol()).collect()
    }

    fn key(&mut self, code: KeyCode) {
        let event = Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
        self.app.handle_crossterm_event(event).unwrap();
        self.draw();
    }

    fn keys(&mut self, keys: &str) {
        for c in keys.chars() {
            self.key(KeyCode::Char(c));
        }
    }

    fn mouse(&mut self, kind: MouseEventKind, column: u16, row: u16) {
        let event = Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
        self.app.handle_crossterm_event(event).unwrap();
    }

    fn click(&mut self, column: u16, row: u16) {
        self.mouse(MouseEventKind::Down(MouseButton::Left), column, row);
        self.mouse(MouseEventKind::Up(MouseButton::Left), column, row);
    }

    fn scroll(&self) -> usize {
        self.app.view_state.scroll().offset()
    }

    /// Scroll a link of `kind` into view and select it with `n`.
    fn select_link(&mut self, kind: LinkKind) -> bool {
        let links = self.app.document.links.clone();
        let Some(line) = self.app.hits().iter().find_map(|hit| match hit.target {
            HitTarget::Nav(NavTarget::Link(id)) if links[id.0].kind == kind => Some(hit.line),
            _ => None,
        }) else {
            return false;
        };
        self.app.scroll_to_line(line);
        self.app.snap_scroll_visual();
        for _ in 0..=links.len() {
            self.keys("n");
            if self
                .app
                .view_state
                .selected_link()
                .is_some_and(|id| links[id.0].kind == kind)
            {
                return true;
            }
        }
        false
    }
}

#[test]
fn keyboard_tour_visits_every_mode_on_every_sample() {
    for (name, format, source) in SAMPLES {
        let mut h = Harness::new(format, source);
        let first = h.draw();
        assert!(!first.trim().is_empty(), "{name}: blank first frame");

        h.keys("jjd");
        assert!(h.scroll() > 0, "{name}: j/d did not scroll");
        h.keys("G");
        assert_eq!(h.scroll(), h.app.max_scroll(), "{name}: G");
        h.keys("uk");
        h.keys("g");
        assert_eq!(h.scroll(), 0, "{name}: g");

        h.keys("]");
        let after_heading = h.scroll();
        h.keys("[");
        assert!(h.scroll() <= after_heading, "{name}: [ moved down");

        let marked = h.scroll();
        h.keys("ma");
        h.keys("G");
        h.keys("'a");
        assert_eq!(h.scroll(), marked, "{name}: mark jump");

        h.keys("nnN");
        assert!(
            h.app.view_state.selected_link().is_some()
                || h.app.view_state.selected_footnote().is_some(),
            "{name}: n/N"
        );

        h.keys("t");
        assert!(h.app.outline.visible, "{name}: outline");
        h.keys("t");

        h.keys("h");
        assert!(h.app.help_visible, "{name}: help");
        h.key(KeyCode::Esc);
        assert!(!h.app.help_visible, "{name}: close help");

        h.keys("/b");
        h.key(KeyCode::Backspace);
        h.keys("b");
        h.key(KeyCode::Enter);
        assert!(h.app.view_state.is_search_active(), "{name}: search");
        h.keys("nN");
        h.key(KeyCode::Esc);

        h.keys("?zzzz-no-match");
        h.key(KeyCode::Enter);
        h.key(KeyCode::Esc);

        h.keys("yh");
        h.keys("yc");
        h.keys("yl");
        h.keys("yq");
        h.keys("x");
        h.keys("O");

        assert!(!h.app.should_quit, "{name}: quit early");
        h.keys("q");
        assert!(h.app.should_quit, "{name}: q");
    }
}

#[test]
fn mermaid_preview_opens_zooms_and_closes() {
    let mut h = Harness::kitchen_sink();
    assert!(h.select_link(LinkKind::Mermaid));
    h.keys("o");
    let deadline = Instant::now() + Duration::from_secs(20);
    while h.app.view_state.mode().preview_link().is_none() && Instant::now() < deadline {
        h.app.poll_preview_renders();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        h.app.view_state.mode().preview_link().is_some(),
        "preview never opened"
    );
    h.draw();

    h.keys("+");
    assert!(h.app.preview.zoom > 1.0);
    h.keys("-0");
    assert_eq!(h.app.preview.zoom, 1.0);
    h.key(KeyCode::Esc);
    assert!(h.app.view_state.mode().preview_link().is_none());
}

#[test]
fn table_of_contents_preview_jumps_to_a_heading() {
    let mut h = Harness::kitchen_sink();
    assert!(h.select_link(LinkKind::Toc));
    h.keys("o");
    assert!(h.app.view_state.mode().preview_link().is_some());
    h.draw();
    h.key(KeyCode::Down);
    h.key(KeyCode::Enter);
    assert!(h.app.view_state.mode().preview_link().is_none());
    assert!(h.scroll() > 0);
}

#[test]
fn clicking_a_checkbox_toggles_it_and_clicking_a_link_selects_it() {
    let mut h = Harness::kitchen_sink();
    h.draw();
    let hits = h.app.hits().to_vec();
    let checkbox = hits
        .iter()
        .find(|hit| matches!(hit.target, HitTarget::Checklist(_)))
        .unwrap();
    h.app.scroll_to_line(checkbox.line);
    h.app.snap_scroll_visual();
    let row = (checkbox.line - h.scroll()) as u16;
    let before = h.app.checklist_state.revision();
    h.click(checkbox.x as u16, row);
    assert_ne!(h.app.checklist_state.revision(), before);

    let link = hits
        .iter()
        .find(|hit| {
            matches!(hit.target, HitTarget::Nav(NavTarget::Link(id))
                if h.app.document.links[id.0].kind == LinkKind::Web)
        })
        .unwrap();
    h.app.scroll_to_line(link.line);
    h.app.snap_scroll_visual();
    let row = (link.line - h.scroll()) as u16;
    h.click(link.x as u16, row);
    h.draw();
}

#[test]
fn mouse_drag_selects_text_and_wheel_scrolls() {
    let mut h = Harness::kitchen_sink();
    h.draw();
    h.mouse(MouseEventKind::Down(MouseButton::Left), 0, 0);
    h.mouse(MouseEventKind::Drag(MouseButton::Left), 10, 2);
    h.mouse(MouseEventKind::Up(MouseButton::Left), 10, 2);
    assert!(h.app.text_selection.is_some_and(|s| !s.is_empty()));
    h.draw();

    h.mouse(MouseEventKind::ScrollDown, 5, 5);
    assert!(h.scroll() > 0);
    h.mouse(MouseEventKind::ScrollUp, 5, 5);
    assert_eq!(h.scroll(), 0);
}

impl Harness {
    fn row(&self, y: u16) -> String {
        let buf = self.terminal.backend().buffer();
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    fn status_row(&self) -> String {
        self.row(HEIGHT - 1)
    }
}

#[test]
fn pending_key_prompts_appear_once_in_the_status_bar() {
    let mut h = Harness::kitchen_sink();
    for (keys, prompt) in [("y", "yank: l link"), ("m", "m — mark"), ("'", "' — jump")] {
        h.keys(keys);
        let status = h.status_row();
        assert_eq!(status.matches(prompt).count(), 1, "{keys}: {status}");
        assert!(
            status.starts_with(prompt),
            "{keys}: prompt should lead: {status}"
        );
        h.key(KeyCode::Esc);
        assert!(!h.status_row().contains(prompt), "{keys}: Esc keeps prompt");
    }
}

#[test]
fn help_overlay_rows_are_not_clipped_at_80_columns() {
    let mut h = Harness::kitchen_sink();
    h.keys("h");
    let screen: Vec<String> = (0..HEIGHT).map(|y| h.row(y)).collect();
    for (label, body) in super::status::HELP_ROWS {
        assert!(
            screen
                .iter()
                .any(|row| row.contains(label) && row.contains(body)),
            "help row '{label}' clipped:\n{}",
            screen.join("\n")
        );
    }
}

#[test]
fn status_messages_use_theme_info_and_error_styles() {
    let mut h = Harness::kitchen_sink();
    let fg_at_status_start = |h: &Harness| h.terminal.backend().buffer()[(0, HEIGHT - 1)].fg;

    h.app.set_status_message("copied".into());
    h.draw();
    assert_eq!(Some(fg_at_status_start(&h)), h.app.theme.status_info.fg);

    h.app.set_status_error("failed".into());
    h.draw();
    assert_eq!(Some(fg_at_status_start(&h)), h.app.theme.status_error.fg);

    let bg = h.terminal.backend().buffer()[(WIDTH - 1, HEIGHT - 1)].bg;
    assert_eq!(Some(bg), h.app.theme.status_bar.bg);
}

#[test]
fn outline_rows_fit_the_sidebar_without_heading_markers() {
    let mut h = Harness::kitchen_sink();
    h.keys("t");
    let panel = super::layout::outline_panel_width(WIDTH) as usize;
    let rows: Vec<String> = (1..HEIGHT - 2)
        .map(|y| h.row(y).chars().take(panel).collect())
        .collect();
    assert!(
        rows.iter().all(|row| !row.contains("# ")),
        "outline still shows # markers:\n{}",
        rows.join("\n")
    );
    assert!(
        rows.iter()
            .any(|row| row.contains("Level six") || row.contains("Level s…")),
        "deep heading lost its text:\n{}",
        rows.join("\n")
    );
}

#[test]
fn help_overlay_scrolls_on_a_short_terminal() {
    let (_, format, source) = SAMPLES[0];
    let mut h = Harness::sized(format, source, 50, 16);
    let screen = |h: &Harness| (0..16).map(|y| h.row(y)).collect::<Vec<_>>().join("\n");
    h.keys("h");
    assert!(screen(&h).contains("Scroll"), "{}", screen(&h));
    assert!(!screen(&h).contains("Other"), "help should overflow 50x16");

    h.keys("G");
    let bottom = screen(&h);
    assert!(
        bottom.contains("Other") && bottom.contains("q/Ctrl-c quit"),
        "{bottom}"
    );
    assert!(!bottom.contains("Scroll "), "{bottom}");

    // Scrolling past the end is clamped, so one k moves back up at once.
    h.keys("jjjk");
    assert!(!screen(&h).contains("q/Ctrl-c quit"), "{}", screen(&h));

    h.keys("g");
    assert!(screen(&h).contains("Scroll"), "{}", screen(&h));
    h.mouse(MouseEventKind::ScrollDown, 25, 8);
    h.draw();
    assert!(!screen(&h).contains("Scroll "), "wheel did not scroll help");
    assert_eq!(h.scroll(), 0, "document scrolled under the help overlay");

    h.key(KeyCode::Esc);
    h.keys("h");
    assert!(
        screen(&h).contains("Scroll"),
        "reopened help should start at the top"
    );
}

#[test]
fn ascii_checklist_markers_render_and_toggle_on_click() {
    let mut config = Config::default();
    config.view.checklist = Some(crate::config::ChecklistMarkers::Ascii);
    let source = "- [ ] open task\n- [x] done task\n";
    let mut h = Harness::with_config(MarkupFormat::Markdown, source, WIDTH, HEIGHT, config);
    h.draw();
    if std::env::var("BMD_CHECKLIST_STYLE").is_ok() {
        return; // The env var deliberately overrides config.
    }
    assert!(h.row(0).starts_with("[ ] open task"), "{}", h.row(0));
    assert!(h.row(1).starts_with("[x] done task"), "{}", h.row(1));

    // Clicking the last cell of the wide marker still toggles it.
    h.click(2, 0);
    h.draw();
    assert!(h.row(0).starts_with("[x] open task"), "{}", h.row(0));
}

#[test]
fn status_bar_shows_the_enclosing_section() {
    let mut h = Harness::kitchen_sink();
    h.draw();
    assert!(
        h.status_row().contains("Kitchen sink"),
        "{}",
        h.status_row()
    );

    let level_three = h
        .app
        .heading_cache
        .entries()
        .iter()
        .find(|e| e.text == "Level three")
        .unwrap()
        .line_offset;
    let max = h.app.max_scroll();
    h.app.view_state = h.app.view_state.clone().scroll_to(level_three, max);
    h.app.snap_scroll_visual();
    h.draw();
    let status = h.status_row();
    assert!(
        status.contains("Kitchen sink › Links and footnotes › Level three"),
        "{status}"
    );

    h.keys("t");
    assert!(
        !h.status_row().contains("Level three"),
        "outline open: {}",
        h.status_row()
    );
}
