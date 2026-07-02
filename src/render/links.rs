//! Link position discovery for navigation.

use ratatui::style::Style;
use ratatui::text::Line;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::domain::{Alignment, Block, DefinitionList, Document, Inline, LinkId, List, Table};

use super::context::RenderContext;
use super::inline::{heading_styles, inlines_to_wrapped_lines};
use super::list_marker::list_marker_width_at;
use super::measure::measure_block_height;
use super::table::{allocate_column_widths, cell_padding, column_alignment, wrap_cell_inlines};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkHit {
    pub id: LinkId,
    pub line: usize,
    pub x: usize,
    pub width: usize,
}

struct TableRowLinkContext<'a> {
    widths: &'a [usize],
    alignments: &'a [Alignment],
    style: Style,
    ctx: &'a RenderContext<'a>,
    base_x: usize,
    row_start_line: usize,
}

struct WordHitCursor<'a> {
    width: usize,
    base_x: usize,
    line: &'a mut usize,
    x: &'a mut usize,
    target_line: Option<usize>,
    hits: &'a mut Vec<LinkHit>,
}

/// Collect screen positions of link text in document order.
pub fn collect_link_hits(document: &Document, width: u16, ctx: &RenderContext) -> Vec<LinkHit> {
    if width == 0 || document.links.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let mut line_offset = 0usize;
    for (block_idx, block) in document.blocks.iter().enumerate() {
        let gap = if block_idx == 0 { 0 } else { 1 };
        collect_block_link_hits(block, block_idx, width, 0, ctx, &mut hits, &mut line_offset);
        line_offset += gap;
    }
    hits
}

/// Find a link whose rendered text contains the click, if any.
pub fn link_at_click(
    document: &Document,
    width: u16,
    ctx: &RenderContext<'_>,
    logical_row: usize,
    local_col: usize,
) -> Option<LinkId> {
    collect_link_hits(document, width, ctx)
        .into_iter()
        .find(|hit| hit.line == logical_row && local_col >= hit.x && local_col < hit.x + hit.width)
        .map(|hit| hit.id)
}

fn collect_block_link_hits(
    block: &Block,
    block_idx: usize,
    width: u16,
    base_x: usize,
    ctx: &RenderContext,
    hits: &mut Vec<LinkHit>,
    line_offset: &mut usize,
) {
    match block {
        Block::Heading(heading) => {
            let prefix_width = heading.level.prefix().width();
            let content_width = if (width as usize) > prefix_width + 1 {
                (width as usize).saturating_sub(prefix_width)
            } else {
                width as usize
            };
            collect_inline_link_hits(
                &heading.content,
                content_width.max(1),
                base_x + prefix_width,
                *line_offset,
                hits,
            );
            *line_offset += measure_block_height(block, block_idx, width, ctx);
        }
        Block::Paragraph(inlines) => {
            collect_inline_link_hits(inlines, width as usize, base_x, *line_offset, hits);
            *line_offset += measure_block_height(block, block_idx, width, ctx);
        }
        Block::BlockQuote(blocks) => {
            let quote_x = base_x + 2;
            let inner_width = (width as usize).saturating_sub(2).max(1) as u16;
            for child in blocks {
                collect_block_link_hits(
                    child,
                    block_idx,
                    inner_width,
                    quote_x,
                    ctx,
                    hits,
                    line_offset,
                );
            }
        }
        Block::List(list) => {
            collect_list_link_hits(list, block_idx, width, base_x, ctx, hits, line_offset);
        }
        Block::DefinitionList(list) => {
            collect_definition_list_link_hits(
                list,
                block_idx,
                width,
                base_x,
                ctx,
                hits,
                line_offset,
            );
        }
        Block::Table(table) => {
            collect_table_link_hits(table, width, base_x, ctx, hits, line_offset);
        }
        Block::CodeBlock(_) | Block::MathBlock(_) | Block::Rule => {
            *line_offset += measure_block_height(block, block_idx, width, ctx);
        }
    }
}

fn collect_list_link_hits(
    list: &List,
    block_idx: usize,
    width: u16,
    base_x: usize,
    ctx: &RenderContext,
    hits: &mut Vec<LinkHit>,
    line_offset: &mut usize,
) {
    for (item_idx, item) in list.items.iter().enumerate() {
        let marker_width = list_marker_width_at(list, item_idx, item, ctx.checklist_state);
        let inner_width = (width as usize).saturating_sub(marker_width).max(1) as u16;
        let content_x = base_x + marker_width;

        if item.content.is_empty() {
            *line_offset += 1;
            continue;
        }

        for child in &item.content {
            collect_block_link_hits(
                child,
                block_idx,
                inner_width,
                content_x,
                ctx,
                hits,
                line_offset,
            );
        }
    }
}

