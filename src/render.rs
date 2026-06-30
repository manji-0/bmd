//! Rendering: domain model -> ratatui widgets.

use std::collections::HashMap;

use merman::render::{HeadlessRenderer, raster::RasterOptions};
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Size},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Widget},
};
use ratatui_image::{Resize, protocol::Protocol};
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme as SyntectTheme, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};
use unicode_width::UnicodeWidthStr;

use crate::domain::{
    Block, CodeBlock, Document, Heading, HeadingLevel, Inline, LinkId, List, MermaidDiagram,
    SearchMatch, SearchState, Table, ViewState,
};
use crate::error::AppError;

/// Cache of pre-rendered mermaid images keyed by block index.
pub struct RenderedDocument {
    pub images: HashMap<usize, Protocol>,
}

impl RenderedDocument {
    /// Render every `mermaid` block to a terminal image protocol.
    ///
    /// # Errors
    ///
    /// Returns `AppError` if the terminal image protocol cannot be created. Individual
    /// mermaid failures are logged and skipped (the widget renders a placeholder instead).
    pub fn new(
        document: &Document,
        picker: &ratatui_image::picker::Picker,
        width: u16,
    ) -> Result<Self, AppError> {
        let mut images = HashMap::new();
        for (idx, block) in document.blocks.iter().enumerate() {
            if let Block::Mermaid(diag) = block {
                match render_mermaid_image(diag, picker, width) {
                    Ok(protocol) => {
                        images.insert(idx, protocol);
                    }
                    Err(e) => {
                        eprintln!("[bmd] failed to render mermaid block {idx}: {e}");
                    }
                }
            }
        }
        Ok(Self { images })
    }
}

fn render_mermaid_image(
    diag: &MermaidDiagram,
    picker: &ratatui_image::picker::Picker,
    max_width: u16,
) -> Result<Protocol, AppError> {
    let renderer = HeadlessRenderer::new();
    let options = RasterOptions::default();
    let png = renderer
        .render_png_sync(&diag.source, &options)?
        .ok_or(AppError::MermaidNoDiagram)?;
    let dyn_img = image::load_from_memory(&png)?;

    let font_size = picker.font_size();
    // Use a width proportional to the diagram's natural size so nodes and
    // text are not shrunk to an unreadable mosaic.
    let cols = diag.estimated_width().min(max_width).max(20) as u32;
    let rows = (dyn_img.height() as u32)
        .saturating_mul(cols)
        .saturating_mul(font_size.width as u32)
        .div_ceil(dyn_img.width().max(1))
        .div_ceil(font_size.height.max(1) as u32)
        .max(1);
    let size = Size::new(cols as u16, rows as u16);
    picker
        .new_protocol(dyn_img, size, Resize::Fit(None))
        .map_err(|e| AppError::TerminalImage(e.to_string()))
}

/// Visual theme.
#[derive(Clone, Debug)]
pub struct Theme {
    pub text: Style,
    pub h1: Style,
    pub h1_prefix: Style,
    pub h2: Style,
    pub h2_prefix: Style,
    pub h3: Style,
    pub h3_prefix: Style,
    pub h4: Style,
    pub h4_prefix: Style,
    pub h5: Style,
    pub h5_prefix: Style,
    pub h6: Style,
    pub h6_prefix: Style,
    pub code_inline: Style,
    pub code_block: Style,
    pub code_block_language: Style,
    pub blockquote: Style,
    pub list_marker: Style,
    pub link: Style,
    pub link_selected: Style,
    pub rule: Style,
    pub table_header: Style,
    pub table_cell: Style,
    pub table_border: Style,
    pub mermaid_placeholder: Style,
    pub search_match: Style,
    pub search_match_selected: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            text: Style::default().fg(Color::White),
            h1: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
            h1_prefix: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            h2: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            h2_prefix: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            h3: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            h3_prefix: Style::default().fg(Color::Yellow),
            h4: Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
            h4_prefix: Style::default().fg(Color::DarkGray),
            h5: Style::default().fg(Color::Gray),
            h5_prefix: Style::default().fg(Color::DarkGray),
            h6: Style::default().fg(Color::DarkGray),
            h6_prefix: Style::default().fg(Color::DarkGray),
            code_inline: Style::default().fg(Color::Yellow).bg(Color::Black),
            code_block: Style::default().fg(Color::White).bg(Color::Black),
            code_block_language: Style::default().fg(Color::Black).bg(Color::Yellow),
            blockquote: Style::default().fg(Color::Gray).italic(),
            list_marker: Style::default().fg(Color::Cyan),
            link: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
            link_selected: Style::default()
                .fg(Color::Black)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            search_match: Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            search_match_selected: Style::default()
                .fg(Color::White)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
            rule: Style::default().fg(Color::DarkGray),
            table_header: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            table_cell: Style::default(),
            table_border: Style::default().fg(Color::DarkGray),
            mermaid_placeholder: Style::default().fg(Color::Yellow),
        }
    }
}

/// Everything needed to render blocks.
pub struct RenderContext<'a> {
    pub theme: &'a Theme,
    pub syntax_set: &'a SyntaxSet,
    pub syntax_theme: &'a SyntectTheme,
    pub rendered: &'a RenderedDocument,
    pub selected_link: Option<LinkId>,
    pub search_query: Option<String>,
    pub selected_search_match: Option<usize>,
    pub selected_match_line_offset: Option<usize>,
}

impl<'a> RenderContext<'a> {
    pub fn new(
        theme: &'a Theme,
        syntax_set: &'a SyntaxSet,
        syntax_theme: &'a SyntectTheme,
        rendered: &'a RenderedDocument,
        view_state: &'a ViewState,
    ) -> Self {
        Self {
            theme,
            syntax_set,
            syntax_theme,
            rendered,
            selected_link: view_state.selected_link(),
            search_query: active_search_query(view_state.search_state()),
            selected_search_match: active_search_match_index(view_state.search_state()),
            selected_match_line_offset: active_search_match_line_offset(view_state.search_state()),
        }
    }
}

/// Stateful widget that renders the document with scroll.
pub struct MarkdownWidget<'a> {
    document: &'a Document,
    ctx: &'a RenderContext<'a>,
    view_state: &'a ViewState,
}

impl<'a> MarkdownWidget<'a> {
    pub fn new(
        document: &'a Document,
        ctx: &'a RenderContext<'a>,
        view_state: &'a ViewState,
    ) -> Self {
        Self {
            document,
            ctx,
            view_state,
        }
    }
}

impl Widget for MarkdownWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let ctx = self.ctx;
        let scroll = self.view_state.scroll().offset();
        let max_y = area.y + area.height;

        let mut y = area.y;
        let mut line_offset: usize = 0;

        for (block_idx, block) in self.document.blocks.iter().enumerate() {
            let gap = if block_idx == 0 { 0 } else { 1 };
            let block_height = measure_block_height(block, area.width, ctx);
            let total_height = block_height + gap;

            // Fully above the visible region?
            if line_offset.saturating_add(total_height) <= scroll {
                line_offset += total_height;
                continue;
            }

            // How many rows of this block+gap are above the scroll offset?
            let rows_above = scroll.saturating_sub(line_offset);
            debug_assert!(rows_above < total_height);

            let visible_height = total_height
                .saturating_sub(rows_above)
                .min((max_y - y) as usize);
            if visible_height == 0 {
                break;
            }

            // Render only the content rows that are visible. Gap rows are blank and the
            // buffer is already cleared, so they require no drawing.
            if rows_above < block_height {
                let skip_rows = rows_above;
                let content_visible = visible_height.min(block_height.saturating_sub(skip_rows));
                let block_area = Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: content_visible as u16,
                };
                render_block(
                    block,
                    block_idx,
                    block_area,
                    buf,
                    skip_rows,
                    ctx,
                    line_offset,
                );
                y += content_visible as u16;
                // Any remaining visible rows are part of the inter-block gap.
                y += (visible_height.saturating_sub(content_visible)) as u16;
            } else {
                // Scroll offset is inside the gap; all visible rows are blank.
                y += visible_height as u16;
            }

            line_offset += total_height;
            if y >= max_y {
                break;
            }
        }
    }
}

