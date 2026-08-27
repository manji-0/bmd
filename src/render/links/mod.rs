//! Link position discovery for navigation.

mod hits;
mod locate;

use crate::domain::{Document, FootnoteId, LinkId, NavTarget};

use super::context::RenderContext;

pub use hits::{collect_footnote_hits, collect_link_hits};
pub use locate::find_link_line_offset;

pub fn collect_visible_nav_targets(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
    scroll: usize,
    visible_lines: usize,
) -> Vec<NavTarget> {
    if width == 0 || visible_lines == 0 {
        return Vec::new();
    }
    let viewport_end = scroll.saturating_add(visible_lines);
    let mut ordered: Vec<(usize, usize, NavTarget)> = Vec::new();

    for hit in collect_link_hits(document, width, ctx) {
        ordered.push((hit.line, hit.x, NavTarget::Link(hit.id)));
    }
    for hit in collect_footnote_hits(document, width, ctx) {
        ordered.push((hit.line, hit.x, NavTarget::Footnote(hit.id)));
    }
    ordered.sort_by_key(|(line, x, _)| (*line, *x));

    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (line, _, target) in ordered {
        if line < scroll || line >= viewport_end {
            continue;
        }
        if seen.insert(target) {
            out.push(target);
        }
    }
    out
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

pub fn find_footnote_ref_line_offset(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
    footnote_id: FootnoteId,
) -> Option<usize> {
    collect_footnote_hits(document, width, ctx)
        .into_iter()
        .find(|hit| hit.id == footnote_id)
        .map(|hit| hit.line)
}
