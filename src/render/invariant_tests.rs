//! Render invariants checked against every sample document at several widths.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::document::render_document;
use super::{RenderContext, SyntaxAssets, Theme, find_search_matches, measure_document_height};
use crate::domain::{ChecklistState, ChecklistStyle, Document};
use crate::parse::{MarkupFormat, parse_document};

const WIDTHS: [u16; 4] = [24, 40, 80, 132];

pub(crate) const SAMPLES: [(&str, MarkupFormat, &str); 5] = [
    (
        "kitchen-sink.md",
        MarkupFormat::Markdown,
        include_str!("../../tests/fixtures/kitchen-sink.md"),
    ),
    (
        "sample.md",
        MarkupFormat::Markdown,
        include_str!("../../sample.md"),
    ),
    (
        "sample-gfm.md",
        MarkupFormat::Markdown,
        include_str!("../../sample-gfm.md"),
    ),
    (
        "sample.rst",
        MarkupFormat::Rest,
        include_str!("../../sample.rst"),
    ),
    (
        "sample.adoc",
        MarkupFormat::AsciiDoc,
        include_str!("../../sample.adoc"),
    ),
];

struct Fixture {
    theme: Theme,
    assets: SyntaxAssets,
    checklist: ChecklistState,
}

impl Fixture {
    fn new() -> Self {
        Self {
            theme: Theme::default(),
            assets: SyntaxAssets::new(),
            checklist: ChecklistState::new(ChecklistStyle::Unicode),
        }
    }

    fn ctx<'a>(&'a self, document: &'a Document) -> RenderContext<'a> {
        RenderContext {
            theme: &self.theme,
            syntax_set: &self.assets.syntax_set,
            syntax_theme: self.assets.theme(),
            links: &document.links,
            selected_link: None,
            selected_footnote: None,
            search_query: None,
            selected_match_line_offset: None,
            checklist_state: &self.checklist,
        }
    }
}

fn render(document: &Document, ctx: &RenderContext, width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    render_document(document, area, &mut buf, ctx);
    buf
}

fn row(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
}

fn for_each_sample(check: impl Fn(&str, &Document, &RenderContext, u16)) {
    let fixture = Fixture::new();
    for (name, format, content) in SAMPLES {
        let document = parse_document(format, content).unwrap_or_else(|e| panic!("{name}: {e}"));
        let ctx = fixture.ctx(&document);
        for width in WIDTHS {
            check(name, &document, &ctx, width);
        }
    }
}

#[test]
fn measured_height_matches_rendered_extent() {
    for_each_sample(|name, document, ctx, width| {
        let total = measure_document_height(document, width, ctx);
        let spare = 3;
        let buf = render(document, ctx, width, u16::try_from(total + spare).unwrap());
        let last = row(&buf, total as u16 - 1);
        assert!(
            !last.trim().is_empty(),
            "{name}@{width}: height overestimated"
        );
        for y in total..total + spare {
            let extra = row(&buf, y as u16);
            assert!(
                extra.trim().is_empty(),
                "{name}@{width}: drew past height: {extra:?}"
            );
        }
    });
}

#[test]
fn search_matches_land_on_rows_containing_the_query() {
    for_each_sample(|name, document, ctx, width| {
        let total = measure_document_height(document, width, ctx);
        let full = render(document, ctx, width, u16::try_from(total).unwrap());
        for query in ["bmd", "link", "b"] {
            for found in find_search_matches(document, width, query, ctx) {
                let line = found.line_offset;
                assert!(line < total, "{name}@{width}: match beyond document");
                let text = row(&full, line as u16).to_lowercase();
                assert!(
                    text.contains(query),
                    "{name}@{width}: row {line} {text:?} lacks {query:?}"
                );
            }
        }
    });
}