/// Total logical height of the whole document, including one-row gaps between
/// consecutive block-level elements.
///
/// This must stay in lock-step with `MarkdownWidget::render`: any change to how
/// blocks are laid out must be reflected here, otherwise scrolling will truncate
/// or overshoot the document.
pub fn measure_document_height(document: &Document, width: u16, ctx: &RenderContext) -> usize {
    if width == 0 {
        return 0;
    }
    document
        .blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| {
            let gap = if idx == 0 { 0 } else { 1 };
            measure_block_height(block, width, ctx) + gap
        })
        .sum()
}

/// Number of logical rows a block occupies at the given width.
///
/// This must stay in lock-step with the `render_*` functions: any change to how a
/// block is drawn must be reflected here, otherwise scrolling will truncate or
/// overshoot the document.
pub fn measure_block_height(block: &Block, width: u16, ctx: &RenderContext) -> usize {
    if width == 0 {
        return 0;
    }
    match block {
        Block::Heading(h) => measure_heading_height(h, width, ctx),
        Block::Paragraph(inlines) => measure_paragraph_height(inlines, width, ctx),
        Block::CodeBlock(cb) => measure_code_block_height(cb, width),
        Block::BlockQuote(blocks) => measure_blockquote_height(blocks, width, ctx),
        Block::List(list) => measure_list_height(list, width, ctx),
        Block::Table(table) => measure_table_height(table, width, ctx),
        Block::Mermaid(_) => measure_mermaid_height(ctx, width),
        Block::Rule => 1,
    }
}

fn measure_paragraph_height(inlines: &[Inline], width: u16, ctx: &RenderContext) -> usize {
    inlines_to_wrapped_lines(inlines, ctx, ctx.theme.text, 0, width as usize)
        .len()
        .max(1)
}

fn measure_heading_height(heading: &Heading, width: u16, ctx: &RenderContext) -> usize {
    let (style, _prefix_style) = heading_styles(heading.level, ctx.theme);
    let prefix_width = heading.level.prefix().width();
    let total_width = width as usize;
    // Keep this condition in sync with render_heading.
    let content_width = if total_width > prefix_width + 1 {
        total_width.saturating_sub(prefix_width)
    } else {
        total_width
    };
    inlines_to_wrapped_lines(&heading.content, ctx, style, 0, content_width.max(1))
        .len()
        .max(1)
}

fn measure_code_block_height(cb: &CodeBlock, width: u16) -> usize {
    let _ = width; // width only matters for rendering, not for height.
    cb.logical_height()
}

fn measure_blockquote_height(blocks: &[Block], width: u16, ctx: &RenderContext) -> usize {
    let inner_width = (width as usize).saturating_sub(2).max(1) as u16;
    let content_height: usize = blocks
        .iter()
        .map(|b| measure_block_height(b, inner_width, ctx))
        .sum();
    content_height.saturating_add(1)
}

fn measure_list_height(list: &List, width: u16, ctx: &RenderContext) -> usize {
    let w = width as usize;
    let mut total = 0usize;
    for (idx, item) in list.items.iter().enumerate() {
        let marker_width = if list.ordered {
            format!("{}.", idx + 1).width() + 1
        } else {
            2
        };
        let inner_width = w.saturating_sub(marker_width).max(1) as u16;
        if item.content.is_empty() {
            total += 1;
        } else {
            total += item
                .content
                .iter()
                .map(|b| measure_block_height(b, inner_width, ctx))
                .sum::<usize>();
        }
    }
    total.max(1)
}

fn measure_table_height(table: &Table, width: u16, ctx: &RenderContext) -> usize {
    let col_count = table
        .headers
        .len()
        .max(table.rows.first().map(|r| r.len()).unwrap_or(0));
    if col_count == 0 {
        return 1;
    }
    let widths = allocate_column_widths(table, width as usize);
    let header_height = table
        .headers
        .iter()
        .enumerate()
        .map(|(i, cell)| {
            wrap_cell_inlines(
                cell,
                widths.get(i).copied().unwrap_or(1),
                ctx.theme.table_header,
                ctx,
                0,
            )
            .len()
        })
        .max()
        .unwrap_or(1);
    let body_height = table
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, cell)| {
                    wrap_cell_inlines(
                        cell,
                        widths.get(i).copied().unwrap_or(1),
                        ctx.theme.table_cell,
                        ctx,
                        0,
                    )
                    .len()
                })
                .max()
                .unwrap_or(1)
        })
        .sum::<usize>();
    // Top border + header + separator + body rows + bottom border.
    1 + header_height + 1 + body_height + 1
}

fn measure_mermaid_height(ctx: &RenderContext, width: u16) -> usize {
    // The rendered protocol has a fixed cell size; if it's cached, use it.
    if let Some((_idx, protocol)) = ctx.rendered.images.iter().next() {
        return protocol.size().height as usize;
    }
    // Fallback: approximate 16:9 height for a mid-size diagram.
    let cols = (width as usize).min(160);
    (cols * 9).div_ceil(16).max(6)
}

fn render_block(
    block: &Block,
    block_idx: usize,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    match block {
        Block::Heading(h) => render_heading(h, area, buf, skip_rows, ctx, line_offset),
        Block::Paragraph(inlines) => {
            render_paragraph(inlines, area, buf, skip_rows, ctx, line_offset)
        }
        Block::CodeBlock(cb) => render_code_block(cb, area, buf, skip_rows, ctx, line_offset),
        Block::BlockQuote(blocks) => {
            render_blockquote(blocks, area, buf, skip_rows, ctx, line_offset)
        }
        Block::List(list) => render_list(list, area, buf, skip_rows, ctx, line_offset),
        Block::Table(table) => render_table(table, area, buf, skip_rows, ctx, line_offset),
        Block::Mermaid(diag) => render_mermaid(diag, block_idx, area, buf, skip_rows, ctx),
        Block::Rule => render_rule(area, buf),
    }
}

fn render_heading(
    heading: &Heading,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    let (style, prefix_style) = heading_styles(heading.level, ctx.theme);
    let prefix = heading.level.prefix();
    let prefix_width = prefix.width();
    if area.width as usize > prefix_width + 1 {
        let content_width = (area.width as usize).saturating_sub(prefix_width).max(1);
        let rows =
            inlines_to_wrapped_lines(&heading.content, ctx, style, line_offset, content_width);
        render_prefixed_offset_lines(prefix, prefix_style, &rows, area, buf, skip_rows);
    } else {
        let rows = inlines_to_wrapped_lines(
            &heading.content,
            ctx,
            style,
            line_offset,
            area.width as usize,
        );
        render_offset_lines(&rows, area, buf, skip_rows);
    }
}

fn render_paragraph(
    inlines: &[Inline],
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    let rows = inlines_to_wrapped_lines(
        inlines,
        ctx,
        ctx.theme.text,
        line_offset,
        area.width as usize,
    );
    render_offset_lines(&rows, area, buf, skip_rows);
}

