use super::blocks::render_code_block;
use super::document::render_document;
use super::headings::collect_heading_offsets;
use super::inline::{highlight_span, highlight_text, inlines_to_text, inlines_to_wrapped_lines};
use super::measure::measure_block_height;
use super::measure::measure_code_block_height;
use super::nav_hits::{NavHit, collect_nav_hits, link_at, visible_links, visible_nav_targets};
use super::table::{allocate_column_widths, render_table_row, wrap_cell_inlines};
use super::{
    DocumentRenderCache, RenderContext, SyntaxAssets, Theme, checklist,
    find_heading_line_by_anchor, find_search_matches, footnote_preview_title,
    measure_document_height, next_heading_line, prev_heading_line, render_footnote_preview,
};
use crate::domain::slugify_heading;
use crate::domain::{
    Alignment, Block, ChecklistState, ChecklistStyle, CodeBlock, Document, FootnoteDefinition,
    FootnoteId, Heading, HeadingLevel, Inline, Link, LinkId, LinkKind, LinkUrl, List, ListItem,
    NavTarget, SearchDirection, SearchMatch, Table, TerminalSize, ViewState,
};
use crate::parse::parse;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use unicode_width::UnicodeWidthStr;

/// Full-document render as a widget, for drawing through a `TestBackend`.
struct DocView<'a>(&'a Document, &'a RenderContext<'a>);

impl ratatui::widgets::Widget for DocView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render_document(self.0, area, buf, self.1);
    }
}

fn hits(document: &Document) -> Vec<NavHit> {
    collect_nav_hits(document, 80, &test_render_context())
}

fn first_line(hits: &[NavHit], target: NavTarget) -> Option<usize> {
    hits.iter()
        .find(|hit| hit.target == target)
        .map(|hit| hit.line)
}

fn test_syntax_assets() -> &'static SyntaxAssets {
    Box::leak(Box::new(SyntaxAssets::new()))
}

fn test_render_context() -> RenderContext<'static> {
    // Leaked for the duration of the test process; acceptable for unit tests.
    let theme: &'static Theme = Box::leak(Box::new(Theme::default()));
    let syntax_assets = test_syntax_assets();
    let links: &'static [crate::domain::Link] = Box::leak(Box::new([]));
    let checklist_state: &'static ChecklistState =
        Box::leak(Box::new(ChecklistState::new(ChecklistStyle::Unicode)));
    RenderContext {
        theme,
        syntax_set: &syntax_assets.syntax_set,
        syntax_theme: syntax_assets.theme(),
        links,
        selected_link: None,
        selected_footnote: None,
        search_query: None,
        selected_match_line_offset: None,
        checklist_state,
        nav_probe: false,
    }
}

fn wrapped_line_count(line: &Line, width: usize) -> usize {
    let inlines: Vec<Inline> = line
        .spans
        .iter()
        .map(|span| Inline::Text(span.content.to_string()))
        .collect();
    let ctx = test_render_context();
    inlines_to_wrapped_lines(&inlines, &ctx, ctx.theme.text, 0, width).len()
}

fn find_matches(document: &Document, width: u16, query: &str) -> Vec<SearchMatch> {
    find_search_matches(document, width, query, &test_render_context())
}

#[test]
fn document_render_cache_blits_fractional_scroll() {
    let ctx = test_render_context();
    let blocks: Vec<Block> = (0..20)
        .map(|i| Block::Paragraph(vec![Inline::Text(format!("line {i}"))]))
        .collect();
    let document =
        Document::new(blocks, Vec::new(), Vec::new(), Vec::new(), Vec::new(), None).unwrap();
    let width = 40u16;
    let height = 5u16;

    let mut cache = DocumentRenderCache::default();
    cache.ensure(&document, &ctx, width);

    let mut integer = Buffer::empty(Rect::new(0, 0, width, height));
    cache.blit(7.0, Rect::new(0, 0, width, height), &mut integer);

    let mut fractional = Buffer::empty(Rect::new(0, 0, width, height));
    cache.blit(7.25, Rect::new(0, 0, width, height), &mut fractional);

    assert_ne!(
        integer[(0, 0)].symbol(),
        fractional[(0, 0)].symbol(),
        "fractional scroll should composite adjacent rows"
    );
}

#[test]
fn document_render_cache_blits_scrolled_viewport() {
    let ctx = test_render_context();
    let blocks: Vec<Block> = (0..20)
        .map(|i| Block::Paragraph(vec![Inline::Text(format!("line {i}"))]))
        .collect();
    let document =
        Document::new(blocks, Vec::new(), Vec::new(), Vec::new(), Vec::new(), None).unwrap();
    let width = 40u16;
    let height = 5u16;

    let mut cache = DocumentRenderCache::default();
    cache.ensure(&document, &ctx, width);

    // Logical layout: line N sits at offset N * 2 (one gap row between blocks).
    let scroll = 8;
    let mut screen = Buffer::empty(Rect::new(0, 0, width, height));
    cache.blit(scroll as f32, Rect::new(0, 0, width, height), &mut screen);

    let row0: String = (0..width)
        .map(|x| {
            screen
                .cell((x, 0))
                .map_or(' ', |c| c.symbol().chars().next().unwrap_or(' '))
        })
        .collect();
    assert!(
        row0.contains('4'),
        "expected scrolled content at offset {scroll}, got {row0:?}"
    );
}

