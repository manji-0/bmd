use super::App;
use super::scroll::{is_line_scroll_key, line_scroll_command};
use crate::domain::{
    Document, Heading, HeadingLevel, Inline, Link, LinkKind, LinkUrl, SearchDirection, TerminalSize,
};
use crate::keymap::Command;
use crate::parse::parse;
use crate::render::{CachedMarkdownView, DocumentRenderCache, RenderContext};
use crossterm::event::KeyCode;
use ratatui_image::picker::Picker;

fn test_terminal_size() -> TerminalSize {
    TerminalSize::new(80, 30).unwrap()
}

fn new_test_app(document: Document) -> App {
    App::new_with_terminal_size(document, Picker::halfblocks(), None, test_terminal_size()).unwrap()
}

fn dummy_document() -> Document {
    Document {
        blocks: vec![crate::domain::Block::Heading(Heading {
            level: HeadingLevel::H1,
            content: vec![Inline::Text("Hello".to_string())],
        })],
        links: vec![Link {
            url: LinkUrl::new("https://example.com".to_string()).unwrap(),
            title: None,
            kind: LinkKind::Web,
        }],
        mermaid_diagrams: vec![],
    }
}

#[test]
fn line_scroll_key_helpers() {
    assert!(is_line_scroll_key(&KeyCode::Char('j')));
    assert!(is_line_scroll_key(&KeyCode::Up));
    assert!(!is_line_scroll_key(&KeyCode::Char('d')));
    assert_eq!(
        line_scroll_command(&KeyCode::Char('j')),
        Command::ScrollDown
    );
    assert_eq!(line_scroll_command(&KeyCode::Char('k')), Command::ScrollUp);
}

#[test]
fn open_link_without_selection_records_error() {
    let doc = dummy_document();
    let mut app = new_test_app(doc);
    app.open_current_link();
    assert!(app.error_message.is_some());
}

#[test]
fn preview_opens_and_closes() {
    let doc = parse("```mermaid\ngraph TD; A-->B;\n```").unwrap();
    let mut app = new_test_app(doc);

    app.next_link();
    app.open_current_link();
    assert!(app.view_state.mode().is_preview());

    app.close_preview();
    assert!(app.view_state.mode().is_normal());
}

#[test]
fn renders_document_to_test_backend() {
    let input = "# Title\n\nA paragraph with **bold** and [a link](https://example.com).\n\n| Name | Value |\n|------|-------|\n| A    | 1     |\n";
    let doc = parse(input).unwrap();
    let app = new_test_app(doc);

    let backend = ratatui::backend::TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let ctx = RenderContext::new(
                &app.theme,
                &app.syntax_assets.syntax_set,
                app.syntax_assets.theme(),
                &app.rendered,
                &app.document.links,
                &app.view_state,
                app.show_terminal_images,
            );
            let width = app.view_state.terminal_size().width();
            let mut cache = DocumentRenderCache::default();
            cache.ensure(&app.document, &ctx, &app.view_state, width);
            let widget = CachedMarkdownView {
                cache: &cache,
                scroll: app.view_state.scroll().offset() as f32,
            };
            f.render_widget(widget, f.area());
        })
        .unwrap();

    assert!(!terminal.backend().buffer().content().is_empty());
}

#[test]
fn half_page_scroll_uses_faster_animation() {
    let mut input = String::from("# Title\n\n");
    for i in 0..100 {
        input.push_str(&format!("paragraph {}\n\n", i));
    }
    let doc = parse(&input).unwrap();
    let mut app = new_test_app(doc);

    app.half_page_down();
    assert_eq!(
        app.scroll_anim_speed,
        super::scroll::HALF_PAGE_SCROLL_ANIM_SPEED
    );
}

#[test]
fn jump_commands_snap_visual_scroll() {
    let mut input = String::from("# Title\n\n");
    for i in 0..100 {
        input.push_str(&format!("paragraph {}\n\n", i));
    }
    let doc = parse(&input).unwrap();
    let mut app = new_test_app(doc);

    app.scroll_down(50);
    assert_ne!(app.view_state.scroll().offset(), 0);
    assert_ne!(app.scroll_visual.round() as usize, 0);

    app.jump_to_top();
    assert_eq!(app.view_state.scroll().offset(), 0);
    assert_eq!(app.scroll_visual.round() as usize, 0);

    app.jump_to_bottom();
    let max = app.max_scroll();
    assert_eq!(app.view_state.scroll().offset(), max);
    assert_eq!(app.scroll_visual.round() as usize, max);
}

#[test]
fn terminal_images_defer_until_scroll_idle() {
    use std::time::{Duration, Instant};

    use super::scroll::IMAGE_REENABLE_DELAY;

    let mut input = String::from("# Title\n\n");
    for i in 0..50 {
        input.push_str(&format!("paragraph {}\n\n", i));
    }
    let doc = parse(&input).unwrap();
    let mut app = new_test_app(doc);
    assert!(app.show_terminal_images);

    let t0 = Instant::now();
    app.scroll_down(4);
    assert!(app.update_terminal_image_visibility(t0));
    assert!(!app.show_terminal_images);

    assert!(!app.update_terminal_image_visibility(t0));
    assert!(!app.show_terminal_images);

    let after_idle = t0 + IMAGE_REENABLE_DELAY + Duration::from_millis(1);
    assert!(app.update_terminal_image_visibility(after_idle));
    assert!(app.show_terminal_images);
}

#[test]
fn short_document_cannot_scroll() {
    let input = "# Title\n\nA paragraph.\n";
    let doc = parse(input).unwrap();
    let app = new_test_app(doc);
    assert_eq!(app.max_scroll(), 0);
}

#[test]
fn search_command_flow_scrolls_to_match() {
    let mut input = String::from("# Alpha\n\n");
    for i in 0..100 {
        input.push_str(&format!("paragraph {}\n\n", i));
    }
    input.push_str("target line\n");
    let doc = parse(&input).unwrap();
    let mut app = new_test_app(doc);

    app.start_search(SearchDirection::Forward);
    assert!(app.view_state.mode().is_search_input());

    for c in "target".chars() {
        app.append_search_input(c);
    }
    app.confirm_search();

    assert!(app.view_state.is_search_active());
    let max_scroll = app.max_scroll();
    assert!(max_scroll > 0);
    assert!(app.view_state.scroll().offset() > 0);

    let before = app.view_state.scroll().offset();
    app.next_search_match();
    assert_eq!(app.view_state.scroll().offset(), before);
}
