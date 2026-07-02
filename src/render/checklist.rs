//! Checklist hit testing for mouse interaction.

use crate::domain::{Block, ChecklistId, Document, List, ListItem};

use super::context::RenderContext;
use super::list_marker::list_marker_width_at;
use super::measure::measure_block_height;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChecklistHit {
    pub id: ChecklistId,
    pub line: usize,
    pub x: usize,
    pub width: usize,
}

/// Collect screen positions of checklist markers in document order.
pub fn collect_checklist_hits(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
) -> Vec<ChecklistHit> {
    if width == 0 {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let mut line_offset = 0usize;
    for (block_idx, block) in document.blocks.iter().enumerate() {
        let gap = if block_idx == 0 { 0 } else { 1 };
        collect_block_checklist_hits(block, block_idx, width, 0, ctx, &mut hits, &mut line_offset);
        line_offset += gap;
    }
    hits
}

fn collect_block_checklist_hits(
    block: &Block,
    block_idx: usize,
    width: u16,
    base_x: usize,
    ctx: &RenderContext,
    hits: &mut Vec<ChecklistHit>,
    line_offset: &mut usize,
) {
    match block {
        Block::List(list) => {
            collect_list_checklist_hits(list, block_idx, width, base_x, ctx, hits, line_offset)
        }
        Block::BlockQuote(blocks) => {
            let quote_x = base_x + 2;
            let inner_width = (width as usize).saturating_sub(2).max(1) as u16;
            for child in blocks {
                collect_block_checklist_hits(
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
        Block::DefinitionList(list) => {
            let inner_width = (width as usize).saturating_sub(2).max(1) as u16;
            for item in &list.items {
                for definition in &item.definitions {
                    for child in definition {
                        collect_block_checklist_hits(
                            child,
                            block_idx,
                            inner_width,
                            base_x + 2,
                            ctx,
                            hits,
                            line_offset,
                        );
                    }
                }
            }
            *line_offset += measure_block_height(block, block_idx, width, ctx);
        }
        _ => {
            *line_offset += measure_block_height(block, block_idx, width, ctx);
        }
    }
}

fn collect_list_checklist_hits(
    list: &List,
    block_idx: usize,
    width: u16,
    base_x: usize,
    ctx: &RenderContext,
    hits: &mut Vec<ChecklistHit>,
    line_offset: &mut usize,
) {
    for (item_idx, item) in list.items.iter().enumerate() {
        let marker_width = list_marker_width_at(list, item_idx, item, ctx.checklist_state);
        let inner_width = (width as usize).saturating_sub(marker_width).max(1) as u16;

        if let Some(id) = item.checklist_id {
            hits.push(ChecklistHit {
                id,
                line: *line_offset,
                x: base_x,
                width: marker_width,
            });
        }

        if item.content.is_empty() {
            *line_offset += 1;
            continue;
        }

        for child in &item.content {
            collect_block_checklist_hits(
                child,
                block_idx,
                inner_width,
                base_x + marker_width,
                ctx,
                hits,
                line_offset,
            );
        }
    }
}

/// Find a checklist item whose marker contains the click, if any.
pub fn checklist_at_click<'a>(
    document: &'a Document,
    width: u16,
    ctx: &RenderContext<'_>,
    logical_row: usize,
    local_col: usize,
) -> Option<&'a ListItem> {
    let hit = collect_checklist_hits(document, width, ctx)
        .into_iter()
        .find(|hit| {
            hit.line == logical_row && local_col >= hit.x && local_col < hit.x + hit.width
        })?;
    find_checklist_item(document, hit.id)
}

fn find_checklist_item(document: &Document, id: ChecklistId) -> Option<&ListItem> {
    document
        .blocks
        .iter()
        .find_map(|block| find_checklist_item_in_block(block, id))
}

fn find_checklist_item_in_block(block: &Block, id: ChecklistId) -> Option<&ListItem> {
    match block {
        Block::List(list) => find_checklist_item_in_list(list, id),
        Block::BlockQuote(blocks) => blocks
            .iter()
            .find_map(|child| find_checklist_item_in_block(child, id)),
        Block::DefinitionList(list) => list.items.iter().find_map(|item| {
            item.definitions
                .iter()
                .find_map(|definition| {
                    definition
                        .iter()
                        .find_map(|child| find_checklist_item_in_block(child, id))
                })
        }),
        _ => None,
    }
}

fn find_checklist_item_in_list(list: &List, id: ChecklistId) -> Option<&ListItem> {
    for item in &list.items {
        if item.checklist_id == Some(id) {
            return Some(item);
        }
        for child in &item.content {
            if let Some(found) = find_checklist_item_in_block(child, id) {
                return Some(found);
            }
        }
    }
    None
}
