#!/usr/bin/env python3
"""Extract line ranges from render.rs into render/ submodules."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
RENDER = SRC / "render"
RENDER.mkdir(exist_ok=True)

lines = (SRC / "render.rs").read_text().splitlines(keepends=True)


def extract(start: int, end: int) -> str:
    return "".join(lines[start - 1 : end])


files: dict[str, str] = {}

files["theme.rs"] = extract(90, 181)
files["search_state.rs"] = extract(1407, 1434)
files["context.rs"] = extract(183, 214)
files["syntax.rs"] = extract(1494, 1520)
files["mermaid.rs"] = (
    extract(28, 88) + extract(457, 465) + extract(1048, 1073)
)
files["inline.rs"] = (
    extract(590, 599)
    + extract(652, 677)
    + extract(1084, 1405)
    + extract(1436, 1492)
)
files["table.rs"] = extract(846, 1042)  # stop before allocate_column_widths wrapper
files["measure.rs"] = extract(308, 455)
files["blocks.rs"] = extract(467, 589) + extract(601, 651) + extract(680, 845) + extract(1075, 1082)
files["search.rs"] = extract(1522, 1735)
files["widget.rs"] = extract(216, 306)
files["cache.rs"] = extract(1737, 1844)

# tests: keep mod tests closing brace
tests_raw = extract(1848, len(lines))
files["tests.rs"] = tests_raw

headers = {
    "theme.rs": "//! Visual theme.\n\nuse ratatui::style::{Color, Modifier, Style};\n\n",
    "search_state.rs": "//! Search-state helpers for render context.\n\nuse crate::domain::SearchState;\n\n",
    "context.rs": (
        "//! Render context.\n\n"
        "use syntect::highlighting::Theme as SyntectTheme;\n"
        "use syntect::parsing::SyntaxSet;\n\n"
        "use crate::domain::{LinkId, ViewState};\n\n"
        "use super::mermaid::RenderedDocument;\n"
        "use super::search_state::{\n"
        "    active_search_match_index, active_search_match_line_offset, active_search_query,\n"
        "};\n"
        "use super::theme::Theme;\n\n"
    ),
    "syntax.rs": (
        "//! Syntect assets.\n\n"
        "use syntect::{\n"
        "    highlighting::{Theme as SyntectTheme, ThemeSet},\n"
        "    parsing::SyntaxSet,\n"
        "};\n\n"
    ),
    "mermaid.rs": (
        "//! Mermaid diagram rendering.\n\n"
        "use std::collections::HashMap;\n\n"
        "use merman::render::{HeadlessRenderer, raster::RasterOptions};\n"
        "use ratatui::{\n"
        "    buffer::Buffer,\n"
        "    layout::{Rect, Size},\n"
        "    text::{Line, Text},\n"
        "    widgets::{Paragraph, Widget},\n"
        "};\n"
        "use ratatui_image::{Resize, protocol::Protocol};\n\n"
        "use crate::domain::{Block, Document, MermaidDiagram};\n"
        "use crate::error::AppError;\n\n"
        "use super::context::RenderContext;\n\n"
    ),
    "inline.rs": (
        "//! Inline text conversion and search highlighting.\n\n"
        "use ratatui::{\n"
        "    style::{Color, Modifier, Style},\n"
        "    text::{Line, Span, Text},\n"
        "};\n"
        "use unicode_width::UnicodeWidthStr;\n\n"
        "use crate::domain::{HeadingLevel, Inline};\n\n"
        "use super::context::RenderContext;\n"
        "use super::theme::Theme;\n\n"
    ),
    "table.rs": (
        "//! Table layout and rendering.\n\n"
        "use ratatui::{\n"
        "    buffer::Buffer,\n"
        "    layout::Rect,\n"
        "    style::Style,\n"
        "    text::{Line, Span},\n"
        "    widgets::Widget,\n"
        "};\n"
        "use unicode_width::UnicodeWidthStr;\n\n"
        "use crate::domain::{Inline, Table};\n\n"
        "use super::context::RenderContext;\n"
        "use super::inline::inlines_to_wrapped_lines;\n\n"
        "pub(crate) fn allocate_column_widths(table: &Table, total_width: usize) -> Vec<usize> {\n"
        "    table.allocate_column_widths(total_width)\n"
        "}\n\n"
    ),
    "measure.rs": (
        "//! Document height measurement.\n\n"
        "use unicode_width::UnicodeWidthStr;\n\n"
        "use crate::domain::{Block, CodeBlock, Document, Heading, Inline, List, Table};\n\n"
        "use super::context::RenderContext;\n"
        "use super::inline::{heading_styles, inlines_to_wrapped_lines};\n"
        "use super::mermaid::measure_mermaid_height;\n"
        "use super::table::{allocate_column_widths, wrap_cell_inlines};\n\n"
    ),
    "blocks.rs": (
        "//! Block-level rendering.\n\n"
        "use ratatui::{\n"
        "    buffer::Buffer,\n"
        "    layout::Rect,\n"
        "    style::{Color, Style},\n"
        "    text::{Line, Span, Text},\n"
        "    widgets::{Paragraph, Widget},\n"
        "};\n"
        "use syntect::{easy::HighlightLines, util::LinesWithEndings};\n"
        "use unicode_width::UnicodeWidthStr;\n\n"
        "use crate::domain::{Block, CodeBlock, Heading, HeadingLevel, Inline, List};\n\n"
        "use super::context::RenderContext;\n"
        "use super::inline::{heading_styles, highlight_line, inlines_to_wrapped_lines, syntect_span};\n"
        "use super::measure::measure_block_height;\n"
        "use super::mermaid::render_mermaid;\n"
        "use super::table::render_table;\n\n"
    ),
    "search.rs": (
        "//! Search match discovery.\n\n"
        "use ratatui::{\n"
        "    style::Style,\n"
        "    text::Line,\n"
        "};\n"
        "use unicode_width::UnicodeWidthStr;\n\n"
        "use crate::domain::{\n"
        "    Block, CodeBlock, Document, Inline, List, SearchMatch, Table,\n"
        "};\n\n"
        "use super::context::RenderContext;\n"
        "use super::inline::{heading_styles, inlines_to_wrapped_lines};\n"
        "use super::measure::measure_block_height;\n"
        "use super::table::{allocate_column_widths, wrap_cell_inlines};\n\n"
    ),
    "widget.rs": (
        "//! Markdown scroll widget.\n\n"
        "use ratatui::{\n"
        "    buffer::Buffer,\n"
        "    layout::Rect,\n"
        "    widgets::Widget,\n"
        "};\n\n"
        "use crate::domain::{Document, ViewState};\n\n"
        "use super::blocks::render_block;\n"
        "use super::context::RenderContext;\n"
        "use super::measure::measure_block_height;\n\n"
    ),
    "cache.rs": (
        "//! Document render cache.\n\n"
        "use ratatui::{\n"
        "    buffer::Buffer,\n"
        "    layout::Rect,\n"
        "    widgets::Widget,\n"
        "};\n\n"
        "use crate::domain::{Document, LinkId, ViewState};\n\n"
        "use super::context::RenderContext;\n"
        "use super::measure::measure_document_height;\n"
        "use super::widget::MarkdownWidget;\n\n"
    ),
    "tests.rs": (
        "use std::collections::HashMap;\n\n"
        "use super::{\n"
        "    find_search_matches, measure_block_height, measure_document_height, CachedMarkdownView,\n"
        "    DocumentRenderCache, MarkdownWidget, RenderContext, RenderedDocument, Theme,\n"
        "};\n"
        "use super::blocks::render_code_block;\n"
        "use super::inline::{highlight_span, highlight_text, inlines_to_text, inlines_to_wrapped_lines};\n"
        "use super::measure::measure_code_block_height;\n"
        "use super::table::{allocate_column_widths, render_table_row, wrap_cell_inlines};\n"
        "use crate::domain::{\n"
        "    Alignment, Block, CodeBlock, Document, Inline, List, ListItem, SearchDirection,\n"
        "    SearchMatch, Table, TerminalSize, ViewState,\n"
        "};\n"
        "use crate::parse::parse;\n"
        "use ratatui::Terminal;\n"
        "use ratatui::backend::TestBackend;\n"
        "use ratatui::buffer::Buffer;\n"
        "use ratatui::layout::Rect;\n"
        "use ratatui::style::{Color, Modifier, Style};\n"
        "use ratatui::text::{Line, Span, Text};\n"
        "use syntect::highlighting::ThemeSet;\n"
        "use syntect::parsing::SyntaxSet;\n"
        "use unicode_width::UnicodeWidthStr;\n\n"
    ),
}

pub_items = {
    "theme.rs": ["Theme"],
    "context.rs": ["RenderContext"],
    "mermaid.rs": ["RenderedDocument"],
    "syntax.rs": ["SyntaxAssets"],
    "measure.rs": ["measure_document_height", "measure_block_height"],
    "search.rs": ["find_search_matches"],
    "cache.rs": ["DocumentRenderCache", "CachedMarkdownView"],
}

pub_crate_items = {
    "mermaid.rs": ["measure_mermaid_height", "render_mermaid"],
    "inline.rs": ["inlines_to_wrapped_lines", "heading_styles", "highlight_line", "syntect_span"],
    "table.rs": ["wrap_cell_inlines", "render_table"],
    "blocks.rs": ["render_block"],
    "widget.rs": ["MarkdownWidget"],
    "search_state.rs": [
        "active_search_query",
        "active_search_match_index",
        "active_search_match_line_offset",
    ],
}


def vis(body: str, fname: str) -> str:
    out = []
    for line in body.splitlines(keepends=True):
        s = line.lstrip()
        if s.startswith("pub ") or s.startswith("#["):
            out.append(line)
            continue
        if s.startswith("fn ") or s.startswith("struct ") or s.startswith("enum "):
            name = s.split("(")[0].split("{")[0].split(" ")[1]
            if name in pub_items.get(fname, []):
                if s.startswith("fn "):
                    line = line.replace("fn ", "pub fn ", 1)
                elif s.startswith("struct "):
                    line = line.replace("struct ", "pub struct ", 1)
            elif name in pub_crate_items.get(fname, []):
                if s.startswith("fn "):
                    line = line.replace("fn ", "pub(crate) fn ", 1)
        out.append(line)
    return "".join(out)


for fname, body in files.items():
    content = headers[fname] + vis(body, fname)
    (RENDER / fname).write_text(content)

mod_rs = """//! Rendering: domain model -> ratatui widgets.

mod blocks;
mod cache;
mod context;
mod inline;
mod measure;
mod mermaid;
mod search;
mod search_state;
mod syntax;
mod table;
mod theme;
mod widget;

#[cfg(test)]
mod tests;

pub use cache::{CachedMarkdownView, DocumentRenderCache};
pub use context::RenderContext;
pub use mermaid::RenderedDocument;
pub use search::find_search_matches;
pub use syntax::SyntaxAssets;
pub use measure::measure_document_height;
pub use theme::Theme;
"""
(RENDER / "mod.rs").write_text(mod_rs)
(SRC / "render.rs").unlink()
print("done")