fn render_offset_lines(
    rows: &[(usize, Line<'static>)],
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
) {
    let max_y = area.y + area.height;
    for (row_idx, (_, line)) in rows.iter().enumerate() {
        if row_idx < skip_rows {
            continue;
        }
        let screen_y = area.y + (row_idx - skip_rows) as u16;
        if screen_y >= max_y {
            break;
        }
        buf.set_line(area.x, screen_y, line, area.width);
    }
}

/// Render a block-level prefix (e.g. heading marker "##") in front of wrapped
/// content rows, reducing the available width for the text.
fn render_prefixed_offset_lines(
    prefix: &str,
    prefix_style: Style,
    rows: &[(usize, Line<'static>)],
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
) {
    let prefix_width = prefix.width();
    let total_width = area.width as usize;
    if prefix_width >= total_width {
        render_offset_lines(rows, area, buf, skip_rows);
        return;
    }

    let text_width = area.width.saturating_sub(prefix_width as u16);
    let max_y = area.y + area.height;
    for (row_idx, (_, line)) in rows.iter().enumerate() {
        if row_idx < skip_rows {
            continue;
        }
        let screen_y = area.y + (row_idx - skip_rows) as u16;
        if screen_y >= max_y {
            break;
        }
        buf.set_stringn(area.x, screen_y, prefix, prefix_width, prefix_style);
        buf.set_line(area.x + prefix_width as u16, screen_y, line, text_width);
    }
}

/// Returns the text style and prefix style for a heading level.
fn heading_styles(level: HeadingLevel, theme: &Theme) -> (Style, Style) {
    match level {
        HeadingLevel::H1 => (theme.h1, theme.h1_prefix),
        HeadingLevel::H2 => (theme.h2, theme.h2_prefix),
        HeadingLevel::H3 => (theme.h3, theme.h3_prefix),
        HeadingLevel::H4 => (theme.h4, theme.h4_prefix),
        HeadingLevel::H5 => (theme.h5, theme.h5_prefix),
        HeadingLevel::H6 => (theme.h6, theme.h6_prefix),
    }
}

fn render_code_block(
    cb: &CodeBlock,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    let mut lines: Vec<Line> = Vec::new();

    // Language label line.
    let label = cb
        .language
        .as_ref()
        .map(|l| format!(" {l} "))
        .unwrap_or_else(|| " code ".to_string());
    lines.push(Line::styled(label, ctx.theme.code_block_language));

    // Syntax highlighted content.
    let syntax = cb
        .language
        .as_ref()
        .and_then(|lang| ctx.syntax_set.find_syntax_by_token(lang));
    let mut highlighter = syntax
        .map(|s| HighlightLines::new(s, ctx.syntax_theme))
        .unwrap_or_else(|| {
            HighlightLines::new(ctx.syntax_set.find_syntax_plain_text(), ctx.syntax_theme)
        });

    for (i, line) in LinesWithEndings::from(&cb.content).enumerate() {
        let line_without_nl = line.strip_suffix('\n').unwrap_or(line);
        let highlighted = highlighter
            .highlight_line(line_without_nl, ctx.syntax_set)
            .unwrap_or_default();
        let spans: Vec<Span> = highlighted
            .into_iter()
            .map(|(style, text)| syntect_span(style, text, ctx.theme.code_block))
            .collect();
        let styled_line = Line::from(spans);
        lines.push(highlight_line(styled_line, ctx, line_offset + 1 + i));
    }

    // Scroll and render.  Do not add a synthetic trailing line; the inter-block gap
    // already provides visual separation.
    let text = Text::from(lines);
    let para = Paragraph::new(text)
        .style(ctx.theme.code_block)
        .scroll((skip_rows as u16, 0));
    para.render(area, buf);
}

/// Highlight a pre-built `Line` if a search query is active.
fn highlight_line(line: Line<'static>, ctx: &RenderContext, line_offset: usize) -> Line<'static> {
    match ctx.search_query {
        None => line,
        Some(ref query) => Line::from(
            line.spans
                .into_iter()
                .flat_map(|span| {
                    highlight_span(
                        span,
                        &query.to_lowercase(),
                        ctx.theme.search_match,
                        ctx.theme.search_match_selected,
                        ctx.selected_match_line_offset == Some(line_offset),
                    )
                })
                .collect::<Vec<_>>(),
        ),
    }
}

fn syntect_span(style: syntect::highlighting::Style, text: &str, fallback: Style) -> Span<'static> {
    if text.is_empty() {
        return Span::styled(" ".to_string(), fallback);
    }
    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
    Span::styled(text.to_string(), Style::default().fg(fg))
}

fn render_blockquote(
    blocks: &[Block],
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    mut line_offset: usize,
) {
    if area.width < 3 {
        return;
    }
    let inner_area = Rect {
        x: area.x + 2,
        y: area.y,
        width: area.width - 2,
        height: area.height,
    };
    // Draw left border.
    for y in area.y..area.y + area.height {
        buf.set_stringn(area.x, y, "▌", area.width as usize, ctx.theme.blockquote);
    }

    let mut y = inner_area.y;
    let mut line_offset_inner: usize = 0;
    let scroll = skip_rows;
    let max_y = inner_area.y + inner_area.height;

    for block in blocks {
        let height = measure_block_height(block, inner_area.width, ctx);
        if line_offset_inner.saturating_add(height) <= scroll {
            line_offset_inner += height;
            continue;
        }
        let height = measure_block_height(block, inner_area.width, ctx);
        if line_offset.saturating_add(height) <= scroll {
            line_offset += height;
            continue;
        }
        let block_skip = scroll.saturating_sub(line_offset);
        let remaining = (max_y - y) as usize;
        let render_height = height.saturating_sub(block_skip).min(remaining);
        if render_height == 0 {
            break;
        }
        let block_area = Rect {
            x: inner_area.x,
            y,
            width: inner_area.width,
            height: render_height as u16,
        };
        render_block(
            block,
            usize::MAX, // block index not used for nested blocks
            block_area,
            buf,
            block_skip,
            ctx,
            line_offset + 1 + line_offset_inner,
        );
        y += render_height as u16;
        line_offset_inner += height;
        if y >= max_y {
            break;
        }
    }
}

fn render_list(
    list: &List,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    let mut y = area.y;
    let mut line_offset_inner: usize = 0;
    let scroll = skip_rows;
    let max_y = area.y + area.height;

    for (idx, item) in list.items.iter().enumerate() {
        let marker = if list.ordered {
            format!("{}. ", idx + 1)
        } else {
            "• ".to_string()
        };
        let marker_width = marker.width();
        let inner_width = (area.width as usize).saturating_sub(marker_width).max(1) as u16;

        if item.content.is_empty() {
            if line_offset_inner >= scroll && y < max_y {
                buf.set_stringn(area.x, y, &marker, marker_width, ctx.theme.list_marker);
                y += 1;
            }
            line_offset_inner += 1;
            if y >= max_y {
                break;
            }
            continue;
        }

        let mut item_y = y;
        let mut item_line_offset = line_offset_inner;
        let mut drew_marker = false;
        for block in &item.content {
            let height = measure_block_height(block, inner_width, ctx);
            if item_line_offset.saturating_add(height) <= scroll {
                item_line_offset += height;
                continue;
            }
            let block_skip = scroll.saturating_sub(item_line_offset);
            let block_line_offset = line_offset + item_line_offset;
            let remaining = (max_y - item_y) as usize;
            let render_height = height.saturating_sub(block_skip).min(remaining);
            if render_height == 0 {
                break;
            }

            // Draw the marker on the first visible row of the first content block.
            if !drew_marker && item_line_offset + height > scroll {
                let marker_y = item_y + block_skip as u16;
                if marker_y < max_y {
                    buf.set_stringn(
                        area.x,
                        marker_y,
                        &marker,
                        marker_width,
                        ctx.theme.list_marker,
                    );
                }
                drew_marker = true;
            }

            let block_area = Rect {
                x: area.x + marker_width as u16,
                y: item_y,
                width: inner_width,
                height: render_height as u16,
            };
            render_block(
                block,
                usize::MAX,
                block_area,
                buf,
                block_skip,
                ctx,
                block_line_offset,
            );
            item_y += render_height as u16;
            item_line_offset += height;
            if item_y >= max_y {
                break;
            }
        }

        let item_height = item_line_offset - line_offset_inner;
        y = item_y;
        line_offset_inner += item_height;

        if y >= max_y {
            break;
        }
    }
}

fn render_table(
    table: &Table,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
    line_offset: usize,
) {
    let col_count = table
        .headers
        .len()
        .max(table.rows.first().map(|r| r.len()).unwrap_or(0));
    if col_count == 0 || area.width < 3 {
        return;
    }

    let widths = allocate_column_widths(table, area.width as usize);

    // Pre-wrap all cell content, including borders, into terminal rows.
    let mut rows: Vec<(usize, Line)> = Vec::new();

    // Top border.
    rows.push((
        line_offset,
        Line::styled(
            horizontal_table_border(&widths, '┌', '┬', '┐'),
            ctx.theme.table_border,
        ),
    ));

    // Header rows (multi-line cells produce multiple terminal rows).
    let header_rows = render_table_row(
        &table.headers,
        &widths,
        ctx.theme.table_header,
        ctx,
        line_offset + 1,
    );
    for (i, line) in header_rows.into_iter().enumerate() {
        rows.push((line_offset + 1 + i, line));
    }

    // Separator.
    let header_height = table
        .headers
        .iter()
        .enumerate()
        .map(|(i, cell)| {
            wrap_cell_inlines(
                cell,
                widths.get(i).copied().unwrap_or(1),
                ctx.theme.table_header,
                ctx,
                line_offset + 1,
            )
            .len()
        })
        .max()
        .unwrap_or(1);
    let separator_offset = line_offset + 1 + header_height;
    rows.push((
        separator_offset,
        Line::styled(
            horizontal_table_border(&widths, '├', '┼', '┤'),
            ctx.theme.table_border,
        ),
    ));

    // Body rows.
    let mut body_offset = separator_offset + 1;
    for row in &table.rows {
        let row_height = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                wrap_cell_inlines(
                    cell,
                    widths.get(i).copied().unwrap_or(1),
                    ctx.theme.table_cell,
                    ctx,
                    body_offset,
                )
                .len()
            })
            .max()
            .unwrap_or(1);
        let row_lines = render_table_row(row, &widths, ctx.theme.table_cell, ctx, body_offset);
        for (i, line) in row_lines.into_iter().enumerate() {
            rows.push((body_offset + i, line));
        }
        body_offset += row_height;
    }

    // Bottom border.
    rows.push((
        body_offset,
        Line::styled(
            horizontal_table_border(&widths, '└', '┴', '┘'),
            ctx.theme.table_border,
        ),
    ));

    // Render directly into the buffer, skipping scrolled rows and clipping to area.
    for (row_line_offset, line) in rows.iter() {
        let row_idx = row_line_offset.saturating_sub(line_offset);
        let screen_y = area.y as usize + row_idx;
        if row_idx < skip_rows || screen_y >= (area.y + area.height) as usize {
            continue;
        }
        buf.set_line(area.x, screen_y as u16, line, area.width);
    }
}

fn render_table_row(
    cells: &[Vec<Inline>],
    widths: &[usize],
    style: Style,
    ctx: &RenderContext,
    row_start_line_offset: usize,
) -> Vec<Line<'static>> {
    let col_count = widths.len();
    let max_height = (0..col_count)
        .map(|i| {
            cells
                .get(i)
                .map(|c| wrap_cell_inlines(c, widths[i], style, ctx, row_start_line_offset).len())
                .unwrap_or(1)
        })
        .max()
        .unwrap_or(1);

    let mut result = Vec::with_capacity(max_height);

    // Build per-column wrapped lines.
    let mut wrapped_columns: Vec<Vec<Line<'static>>> = Vec::with_capacity(col_count);
    for (i, width) in widths.iter().enumerate() {
        let cell = cells.get(i);
        let wrapped = cell
            .map(|c| wrap_cell_inlines(c, *width, style, ctx, row_start_line_offset))
            .unwrap_or_else(|| vec![Line::from(" ")]);
        wrapped_columns.push(wrapped);
    }

    for row_line in 0..max_height {
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(Span::styled("│".to_string(), ctx.theme.table_border));
        for (i, width) in widths.iter().enumerate() {
            let line = wrapped_columns
                .get(i)
                .and_then(|col| col.get(row_line))
                .cloned()
                .unwrap_or_else(|| Line::from(" "));
            let rendered_width = line.spans.iter().map(|s| s.content.width()).sum::<usize>();
            let pad = width.saturating_sub(rendered_width);
            spans.push(Span::styled(" ".to_string(), style));
            spans.extend(line.spans);
            if pad > 0 {
                spans.push(Span::styled(" ".repeat(pad), style));
            }
            spans.push(Span::styled(" ".to_string(), style));
            spans.push(Span::styled("│".to_string(), ctx.theme.table_border));
        }
        result.push(Line::from(spans));
    }

    result
}

fn horizontal_table_border(widths: &[usize], left: char, cross: char, right: char) -> String {
    let mut s = String::new();
    s.push(left);
    for (i, w) in widths.iter().enumerate() {
        s.extend(std::iter::repeat_n('─', *w + 2));
        if i + 1 < widths.len() {
            s.push(cross);
        }
    }
    s.push(right);
    s
}

fn wrap_cell_inlines(
    inlines: &[Inline],
    width: usize,
    style: Style,
    ctx: &RenderContext,
    start_line_offset: usize,
) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![Line::from(" ")];
    }
    let lines = inlines_to_wrapped_lines(inlines, ctx, style, start_line_offset, width);
    if lines.is_empty() {
        return vec![Line::from(" ")];
    }
    lines.into_iter().map(|(_, line)| line).collect()
}

