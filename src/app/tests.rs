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
    App::new_with_terminal_size(
        document,
        Picker::halfblocks(),
        None,
        None,
        test_terminal_size(),
    )
    .unwrap()
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
    assert!(app.status_message.is_some());
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
                &app.syntax_assets,
                &app.rendered,
                &app.document.links,
                &app.view_state,
                app.show_terminal_images,
                &app.checklist_state,
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
fn next_link_only_selects_links_in_viewport() {
    let mut input = String::from("# Top\n\n[visible link](https://example.com/a)\n\n");
    for i in 0..80 {
        input.push_str(&format!("paragraph {}\n\n", i));
    }
    input.push_str("[off-screen link](https://example.com/b)\n");
    let doc = parse(&input).unwrap();
    let mut app = new_test_app(doc);
    let scroll_before = app.view_state.scroll().offset();

    app.next_link();
    assert_eq!(app.view_state.scroll().offset(), scroll_before);
    assert_eq!(
        app.view_state.selected_link(),
        Some(crate::domain::LinkId(0))
    );

    app.next_link();
    assert_eq!(
        app.view_state.selected_link(),
        Some(crate::domain::LinkId(0))
    );
}

#[test]
fn next_heading_scrolls_to_later_section() {
    let mut input = String::from("# Top\n\n");
    for i in 0..80 {
        input.push_str(&format!("paragraph {}\n\n", i));
    }
    input.push_str("## Bottom section\n\n");
    let doc = parse(&input).unwrap();
    let mut app = new_test_app(doc);

    app.next_heading();
    assert!(app.view_state.scroll().offset() > 0);
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

#[test]
fn anchor_navigation_stack_push_pop_and_reset() {
    let mut input = String::from("# Top\n\n");
    for i in 0..60 {
        input.push_str(&format!("paragraph {}\n\n", i));
    }
    input.push_str("## Middle\n\n");
    for i in 0..60 {
        input.push_str(&format!("filler {}\n\n", i));
    }
    input.push_str("## Bottom section\n\n");
    input.push_str("[go middle](#middle)\n\n[go bottom](#bottom-section)\n");
    let doc = parse(&input).unwrap();
    let mut app = new_test_app(doc);

    app.view_state = app
        .view_state
        .clone()
        .select_next_link_in(&[crate::domain::LinkId(0)]);
    let before_first = app.view_state.scroll().offset();
    app.open_current_link();
    let at_middle = app.view_state.scroll().offset();
    assert!(at_middle > before_first);
    assert!(!app.nav_stack.is_empty());

    app.view_state = app
        .view_state
        .clone()
        .select_next_link_in(&[crate::domain::LinkId(1)]);
    let before_second = app.view_state.scroll().offset();
    assert_eq!(before_second, at_middle);
    app.open_current_link();
    let at_bottom = app.view_state.scroll().offset();
    assert!(at_bottom > before_second);

    app.nav_back();
    assert_eq!(app.view_state.scroll().offset(), before_second);

    app.nav_back();
    assert_eq!(app.view_state.scroll().offset(), before_first);
    assert!(app.nav_stack.is_empty());

    app.view_state = app
        .view_state
        .clone()
        .select_next_link_in(&[crate::domain::LinkId(0)]);
    app.open_current_link();
    app.view_state = app
        .view_state
        .clone()
        .select_next_link_in(&[crate::domain::LinkId(1)]);
    app.open_current_link();
    assert!(app.view_state.scroll().offset() > before_first);

    app.nav_reset();
    assert_eq!(app.view_state.scroll().offset(), before_first);
    assert!(app.nav_stack.is_empty());
}

fn temp_markdown_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("bmd-{name}-{}", std::process::id()))
}

#[test]
fn document_stack_back_and_reset() {
    let dir = temp_markdown_dir("doc-stack");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.md");
    let b = dir.join("b.md");
    let c = dir.join("c.md");
    std::fs::write(&a, "# A\n\n[open b](b.md)\n").unwrap();
    std::fs::write(&b, "# B\n\n[open c](c.md)\n").unwrap();
    std::fs::write(&c, "# C\n\nend\n").unwrap();

    let doc = parse(&std::fs::read_to_string(&a).unwrap()).unwrap();
    let mut app = App::new_with_terminal_size(
        doc,
        Picker::halfblocks(),
        Some(a.clone()),
        Some("a.md".into()),
        test_terminal_size(),
    )
    .unwrap();

    app.view_state = app
        .view_state
        .clone()
        .select_next_link_in(&[crate::domain::LinkId(0)]);
    app.open_current_link();
    assert_eq!(app.source_label.as_deref(), Some("b.md"));
    assert_eq!(app.doc_stack.len_frames(), 1);

    app.view_state = app
        .view_state
        .clone()
        .select_next_link_in(&[crate::domain::LinkId(0)]);
    app.open_current_link();
    assert_eq!(app.source_label.as_deref(), Some("c.md"));
    assert_eq!(app.doc_stack.len_frames(), 2);

    app.nav_back();
    assert_eq!(app.source_label.as_deref(), Some("b.md"));
    assert_eq!(app.doc_stack.len_frames(), 1);

    app.nav_back();
    assert_eq!(app.source_label.as_deref(), Some("a.md"));
    assert!(app.doc_stack.is_empty());

    app.view_state = app
        .view_state
        .clone()
        .select_next_link_in(&[crate::domain::LinkId(0)]);
    app.open_current_link();
    app.view_state = app
        .view_state
        .clone()
        .select_next_link_in(&[crate::domain::LinkId(0)]);
    app.open_current_link();
    app.nav_reset();
    assert_eq!(app.source_label.as_deref(), Some("a.md"));
    assert!(app.doc_stack.is_empty());

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn anchor_stack_takes_priority_over_document_stack() {
    let dir = temp_markdown_dir("doc-anchor-priority");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.md");
    let b = dir.join("b.md");
    std::fs::write(&a, "# A\n\n[open b](b.md)\n").unwrap();
    std::fs::write(
        &b,
        "# Top\n\n[jump](#target)\n\n## Target\n\nsection body\n",
    )
    .unwrap();

    let doc = parse(&std::fs::read_to_string(&a).unwrap()).unwrap();
    let mut app = App::new_with_terminal_size(
        doc,
        Picker::halfblocks(),
        Some(a.clone()),
        Some("a.md".into()),
        test_terminal_size(),
    )
    .unwrap();

    app.view_state = app
        .view_state
        .clone()
        .select_next_link_in(&[crate::domain::LinkId(0)]);
    app.open_current_link();
    assert_eq!(app.source_label.as_deref(), Some("b.md"));

    app.view_state = app
        .view_state
        .clone()
        .select_next_link_in(&[crate::domain::LinkId(0)]);
    app.open_current_link();
    assert!(!app.nav_stack.is_empty());

    app.nav_back();
    assert_eq!(app.source_label.as_deref(), Some("b.md"));
    assert!(!app.doc_stack.is_empty());

    let _ = std::fs::remove_dir_all(dir);
}