#[test]
fn document_render_cache_rebuilds_on_width_change() {
    let ctx = test_render_context();
    let document = Document::new(
        vec![Block::Paragraph(vec![Inline::Text(
            "hello world with enough words to wrap when the terminal is narrow".to_string(),
        )])],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
    )
    .unwrap();

    let mut cache = DocumentRenderCache::default();
    cache.ensure(&document, &ctx, 80);
    let height_at_80 = cache.total_height();
    cache.ensure(&document, &ctx, 20);
    let height_at_20 = cache.total_height();
    assert!(height_at_20 > height_at_80);
}

#[test]
fn find_search_matches_finds_text_in_paragraphs() {
    let document = Document::new(
        vec![
            Block::Paragraph(vec![Inline::Text("hello world".to_string())]),
            Block::Paragraph(vec![Inline::Text("foo bar".to_string())]),
            Block::Paragraph(vec![Inline::Text("hello again".to_string())]),
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
    )
    .unwrap();
    let matches = find_matches(&document, 80, "hello");
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].line_offset, 0);
    assert_eq!(matches[1].line_offset, 4);
}

#[test]
fn find_search_matches_is_case_insensitive() {
    let document = Document::new(
        vec![Block::Paragraph(vec![Inline::Text(
            "Hello World".to_string(),
        )])],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
    )
    .unwrap();
    let matches = find_matches(&document, 80, "world");
    assert_eq!(matches.len(), 1);
}

#[test]
fn find_search_matches_searches_code_blocks() {
    let document = Document::new(
        vec![Block::CodeBlock(CodeBlock {
            language: Some("rust".to_string()),
            content: "fn main() {}".to_string(),
        })],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
    )
    .unwrap();
    let matches = find_matches(&document, 80, "main");
    assert_eq!(matches.len(), 1);
}

#[test]
fn find_search_matches_empty_query_returns_no_matches() {
    let document = Document::new(
        vec![Block::Paragraph(vec![Inline::Text("hello".to_string())])],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
    )
    .unwrap();
    assert!(find_matches(&document, 80, "").is_empty());
}

#[test]
fn find_search_matches_zero_width_returns_no_matches() {
    let document = Document::new(
        vec![Block::Paragraph(vec![Inline::Text("hello".to_string())])],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
    )
    .unwrap();
    assert!(find_matches(&document, 0, "hello").is_empty());
}

#[test]
fn find_search_matches_respects_hard_breaks() {
    let document = Document::new(
        vec![Block::Paragraph(vec![
            Inline::Text("first".to_string()),
            Inline::HardBreak,
            Inline::Text("second".to_string()),
        ])],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
    )
    .unwrap();
    let matches = find_matches(&document, 80, "second");
    assert_eq!(matches.len(), 1);
    // Hard break creates a second logical line within the same paragraph.
    assert_eq!(matches[0].line_offset, 1);
}

#[test]
fn find_search_matches_list_offsets_exclude_inner_gaps() {
    let document = Document::new(
        vec![Block::List(List {
            ordered: false,
            items: vec![
                ListItem::plain(vec![
                    Block::Paragraph(vec![Inline::Text("alpha".to_string())]),
                    Block::Paragraph(vec![Inline::Text("beta".to_string())]),
                ]),
                ListItem::plain(vec![Block::Paragraph(vec![Inline::Text(
                    "gamma".to_string(),
                )])]),
            ],
        })],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
    )
    .unwrap();
    let matches = find_matches(&document, 80, "gamma");
    assert_eq!(matches.len(), 1);
    // alpha (line 0), beta (line 1), gamma (line 2) — no synthetic gaps.
    assert_eq!(matches[0].line_offset, 2);
}