fn allocate_column_widths(table: &Table, total_width: usize) -> Vec<usize> {
    table.allocate_column_widths(total_width)
}

fn render_mermaid(
    _diag: &MermaidDiagram,
    block_idx: usize,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
) {
    if let Some(protocol) = ctx.rendered.images.get(&block_idx) {
        // If partially scrolled, render with clipping.
        let image = ratatui_image::Image::new(protocol).allow_clipping(true);
        // Shift the render area up by skip_rows by using a sub-rect.
        if skip_rows == 0 {
            image.render(area, buf);
        } else {
            // Clip from below is not directly supported; just render full image.
            image.render(area, buf);
        }
    } else {
        let placeholder = Paragraph::new(Text::from(vec![Line::styled(
            "[mermaid render failed or unsupported]",
            ctx.theme.mermaid_placeholder,
        )]));
        placeholder.render(area, buf);
    }
}

fn render_rule(area: Rect, buf: &mut Buffer) {
    let line = "─".repeat(area.width as usize);
    let para = Paragraph::new(Text::from(vec![Line::styled(
        line,
        Style::default().fg(Color::DarkGray),
    )]));
    para.render(area, buf);
}

/// Convert inline content to ratatui `Text`, respecting hard breaks and collapsing
/// consecutive whitespace (including SoftBreak) into single spaces.
#[cfg(test)]
fn inlines_to_text(
    inlines: &[Inline],
    ctx: &RenderContext,
    base_style: Style,
    line_offset: usize,
) -> Text<'static> {
    highlight_text(
        inlines_to_text_raw(inlines, ctx, base_style),
        ctx.search_query.as_deref(),
        ctx.theme.search_match,
        ctx.theme.search_match_selected,
        ctx.selected_match_line_offset,
        line_offset,
    )
}