fn collect_definition_list_link_hits(
    list: &DefinitionList,
    block_idx: usize,
    width: u16,
    base_x: usize,
    ctx: &RenderContext,
    hits: &mut Vec<LinkHit>,
    line_offset: &mut usize,
) {
    let inner_width = (width as usize).saturating_sub(2).max(1) as u16;
    let content_x = base_x + 2;
    for item in &list.items {
        if !item.term.is_empty() {
            collect_inline_link_hits(&item.term, width as usize, base_x, *line_offset, hits);
            *line_offset +=
                measure_block_height(&Block::Paragraph(item.term.clone()), block_idx, width, ctx);
        }
        for definition in &item.definitions {
            for child in definition {
                collect_block_link_hits(
                    child,
                    block_idx,
                    inner_width,
                    content_x,
                    ctx,
                    hits,
                    line_offset,
                );
            }
        }
    }
}

fn collect_table_link_hits(
    table: &Table,
    width: u16,
    base_x: usize,
    ctx: &RenderContext,
    hits: &mut Vec<LinkHit>,
    line_offset: &mut usize,
) {
    let col_count = table.column_count();
    if col_count == 0 || width < 3 {
        *line_offset += 1;
        return;
    }

    let widths = allocate_column_widths(table, width as usize);

    // Top border.
    *line_offset += 1;

    // Header rows.
    let header_height = table_row_height(&table.headers, &widths, ctx.theme.table_header, ctx);
    collect_table_row_link_hits(
        &table.headers,
        &TableRowLinkContext {
            widths: &widths,
            alignments: &table.alignments,
            style: ctx.theme.table_header,
            ctx,
            base_x,
            row_start_line: *line_offset,
        },
        hits,
    );
    *line_offset += header_height;

    // Separator.
    *line_offset += 1;

    // Body rows.
    for row in &table.rows {
        let row_height = table_row_height(row, &widths, ctx.theme.table_cell, ctx);
        collect_table_row_link_hits(
            row,
            &TableRowLinkContext {
                widths: &widths,
                alignments: &table.alignments,
                style: ctx.theme.table_cell,
                ctx,
                base_x,
                row_start_line: *line_offset,
            },
            hits,
        );
        *line_offset += row_height;
    }

    // Bottom border.
    *line_offset += 1;
}

fn table_row_height(
    cells: &[Vec<Inline>],
    widths: &[usize],
    style: Style,
    ctx: &RenderContext,
) -> usize {
    widths
        .iter()
        .enumerate()
        .map(|(i, width)| {
            cells
                .get(i)
                .map(|cell| wrap_cell_inlines(cell, *width, style, ctx, 0).len())
                .unwrap_or(1)
        })
        .max()
        .unwrap_or(1)
        .max(1)
}

fn collect_table_row_link_hits(
    cells: &[Vec<Inline>],
    row: &TableRowLinkContext<'_>,
    hits: &mut Vec<LinkHit>,
) {
    let max_height = table_row_height(cells, row.widths, row.style, row.ctx);

    for row_line in 0..max_height {
        let line = row.row_start_line + row_line;
        let mut col_x = row.base_x + 1;
        for (i, width) in row.widths.iter().enumerate() {
            let cell = cells.get(i);
            let wrapped = cell
                .map(|c| wrap_cell_inlines(c, *width, row.style, row.ctx, row.row_start_line))
                .unwrap_or_else(|| vec![Line::from(" ")]);
            let line_content = wrapped
                .get(row_line)
                .cloned()
                .unwrap_or_else(|| Line::from(" "));
            let rendered_width = line_content
                .spans
                .iter()
                .map(|s| s.content.width())
                .sum::<usize>();
            let alignment = column_alignment(row.alignments, i);
            let (pad_left, _) = cell_padding(alignment, rendered_width, *width);
            let content_x = col_x + 1 + pad_left;

            if let Some(cell_inlines) = cell {
                collect_inline_link_hits_filtered(
                    cell_inlines,
                    *width,
                    content_x,
                    row.row_start_line,
                    Some(line),
                    hits,
                );
            }

            col_x += 1 + width + 1 + 1;
        }
    }
}