#[test]
fn find_search_matches_blockquote_includes_padding() {
    let document = Document::new(
        vec![
            Block::Quote(vec![Block::Paragraph(vec![Inline::Text(
                "quoted".to_string(),
            )])]),
            Block::Paragraph(vec![Inline::Text("after".to_string())]),
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
    )
    .unwrap();
    let matches = find_matches(&document, 80, "after");
    assert_eq!(matches.len(), 1);
    // quoted (line 0) + blockquote padding (line 1) + gap (line 2) -> after at line 3.
    assert_eq!(matches[0].line_offset, 3);
}

#[test]
fn find_search_matches_table_includes_borders() {
    let document = Document::new(
        vec![
            Block::Table(Table {
                headers: vec![vec![Inline::Text("Header".to_string())]],
                rows: vec![vec![vec![Inline::Text("Cell".to_string())]]],
                alignments: vec![Alignment::Left],
            }),
            Block::Paragraph(vec![Inline::Text("after".to_string())]),
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
    )
    .unwrap();
    let matches = find_matches(&document, 80, "after");
    assert_eq!(matches.len(), 1);
    // top border (0) + header (1) + separator (2) + cell (3) + bottom border (4) + gap (5) -> after at 6.
    assert_eq!(matches[0].line_offset, 6);
}

#[test]
fn selected_search_match_renders_selected_style_in_buffer() {
    let theme = Theme::default();
    let ctx_base = test_render_context();
    let document = Document::new(
        vec![
            Block::Paragraph(vec![Inline::Text("alpha needle".to_string())]),
            Block::Paragraph(vec![Inline::Text("beta needle here".to_string())]),
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
    )
    .unwrap();

    let width = 80u16;
    let height = 10u16;
    let matches = find_search_matches(&document, width, "needle", &ctx_base);
    assert_eq!(matches.len(), 2);

    let size = TerminalSize::new(width, height).unwrap();
    let view_state = ViewState::new(size)
        .start_search(SearchDirection::Forward)
        .append_search_input('n')
        .unwrap()
        .append_search_input('e')
        .unwrap()
        .append_search_input('e')
        .unwrap()
        .append_search_input('d')
        .unwrap()
        .append_search_input('l')
        .unwrap()
        .append_search_input('e')
        .unwrap()
        .confirm_search(matches)
        .unwrap()
        .next_search_match(1000)
        .scroll_to(0, 0);

    let ctx = RenderContext::new(
        ctx_base.theme,
        test_syntax_assets(),
        ctx_base.links,
        &view_state,
        ctx_base.checklist_state,
    );

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let widget = DocView(&document, &ctx);
            f.render_widget(widget, f.area());
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    let selected_bg = theme.search_match_selected.bg.unwrap();
    let normal_bg = theme.search_match.bg.unwrap();

    let mut selected_needle = false;
    let mut normal_needle = false;
    for y in 0..height {
        for x in 0..width {
            let cell = buf.cell((x, y)).unwrap();
            if cell.bg == selected_bg && cell.symbol() != " " {
                selected_needle = true;
            }
            if cell.bg == normal_bg && cell.symbol() != " " {
                normal_needle = true;
            }
        }
    }

    assert!(
        selected_needle,
        "expected selected search match background in buffer"
    );
    assert!(
        normal_needle,
        "expected non-selected search match background in buffer"
    );
}

#[test]
fn theme_default_has_expected_styles() {
    let theme = Theme::default();
    let midnight = Theme::from_preset(crate::render::DEFAULT_PRESET).unwrap();
    assert_eq!(theme, midnight);
    assert!(theme.h1.add_modifier.contains(Modifier::BOLD));
    assert!(theme.h1.add_modifier.contains(Modifier::UNDERLINED));
    assert!(theme.link.add_modifier.contains(Modifier::UNDERLINED));
}

#[test]
fn code_block_height_counts_label_and_lines() {
    let cb = CodeBlock {
        language: Some("rust".to_string()),
        content: "line one\nline two".to_string(),
    };
    assert_eq!(cb.logical_height(), 3);
    assert_eq!(measure_code_block_height(&cb, 80), 3);
}

#[test]
fn code_block_height_empty_block_is_one_line_plus_label() {
    let cb = CodeBlock {
        language: None,
        content: String::new(),
    };
    assert_eq!(cb.logical_height(), 2);
    assert_eq!(measure_code_block_height(&cb, 80), 2);
}

#[test]
fn wrapped_line_count_zero_width_returns_one() {
    let line = Line::from("hello world");
    assert_eq!(wrapped_line_count(&line, 0), 1);
}

#[test]
fn wrapped_line_count_empty_line_returns_one() {
    let line = Line::from("   ");
    assert_eq!(wrapped_line_count(&line, 10), 1);
}

#[test]
fn wrapped_line_count_single_line() {
    let line = Line::from("hello world");
    assert_eq!(wrapped_line_count(&line, 80), 1);
}

#[test]
fn wrapped_line_count_wraps_long_line() {
    let line = Line::from("hello world");
    assert_eq!(wrapped_line_count(&line, 5), 2);
}

#[test]
fn wrapped_line_count_respects_multiple_spans() {
    let line = Line::from(vec![
        Span::styled("hello ", Style::default()),
        Span::styled("world", Style::default()),
    ]);
    assert_eq!(wrapped_line_count(&line, 5), 2);
}

#[test]
fn wrapped_lines_break_cjk_without_spaces() {
    let ctx = test_render_context();
    let text = "こんにちは世界";
    let inlines = vec![Inline::Text(text.into())];
    let width = 4;
    let rows = inlines_to_wrapped_lines(&inlines, &ctx, ctx.theme.text, 0, width);
    assert!(
        rows.len() > 1,
        "expected CJK text to wrap across multiple lines"
    );
    for (_, line) in &rows {
        let line_width: usize = line.spans.iter().map(|s| s.content.width()).sum();
        assert!(
            line_width <= width,
            "line {line:?} exceeds width {width}: {line_width}"
        );
    }
    let combined: String = rows
        .iter()
        .flat_map(|(_, line)| line.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    assert_eq!(combined, text);
}

#[test]
fn inlines_to_text_preserves_text_and_formatting() {
    let ctx = test_render_context();
    let inlines = vec![
        Inline::Text("Hello ".into()),
        Inline::Strong(vec![Inline::Text("world".into())]),
    ];
    let text = inlines_to_text(&inlines, &ctx, ctx.theme.text, 0);
    assert_eq!(text.lines.len(), 1);
    // Text + Strong wrapper is split into separate spans.
    assert_eq!(text.lines[0].spans.len(), 3);
}

#[test]
fn inlines_to_text_highlights_search_query() {
    let mut ctx = test_render_context();
    ctx.search_query = Some("world".to_string());
    let inlines = vec![
        Inline::Text("Hello ".into()),
        Inline::Strong(vec![Inline::Text("world".into())]),
    ];
    let text = inlines_to_text(&inlines, &ctx, ctx.theme.text, 0);
    let spans = &text.lines[0].spans;
    let has_highlight = spans.iter().any(|s| s.style == ctx.theme.search_match);
    assert!(has_highlight);
}

#[test]
fn inlines_to_text_selected_search_match_line_uses_selected_style() {
    let mut ctx = test_render_context();
    ctx.search_query = Some("hello".to_string());
    ctx.selected_match_line_offset = Some(0);
    let inlines = vec![Inline::Text("hello".into())];
    let text = inlines_to_text(&inlines, &ctx, ctx.theme.text, 0);
    let styles: Vec<Style> = text.lines[0].spans.iter().map(|s| s.style).collect();
    assert!(styles.contains(&ctx.theme.search_match_selected));
}

#[test]
fn inlines_to_text_non_selected_search_match_line_uses_match_style() {
    let mut ctx = test_render_context();
    ctx.search_query = Some("hello".to_string());
    ctx.selected_match_line_offset = Some(5);
    let inlines = vec![Inline::Text("hello".into())];
    let text = inlines_to_text(&inlines, &ctx, ctx.theme.text, 0);
    let styles: Vec<Style> = text.lines[0].spans.iter().map(|s| s.style).collect();
    assert!(!styles.contains(&ctx.theme.search_match_selected));
    assert!(styles.contains(&ctx.theme.search_match));
}

#[test]
fn inlines_to_text_case_insensitive_highlight() {
    let mut ctx = test_render_context();
    ctx.search_query = Some("WORLD".to_string());
    let inlines = vec![Inline::Text("hello world".into())];
    let text = inlines_to_text(&inlines, &ctx, ctx.theme.text, 0);
    let spans = &text.lines[0].spans;
    let highlighted: Vec<&str> = spans
        .iter()
        .filter(|s| s.style == ctx.theme.search_match)
        .map(|s| s.content.as_ref())
        .collect();
    assert_eq!(highlighted, vec!["world"]);
}

#[test]
fn wrapped_lines_highlight_search_on_second_visual_row() {
    let mut ctx = test_render_context();
    ctx.search_query = Some("target".to_string());
    ctx.selected_match_line_offset = Some(1);
    let inlines = vec![Inline::Text("aaa target".into())];
    let rows = inlines_to_wrapped_lines(&inlines, &ctx, ctx.theme.text, 0, 6);
    assert_eq!(rows.len(), 2);
    let selected_styles: Vec<Style> = rows[1].1.spans.iter().map(|s| s.style).collect();
    assert!(selected_styles.contains(&ctx.theme.search_match_selected));
}

#[test]
fn table_cell_highlights_search_query() {
    let mut ctx = test_render_context();
    ctx.search_query = Some("needle".to_string());
    let lines = wrap_cell_inlines(
        &[Inline::Text("needle here".into())],
        12,
        ctx.theme.table_cell,
        &ctx,
        3,
    );
    assert_eq!(lines.len(), 1);
    let has_highlight = lines[0]
        .spans
        .iter()
        .any(|s| s.style == ctx.theme.search_match);
    assert!(has_highlight);
}

#[test]
fn highlight_text_increments_line_offset_for_hard_breaks() {
    let mut ctx = test_render_context();
    ctx.search_query = Some("line".to_string());
    ctx.selected_match_line_offset = Some(1);
    let text = Text::from(vec![Line::from("first"), Line::from("second line")]);
    let highlighted = highlight_text(
        text,
        Some("line"),
        ctx.theme.search_match,
        ctx.theme.search_match_selected,
        Some(1),
        0,
    );
    let first_styles: Vec<Style> = highlighted.lines[0].spans.iter().map(|s| s.style).collect();
    let second_styles: Vec<Style> = highlighted.lines[1].spans.iter().map(|s| s.style).collect();
    assert!(!first_styles.contains(&ctx.theme.search_match_selected));
    assert!(second_styles.contains(&ctx.theme.search_match_selected));
}

#[test]
fn highlight_span_handles_case_folding_byte_length_changes() {
    let span = Span::styled("groß".to_string(), Style::default());
    let matched = highlight_span(
        span,
        "gross",
        Style::default().fg(Color::Yellow),
        Style::default().fg(Color::Green),
        false,
    );
    let text: String = matched.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(text, "groß");
}

#[test]
fn table_logical_height_accounts_for_borders_and_header() {
    let ctx = test_render_context();
    let table = Table {
        headers: vec![vec![Inline::Text("Header".into())]],
        rows: vec![vec![vec![Inline::Text("Cell".into())]]],
        alignments: vec![Alignment::Left],
    };
    assert_eq!(measure_block_height(&Block::Table(table), 20, &ctx), 5);
}

#[test]
fn allocate_column_widths_uses_ideal_when_content_fits() {
    let table = Table {
        headers: vec![
            vec![Inline::Text("A".into())],
            vec![Inline::Text("B".into())],
        ],
        rows: vec![vec![
            vec![Inline::Text("wide content".into())],
            vec![Inline::Text("x".into())],
        ]],
        alignments: vec![Alignment::Left, Alignment::Left],
    };
    let widths = allocate_column_widths(&table, 40);
    assert_eq!(widths, vec![12, 1]);
    assert!(Table::table_frame_width(&widths) < 40);
    assert!(widths.iter().all(|w| *w >= 1));
}

#[test]
fn render_table_row_width_matches_frame_when_content_overflows() {
    let ctx = test_render_context();
    let table = Table {
        headers: vec![
            vec![Inline::Text("long header column".into())],
            vec![Inline::Text("b".into())],
        ],
        rows: vec![vec![
            vec![Inline::Text("wrapped cell content here".into())],
            vec![Inline::Text("x".into())],
        ]],
        alignments: vec![Alignment::Left, Alignment::Left],
    };
    let total = 24usize;
    let widths = allocate_column_widths(&table, total);
    assert_eq!(Table::table_frame_width(&widths), total);
    let lines = render_table_row(
        &table.headers,
        &widths,
        &table.alignments,
        ctx.theme.table_header,
        &ctx,
        0,
    );
    let rendered_width: usize = lines[0].spans.iter().map(|s| s.content.width()).sum();
    assert_eq!(rendered_width, total);
}

#[test]
fn collect_heading_offsets_finds_each_heading() {
    let doc = parse("# One\n\n## Two\n\nbody\n").unwrap();
    let ctx = test_render_context();
    let headings = collect_heading_offsets(&doc, 80, &ctx);
    assert_eq!(headings.len(), 2);
    assert_eq!(headings[0].1, HeadingLevel::H1);
    assert!(headings[1].0 > headings[0].0);
}

#[test]
fn heading_navigation_picks_adjacent_sections() {
    let doc = Document::new(
        vec![
            Block::Heading(Heading {
                level: HeadingLevel::H1,
                content: vec![Inline::Text("A".into())],
                anchor: None,
            }),
            Block::Paragraph(vec![Inline::Text("gap".into())]),
            Block::Heading(Heading {
                level: HeadingLevel::H2,
                content: vec![Inline::Text("B".into())],
                anchor: None,
            }),
        ],
        vec![],
        vec![],
        vec![],
        vec![],
        None,
    )
    .unwrap();
    let ctx = test_render_context();
    let headings = collect_heading_offsets(&doc, 80, &ctx);
    assert_eq!(next_heading_line(&headings, 0), Some(headings[1].0));
    assert_eq!(
        prev_heading_line(&headings, headings[1].0),
        Some(headings[0].0)
    );
}

#[test]
fn find_heading_line_by_anchor_matches_slug() {
    let doc = parse("# Hello World\n\n## Foo Bar\n\nbody\n").unwrap();
    let ctx = test_render_context();
    assert_eq!(slugify_heading("Hello World"), "hello-world");
    assert_eq!(
        find_heading_line_by_anchor(&doc, 80, &ctx, "foo-bar"),
        Some(collect_heading_offsets(&doc, 80, &ctx)[1].0)
    );
}

#[test]
fn find_heading_line_by_anchor_uses_explicit_id() {
    let doc = parse("# Hello World {#custom-anchor}\n\n## Foo Bar\n\nbody\n").unwrap();
    let ctx = test_render_context();
    assert_eq!(
        find_heading_line_by_anchor(&doc, 80, &ctx, "custom-anchor"),
        Some(collect_heading_offsets(&doc, 80, &ctx)[0].0)
    );
    assert_eq!(
        find_heading_line_by_anchor(&doc, 80, &ctx, "hello-world"),
        None
    );
}

#[test]
fn find_heading_line_by_anchor_normalizes_fragment() {
    use crate::parse::MarkupFormat;
    use crate::parse::parse_document;

    let doc = parse_document(MarkupFormat::Rest, "Hello World\n===========\n\nBody.\n").unwrap();
    let ctx = test_render_context();
    assert_eq!(
        find_heading_line_by_anchor(&doc, 80, &ctx, "#hello-world"),
        Some(0)
    );
    assert_eq!(
        find_heading_line_by_anchor(&doc, 80, &ctx, "_Hello_World"),
        Some(0)
    );
}

#[test]
fn collect_visible_links_filters_by_viewport() {
    let doc = Document::new(
        vec![
            Block::Paragraph(vec![Inline::Link(
                LinkId(0),
                vec![Inline::Text("top".into())],
            )]),
            Block::Paragraph(vec![Inline::Text("filler".into())]),
            Block::Paragraph(vec![Inline::Link(
                LinkId(1),
                vec![Inline::Text("bottom".into())],
            )]),
        ],
        vec![
            Link {
                url: LinkUrl::new("https://a".into()).unwrap(),
                title: None,
                kind: LinkKind::Web,
            },
            Link {
                url: LinkUrl::new("https://b".into()).unwrap(),
                title: None,
                kind: LinkKind::Web,
            },
        ],
        vec![],
        vec![],
        vec![],
        None,
    )
    .unwrap();
    let hits = hits(&doc);
    let top_line = first_line(&hits, NavTarget::Link(LinkId(0))).unwrap();
    let bottom_line = first_line(&hits, NavTarget::Link(LinkId(1))).unwrap();
    assert!(bottom_line > top_line);
    assert_eq!(visible_links(&hits, top_line, 1), vec![LinkId(0)]);
    assert_eq!(visible_links(&hits, bottom_line, 1), vec![LinkId(1)]);
}

#[test]
fn footnote_nav_collects_reference_and_definition_lines() {
    let doc = Document::new(
        vec![Block::Paragraph(vec![
            Inline::Text("Text ".into()),
            Inline::FootnoteReference(FootnoteId(0), 1),
            Inline::Text(" end.".into()),
        ])],
        vec![],
        vec![],
        vec![FootnoteDefinition {
            label: "note".into(),
            content: vec![Block::Paragraph(vec![Inline::Text(
                "Footnote body.".into(),
            )])],
        }],
        vec![FootnoteId(0)],
        None,
    )
    .unwrap();
    let hits = hits(&doc);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].target, NavTarget::Footnote(FootnoteId(0)));
}

#[test]
fn collect_visible_nav_targets_orders_links_and_footnotes() {
    let doc = Document::new(
        vec![Block::Paragraph(vec![
            Inline::FootnoteReference(FootnoteId(0), 1),
            Inline::Text(" ".into()),
            Inline::Link(LinkId(0), vec![Inline::Text("link".into())]),
        ])],
        vec![Link {
            url: LinkUrl::new("https://example.com".into()).unwrap(),
            title: None,
            kind: LinkKind::Web,
        }],
        vec![],
        vec![FootnoteDefinition {
            label: "n".into(),
            content: vec![Block::Paragraph(vec![Inline::Text("body".into())])],
        }],
        vec![FootnoteId(0)],
        None,
    )
    .unwrap();
    let visible = visible_nav_targets(&hits(&doc), 0, 10);
    assert_eq!(
        visible,
        vec![
            NavTarget::Footnote(FootnoteId(0)),
            NavTarget::Link(LinkId(0))
        ]
    );
}

#[test]
fn nav_target_selection_wraps_mixed_targets() {
    let visible = [
        NavTarget::Footnote(FootnoteId(0)),
        NavTarget::Link(LinkId(1)),
    ];
    let size = crate::domain::TerminalSize::new(80, 24).unwrap();
    let state = ViewState::new(size)
        .select_next_nav_in(&visible)
        .select_next_nav_in(&visible);
    assert_eq!(state.selected_link(), Some(LinkId(1)));
    let state = state.select_next_nav_in(&visible);
    assert_eq!(state.selected_footnote(), Some(FootnoteId(0)));
}

#[test]
fn footnote_preview_renders_definition_body() {
    let doc = Document::new(
        vec![Block::Paragraph(vec![Inline::FootnoteReference(
            FootnoteId(0),
            1,
        )])],
        vec![],
        vec![],
        vec![FootnoteDefinition {
            label: "note".into(),
            content: vec![Block::Paragraph(vec![Inline::Text(
                "Footnote body.".into(),
            )])],
        }],
        vec![FootnoteId(0)],
        None,
    )
    .unwrap();
    let ctx = test_render_context();
    assert_eq!(footnote_preview_title(&doc, FootnoteId(0)), "[1] note");

    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    assert!(render_footnote_preview(
        &doc,
        FootnoteId(0),
        area,
        &mut buf,
        &ctx
    ));
    let mut text = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            text.push_str(buf[(x, y)].symbol());
        }
    }
    assert!(text.contains("Footnote body."));
}