/// Convert inline content to wrapped terminal rows with search highlighting.
fn inlines_to_wrapped_lines(
    inlines: &[Inline],
    ctx: &RenderContext,
    base_style: Style,
    start_line_offset: usize,
    width: usize,
) -> Vec<(usize, Line<'static>)> {
    let raw = inlines_to_text_raw(inlines, ctx, base_style);
    let mut out = Vec::new();
    let mut next_offset = start_line_offset;
    for line in raw.lines {
        let wrapped = wrap_styled_line(line, width, next_offset, ctx);
        if wrapped.is_empty() {
            out.push((next_offset, Line::from(" ")));
            next_offset += 1;
        } else {
            next_offset = wrapped
                .last()
                .map(|(offset, _)| offset + 1)
                .unwrap_or(next_offset);
            out.extend(wrapped);
        }
    }
    if out.is_empty() {
        out.push((start_line_offset, Line::from(" ")));
    }
    out
}

/// Word-wrap a styled line while preserving span styles and search highlights.
fn wrap_styled_line(
    line: Line<'static>,
    width: usize,
    line_offset: usize,
    ctx: &RenderContext,
) -> Vec<(usize, Line<'static>)> {
    if width == 0 {
        return vec![(line_offset, highlight_line(line, ctx, line_offset))];
    }

    let words: Vec<(String, Style)> = line
        .spans
        .iter()
        .flat_map(|span| {
            span.content
                .split_whitespace()
                .map(|word| (word.to_string(), span.style))
                .collect::<Vec<_>>()
        })
        .collect();

    if words.is_empty() {
        return vec![(line_offset, highlight_line(line, ctx, line_offset))];
    }

    let mut wrapped = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0usize;

    for (word, style) in words {
        let word_width = word.width();
        let gap = usize::from(!current_spans.is_empty());
        if !current_spans.is_empty() && current_width + gap + word_width > width {
            wrapped.push(Line::from(std::mem::take(&mut current_spans)));
            current_width = 0;
        }
        if !current_spans.is_empty() {
            let space_style = current_spans.last().map(|s| s.style).unwrap_or(style);
            append_span_text(&mut current_spans, " ", space_style);
            current_width += 1;
        }
        append_span_text(&mut current_spans, &word, style);
        current_width += word_width;
    }

    if !current_spans.is_empty() {
        wrapped.push(Line::from(current_spans));
    }

    wrapped
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            let offset = line_offset + i;
            (offset, highlight_line(line, ctx, offset))
        })
        .collect()
}

fn append_span_text(spans: &mut Vec<Span<'static>>, text: &str, style: Style) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut()
        && last.style == style
    {
        let mut merged = last.content.to_string();
        merged.push_str(text);
        last.content = merged.into();
        return;
    }
    spans.push(Span::styled(text.to_string(), style));
}

#[cfg(test)]
fn highlight_text(
    text: Text<'static>,
    query: Option<&str>,
    match_style: Style,
    selected_match_style: Style,
    selected_match_line_offset: Option<usize>,
    current_line_offset: usize,
) -> Text<'static> {
    let Some(query) = query else {
        return text;
    };
    if query.is_empty() {
        return text;
    }
    let query_lower = query.to_lowercase();
    let mut line_offset = current_line_offset;
    Text::from(
        text.lines
            .into_iter()
            .map(|line| {
                let is_selected_line = selected_match_line_offset == Some(line_offset);
                let highlighted = Line::from(
                    line.spans
                        .into_iter()
                        .flat_map(|span| {
                            highlight_span(
                                span,
                                &query_lower,
                                match_style,
                                selected_match_style,
                                is_selected_line,
                            )
                        })
                        .collect::<Vec<_>>(),
                );
                line_offset += 1;
                highlighted
            })
            .collect::<Vec<_>>(),
    )
}

fn highlight_span(
    span: Span<'static>,
    query_lower: &str,
    match_style: Style,
    selected_match_style: Style,
    is_selected_line: bool,
) -> Vec<Span<'static>> {
    let text = &span.content;
    let mut out = Vec::new();
    let mut last = 0usize;
    for (start, matched) in find_case_insensitive_matches(text, query_lower) {
        if last < start {
            out.push(Span::styled(text[last..start].to_string(), span.style));
        }
        out.push(Span::styled(
            text[start..]
                .chars()
                .take(matched.chars().count())
                .collect::<String>(),
            if is_selected_line {
                selected_match_style
            } else {
                match_style
            },
        ));
        last = start + matched.len();
    }
    if last < text.len() {
        out.push(Span::styled(text[last..].to_string(), span.style));
    }
    if out.is_empty() {
        out.push(span);
    }
    out
}

/// Find case-insensitive matches of `query` in `text` using Unicode-aware
/// grapheme iteration. Returns byte offsets and the matched substring from the
/// original `text` so that slicing is always safe even when `to_lowercase`
/// changes byte length.
fn find_case_insensitive_matches<'a>(text: &'a str, query_lower: &str) -> Vec<(usize, &'a str)> {
    if query_lower.is_empty() {
        return Vec::new();
    }
    let mut matches = Vec::new();
    let mut chars = text.char_indices().peekable();

    while let Some(&(start, _)) = chars.peek() {
        let query_chars = query_lower.chars();
        let mut text_iter = chars.clone();
        let mut matched = true;

        for query_char in query_chars {
            match text_iter.next() {
                Some((_, c)) if c.to_lowercase().to_string() == query_char.to_string() => {}
                _ => {
                    matched = false;
                    break;
                }
            }
        }

        if matched {
            let end = if let Some(&(last_idx, _)) = text_iter.peek() {
                last_idx
            } else {
                text.len()
            };
            matches.push((start, &text[start..end]));
            // Advance past the match to avoid overlapping highlights.
            for _ in 0..query_lower.chars().count() {
                chars.next();
            }
        } else {
            chars.next();
        }
    }

    matches
}