fn table_first_link_line(
    table: &Table,
    width: u16,
    ctx: &RenderContext,
    link_id: LinkId,
) -> Option<usize> {
    let col_count = table.column_count();
    if col_count == 0 || width < 3 {
        return None;
    }

    let widths = allocate_column_widths(table, width as usize);
    let mut line = 0usize;

    // Top border.
    line += 1;

    // Header rows.
    if let Some(local) = first_link_line_in_table_row(
        &table.headers,
        &widths,
        &table.alignments,
        ctx.theme.table_header,
        ctx,
        link_id,
    ) {
        return Some(line + local);
    }
    line += table_row_height(&table.headers, &widths, ctx.theme.table_header, ctx);

    // Separator.
    line += 1;

    // Body rows.
    for row in &table.rows {
        if let Some(local) = first_link_line_in_table_row(
            row,
            &widths,
            &table.alignments,
            ctx.theme.table_cell,
            ctx,
            link_id,
        ) {
            return Some(line + local);
        }
        line += table_row_height(row, &widths, ctx.theme.table_cell, ctx);
    }

    None
}

fn first_link_line_in_table_row(
    cells: &[Vec<Inline>],
    widths: &[usize],
    _alignments: &[Alignment],
    style: Style,
    ctx: &RenderContext,
    link_id: LinkId,
) -> Option<usize> {
    let mut first: Option<usize> = None;
    for (i, cell) in cells.iter().enumerate() {
        if !inlines_contain_link(cell, link_id) {
            continue;
        }
        let width = widths.get(i).copied().unwrap_or(1);
        let wrapped = inlines_to_wrapped_lines(cell, ctx, style, 0, width);
        if let Some(local) = first_link_line_in_wrapped(&wrapped, cell, link_id) {
            first = Some(match first {
                Some(f) => f.min(local),
                None => local,
            });
        }
    }
    first
}

#[derive(Clone, Debug)]
enum FlatPiece {
    Word {
        text: String,
        link_id: Option<LinkId>,
    },
    Space {
        link_id: Option<LinkId>,
    },
    Break,
}

fn collect_inline_link_hits(
    inlines: &[Inline],
    width: usize,
    base_x: usize,
    start_line: usize,
    hits: &mut Vec<LinkHit>,
) {
    collect_inline_link_hits_filtered(inlines, width, base_x, start_line, None, hits);
}

fn collect_inline_link_hits_filtered(
    inlines: &[Inline],
    width: usize,
    base_x: usize,
    start_line: usize,
    target_line: Option<usize>,
    hits: &mut Vec<LinkHit>,
) {
    if width == 0 {
        return;
    }
    let pieces = flatten_inline_pieces(inlines, None);
    let mut line = start_line;
    let mut x = 0usize;

    for piece in pieces {
        match piece {
            FlatPiece::Break => {
                line += 1;
                x = 0;
            }
            FlatPiece::Space { link_id } => {
                if x == 0 {
                    continue;
                }
                if x + 1 > width {
                    line += 1;
                    x = 0;
                    continue;
                }
                if let Some(id) = link_id {
                    if target_line.is_none_or(|target| target == line) {
                        push_link_hit(hits, id, line, base_x + x, 1);
                    }
                }
                x += 1;
            }
            FlatPiece::Word { text, link_id } => {
                append_word_hits_filtered(
                    &text,
                    link_id,
                    &mut WordHitCursor {
                        width,
                        base_x,
                        line: &mut line,
                        x: &mut x,
                        target_line,
                        hits,
                    },
                );
            }
        }
    }
}

fn append_word_hits_filtered(word: &str, link_id: Option<LinkId>, cursor: &mut WordHitCursor<'_>) {
    let word_width = word.width();
    if word_width <= cursor.width {
        append_fitting_word_filtered(word, link_id, cursor);
        return;
    }
    for grapheme in word.graphemes(true) {
        let grapheme_width = grapheme.width();
        if *cursor.x > 0 && *cursor.x + grapheme_width > cursor.width {
            *cursor.line += 1;
            *cursor.x = 0;
        }
        if let Some(id) = link_id {
            if cursor
                .target_line
                .is_none_or(|target| target == *cursor.line)
            {
                push_link_hit(
                    cursor.hits,
                    id,
                    *cursor.line,
                    cursor.base_x + *cursor.x,
                    grapheme_width,
                );
            }
        }
        *cursor.x += grapheme_width;
    }
}