#[test]
fn allocate_column_widths_returns_empty_for_zero_columns() {
    let table = Table {
        headers: vec![],
        rows: vec![],
        alignments: vec![],
    };
    assert!(allocate_column_widths(&table, 20).is_empty());
}

#[test]
fn render_table_row_pads_short_cells() {
    let ctx = test_render_context();
    let cells = vec![vec![Inline::Text("hi".into())]];
    let lines = render_table_row(
        &cells,
        &[8],
        &[Alignment::Left],
        ctx.theme.table_cell,
        &ctx,
        0,
    );
    assert_eq!(lines.len(), 1);
    let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(text.contains("hi"));
    assert!(text.starts_with('│'));
    assert!(text.ends_with('│'));
}

#[test]
fn render_table_row_respects_column_alignment() {
    let ctx = test_render_context();
    let cells = vec![vec![Inline::Text("hi".into())]];

    let left = render_table_row(
        &cells,
        &[6],
        &[Alignment::Left],
        ctx.theme.table_cell,
        &ctx,
        0,
    );
    let right = render_table_row(
        &cells,
        &[6],
        &[Alignment::Right],
        ctx.theme.table_cell,
        &ctx,
        0,
    );
    let center = render_table_row(
        &cells,
        &[6],
        &[Alignment::Center],
        ctx.theme.table_cell,
        &ctx,
        0,
    );

    let left_text: String = left[0].spans.iter().map(|s| s.content.as_ref()).collect();
    let right_text: String = right[0].spans.iter().map(|s| s.content.as_ref()).collect();
    let center_text: String = center[0].spans.iter().map(|s| s.content.as_ref()).collect();

    let hi_column = |text: &str| text.find('h').unwrap();
    assert!(hi_column(&left_text) < hi_column(&center_text));
    assert!(hi_column(&center_text) < hi_column(&right_text));
}