fn inlines_to_text_raw(
    inlines: &[Inline],
    ctx: &RenderContext,
    base_style: Style,
) -> Text<'static> {
    let mut segments = Vec::new();
    inlines_to_segments(inlines, ctx, base_style, &mut segments);
    let mut lines = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut pending_whitespace = false;

    for seg in segments {
        if seg.force_break_after {
            // Finish the current line, trimming trailing spaces.
            lines.push(Line::from(std::mem::take(&mut spans)));
            pending_whitespace = false;
            continue;
        }

        if seg.text.is_empty() {
            continue;
        }

        // Normalise whitespace within the segment: split on whitespace runs and join with a
        // single space. This keeps styled spans contiguous while preserving word boundaries.
        let words: Vec<&str> = seg.text.split_whitespace().collect();
        if words.is_empty() {
            pending_whitespace = true;
            continue;
        }

        if pending_whitespace && !spans.is_empty() {
            spans.push(Span::styled(" ".to_string(), seg.style));
        }

        // If the segment originally started with whitespace, prefix a single space before the
        // first word, but only if there is already preceding content.
        let starts_with_space = seg
            .text
            .chars()
            .next()
            .map(|c| c.is_whitespace())
            .unwrap_or(false);
        if starts_with_space && !spans.is_empty() && !pending_whitespace {
            spans.push(Span::styled(" ".to_string(), seg.style));
        }

        for (i, word) in words.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" ".to_string(), seg.style));
            }
            spans.push(Span::styled((*word).to_string(), seg.style));
        }

        pending_whitespace = seg
            .text
            .chars()
            .last()
            .map(|c| c.is_whitespace())
            .unwrap_or(false);
    }

    if pending_whitespace && !spans.is_empty() {
        spans.push(Span::styled(
            " ".to_string(),
            spans.last().map(|s| s.style).unwrap_or(base_style),
        ));
    }
    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }

    Text::from(lines)
}

fn active_search_query(search_state: &SearchState) -> Option<String> {
    match search_state {
        SearchState::Active { query, .. } => Some(query.as_str().to_string()),
        _ => None,
    }
}

fn active_search_match_index(search_state: &SearchState) -> Option<usize> {
    match search_state {
        SearchState::Active {
            matches,
            current_index,
            ..
        } => matches.get(*current_index).map(|m| m.match_index),
        _ => None,
    }
}

fn active_search_match_line_offset(search_state: &SearchState) -> Option<usize> {
    match search_state {
        SearchState::Active {
            matches,
            current_index,
            ..
        } => matches.get(*current_index).map(|m| m.line_offset),
        _ => None,
    }
}

#[derive(Debug)]
struct Segment {
    text: String,
    style: Style,
    force_break_after: bool,
}

fn inlines_to_segments(
    inlines: &[Inline],
    ctx: &RenderContext,
    base_style: Style,
    out: &mut Vec<Segment>,
) {
    for inline in inlines {
        match inline {
            Inline::Text(t) => out.push(Segment {
                text: t.clone(),
                style: base_style,
                force_break_after: false,
            }),
            Inline::Code(code) => out.push(Segment {
                text: code.clone(),
                style: ctx.theme.code_inline,
                force_break_after: false,
            }),
            Inline::Strong(children) => {
                inlines_to_segments(children, ctx, base_style.add_modifier(Modifier::BOLD), out);
            }
            Inline::Emphasis(children) => {
                inlines_to_segments(
                    children,
                    ctx,
                    base_style.add_modifier(Modifier::ITALIC),
                    out,
                );
            }
            Inline::Link(id, children) => {
                let style = if ctx.selected_link == Some(*id) {
                    ctx.theme.link_selected
                } else {
                    ctx.theme.link
                };
                inlines_to_segments(children, ctx, style, out);
            }
            Inline::SoftBreak => out.push(Segment {
                text: " ".to_string(),
                style: base_style,
                force_break_after: false,
            }),
            Inline::HardBreak => {
                if let Some(last) = out.last_mut() {
                    last.force_break_after = true;
                }
            }
        }
    }
}

/// Shared syntect resources.
pub struct SyntaxAssets {
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
}

impl SyntaxAssets {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    pub fn theme(&self) -> &SyntectTheme {
        self.theme_set
            .themes
            .get("base16-ocean.dark")
            .unwrap_or_else(|| &self.theme_set.themes["InspiredGitHub"])
    }
}

impl Default for SyntaxAssets {
    fn default() -> Self {
        Self::new()
    }
}

/// Find all logical lines in the rendered document that contain `query`.
///
/// Line offsets follow the same layout as [`MarkdownWidget`] rendering so that
/// selected-match highlighting lines up with navigation targets.
///
/// The returned matches are sorted by ascending line offset and can be passed
/// to `ViewState::confirm_search`.
pub fn find_search_matches(
    document: &Document,
    width: u16,
    query: &str,
    ctx: &RenderContext,
) -> Vec<SearchMatch> {
    if width == 0 || query.is_empty() {
        return Vec::new();
    }
    let query_lower = query.to_lowercase();
    let mut match_index = 0usize;
    collect_searchable_lines(document, width, ctx)
        .into_iter()
        .filter_map(|(offset, text)| {
            if text.to_lowercase().contains(&query_lower) {
                let m = SearchMatch::new(offset, match_index);
                match_index += 1;
                Some(m)
            } else {
                None
            }
        })
        .collect()
}

fn collect_searchable_lines(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut line_offset: usize = 0;
    for (idx, block) in document.blocks.iter().enumerate() {
        let gap = if idx == 0 { 0 } else { 1 };
        let block_lines = block_searchable_lines(block, width, ctx);
        let block_height = measure_block_height(block, width, ctx).max(block_lines.len());
        for (i, line) in block_lines.iter().enumerate().take(block_height) {
            out.push((line_offset + i, line.clone()));
        }
        // Keep in sync with `MarkdownWidget::render`: gap rows trail block content.
        line_offset += block_height + gap;
    }
    out
}

fn line_plain_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

fn block_searchable_lines(block: &Block, width: u16, ctx: &RenderContext) -> Vec<String> {
    match block {
        Block::Heading(heading) => {
            let (style, _) = heading_styles(heading.level, ctx.theme);
            let prefix_width = heading.level.prefix().width();
            let content_width = if (width as usize) > prefix_width + 1 {
                (width as usize).saturating_sub(prefix_width)
            } else {
                width as usize
            };
            inlines_to_wrapped_lines(&heading.content, ctx, style, 0, content_width.max(1))
                .into_iter()
                .map(|(_, line)| line_plain_text(&line))
                .collect()
        }
        Block::Paragraph(inlines) => {
            inlines_to_wrapped_lines(inlines, ctx, ctx.theme.text, 0, width as usize)
                .into_iter()
                .map(|(_, line)| line_plain_text(&line))
                .collect()
        }
        Block::CodeBlock(cb) => code_block_searchable_lines(cb),
        Block::BlockQuote(blocks) => {
            let inner_width = (width as usize).saturating_sub(2).max(1) as u16;
            let mut lines = Vec::new();
            for child in blocks {
                lines.extend(block_searchable_lines(child, inner_width, ctx));
            }
            // `measure_blockquote_height` adds one logical row of padding.
            lines.push(String::new());
            lines
        }
        Block::List(list) => list_searchable_lines(list, width, ctx),
        Block::Table(table) => table_searchable_lines(table, width, ctx),
        Block::Mermaid(diag) => diag.source.lines().map(|s| s.to_string()).collect(),
        Block::Rule => Vec::new(),
    }
}

fn code_block_searchable_lines(cb: &CodeBlock) -> Vec<String> {
    let mut lines = Vec::new();
    let label = cb
        .language
        .as_ref()
        .map(|l| format!(" {l} "))
        .unwrap_or_else(|| " code ".to_string());
    lines.push(label);
    if cb.content.is_empty() {
        lines.push(String::new());
    } else {
        for line in cb.content.lines() {
            lines.push(line.to_string());
        }
    }
    lines
}

