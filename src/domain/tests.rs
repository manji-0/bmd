use super::{
    Alignment, CodeBlock, Document, Heading, HeadingLevel, Inline, Link, LinkId, LinkKind, LinkUrl,
    LinkUrlError, NormalSearch, SearchDirection, SearchMatch, SearchQuery, SearchQueryError, Table,
    TerminalSize, TerminalSizeError, UiMode, ViewState,
};

#[test]
fn link_url_rejects_empty() {
    assert!(matches!(
        LinkUrl::new("".to_string()),
        Err(LinkUrlError::Empty)
    ));
    assert!(matches!(
        LinkUrl::new("   ".to_string()),
        Err(LinkUrlError::Empty)
    ));
}

#[test]
fn link_url_accepts_non_empty() {
    let url = LinkUrl::new("https://example.com".to_string()).unwrap();
    assert_eq!(url.as_str(), "https://example.com");
}

#[test]
fn terminal_size_rejects_zero() {
    assert!(matches!(
        TerminalSize::new(0, 24),
        Err(TerminalSizeError::ZeroDimension)
    ));
    assert!(matches!(
        TerminalSize::new(80, 0),
        Err(TerminalSizeError::ZeroDimension)
    ));
}

#[test]
fn scroll_down_clamps() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size);
    let state = state.scroll_down(100, 10);
    assert_eq!(state.scroll().offset(), 10);
}

#[test]
fn scroll_up_saturates() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size);
    let state = state.scroll_up(5);
    assert_eq!(state.scroll().offset(), 0);
}

#[test]
fn link_selection_wraps_within_visible_set() {
    let visible = [LinkId(1), LinkId(3), LinkId(5)];
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size);
    let state = state.select_next_link_in(&visible);
    assert_eq!(state.selected_link(), Some(LinkId(1)));
    let state = state.select_next_link_in(&visible);
    assert_eq!(state.selected_link(), Some(LinkId(3)));
    let state = state.select_next_link_in(&visible);
    assert_eq!(state.selected_link(), Some(LinkId(5)));
    let state = state.select_next_link_in(&visible);
    assert_eq!(state.selected_link(), Some(LinkId(1)));
}

#[test]
fn heading_level_prefixes() {
    assert_eq!(HeadingLevel::H1.prefix(), "# ");
    assert_eq!(HeadingLevel::H6.prefix(), "###### ");
}

#[test]
fn heading_prefix_delegates_to_level() {
    let h = Heading {
        level: HeadingLevel::H2,
        content: vec![],
    };
    assert_eq!(h.prefix(), "## ");
}

#[test]
fn code_block_logical_height() {
    let cb = CodeBlock {
        language: Some("rust".to_string()),
        content: "line one\nline two".to_string(),
    };
    assert_eq!(cb.logical_height(), 3);
}

#[test]
fn inline_text_width_counts_code_and_text() {
    let inlines = vec![
        Inline::Text("hello".to_string()),
        Inline::Code("world".to_string()),
    ];
    assert_eq!(Inline::text_width(&inlines), 10);
}

#[test]
fn inline_min_word_width_ignores_breaks() {
    let inlines = vec![Inline::Text("a longword".to_string()), Inline::SoftBreak];
    assert_eq!(Inline::min_word_width(&inlines), 8);
}

#[test]
fn table_column_count_derives_from_headers_and_rows() {
    let table = Table {
        headers: vec![vec![], vec![]],
        rows: vec![vec![vec![]]],
        alignments: vec![],
    };
    assert_eq!(table.column_count(), 2);
}

#[test]
fn table_allocate_column_widths_uses_ideal_when_content_fits() {
    let table = Table {
        headers: vec![
            vec![Inline::Text("A".to_string())],
            vec![Inline::Text("B".to_string())],
        ],
        rows: vec![vec![
            vec![Inline::Text("wide".to_string())],
            vec![Inline::Text("x".to_string())],
        ]],
        alignments: vec![Alignment::Left, Alignment::Left],
    };
    let widths = table.allocate_column_widths(20);
    assert_eq!(widths, vec![4, 1]);
    assert!(Table::table_frame_width(&widths) < 20);
    assert!(widths.iter().all(|w| *w >= 1));
}