fn append_fitting_word_filtered(
    word: &str,
    link_id: Option<LinkId>,
    cursor: &mut WordHitCursor<'_>,
) {
    let word_width = word.width();
    let gap = usize::from(*cursor.x > 0);
    if *cursor.x > 0 && *cursor.x + gap + word_width > cursor.width {
        *cursor.line += 1;
        *cursor.x = 0;
    }
    if *cursor.x > 0 {
        *cursor.x += 1;
    }
    if let Some(id) = link_id {
        if cursor
            .target_line
            .is_none_or(|target| target == *cursor.line)
        {
            push_link_hit(
                cursor.hits,
                id,
                *cursor.line,
                cursor.base_x + *cursor.x,
                word_width,
            );
        }
    }
    *cursor.x += word_width;
}

fn push_link_hit(hits: &mut Vec<LinkHit>, id: LinkId, line: usize, x: usize, width: usize) {
    if width == 0 {
        return;
    }
    hits.push(LinkHit { id, line, x, width });
}

fn flatten_inline_pieces(inlines: &[Inline], active_link: Option<LinkId>) -> Vec<FlatPiece> {
    let mut out = Vec::new();
    flatten_inline_pieces_inner(inlines, active_link, &mut out);
    out
}

fn flatten_inline_pieces_inner(
    inlines: &[Inline],
    active_link: Option<LinkId>,
    out: &mut Vec<FlatPiece>,
) {
    for inline in inlines {
        match inline {
            Inline::Text(text) => flatten_text_pieces(text, active_link, out),
            Inline::Code(code) => {
                let mut first = true;
                for word in code.split_whitespace() {
                    if !first {
                        out.push(FlatPiece::Space { link_id: None });
                    }
                    out.push(FlatPiece::Word {
                        text: word.to_string(),
                        link_id: None,
                    });
                    first = false;
                }
            }
            Inline::Strong(children)
            | Inline::Emphasis(children)
            | Inline::Strikethrough(children)
            | Inline::Subscript(children)
            | Inline::Superscript(children) => {
                flatten_inline_pieces_inner(children, active_link, out);
            }
            Inline::Link(id, children) => {
                flatten_inline_pieces_inner(children, Some(*id), out);
            }
            Inline::FootnoteReference(_, display) => {
                out.push(FlatPiece::Word {
                    text: format!("[{display}]"),
                    link_id: None,
                });
            }
            Inline::Math(latex) => flatten_text_pieces(latex, active_link, out),
            Inline::SoftBreak => out.push(FlatPiece::Space {
                link_id: active_link,
            }),
            Inline::HardBreak => out.push(FlatPiece::Break),
        }
    }
}

fn flatten_text_pieces(text: &str, link_id: Option<LinkId>, out: &mut Vec<FlatPiece>) {
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() {
            out.push(FlatPiece::Space { link_id });
            while chars.peek().is_some_and(|next| next.is_whitespace()) {
                chars.next();
            }
        } else {
            let mut word = String::from(ch);
            while let Some(&next) = chars.peek() {
                if next.is_whitespace() {
                    break;
                }
                word.push(next);
                chars.next();
            }
            out.push(FlatPiece::Word {
                text: word,
                link_id,
            });
        }
    }
}

/// Link IDs whose first line falls within the visible scroll viewport.
pub fn collect_visible_links(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
    scroll: usize,
    visible_lines: usize,
) -> Vec<LinkId> {
    if width == 0 || document.links.is_empty() || visible_lines == 0 {
        return Vec::new();
    }
    let viewport_end = scroll.saturating_add(visible_lines);
    (0..document.links.len())
        .filter_map(|i| {
            let id = LinkId(i);
            let line = find_link_line_offset(document, width, ctx, id)?;
            (line >= scroll && line < viewport_end).then_some(id)
        })
        .collect()
}

/// First logical line offset where `link_id` appears in the rendered document.
pub fn find_link_line_offset(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
    link_id: LinkId,
) -> Option<usize> {
    if width == 0 {
        return None;
    }
    let mut line_offset = 0usize;
    for (block_idx, block) in document.blocks.iter().enumerate() {
        let gap = if block_idx == 0 { 0 } else { 1 };
        if let Some(local) = block_first_link_line(block, block_idx, width, ctx, link_id) {
            return Some(line_offset + local);
        }
        line_offset += measure_block_height(block, block_idx, width, ctx) + gap;
    }
    None
}

