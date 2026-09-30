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
        let document = parse_document(format, source).unwrap();
        let size = TerminalSize::new(WIDTH, HEIGHT).unwrap();
        let app = App::new_with_terminal_size(
            document,
            Picker::halfblocks(),
            None,
            None,
            size,
            Config::default(),
        )
        .unwrap();
        let terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
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
