use super::App;
use crate::config::Config;
use crate::domain::{
    ANCHOR_STACK_MAX_FRAMES, AnchorIdle, Block, DOCUMENT_STACK_MAX_LAYERS, Document, Heading,
    HeadingLevel, Inline, Link, LinkKind, LinkUrl, SearchDirection, TerminalSize,
    anchor_stack_limit_message, document_stack_limit_message, normalize_document_path,
};
use crate::keymap::{Command, Keymap};
use crate::parse::parse;
use crate::render::{CachedMarkdownView, DocumentRenderCache, RenderContext};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui_image::picker::Picker;

fn test_terminal_size() -> TerminalSize {
    TerminalSize::new(80, 30).unwrap()
}

fn anchor_idle(app: &App) -> AnchorIdle {
    AnchorIdle::from_stack(&app.nav_stack)
        .expect("anchor stack must be idle for document navigation")
}

fn new_test_app(document: Document) -> App {
    App::new_with_terminal_size(
        document,
        Picker::halfblocks(),
        None,
        None,
        test_terminal_size(),
        Config::default(),
    )
    .unwrap()
}

fn dummy_document() -> Document {
    Document {
        blocks: vec![crate::domain::Block::Heading(Heading {
            level: HeadingLevel::H1,
            content: vec![Inline::Text("Hello".to_string())],
            anchor: None,
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
    let keymap = Keymap::default();
    let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    let up = KeyEvent::new(KeyCode::Up, KeyModifiers::empty());
    let d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty());
    assert!(keymap.is_line_scroll_key(&j));
    assert!(keymap.is_line_scroll_key(&up));
    assert!(!keymap.is_line_scroll_key(&d));
    assert_eq!(keymap.line_scroll_command(&j), Command::ScrollDown);
    assert_eq!(
        keymap.line_scroll_command(&KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty())),
        Command::ScrollUp
    );
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
    use std::time::{Duration, Instant};

    let doc = parse("```mermaid\ngraph TD; A-->B;\n```").unwrap();
    let mut app = new_test_app(doc);

    app.next_link();
    app.open_current_link();
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.pending_preview.is_some() && Instant::now() < deadline {
        app.poll_preview_renders();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        app.view_state.mode().is_preview(),
        "preview should open after background render completes"
    );

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

#[test]
fn scrolling_and_search_do_not_push_anchor_stack() {
    let mut input = String::from("# Top\n\n");
    for i in 0..80 {
        input.push_str(&format!("paragraph {}\n\n", i));
    }
    input.push_str("## Target\n\nfindme\n");
    let doc = parse(&input).unwrap();
    let mut app = new_test_app(doc);

    app.scroll_down(20);
    app.half_page_down();
    app.jump_to_bottom();
    app.start_search(crate::domain::SearchDirection::Forward);
    app.append_search_input('f');
    app.confirm_search();
    assert!(app.view_state.scroll().offset() > 0);
    assert!(app.nav_stack.is_empty());
    assert_eq!(app.nav_stack.current_layer(), 1);
}

fn temp_markdown_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("bmd-{name}-{}", std::process::id()))
}

fn file_backed_app(dir: &std::path::Path, file_name: &str, markdown: &str) -> App {
    let path = dir.join(file_name);
    std::fs::write(&path, markdown).unwrap();
    let doc = parse(markdown).unwrap();
    App::new_with_terminal_size(
        doc,
        Picker::halfblocks(),
        Some(path),
        Some(file_name.into()),
        test_terminal_size(),
        Config::default(),
    )
    .unwrap()
}

fn first_heading_text(document: &Document) -> Option<String> {
    document.blocks.iter().find_map(|block| match block {
        Block::Heading(heading) => heading.content.iter().find_map(|inline| {
            if let Inline::Text(text) = inline {
                Some(text.clone())
            } else {
                None
            }
        }),
        _ => None,
    })
}

fn wait_for_background_work(app: &mut App) {
    use std::time::{Duration, Instant};

    let deadline = Instant::now() + Duration::from_secs(5);
    while app.preview_work_pending() && Instant::now() < deadline {
        app.poll_preview_renders();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn long_body_with_tail_link(tail_link: &str) -> String {
    let mut body = String::from("# Index\n\n");
    for index in 0..60 {
        body.push_str(&format!("filler paragraph {index}\n\n"));
    }
    body.push_str(tail_link);
    body
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
        Config::default(),
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
    assert!(app.doc_stack.len_frames() == 0);

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
    assert!(app.doc_stack.len_frames() == 0);

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
        Config::default(),
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
    assert!(app.doc_stack.len_frames() != 0);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn open_document_link_requires_file_backed_base() {
    let doc = parse("[guide](./guide.md)").unwrap();
    let mut app = new_test_app(doc);

    app.open_document_link("./guide.md");

    assert!(app.status_message.is_some());
    assert!(app.doc_stack.len_frames() == 0);
}

#[test]
fn open_document_link_missing_file_preserves_stack() {
    let dir = temp_markdown_dir("doc-missing");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.md");
    std::fs::write(&a, "# A\n\n[missing](missing.md)\n").unwrap();

    let doc = parse(&std::fs::read_to_string(&a).unwrap()).unwrap();
    let mut app = App::new_with_terminal_size(
        doc,
        Picker::halfblocks(),
        Some(a.clone()),
        Some("a.md".into()),
        test_terminal_size(),
        Config::default(),
    )
    .unwrap();

    app.open_document_link("missing.md");

    assert!(app.status_message.is_some());
    assert_eq!(app.source_label.as_deref(), Some("a.md"));
    assert!(app.doc_stack.len_frames() == 0);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn open_document_link_rolls_back_stack_on_apply_failure() {
    let dir = temp_markdown_dir("doc-apply-fail");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.md");
    let b = dir.join("b.md");
    std::fs::write(&a, "# A\n\n[open b](b.md)\n").unwrap();
    std::fs::write(&b, "# B\n\ncontent\n").unwrap();

    let doc = parse(&std::fs::read_to_string(&a).unwrap()).unwrap();
    let mut app = App::new_with_terminal_size(
        doc,
        Picker::halfblocks(),
        Some(a.clone()),
        Some("a.md".into()),
        test_terminal_size(),
        Config::default(),
    )
    .unwrap();

    app.fail_apply_document = true;
    app.open_document_link("b.md");

    assert!(app.status_message.is_some());
    assert_eq!(app.source_label.as_deref(), Some("a.md"));
    assert!(app.doc_stack.len_frames() == 0);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn doc_back_preserves_stack_when_restore_fails() {
    let dir = temp_markdown_dir("doc-back-fail");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.md");
    let b = dir.join("b.md");
    std::fs::write(&a, "# A\n\n[open b](b.md)\n").unwrap();
    std::fs::write(&b, "# B\n\ncontent\n").unwrap();

    let doc = parse(&std::fs::read_to_string(&a).unwrap()).unwrap();
    let mut app = App::new_with_terminal_size(
        doc,
        Picker::halfblocks(),
        Some(a.clone()),
        Some("a.md".into()),
        test_terminal_size(),
        Config::default(),
    )
    .unwrap();

    app.open_document_link("b.md");
    assert_eq!(app.doc_stack.len_frames(), 1);
    assert_eq!(app.source_label.as_deref(), Some("b.md"));

    app.fail_document_restore = true;
    app.doc_back(anchor_idle(&app));

    assert!(app.status_message.is_some());
    assert_eq!(app.source_label.as_deref(), Some("b.md"));
    assert_eq!(app.doc_stack.len_frames(), 1);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn doc_reset_preserves_stack_when_restore_fails() {
    let dir = temp_markdown_dir("doc-reset-fail");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.md");
    let b = dir.join("b.md");
    std::fs::write(&a, "# A\n\n[open b](b.md)\n").unwrap();
    std::fs::write(&b, "# B\n\ncontent\n").unwrap();

    let doc = parse(&std::fs::read_to_string(&a).unwrap()).unwrap();
    let mut app = App::new_with_terminal_size(
        doc,
        Picker::halfblocks(),
        Some(a.clone()),
        Some("a.md".into()),
        test_terminal_size(),
        Config::default(),
    )
    .unwrap();

    app.open_document_link("b.md");
    assert_eq!(app.doc_stack.len_frames(), 1);

    app.fail_document_restore = true;
    app.doc_reset(anchor_idle(&app));

    assert!(app.status_message.is_some());
    assert_eq!(app.source_label.as_deref(), Some("b.md"));
    assert_eq!(app.doc_stack.len_frames(), 1);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn nav_reset_drains_anchor_before_returning_to_root_document() {
    let dir = temp_markdown_dir("doc-anchor-drain");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.md");
    let b = dir.join("b.md");
    let c = dir.join("c.md");
    std::fs::write(&a, "# A\n\n[open b](b.md)\n").unwrap();
    std::fs::write(&b, "# B\n\n[open c](c.md#target)\n").unwrap();
    let mut body = String::from("# Top\n\n");
    for i in 0..60 {
        body.push_str(&format!("paragraph {}\n\n", i));
    }
    body.push_str("## Target\n\nend\n");
    std::fs::write(&c, &body).unwrap();

    let doc = parse(&std::fs::read_to_string(&a).unwrap()).unwrap();
    let mut app = App::new_with_terminal_size(
        doc,
        Picker::halfblocks(),
        Some(a.clone()),
        Some("a.md".into()),
        test_terminal_size(),
        Config::default(),
    )
    .unwrap();

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
    assert_eq!(app.source_label.as_deref(), Some("c.md"));
    assert_eq!(app.doc_stack.len_frames(), 2);
    assert!(!app.nav_stack.is_empty());

    app.nav_reset();
    assert_eq!(app.source_label.as_deref(), Some("c.md"));
    assert!(app.nav_stack.is_empty());

    app.nav_reset();
    assert_eq!(app.source_label.as_deref(), Some("a.md"));
    assert!(app.doc_stack.is_empty());

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn anchor_stack_rejects_jump_beyond_max_depth() {
    let mut input = String::from("# Top\n\n");
    for i in 0..60 {
        input.push_str(&format!("paragraph {}\n\n", i));
    }
    input.push_str("## Target\n\nbody\n");
    let doc = parse(&input).unwrap();
    let mut app = new_test_app(doc);

    for _ in 0..ANCHOR_STACK_MAX_FRAMES {
        app.follow_anchor("target");
    }
    assert_eq!(app.nav_stack.depth(), ANCHOR_STACK_MAX_FRAMES);

    app.follow_anchor("target");
    assert_eq!(
        app.status_message.as_deref(),
        Some(anchor_stack_limit_message().as_str())
    );
    assert_eq!(app.nav_stack.depth(), ANCHOR_STACK_MAX_FRAMES);
}

#[test]
fn document_stack_supports_max_depth_and_rejects_overflow() {
    let dir = temp_markdown_dir("doc-max-depth");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let overflow_file = DOCUMENT_STACK_MAX_LAYERS;
    for i in 0..=overflow_file {
        let path = dir.join(format!("{i}.md"));
        let body = if i < overflow_file {
            format!("# {i}\n\n[next]({}.md)\n", i + 1)
        } else {
            format!("# {i}\n\nterminal\n")
        };
        std::fs::write(path, body).unwrap();
    }

    let root = dir.join("0.md");
    let doc = parse(&std::fs::read_to_string(&root).unwrap()).unwrap();
    let mut app = App::new_with_terminal_size(
        doc,
        Picker::halfblocks(),
        Some(root.clone()),
        Some("0.md".into()),
        test_terminal_size(),
        Config::default(),
    )
    .unwrap();
    assert_eq!(app.source_label.as_deref(), Some("0.md"));
    assert_eq!(app.doc_stack.len_frames(), 0);

    for layer in 2..=DOCUMENT_STACK_MAX_LAYERS {
        app.view_state = app
            .view_state
            .clone()
            .select_next_link_in(&[crate::domain::LinkId(0)]);
        app.open_current_link();
        assert_eq!(app.doc_stack.len_frames(), layer - 1);
        assert_eq!(
            app.source_label.as_deref(),
            Some(format!("{}.md", layer - 1).as_str())
        );
    }

    app.view_state = app
        .view_state
        .clone()
        .select_next_link_in(&[crate::domain::LinkId(0)]);
    app.open_current_link();
    assert_eq!(
        app.status_message.as_deref(),
        Some(document_stack_limit_message().as_str())
    );
    assert_eq!(app.doc_stack.len_frames(), DOCUMENT_STACK_MAX_LAYERS - 1);
    assert_eq!(
        app.source_label.as_deref(),
        Some(format!("{}.md", DOCUMENT_STACK_MAX_LAYERS - 1).as_str())
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn document_prior_restore_preserves_render_caches() {
    let dir = temp_markdown_dir("doc-prior-cache");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.md");
    let b = dir.join("b.md");
    std::fs::write(&a, "# A\n\n[open b](b.md)\n").unwrap();
    std::fs::write(&b, "# B\n\n").unwrap();

    let doc = parse(&std::fs::read_to_string(&a).unwrap()).unwrap();
    let mut app = App::new_with_terminal_size(
        doc,
        Picker::halfblocks(),
        Some(a.clone()),
        Some("a.md".into()),
        test_terminal_size(),
        Config::default(),
    )
    .unwrap();

    let backend = ratatui::backend::TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    app.draw_frame(&mut terminal).unwrap();
    let cache_height_before = app.document_cache_total_height();
    assert!(cache_height_before > 0);

    app.open_document_link("b.md");
    assert_eq!(app.source_label.as_deref(), Some("b.md"));
    assert_eq!(app.document_cache_total_height(), 0);

    app.doc_back(anchor_idle(&app));
    assert_eq!(app.source_label.as_deref(), Some("a.md"));
    assert_eq!(app.document_cache_total_height(), cache_height_before);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn open_document_link_follows_fragment_in_linked_file() {
    let dir = temp_markdown_dir("doc-fragment");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.md");
    let b = dir.join("b.md");
    std::fs::write(&a, "# A\n\n[open section](b.md#target)\n").unwrap();
    let mut body = String::from("# Top\n\n");
    for i in 0..60 {
        body.push_str(&format!("paragraph {}\n\n", i));
    }
    body.push_str("## Target\n\nsection body\n");
    std::fs::write(&b, &body).unwrap();

    let doc = parse(&std::fs::read_to_string(&a).unwrap()).unwrap();
    let mut app = App::new_with_terminal_size(
        doc,
        Picker::halfblocks(),
        Some(a.clone()),
        Some("a.md".into()),
        test_terminal_size(),
        Config::default(),
    )
    .unwrap();

    app.open_document_link("b.md#target");

    assert_eq!(app.source_label.as_deref(), Some("b.md"));
    assert!(app.view_state.scroll().offset() > 0);
    assert!(!app.nav_stack.is_empty());

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn doc_back_restores_document_after_child_navigation() {
    let dir = temp_markdown_dir("doc-prefetch-restore");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.md");
    let b = dir.join("b.md");
    std::fs::write(&a, "# A\n\n[open b](b.md)\n").unwrap();
    std::fs::write(&b, "# B\n\nchild\n").unwrap();

    let doc = parse(&std::fs::read_to_string(&a).unwrap()).unwrap();
    let mut app = App::new_with_terminal_size(
        doc,
        Picker::halfblocks(),
        Some(a.clone()),
        Some("a.md".into()),
        test_terminal_size(),
        Config::default(),
    )
    .unwrap();

    app.open_document_link("b.md");
    assert_eq!(app.source_label.as_deref(), Some("b.md"));

    app.doc_back(anchor_idle(&app));
    assert_eq!(app.source_label.as_deref(), Some("a.md"));
    assert_eq!(app.doc_stack.len_frames(), 0);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn maybe_prefetch_skips_when_viewport_unchanged() {
    let mut app = new_test_app(dummy_document());
    app.maybe_prefetch_visible_links();
    app.maybe_prefetch_visible_links();
}

// --- Document prefetch use cases ---

/// User reads a page, waits briefly, then follows a visible child link.
#[test]
fn prefetched_visible_child_link_opens_target_document() {
    let dir = temp_markdown_dir("uc-prefetch-open");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("child.md"), "# Child target\n\nbody\n").unwrap();

    let mut app = file_backed_app(&dir, "parent.md", "# Parent\n\n[open child](child.md)\n");
    let child_path = normalize_document_path(dir.join("child.md"));

    wait_for_background_work(&mut app);
    assert!(app.prefetched_document_ready(&child_path));

    app.open_document_link("child.md");

    assert_eq!(app.source_label.as_deref(), Some("child.md"));
    assert_eq!(
        first_heading_text(&app.document).as_deref(),
        Some("Child target")
    );
    assert_eq!(app.doc_stack.len_frames(), 1);

    let _ = std::fs::remove_dir_all(dir);
}

/// User scrolls until a below-the-fold child link becomes visible; prefetch then open.
#[test]
fn scrolling_into_view_prefetches_and_opens_child_link() {
    let dir = temp_markdown_dir("uc-prefetch-scroll");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("below.md"), "# Below fold\n\nend\n").unwrap();

    let parent_body = long_body_with_tail_link("[open below](below.md)\n");
    let mut app = file_backed_app(&dir, "parent.md", &parent_body);
    let below_path = normalize_document_path(dir.join("below.md"));

    assert!(!app.prefetched_document_ready(&below_path));

    app.jump_to_bottom();
    app.maybe_prefetch_visible_links();
    wait_for_background_work(&mut app);

    assert!(app.prefetched_document_ready(&below_path));
    app.open_document_link("below.md");

    assert_eq!(app.source_label.as_deref(), Some("below.md"));
    assert_eq!(
        first_heading_text(&app.document).as_deref(),
        Some("Below fold")
    );

    let _ = std::fs::remove_dir_all(dir);
}

/// Child file changes on disk after prefetch; opening still shows latest content.
#[test]
fn opening_child_after_disk_edit_ignores_stale_prefetch() {
    use std::thread;
    use std::time::Duration;

    let dir = temp_markdown_dir("uc-prefetch-stale");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let child = dir.join("child.md");
    std::fs::write(&child, "# Version one\n\n").unwrap();

    let mut app = file_backed_app(&dir, "parent.md", "# Parent\n\n[open child](child.md)\n");
    let child_path = normalize_document_path(child.clone());

    wait_for_background_work(&mut app);
    assert!(app.prefetched_document_ready(&child_path));

    thread::sleep(Duration::from_millis(1100));
    std::fs::write(&child, "# Version two\n\n").unwrap();

    app.open_document_link("child.md");

    assert_eq!(
        first_heading_text(&app.document).as_deref(),
        Some("Version two")
    );

    let _ = std::fs::remove_dir_all(dir);
}

/// User reloads the current file; stale child prefetch is cleared, then visible links prefetch again.
#[test]
fn reload_current_file_clears_prefetched_children() {
    use std::thread;
    use std::time::Duration;

    let dir = temp_markdown_dir("uc-prefetch-reload");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("child.md"), "# Child\n\n").unwrap();

    let parent = dir.join("parent.md");
    std::fs::write(&parent, "# Parent v1\n\n[open child](child.md)\n").unwrap();
    let mut app = file_backed_app(&dir, "parent.md", "# Parent v1\n\n[open child](child.md)\n");
    let child_path = normalize_document_path(dir.join("child.md"));

    wait_for_background_work(&mut app);
    assert!(app.prefetched_document_ready(&child_path));

    thread::sleep(Duration::from_millis(1100));
    std::fs::write(&parent, "# Parent v2\n\n[open child](child.md)\n").unwrap();
    assert!(app.reload_from_disk().unwrap());
    assert!(!app.prefetched_document_ready(&child_path));

    wait_for_background_work(&mut app);
    assert_eq!(
        first_heading_text(&app.document).as_deref(),
        Some("Parent v2")
    );
    assert!(app.prefetched_document_ready(&child_path));

    let _ = std::fs::remove_dir_all(dir);
}

/// User jumps to a child without waiting for prefetch; sync load still succeeds.
#[test]
fn child_link_opens_immediately_when_prefetch_not_ready() {
    let dir = temp_markdown_dir("uc-prefetch-sync");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("child.md"), "# Immediate\n\n").unwrap();

    let mut app = file_backed_app(&dir, "parent.md", "# Parent\n\n[open child](child.md)\n");
    let child_path = normalize_document_path(dir.join("child.md"));
    assert!(!app.prefetched_document_ready(&child_path));

    app.open_document_link("child.md");

    assert_eq!(app.source_label.as_deref(), Some("child.md"));
    assert_eq!(
        first_heading_text(&app.document).as_deref(),
        Some("Immediate")
    );

    let _ = std::fs::remove_dir_all(dir);
}

/// User navigates into a prefetched child and back; parent context is restored.
#[test]
fn prefetched_child_navigation_round_trip_restores_parent() {
    let dir = temp_markdown_dir("uc-prefetch-roundtrip");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("child.md"), "# Child\n\n").unwrap();

    let mut app = file_backed_app(
        &dir,
        "parent.md",
        "# Parent home\n\n[open child](child.md)\n",
    );
    let child_path = normalize_document_path(dir.join("child.md"));

    wait_for_background_work(&mut app);
    assert!(app.prefetched_document_ready(&child_path));
    app.open_document_link("child.md");
    assert_eq!(app.source_label.as_deref(), Some("child.md"));

    app.doc_back(anchor_idle(&app));

    assert_eq!(app.source_label.as_deref(), Some("parent.md"));
    assert_eq!(
        first_heading_text(&app.document).as_deref(),
        Some("Parent home")
    );
    assert_eq!(app.doc_stack.len_frames(), 0);

    let _ = std::fs::remove_dir_all(dir);
}