fn block_first_link_line(
    block: &Block,
    block_idx: usize,
    width: u16,
    ctx: &RenderContext,
    link_id: LinkId,
) -> Option<usize> {
    match block {
        Block::Heading(heading) => {
            let (style, _) = heading_styles(heading.level, ctx.theme);
            let prefix_width = heading.level.prefix().width();
            let content_width = if (width as usize) > prefix_width + 1 {
                (width as usize).saturating_sub(prefix_width)
            } else {
                width as usize
            };
            first_link_line_in_wrapped(
                &inlines_to_wrapped_lines(&heading.content, ctx, style, 0, content_width.max(1)),
                &heading.content,
                link_id,
            )
        }
        Block::Paragraph(inlines) => first_link_line_in_wrapped(
            &inlines_to_wrapped_lines(inlines, ctx, ctx.theme.text, 0, width as usize),
            inlines,
            link_id,
        ),
        Block::BlockQuote(blocks) => {
            let inner_width = (width as usize).saturating_sub(2).max(1) as u16;
            let mut inner_offset = 0usize;
            for child in blocks {
                if let Some(local) =
                    block_first_link_line(child, block_idx, inner_width, ctx, link_id)
                {
                    return Some(inner_offset + local);
                }
                inner_offset += measure_block_height(child, block_idx, inner_width, ctx);
            }
            None
        }
        Block::List(list) => list_first_link_line(list, block_idx, width, ctx, link_id),
        Block::DefinitionList(list) => {
            definition_list_first_link_line(list, block_idx, width, ctx, link_id)
        }
        Block::Table(table) => table_first_link_line(table, width, ctx, link_id),
        Block::CodeBlock(_) | Block::MathBlock(_) | Block::Rule => None,
    }
}

fn list_first_link_line(
    list: &List,
    block_idx: usize,
    width: u16,
    ctx: &RenderContext,
    link_id: LinkId,
) -> Option<usize> {
    let mut line_offset = 0usize;
    for (item_idx, item) in list.items.iter().enumerate() {
        let marker_width = list_marker_width_at(list, item_idx, item, ctx.checklist_state);
        let item_width = (width as usize).saturating_sub(marker_width).max(1) as u16;
        let mut item_line = 0usize;
        for child in &item.content {
            if let Some(local) = block_first_link_line(child, block_idx, item_width, ctx, link_id) {
                return Some(line_offset + item_line + local);
            }
            item_line += measure_block_height(child, block_idx, item_width, ctx);
        }
        line_offset += item_line.max(1);
    }
    None
}

fn definition_list_first_link_line(
    list: &DefinitionList,
    block_idx: usize,
    width: u16,
    ctx: &RenderContext,
    link_id: LinkId,
) -> Option<usize> {
    let inner_width = (width as usize).saturating_sub(2).max(1) as u16;
    let mut line_offset = 0usize;
    for item in &list.items {
        if !item.term.is_empty() {
            if inlines_contain_link(&item.term, link_id) {
                return Some(line_offset);
            }
            line_offset +=
                measure_block_height(&Block::Paragraph(item.term.clone()), block_idx, width, ctx);
        }
        for definition in &item.definitions {
            for child in definition {
                if let Some(local) =
                    block_first_link_line(child, block_idx, inner_width, ctx, link_id)
                {
                    return Some(line_offset + local);
                }
                line_offset += measure_block_height(child, block_idx, inner_width, ctx);
            }
        }
    }
    None
}

fn first_link_line_in_wrapped(
    rows: &[(usize, Line<'_>)],
    inlines: &[Inline],
    link_id: LinkId,
) -> Option<usize> {
    if !inlines_contain_link(inlines, link_id) {
        return None;
    }
    rows.first().map(|(offset, _)| *offset)
}

fn inlines_contain_link(inlines: &[Inline], link_id: LinkId) -> bool {
    inlines.iter().any(|inline| match inline {
        Inline::Link(id, children) => *id == link_id || inlines_contain_link(children, link_id),
        Inline::Strong(c)
        | Inline::Emphasis(c)
        | Inline::Strikethrough(c)
        | Inline::Subscript(c)
        | Inline::Superscript(c) => inlines_contain_link(c, link_id),
        Inline::Text(_)
        | Inline::Code(_)
        | Inline::Math(_)
        | Inline::HardBreak
        | Inline::SoftBreak
        | Inline::FootnoteReference(_, _) => false,
    })
}