#[test]
fn table_frame_width_fills_terminal_when_content_overflows() {
    let table = Table {
        headers: vec![
            vec![Inline::Text("とても長い説明文が入るカラム".to_string())],
            vec![Inline::Text("短".to_string())],
            vec![Inline::Text("値".to_string())],
        ],
        rows: vec![vec![
            vec![Inline::Text("これは折り返しのテストです".to_string())],
            vec![Inline::Text("1".to_string())],
            vec![Inline::Text("x".to_string())],
        ]],
        alignments: vec![Alignment::Left, Alignment::Left, Alignment::Left],
    };
    let widths = table.allocate_column_widths(40);
    assert_eq!(Table::table_frame_width(&widths), 40);

    let widths = table.allocate_column_widths(80);
    assert!(Table::table_frame_width(&widths) < 80);
}

#[test]
fn search_query_rejects_empty() {
    assert!(matches!(
        SearchQuery::new("".to_string()),
        Err(SearchQueryError::Empty)
    ));
}

#[test]
fn view_state_reset_for_reload_preserves_clamped_scroll() {
    let size = TerminalSize::new(80, 24).unwrap();
    let _document = Document::new(
        vec![],
        vec![Link {
            url: LinkUrl::new("https://example.com".to_string()).unwrap(),
            title: None,
            kind: LinkKind::Web,
        }],
        vec![],
    )
    .unwrap();
    let state = ViewState::new(size)
        .select_next_link_in(&[LinkId(0)])
        .reset_for_reload(42, 10);
    assert_eq!(state.scroll().offset(), 10);
    assert_eq!(state.selected_link(), None);
    assert!(!state.is_search_active());
    assert!(state.mode().is_normal());
}

#[test]
fn view_state_starts_search_in_input_mode() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size).start_search(SearchDirection::Forward);
    assert!(matches!(
        state.mode(),
        UiMode::SearchInput {
            direction: SearchDirection::Forward,
            query,
        } if query.is_empty()
    ));
}

#[test]
fn view_state_appends_search_input() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size)
        .start_search(SearchDirection::Forward)
        .append_search_input('f')
        .append_search_input('o')
        .append_search_input('o');
    assert!(matches!(
        state.mode(),
        UiMode::SearchInput { query, .. } if query == "foo"
    ));
}

#[test]
fn view_state_backspace_search_input() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size)
        .start_search(SearchDirection::Forward)
        .append_search_input('b')
        .append_search_input('a')
        .append_search_input('r')
        .backspace_search_input();
    assert!(matches!(
        state.mode(),
        UiMode::SearchInput { query, .. } if query == "ba"
    ));
}

#[test]
fn view_state_confirms_search_selects_first_forward_match() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size)
        .scroll_down(5, 100)
        .start_search(SearchDirection::Forward)
        .append_search_input('f')
        .append_search_input('o')
        .append_search_input('o');
    let matches = vec![SearchMatch::new(2, 0), SearchMatch::new(7, 1)];
    let state = state.confirm_search(matches).unwrap();
    assert!(matches!(
        state.normal_search(),
        NormalSearch::Active {
            current_index: 1,
            ..
        }
    ));
}

#[test]
fn view_state_confirms_search_selects_last_backward_match() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size)
        .scroll_down(5, 100)
        .start_search(SearchDirection::Backward)
        .append_search_input('f')
        .append_search_input('o')
        .append_search_input('o');
    let matches = vec![SearchMatch::new(2, 0), SearchMatch::new(7, 1)];
    let state = state.confirm_search(matches).unwrap();
    assert!(matches!(
        state.normal_search(),
        NormalSearch::Active {
            current_index: 0,
            ..
        }
    ));
}

#[test]
fn view_state_search_navigation_wraps() {
    let size = TerminalSize::new(80, 24).unwrap();
    let matches = vec![SearchMatch::new(1, 0), SearchMatch::new(3, 1)];
    let state = ViewState::new(size)
        .start_search(SearchDirection::Forward)
        .append_search_input('f')
        .append_search_input('o')
        .append_search_input('o')
        .confirm_search(matches)
        .unwrap();
    let state = state.next_search_match(100);
    assert!(matches!(
        state.normal_search(),
        NormalSearch::Active {
            current_index: 1,
            ..
        }
    ));
    let state = state.next_search_match(100);
    assert!(matches!(
        state.normal_search(),
        NormalSearch::Active {
            current_index: 0,
            ..
        }
    ));
}

#[test]
fn view_state_preview_transitions() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size).open_preview(LinkId(0));
    assert!(state.mode().is_preview());
    let state = state.close_preview();
    assert!(state.mode().is_normal());
}