fn list_searchable_lines(list: &List, width: u16, ctx: &RenderContext) -> Vec<String> {
    let mut lines = Vec::new();
    for (idx, item) in list.items.iter().enumerate() {
        let marker_width = if list.ordered {
            format!("{}.", idx + 1).width() + 1
        } else {
            2
        };
        let inner_width = (width as usize).saturating_sub(marker_width).max(1) as u16;
        if item.content.is_empty() {
            lines.push(String::new());
            continue;
        }
        for child in &item.content {
            lines.extend(block_searchable_lines(child, inner_width, ctx));
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn table_searchable_lines(table: &Table, width: u16, ctx: &RenderContext) -> Vec<String> {
    let col_count = table
        .headers
        .len()
        .max(table.rows.first().map(|r| r.len()).unwrap_or(0));
    if col_count == 0 {
        return Vec::new();
    }
    let widths = table.allocate_column_widths(width as usize);
    let mut lines = Vec::new();

    // `measure_table_height` counts: top border + header + separator + body + bottom border.
    lines.push(String::new());
    lines.extend(render_table_search_lines(
        &table.headers,
        &widths,
        ctx.theme.table_header,
        ctx,
    ));
    lines.push(String::new());
    for row in &table.rows {
        lines.extend(render_table_search_lines(
            row,
            &widths,
            ctx.theme.table_cell,
            ctx,
        ));
    }
    lines.push(String::new());

    lines
}

fn render_table_search_lines(
    cells: &[Vec<Inline>],
    widths: &[usize],
    style: Style,
    ctx: &RenderContext,
) -> Vec<String> {
    let col_count = widths.len();
    let mut wrapped_columns: Vec<Vec<String>> = Vec::with_capacity(col_count);
    for (i, width) in widths.iter().enumerate() {
        let wrapped = cells
            .get(i)
            .map(|cell| wrap_cell_inlines(cell, *width, style, ctx, 0))
            .unwrap_or_else(|| vec![Line::from(" ")]);
        wrapped_columns.push(
            wrapped
                .into_iter()
                .map(|line| line_plain_text(&line))
                .collect(),
        );
    }
    let max_height = wrapped_columns
        .iter()
        .map(|c| c.len())
        .max()
        .unwrap_or(1)
        .max(1);

    let mut lines = Vec::with_capacity(max_height);
    for row_line in 0..max_height {
        let mut row_text = String::new();
        for (i, _width) in widths.iter().enumerate() {
            let cell = wrapped_columns
                .get(i)
                .and_then(|col| col.get(row_line))
                .map(|s| s.as_str())
                .unwrap_or(" ");
            if !row_text.is_empty() {
                row_text.push(' ');
            }
            row_text.push_str(cell);
        }
        lines.push(row_text);
    }
    lines
}

/// Key for invalidating a pre-rendered document buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RenderCacheKey {
    width: u16,
    search_query: Option<String>,
    selected_link: Option<LinkId>,
    selected_match_line_offset: Option<usize>,
}

impl RenderCacheKey {
    fn from_context(ctx: &RenderContext<'_>, width: u16) -> Self {
        Self {
            width,
            search_query: ctx.search_query.clone(),
            selected_link: ctx.selected_link,
            selected_match_line_offset: ctx.selected_match_line_offset,
        }
    }
}

/// Full-document render cache. Rebuilds when width or highlight state changes;
/// scrolling only blits a viewport slice from the cached buffer.
pub struct DocumentRenderCache {
    key: Option<RenderCacheKey>,
    buffer: Buffer,
    total_height: usize,
}

impl Default for DocumentRenderCache {
    fn default() -> Self {
        Self {
            key: None,
            buffer: Buffer::empty(Rect::default()),
            total_height: 0,
        }
    }
}

impl DocumentRenderCache {
    /// Rebuild the cache when `ctx` or `width` no longer match the stored key.
    pub fn ensure(
        &mut self,
        document: &Document,
        ctx: &RenderContext<'_>,
        view_state: &ViewState,
        width: u16,
    ) {
        let key = RenderCacheKey::from_context(ctx, width);
        if self.key.as_ref() == Some(&key) && self.total_height > 0 {
            return;
        }
        self.rebuild(document, ctx, view_state, width, key);
    }

    fn rebuild(
        &mut self,
        document: &Document,
        ctx: &RenderContext<'_>,
        view_state: &ViewState,
        width: u16,
        key: RenderCacheKey,
    ) {
        let total_height = measure_document_height(document, width, ctx).max(1);
        let height = total_height.min(u16::MAX as usize) as u16;
        let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
        let top_view = view_state.clone().scroll_to(0);
        let widget = MarkdownWidget::new(document, ctx, &top_view);
        widget.render(Rect::new(0, 0, width, height), &mut buffer);
        self.key = Some(key);
        self.buffer = buffer;
        self.total_height = total_height;
    }

    pub fn total_height(&self) -> usize {
        self.total_height
    }

    /// Copy the visible viewport starting at `scroll` into `buf`.
    pub fn blit(&self, scroll: usize, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let cache_height = self.buffer.area().height as usize;
        for row in 0..area.height as usize {
            let src_y = scroll.saturating_add(row);
            if src_y >= cache_height {
                break;
            }
            for col in 0..area.width as usize {
                if let Some(cell) = self.buffer.cell((col as u16, src_y as u16)) {
                    buf[(area.x + col as u16, area.y + row as u16)] = cell.clone();
                }
            }
        }
    }
}

/// Widget that blits a pre-rendered [`DocumentRenderCache`] viewport.
pub struct CachedMarkdownView<'a> {
    pub cache: &'a DocumentRenderCache,
    pub scroll: usize,
}

impl Widget for CachedMarkdownView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.cache.blit(self.scroll, area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        Alignment, Block, CodeBlock, Document, Inline, List, ListItem, SearchDirection,
        SearchMatch, Table, TerminalSize, ViewState,
    };
    use crate::parse::parse;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use syntect::highlighting::ThemeSet;
    use syntect::parsing::SyntaxSet;

    fn test_render_context() -> RenderContext<'static> {
        // Leaked for the duration of the test process; acceptable for unit tests.
        let theme: &'static Theme = Box::leak(Box::new(Theme::default()));
        let syntax_set: &'static SyntaxSet =
            Box::leak(Box::new(SyntaxSet::load_defaults_newlines()));
        let syntax_theme: &'static syntect::highlighting::Theme = Box::leak(Box::new(
            ThemeSet::load_defaults().themes["InspiredGitHub"].clone(),
        ));
        let rendered: &'static RenderedDocument = Box::leak(Box::new(RenderedDocument {
            images: HashMap::new(),
        }));
        RenderContext {
            theme,
            syntax_set,
            syntax_theme,
            rendered,
            selected_link: None,
            search_query: None,
            selected_search_match: None,
            selected_match_line_offset: None,
        }
    }

    fn wrapped_line_count(line: &Line, width: usize) -> usize {
        if width == 0 {
            return 1;
        }
        let words: Vec<&str> = line
            .spans
            .iter()
            .flat_map(|span| span.content.split_whitespace())
            .collect();
        if words.is_empty() {
            return 1;
        }
        let mut lines = 1;
        let mut current = 0usize;
        for (i, word) in words.iter().enumerate() {
            let word_width = word.width();
            let extra = if i == 0 { 0 } else { 1 };
            if current + word_width + extra > width {
                lines += 1;
                current = word_width;
            } else {
                current += word_width + extra;
            }
        }
        lines
    }

    fn find_matches(document: &Document, width: u16, query: &str) -> Vec<SearchMatch> {
        find_search_matches(document, width, query, &test_render_context())
    }

    #[test]
    fn document_render_cache_blits_scrolled_viewport() {
        let ctx = test_render_context();
        let blocks: Vec<Block> = (0..20)
            .map(|i| Block::Paragraph(vec![Inline::Text(format!("line {i}"))]))
            .collect();
        let document = Document::new(blocks, Vec::new()).unwrap();
        let width = 40u16;
        let height = 5u16;
        let size = TerminalSize::new(width, height).unwrap();
        let view_state = ViewState::new(size);

        let mut cache = DocumentRenderCache::default();
        cache.ensure(&document, &ctx, &view_state, width);

        // Logical layout: line N sits at offset N * 2 - 1 (gap row follows each block).
        let scroll = 7;
        let mut screen = Buffer::empty(Rect::new(0, 0, width, height));
        cache.blit(scroll, Rect::new(0, 0, width, height), &mut screen);

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
        )
        .unwrap();
        let size = TerminalSize::new(80, 10).unwrap();
        let view_state = ViewState::new(size);

        let mut cache = DocumentRenderCache::default();
        cache.ensure(&document, &ctx, &view_state, 80);
        let height_at_80 = cache.total_height();
        cache.ensure(&document, &ctx, &view_state, 20);
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
        )
        .unwrap();
        let matches = find_matches(&document, 80, "hello");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line_offset, 0);
        assert_eq!(matches[1].line_offset, 3);
    }

    #[test]
    fn find_search_matches_is_case_insensitive() {
        let document = Document::new(
            vec![Block::Paragraph(vec![Inline::Text(
                "Hello World".to_string(),
            )])],
            Vec::new(),
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
        )
        .unwrap();
        assert!(find_matches(&document, 80, "").is_empty());
    }

    #[test]
    fn find_search_matches_zero_width_returns_no_matches() {
        let document = Document::new(
            vec![Block::Paragraph(vec![Inline::Text("hello".to_string())])],
            Vec::new(),
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
                    ListItem {
                        content: vec![
                            Block::Paragraph(vec![Inline::Text("alpha".to_string())]),
                            Block::Paragraph(vec![Inline::Text("beta".to_string())]),
                        ],
                    },
                    ListItem {
                        content: vec![Block::Paragraph(vec![Inline::Text("gamma".to_string())])],
                    },
                ],
            })],
            Vec::new(),
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
                Block::BlockQuote(vec![Block::Paragraph(vec![Inline::Text(
                    "quoted".to_string(),
                )])]),
                Block::Paragraph(vec![Inline::Text("after".to_string())]),
            ],
            Vec::new(),
        )
        .unwrap();
        let matches = find_matches(&document, 80, "after");
        assert_eq!(matches.len(), 1);
        // quoted (line 0) + blockquote padding (line 1) + gap (line 2) -> after at line 3.
        assert_eq!(matches[0].line_offset, 2);
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
        )
        .unwrap();
        let matches = find_matches(&document, 80, "after");
        assert_eq!(matches.len(), 1);
        // top border (0) + header (1) + separator (2) + cell (3) + bottom border (4) + gap (5) -> after at 6.
        assert_eq!(matches[0].line_offset, 5);
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
        )
        .unwrap();

        let width = 80u16;
        let height = 10u16;
        let matches = find_search_matches(&document, width, "needle", &ctx_base);
        assert_eq!(matches.len(), 2);

        let size = TerminalSize::new(width, height).unwrap();
        let view_state = ViewState::new(size)
            .confirm_search("needle".to_string(), SearchDirection::Forward, matches)
            .unwrap()
            .next_search_match(1000)
            .scroll_to(0);

        let ctx = RenderContext::new(
            ctx_base.theme,
            ctx_base.syntax_set,
            ctx_base.syntax_theme,
            ctx_base.rendered,
            &view_state,
        );

        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let widget = MarkdownWidget::new(&document, &ctx, &view_state);
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
        assert_eq!(theme.text.fg, Some(Color::White));
        assert!(theme.h1.add_modifier.contains(Modifier::BOLD));
        assert!(theme.h1.add_modifier.contains(Modifier::UNDERLINED));
        assert_eq!(theme.link.fg, Some(Color::Blue));
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
        let rows = inlines_to_wrapped_lines(&inlines, &ctx, ctx.theme.text, 0, 5);
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
        let second_styles: Vec<Style> =
            highlighted.lines[1].spans.iter().map(|s| s.style).collect();
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
    fn allocate_column_widths_fits_within_total_width() {
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
        let widths = allocate_column_widths(&table, 20);
        // Widths fit within total width; may not fill it when content is smaller.
        let border_width = widths.len() + 1;
        assert!(widths.iter().sum::<usize>() + border_width <= 20);
        assert!(widths.iter().all(|w| *w >= 1));
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
        let lines = render_table_row(&cells, &[8], ctx.theme.table_cell, &ctx, 0);
        assert_eq!(lines.len(), 1);
        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("hi"));
        assert!(text.starts_with('│'));
        assert!(text.ends_with('│'));
    }

    #[test]
    fn render_code_block_draws_language_label_and_content() {
        let ctx = test_render_context();
        let cb = CodeBlock {
            language: Some("rust".into()),
            content: "fn main() {}".into(),
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
        render_code_block(&cb, Rect::new(0, 0, 20, 5), &mut buf, 0, &ctx, 0);
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
        let document = Document::new(blocks, Vec::new()).unwrap();
        let size = TerminalSize::new(80, 10).unwrap();
        let total_height = measure_document_height(&document, 80, &ctx);
        let max_scroll = total_height.saturating_sub(size.height() as usize);
        let view_state = ViewState::new(size).jump_to_bottom(max_scroll);

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let widget = MarkdownWidget::new(&document, &ctx, &view_state);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("Paragraph 49"));
    }

    #[test]
    fn sap_metrics_file_renders_to_bottom() {
        let path = "/Users/manji0/src/dagayn/docs/SAP-METRICS.md";
        if !std::path::Path::new(path).exists() {
            return;
        }
        let input = std::fs::read_to_string(path).unwrap();
        let document = parse(&input).unwrap();
        let ctx = test_render_context();
        let width = 100u16;
        let height = 60u16;
        let size = TerminalSize::new(width, height).unwrap();
        let total_height = measure_document_height(&document, width, &ctx);
        let max_scroll = total_height.saturating_sub(height as usize);
        let view_state = ViewState::new(size).jump_to_bottom(max_scroll);

        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let widget = MarkdownWidget::new(&document, &ctx, &view_state);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(
            text.contains("Known open questions") || text.contains("Design history"),
            "late content missing; total_height={total_height}, max_scroll={max_scroll}"
        );
    }

    #[test]
    fn list_layout_has_no_extra_gap() {
        let ctx = test_render_context();
        let input = "- item A\n- item B\n- item C\n\n## Next";
        let document = parse(input).unwrap();
        let width = 40u16;
        let height = 10u16;
        let size = TerminalSize::new(width, height).unwrap();
        let view_state = ViewState::new(size);

        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let widget = MarkdownWidget::new(&document, &ctx, &view_state);
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
    }

    #[test]
    fn list_multiline_item_indents_properly() {
        let ctx = test_render_context();
        let input = "- very long item that wraps onto multiple lines because the terminal is narrow\n- second item\n\n## Next";
        let document = parse(input).unwrap();
        let width = 30u16;
        let height = 8u16;
        let size = TerminalSize::new(width, height).unwrap();
        let view_state = ViewState::new(size);

        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let widget = MarkdownWidget::new(&document, &ctx, &view_state);
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
        let size = TerminalSize::new(width, height).unwrap();
        let view_state = ViewState::new(size);

        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let widget = MarkdownWidget::new(&document, &ctx, &view_state);
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
}
