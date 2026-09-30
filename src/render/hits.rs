//! Screen positions of links, footnote references, and task-list markers, read
//! back from a probe render.
//!
//! Instead of re-deriving layout, the document is rendered once with every
//! interactive span tagged by an `underline_color` that encodes its target.
//! Scanning the buffer for tags yields hit rectangles that match the real render
//! by construction.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use unicode_width::UnicodeWidthStr;

use crate::domain::{ChecklistId, Document, FootnoteId, LinkId, NavTarget};

use super::context::RenderContext;
use super::document::render_document;
use super::measure::measure_document_height;

const LINK_TAG: u8 = 0xB1;
const FOOTNOTE_TAG: u8 = 0xB2;
const CHECKLIST_TAG: u8 = 0xB3;

/// Something the user can select or click in the rendered document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitTarget {
    Nav(NavTarget),
    Checklist(ChecklistId),
}

/// One contiguous run of a target on a rendered row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub target: HitTarget,
    pub line: usize,
    pub x: usize,
    pub width: usize,
}

impl From<NavTarget> for HitTarget {
    fn from(target: NavTarget) -> Self {
        Self::Nav(target)
    }
}

/// Probe style for spans of `target`; `None` when the id does not fit the tag.
pub(crate) fn probe_style(target: impl Into<HitTarget>) -> Option<Style> {
    let (tag, id) = match target.into() {
        HitTarget::Nav(NavTarget::Link(LinkId(id))) => (LINK_TAG, id),
        HitTarget::Nav(NavTarget::Footnote(FootnoteId(id))) => (FOOTNOTE_TAG, id),
        HitTarget::Checklist(ChecklistId(id)) => (CHECKLIST_TAG, id as usize),
    };
    let [hi, lo] = u16::try_from(id).ok()?.to_be_bytes();
    Some(Style::default().underline_color(Color::Rgb(tag, hi, lo)))
}

fn probe_target(style: Style) -> Option<HitTarget> {
    let Some(Color::Rgb(tag, hi, lo)) = style.underline_color else {
        return None;
    };
    let id = u16::from_be_bytes([hi, lo]);
    match tag {
        LINK_TAG => Some(NavTarget::Link(LinkId(id.into())).into()),
        FOOTNOTE_TAG => Some(NavTarget::Footnote(FootnoteId(id.into())).into()),
        CHECKLIST_TAG => Some(HitTarget::Checklist(ChecklistId(id.into()))),
        _ => None,
    }
}

/// Whether `style` is a probe tag, so nested inline styles keep it.
pub(crate) fn is_probe_style(style: Style) -> bool {
    probe_target(style).is_some()
}

/// Render `document` in probe mode at `width` and collect every tagged run.
pub(crate) fn collect_hits(document: &Document, width: u16, ctx: &RenderContext) -> Vec<Hit> {
    let probe_ctx = RenderContext {
        nav_probe: true,
        selected_link: None,
        selected_footnote: None,
        search_query: None,
        selected_match_line_offset: None,
        ..ctx.clone()
    };
    let height = measure_document_height(document, width, &probe_ctx).min(u16::MAX as usize);
    let area = Rect::new(0, 0, width, height as u16);
    let mut buf = Buffer::empty(area);
    render_document(document, area, &mut buf, &probe_ctx);
    scan_hits(&buf)
}

fn scan_hits(buf: &Buffer) -> Vec<Hit> {
    let mut hits: Vec<Hit> = Vec::new();
    for y in 0..buf.area.height {
        let mut x = 0u16;
        while x < buf.area.width {
            let cell = &buf[(x, y)];
            let cell_width = cell.symbol().width().max(1);
            if let Some(target) = probe_target(cell.style()) {
                match hits.last_mut() {
                    Some(last)
                        if last.target == target
                            && last.line == y as usize
                            && last.x + last.width == x as usize =>
                    {
                        last.width += cell_width;
                    }
                    _ => hits.push(Hit {
                        target,
                        line: y as usize,
                        x: x as usize,
                        width: cell_width,
                    }),
                }
            }
            x += cell_width as u16;
        }
    }
    hits
}

/// Link targets for `n`/`N` cycling in reading order, each once. Footnote
/// references are excluded so link hopping is not interrupted; they open by click.
pub fn nav_targets(hits: &[Hit]) -> Vec<NavTarget> {
    link_targets_in(hits, 0, usize::MAX)
}

/// Link targets with a hit in `[scroll, scroll + lines)`, in reading order, each once.
pub fn visible_nav_targets(hits: &[Hit], scroll: usize, lines: usize) -> Vec<NavTarget> {
    link_targets_in(hits, scroll, lines)
}

fn link_targets_in(hits: &[Hit], scroll: usize, lines: usize) -> Vec<NavTarget> {
    let end = scroll.saturating_add(lines);
    let mut out: Vec<NavTarget> = Vec::new();
    for hit in hits.iter().filter(|hit| (scroll..end).contains(&hit.line)) {
        if let HitTarget::Nav(target @ NavTarget::Link(_)) = hit.target
            && !out.contains(&target)
        {
            out.push(target);
        }
    }
    out
}

/// First rendered line of `target`.
pub fn target_line(hits: &[Hit], target: NavTarget) -> Option<usize> {
    hits.iter()
        .find(|hit| hit.target == HitTarget::Nav(target))
        .map(|hit| hit.line)
}

/// Links with a hit in `[scroll, scroll + lines)`, each once.
pub fn visible_links(hits: &[Hit], scroll: usize, lines: usize) -> Vec<LinkId> {
    visible_nav_targets(hits, scroll, lines)
        .into_iter()
        .filter_map(NavTarget::link_id)
        .collect()
}

/// The target drawn at logical `line`, column `col`, if any.
pub fn hit_at(hits: &[Hit], line: usize, col: usize) -> Option<HitTarget> {
    hits.iter()
        .find(|hit| hit.line == line && (hit.x..hit.x + hit.width).contains(&col))
        .map(|hit| hit.target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_style_round_trips_and_rejects_oversized_ids() {
        for target in [
            NavTarget::Link(LinkId(0)).into(),
            NavTarget::Link(LinkId(65_535)).into(),
            NavTarget::Footnote(FootnoteId(7)).into(),
            HitTarget::Checklist(ChecklistId(9)),
        ] {
            assert_eq!(probe_target(probe_style(target).unwrap()), Some(target));
        }
        assert_eq!(probe_style(NavTarget::Link(LinkId(65_536))), None);
        assert_eq!(
            probe_target(Style::default().underline_color(Color::Red)),
            None
        );
    }

    #[test]
    fn scan_merges_runs_and_counts_wide_cells() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 2));
        let link = probe_style(NavTarget::Link(LinkId(3))).unwrap();
        buf.set_string(1, 0, "ab", link);
        buf.set_string(3, 0, "リ", link);
        buf.set_string(0, 1, "x", Style::default());
        buf.set_string(2, 1, "y", link);
        let hits = scan_hits(&buf);
        let target = HitTarget::Nav(NavTarget::Link(LinkId(3)));
        assert_eq!(
            hits,
            [
                Hit {
                    target,
                    line: 0,
                    x: 1,
                    width: 4
                },
                Hit {
                    target,
                    line: 1,
                    x: 2,
                    width: 1
                },
            ]
        );
        assert_eq!(hit_at(&hits, 0, 4), Some(target));
        assert_eq!(hit_at(&hits, 1, 0), None);
        assert_eq!(visible_links(&hits, 1, 1), [LinkId(3)]);
        assert!(visible_nav_targets(&hits, 2, 5).is_empty());
    }
}
