//! Link position discovery for navigation.

mod inline;
mod table;

use unicode_width::UnicodeWidthStr;

use crate::domain::{Block, DefinitionList, Document, FootnoteId, LinkId, List};

use super::super::callout::callout_inner_width;
use super::super::context::RenderContext;
use super::super::list_marker::list_marker_width_at;
use super::super::measure::measure_block_height;
use inline::{collect_inline_footnote_hits, collect_inline_link_hits};
use table::{collect_table_footnote_hits, collect_table_link_hits};

pub(super) use table::table_first_link_line;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkHit {
    pub id: LinkId,
    pub line: usize,
    pub x: usize,
    pub width: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FootnoteHit {
    pub id: FootnoteId,
    pub line: usize,
    pub x: usize,
    pub width: usize,
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

/// Collect screen positions of footnote reference markers in document order.
pub fn collect_footnote_hits(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
) -> Vec<FootnoteHit> {
    if width == 0 || document.footnote_order.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let mut line_offset = 0usize;
    for (block_idx, block) in document.blocks.iter().enumerate() {
        let gap = if block_idx == 0 { 0 } else { 1 };
        collect_block_footnote_hits(block, block_idx, width, 0, ctx, &mut hits, &mut line_offset);
        line_offset += gap;
    }
    hits
}

/// Navigation targets whose first marker falls within the visible scroll viewport.
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
        Block::Callout(callout) => {
            *line_offset += 1;
            let inner_x = base_x + 1;
            let inner_width = callout_inner_width(callout, width);
            for child in &callout.body {
                collect_block_link_hits(
                    child,
                    block_idx,
                    inner_width,
                    inner_x,
                    ctx,
                    hits,
                    line_offset,
                );
            }
            *line_offset += 1;
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

fn collect_block_footnote_hits(
    block: &Block,
    block_idx: usize,
    width: u16,
    base_x: usize,
    ctx: &RenderContext,
    hits: &mut Vec<FootnoteHit>,
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
            collect_inline_footnote_hits(
                &heading.content,
                content_width.max(1),
                base_x + prefix_width,
                *line_offset,
                hits,
            );
            *line_offset += measure_block_height(block, block_idx, width, ctx);
        }
        Block::Paragraph(inlines) => {
            collect_inline_footnote_hits(inlines, width as usize, base_x, *line_offset, hits);
            *line_offset += measure_block_height(block, block_idx, width, ctx);
        }
        Block::BlockQuote(blocks) => {
            let quote_x = base_x + 2;
            let inner_width = (width as usize).saturating_sub(2).max(1) as u16;
            for child in blocks {
                collect_block_footnote_hits(
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
        Block::Callout(callout) => {
            *line_offset += 1;
            let inner_x = base_x + 1;
            let inner_width = callout_inner_width(callout, width);
            for child in &callout.body {
                collect_block_footnote_hits(
                    child,
                    block_idx,
                    inner_width,
                    inner_x,
                    ctx,
                    hits,
                    line_offset,
                );
            }
            *line_offset += 1;
        }
        Block::List(list) => {
            collect_list_footnote_hits(list, block_idx, width, base_x, ctx, hits, line_offset);
        }
        Block::DefinitionList(list) => {
            collect_definition_list_footnote_hits(
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
            collect_table_footnote_hits(table, width, base_x, ctx, hits, line_offset);
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

fn collect_list_footnote_hits(
    list: &List,
    block_idx: usize,
    width: u16,
    base_x: usize,
    ctx: &RenderContext,
    hits: &mut Vec<FootnoteHit>,
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
            collect_block_footnote_hits(
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

fn collect_definition_list_footnote_hits(
    list: &DefinitionList,
    block_idx: usize,
    width: u16,
    base_x: usize,
    ctx: &RenderContext,
    hits: &mut Vec<FootnoteHit>,
    line_offset: &mut usize,
) {
    let inner_width = (width as usize).saturating_sub(2).max(1) as u16;
    let content_x = base_x + 2;
    for item in &list.items {
        if !item.term.is_empty() {
            collect_inline_footnote_hits(&item.term, width as usize, base_x, *line_offset, hits);
            *line_offset +=
                measure_block_height(&Block::Paragraph(item.term.clone()), block_idx, width, ctx);
        }
        for definition in &item.definitions {
            for child in definition {
                collect_block_footnote_hits(
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
