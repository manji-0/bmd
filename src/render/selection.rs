//! Text selection extraction and viewport highlighting.

use ratatui::{buffer::Buffer, layout::Rect, style::Style};

use crate::domain::TextSelection;

use super::cache::DocumentRenderCache;

/// Extract plain text for `selection` from a pre-rendered document buffer.
pub fn extract_selected_text(cache: &DocumentRenderCache, selection: TextSelection) -> String {
    if selection.is_empty() {
        return String::new();
    }
    let (start, end) = selection.normalized_inclusive();
    let buffer = cache.buffer();
    let width = buffer.area().width as usize;
    let height = buffer.area().height as usize;
    if width == 0 || height == 0 {
        return String::new();
    }

    let mut out = String::new();
    for line in start.line..=end.line {
        if line >= height {
            break;
        }
        let col_start = if line == start.line { start.col } else { 0 };
        let col_end = if line == end.line {
            end.col
        } else {
            last_content_col(buffer, line, width)
        };
        if col_start <= col_end {
            for col in col_start..=col_end.min(width.saturating_sub(1)) {
                if let Some(cell) = buffer.cell((col as u16, line as u16)) {
                    out.push_str(cell.symbol());
                }
            }
        }
        if line < end.line {
            out.push('\n');
        }
    }
    out
}

fn last_content_col(buffer: &Buffer, line: usize, width: usize) -> usize {
    (0..width)
        .rev()
        .find(|&col| {
            buffer
                .cell((col as u16, line as u16))
                .is_some_and(|cell| cell.symbol() != " ")
        })
        .unwrap_or(width.saturating_sub(1))
}

/// Highlight the selected region over a viewport that starts at fractional `scroll`.
pub fn paint_selection_overlay(
    buf: &mut Buffer,
    area: Rect,
    scroll: f32,
    selection: TextSelection,
    style: Style,
) {
    if selection.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }
    let (start, end) = selection.normalized_inclusive();
    let scroll_line = scroll.floor() as usize;

    for row in 0..area.height as usize {
        let logical_line = scroll_line + row;
        if logical_line < start.line || logical_line > end.line {
            continue;
        }
        let col_start = if logical_line == start.line {
            start.col
        } else {
            0
        };
        let col_end = if logical_line == end.line {
            end.col
        } else {
            area.width as usize - 1
        };
        for col in col_start..=col_end.min(area.width as usize - 1) {
            let x = area.x + col as u16;
            let y = area.y + row as u16;
            let cell = &mut buf[(x, y)];
            let mut cell_style = cell.style();
            if let Some(bg) = style.bg {
                cell_style.bg = Some(bg);
            }
            if let Some(fg) = style.fg {
                cell_style.fg = Some(fg);
            }
            cell.set_style(cell_style);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::domain::{
        Block, ChecklistState, ChecklistStyle, Document, Inline, TextPoint, ViewState,
    };
    use crate::render::{
        DocumentRenderCache, RenderContext, RenderedDocument, SyntaxAssets, Theme,
    };

    fn render_cache(text: &str) -> DocumentRenderCache {
        let document = Document {
            blocks: vec![Block::Paragraph(vec![Inline::Text(text.to_string())])],
            links: vec![],
            mermaid_diagrams: vec![],
            footnotes: vec![],
            footnote_order: vec![],
            front_matter: None,
        };
        let theme = Theme::default();
        let syntax = SyntaxAssets::new();
        let rendered = RenderedDocument {
            mermaid_images: HashMap::new(),
            markdown_images: HashMap::new(),
        };
        let view_state = ViewState::new(crate::domain::TerminalSize::new(40, 24).unwrap());
        let checklist = ChecklistState::new(ChecklistStyle::Unicode);
        let ctx = RenderContext::new(
            &theme,
            &syntax,
            &rendered,
            &[],
            &view_state,
            true,
            &checklist,
        );
        let mut cache = DocumentRenderCache::default();
        cache.ensure(&document, &ctx, &view_state, 40);
        cache
    }

    #[test]
    fn extract_single_line_selection() {
        let cache = render_cache("hello world");
        let text = extract_selected_text(
            &cache,
            TextSelection::new(TextPoint::new(0, 0), TextPoint::new(0, 4)),
        );
        assert_eq!(text, "hello");
    }

    #[test]
    fn extract_reversed_selection() {
        let cache = render_cache("abcdef");
        let text = extract_selected_text(
            &cache,
            TextSelection::new(TextPoint::new(0, 3), TextPoint::new(0, 1)),
        );
        assert_eq!(text, "bcd");
    }
}