#[test]
fn parse_table_column_alignments() {
    let doc = parse("| L | C | R |\n|:---|:---:|---:|\n| a | b | c |").unwrap();
    let Block::Table(table) = &doc.blocks[0] else {
        panic!("expected table");
    };
    assert_eq!(
        table.alignments,
        vec![Alignment::Left, Alignment::Center, Alignment::Right]
    );
}

#[test]
fn render_code_block_draws_language_label_and_content() {
    let ctx = test_render_context();
    let cb = CodeBlock {
        language: Some("rust".into()),
        content: "fn main() {}".into(),
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
    render_code_block(&cb, Rect::new(0, 0, 20, 5), &mut buf, &ctx, 0);
    let row_0 = (0..20)
        .map(|x| buf.cell((x, 0)).map_or(" ", |c| c.symbol()))
        .collect::<String>();
    assert!(row_0.contains("rust"));
}

#[test]
fn long_document_renders_last_block_at_bottom_scroll() {
    let ctx = test_render_context();
    let blocks: Vec<Block> = (0..50)
        .map(|i| Block::Paragraph(vec![Inline::Text(format!("Paragraph {i}"))]))
        .collect();
    let document =
        Document::new(blocks, Vec::new(), Vec::new(), Vec::new(), Vec::new(), None).unwrap();
    let mut cache = DocumentRenderCache::default();
    cache.ensure(&document, &ctx, 80);
    let max_scroll = measure_document_height(&document, 80, &ctx) - 10;
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    cache.blit(max_scroll as f32, area, &mut buf);
    let last_row: String = (0..80).map(|x| buf[(x, 9)].symbol()).collect();
    assert!(last_row.starts_with("Paragraph 49"), "{last_row:?}");
}

#[test]
fn list_multiline_item_indents_properly() {
    let ctx = test_render_context();
    let input = "- very long item that wraps onto multiple lines because the terminal is narrow\n- second item\n\n## Next";
    let document = parse(input).unwrap();
    let width = 30u16;
    let height = 8u16;

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let widget = DocView(&document, &ctx);
            f.render_widget(widget, f.area());
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    for y in 0..height {
        let row: String = (0..width)
            .map(|x| {
                buf.cell((x, y)).map_or(' ', |c| {
                    let s = c.symbol();
                    if s.chars().next().map(|c| c.is_whitespace()).unwrap_or(false) {
                        ' '
                    } else {
                        s.chars().next().unwrap()
                    }
                })
            })
            .collect();
        eprintln!("{y:02}: {row:?}");
    }

    // Row 0 has the marker; rows 1-2 should be indented by the marker width.
    assert_eq!(buf.cell((0, 0)).map(|c| c.symbol()), Some("•"));
    assert_eq!(buf.cell((0, 1)).map(|c| c.symbol()), Some(" "));
    assert_eq!(buf.cell((1, 1)).map(|c| c.symbol()), Some(" "));
    assert_eq!(buf.cell((0, 2)).map(|c| c.symbol()), Some(" "));
    assert_eq!(buf.cell((1, 2)).map(|c| c.symbol()), Some(" "));
}

#[test]
fn list_item_with_multiple_blocks_indents_all() {
    let ctx = test_render_context();
    let input = "- first paragraph\n\n  ```rust\n  fn main() {}\n  ```\n- second item\n";
    let document = parse(input).unwrap();
    let width = 40u16;
    let height = 12u16;

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let widget = DocView(&document, &ctx);
            f.render_widget(widget, f.area());
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    for y in 0..height {
        let row: String = (0..width)
            .map(|x| {
                buf.cell((x, y)).map_or(' ', |c| {
                    let s = c.symbol();
                    if s.chars().next().map(|c| c.is_whitespace()).unwrap_or(false) {
                        ' '
                    } else {
                        s.chars().next().unwrap()
                    }
                })
            })
            .collect();
        eprintln!("{y:02}: {row:?}");
    }

    // First item's paragraph starts at x=2 because of the marker.
    assert_eq!(buf.cell((0, 0)).map(|c| c.symbol()), Some("•"));
    assert_eq!(buf.cell((1, 0)).map(|c| c.symbol()), Some(" "));
    // Code block inside the same item should also be indented.
    let code_label_row = 2;
    assert_eq!(buf.cell((0, code_label_row)).map(|c| c.symbol()), Some(" "));
    assert_eq!(buf.cell((1, code_label_row)).map(|c| c.symbol()), Some(" "));
}

#[test]
fn checklist_hit_region_matches_unicode_marker_width() {
    let document = parse("- [ ] click me").unwrap();
    let ctx = test_render_context();
    let hits = checklist::collect_checklist_hits(&document, 80, &ctx);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].line, 0);
    assert_eq!(hits[0].x, 0);
    assert_eq!(hits[0].width, ChecklistStyle::Unicode.marker_width());
}

