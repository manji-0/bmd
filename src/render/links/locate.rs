//! First-line location of a link in the rendered document.

use ratatui::text::Line;
use unicode_width::UnicodeWidthStr;

use crate::domain::{Block, DefinitionList, Document, Inline, LinkId, List};

use super::super::callout::callout_inner_width;
use super::super::context::RenderContext;
use super::super::inline::{heading_styles, inlines_to_wrapped_lines};
use super::super::list_marker::list_marker_width_at;
use super::super::measure::measure_block_height;
use super::hits::table_first_link_line;

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
        Block::Callout(callout) => {
            let inner_width = callout_inner_width(callout, width);
            let mut inner_offset = 1usize;
            for child in &callout.body {
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

pub(super) fn first_link_line_in_wrapped(
    rows: &[(usize, Line<'_>)],
    inlines: &[Inline],
    link_id: LinkId,
) -> Option<usize> {
    if !inlines_contain_link(inlines, link_id) {
        return None;
    }
    rows.first().map(|(offset, _)| *offset)
}

pub(super) fn inlines_contain_link(inlines: &[Inline], link_id: LinkId) -> bool {
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
