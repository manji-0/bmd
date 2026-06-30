use super::{
    Alignment, CodeBlock, Document, Heading, HeadingLevel, Inline, Link, LinkId, LinkUrl,
    LinkUrlError, SearchDirection, SearchMatch, SearchQuery, SearchQueryError, SearchState, Table,
    TerminalSize, TerminalSizeError, ViewState,
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
fn link_selection_wraps() {
    let doc = Document {
        blocks: vec![],
        links: vec![
            Link {
                url: LinkUrl::new("a".to_string()).unwrap(),
                title: None,
            },
            Link {
                url: LinkUrl::new("b".to_string()).unwrap(),
                title: None,
            },
        ],
    };
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size);
    let state = state.select_next_link(&doc);
    assert_eq!(state.selected_link(), Some(LinkId(0)));
    let state = state.select_next_link(&doc);
    assert_eq!(state.selected_link(), Some(LinkId(1)));
    let state = state.select_next_link(&doc);
    assert_eq!(state.selected_link(), Some(LinkId(0)));
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
fn table_allocate_column_widths_fits_total_width() {
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
    let border_width = widths.len() + 1;
    assert!(widths.iter().sum::<usize>() + border_width <= 20);
    assert!(widths.iter().all(|w| *w >= 1));
}

#[test]
fn search_query_rejects_empty() {
    assert!(matches!(
        SearchQuery::new("".to_string()),
        Err(SearchQueryError::Empty)
    ));
}

#[test]
fn view_state_starts_search_in_input_mode() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size).start_search(SearchDirection::Forward);
    assert!(matches!(
        state.search_state(),
        SearchState::Input {
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
        state.search_state(),
        SearchState::Input { query, .. } if query == "foo"
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
        state.search_state(),
        SearchState::Input { query, .. } if query == "ba"
    ));
}

#[test]
fn view_state_confirms_search_selects_first_forward_match() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size).scroll_down(5, 100);
    let matches = vec![SearchMatch::new(2, 0), SearchMatch::new(7, 1)];
    let state = state
        .confirm_search("foo".to_string(), SearchDirection::Forward, matches)
        .unwrap();
    assert!(matches!(
        state.search_state(),
        SearchState::Active {
            current_index: 1,
            ..
        }
    ));
}

#[test]
fn view_state_confirms_search_selects_last_backward_match() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size).scroll_down(5, 100);
    let matches = vec![SearchMatch::new(2, 0), SearchMatch::new(7, 1)];
    let state = state
        .confirm_search("foo".to_string(), SearchDirection::Backward, matches)
        .unwrap();
    assert!(matches!(
        state.search_state(),
        SearchState::Active {
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
        .confirm_search("foo".to_string(), SearchDirection::Forward, matches)
        .unwrap();
    let state = state.next_search_match(100);
    assert!(matches!(
        state.search_state(),
        SearchState::Active {
            current_index: 1,
            ..
        }
    ));
    let state = state.next_search_match(100);
    assert!(matches!(
        state.search_state(),
        SearchState::Active {
            current_index: 0,
            ..
        }
    ));
}

#[test]
fn view_state_cancel_search_returns_to_inactive() {
    let size = TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size)
        .start_search(SearchDirection::Forward)
        .cancel_search();
    assert!(matches!(state.search_state(), SearchState::Inactive));
}