#[test]
fn checklist_toggle_updates_marker_label() {
    let document = parse("- [ ] task").unwrap();
    let list = match &document.blocks[0] {
        Block::List(list) => list,
        _ => panic!("expected list"),
    };
    let item = &list.items[0];
    let mut state = ChecklistState::default();

    assert_eq!(
        super::list_marker::list_marker_label(list, 0, item, &state),
        "☐ "
    );
    state.toggle(item);
    assert_eq!(
        super::list_marker::list_marker_label(list, 0, item, &state),
        "☑ "
    );
}

#[test]
fn checklist_at_click_finds_item_on_marker_column() {
    let document = parse("- [ ] task").unwrap();
    let ctx = test_render_context();
    let item = checklist::checklist_at_click(&document, 80, &ctx, 0, 0);
    assert!(item.is_some());
    assert!(checklist::checklist_at_click(&document, 80, &ctx, 0, 2).is_none());
}

#[test]
fn link_at_click_finds_paragraph_link() {
    let document = parse("Click [here](https://example.com) now").unwrap();
    let hits = hits(&document);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].line, 0);
    assert_eq!(hits[0].x, 6);
    assert_eq!(hits[0].width, 4);
    assert_eq!(link_at(&hits, 0, 6), Some(LinkId(0)));
    assert!(link_at(&hits, 0, 5).is_none());
    assert!(link_at(&hits, 0, 10).is_none());
}