#[test]
fn view_state_cancel_search_returns_to_inactive() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size)
        .start_search(SearchDirection::Forward)
        .cancel_search();
    assert!(matches!(state.normal_search(), NormalSearch::Inactive));
    assert!(state.mode().is_normal());
}

#[test]
fn ui_mode_helpers() {
    let normal = UiMode::normal();
    assert!(normal.is_normal());
    assert!(!normal.is_search_input());
    assert!(!normal.is_preview());
    assert_eq!(normal.preview_link(), None);
    assert_eq!(normal.search_input_query(), None);

    let search = UiMode::SearchInput {
        direction: SearchDirection::Backward,
        query: "foo".to_string(),
    };
    assert!(search.is_search_input());
    assert_eq!(
        search.search_input_query(),
        Some((&SearchDirection::Backward, "foo"))
    );

    let preview = UiMode::Preview { link_id: LinkId(3) };
    assert!(preview.is_preview());
    assert_eq!(preview.preview_link(), Some(LinkId(3)));
}

#[test]
fn normal_search_active_flag() {
    assert!(!NormalSearch::inactive().is_active());
    let active = NormalSearch::Active {
        direction: SearchDirection::Forward,
        query: SearchQuery::new("x".to_string()).unwrap(),
        matches: vec![],
        current_index: 0,
    };
    assert!(active.is_active());
}

#[test]
fn link_kind_preview_flag() {
    assert!(!LinkKind::Web.is_preview());
    assert!(!LinkKind::Anchor.is_preview());
    assert!(!LinkKind::Document.is_preview());
    assert!(LinkKind::Image.is_preview());
    assert!(LinkKind::Mermaid.is_preview());
    assert_eq!(LinkKind::for_link_dest("#section"), LinkKind::Anchor);
    assert_eq!(LinkKind::for_link_dest("https://x.com"), LinkKind::Web);
    assert_eq!(LinkKind::for_link_dest("./page.md"), LinkKind::Document);
}

#[test]
fn link_selection_prev_wraps_within_visible_set() {
    let visible = [LinkId(1), LinkId(3)];
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size).select_prev_link_in(&visible);
    assert_eq!(state.selected_link(), Some(LinkId(3)));
    let state = state.select_prev_link_in(&visible);
    assert_eq!(state.selected_link(), Some(LinkId(1)));
}

#[test]
fn clear_link_selection() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size)
        .select_next_link_in(&[LinkId(0)])
        .clear_link_selection();
    assert_eq!(state.selected_link(), None);
}

#[test]
fn view_state_prev_search_match_wraps() {
    let size = TerminalSize::new(80, 24).unwrap();
    let matches = vec![SearchMatch::new(1, 0), SearchMatch::new(3, 1)];
    let state = ViewState::new(size)
        .start_search(SearchDirection::Forward)
        .append_search_input('x')
        .confirm_search(matches)
        .unwrap();
    let state = state.prev_search_match(100);
    assert!(matches!(
        state.normal_search(),
        NormalSearch::Active {
            current_index: 1,
            ..
        }
    ));
    let state = state.prev_search_match(100);
    assert!(matches!(
        state.normal_search(),
        NormalSearch::Active {
            current_index: 0,
            ..
        }
    ));
}

#[test]
fn half_page_scroll_uses_terminal_height() {
    let size = TerminalSize::new(80, 20).unwrap();
    let state = ViewState::new(size).half_page_down(100);
    assert_eq!(state.scroll().offset(), 10);
    let state = state.half_page_up();
    assert_eq!(state.scroll().offset(), 0);
}

#[test]
fn resize_preserves_scroll_offset() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size).scroll_down(5, 100);
    let resized = TerminalSize::new(120, 40).unwrap();
    let state = state.resize(resized);
    assert_eq!(state.scroll().offset(), 5);
    assert_eq!(state.terminal_size(), resized);
}

#[test]
fn cancel_search_in_preview_is_noop() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size).open_preview(LinkId(0)).cancel_search();
    assert!(state.mode().is_preview());
}

#[test]
fn append_search_input_outside_search_mode_is_noop() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size).append_search_input('x');
    assert!(state.mode().is_normal());
}

#[test]
fn confirm_search_outside_search_input_is_noop() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size)
        .confirm_search(vec![SearchMatch::new(0, 0)])
        .unwrap();
    assert!(matches!(state.normal_search(), NormalSearch::Inactive));
}