#[test]
fn link_at_click_finds_link_in_heading() {
    let document = parse("## See [docs](./README.md)").unwrap();
    assert_eq!(link_at(&hits(&document), 0, 8), Some(LinkId(0)));
}

#[test]
fn link_at_click_finds_link_in_table_cell() {
    let document = parse("| A | B |\n|---|---|\n| [link](https://example.com) | text |").unwrap();
    let hits = hits(&document);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].target, NavTarget::Link(LinkId(0)));
    // top border (0), header (1), separator (2), body row (3), bottom border (4)
    assert_eq!(hits[0].line, 3);
    assert_eq!(link_at(&hits, 3, hits[0].x), Some(LinkId(0)));
    assert!(link_at(&hits, 3, hits[0].x - 1).is_none());
}

#[test]
fn footnotes_section_renders_below_body_not_over_it() {
    let doc = parse("Body first line.\n\nSecond paragraph[^n].\n\n[^n]: The note.\n").unwrap();
    let ctx = test_render_context();
    let mut cache = DocumentRenderCache::default();
    cache.ensure(&doc, &ctx, 40);
    let rows: Vec<String> = (0..cache.total_height() as u16)
        .map(|y| (0..40).map(|x| cache.buffer()[(x, y)].symbol()).collect())
        .collect();
    assert!(rows[0].starts_with("Body first line."), "{rows:#?}");
    assert!(rows.last().unwrap().contains("The note."), "{rows:#?}");
    assert_eq!(
        rows.iter().filter(|row| row.contains("The note.")).count(),
        1,
        "{rows:#?}"
    );
}

fn rendered_rows(markdown: &str, width: u16) -> Vec<String> {
    let doc = parse(markdown).unwrap();
    let ctx = test_render_context();
    let mut cache = DocumentRenderCache::default();
    cache.ensure(&doc, &ctx, width);
    (0..cache.buffer().area.height)
        .map(|y| {
            let row: String = (0..width)
                .map(|x| cache.buffer()[(x, y)].symbol())
                .collect();
            row.trim_end().to_string()
        })
        .collect()
}

#[test]
fn styled_spans_do_not_gain_spaces_at_their_boundaries() {
    let rows = rendered_rows("**Bold**, *it*alic, `code`; [link](https://x.y). H~2~O", 80);
    assert_eq!(rows, ["Bold, italic, code; link. H2O"]);
}

#[test]
fn overlong_words_start_on_a_fresh_row_without_losing_spaces() {
    let rows = rendered_rows("a much longerword", 6);
    assert_eq!(rows, ["a much", "longer", "word"]);
}
