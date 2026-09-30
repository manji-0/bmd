//! Screen positions of links and footnote references, read back from a probe render.
//!
//! Instead of re-deriving layout, the document is rendered once with every link and
//! footnote reference span tagged by an `underline_color` that encodes its target.
//! Scanning the buffer for tags yields hit rectangles that match the real render
//! by construction.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use unicode_width::UnicodeWidthStr;

use crate::domain::{Document, FootnoteId, LinkId, NavTarget};

use super::context::RenderContext;
use super::document::render_document;
use super::measure::measure_document_height;

const LINK_TAG: u8 = 0xB1;
const FOOTNOTE_TAG: u8 = 0xB2;

/// One contiguous run of a navigation target on a rendered row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavHit {
    pub target: NavTarget,
    pub line: usize,
    pub x: usize,
    pub width: usize,
}

/// Probe style for spans of `target`; `None` when the id does not fit the tag.
pub(crate) fn probe_style(target: NavTarget) -> Option<Style> {
    let (tag, id) = match target {
        NavTarget::Link(LinkId(id)) => (LINK_TAG, id),
        NavTarget::Footnote(FootnoteId(id)) => (FOOTNOTE_TAG, id),
    };
    let [hi, lo] = u16::try_from(id).ok()?.to_be_bytes();
    Some(Style::default().underline_color(Color::Rgb(tag, hi, lo)))
}

fn probe_target(style: Style) -> Option<NavTarget> {
    let Some(Color::Rgb(tag, hi, lo)) = style.underline_color else {
        return None;
    };
    let id = usize::from(u16::from_be_bytes([hi, lo]));
    match tag {
        LINK_TAG => Some(NavTarget::Link(LinkId(id))),
        FOOTNOTE_TAG => Some(NavTarget::Footnote(FootnoteId(id))),
        _ => None,
    }
}

/// Whether `style` is a probe tag, so nested inline styles keep it.
pub(crate) fn is_probe_style(style: Style) -> bool {
    probe_target(style).is_some()
}

/// Render `document` in probe mode at `width` and collect every tagged run.
pub(crate) fn collect_nav_hits(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
) -> Vec<NavHit> {
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

fn scan_hits(buf: &Buffer) -> Vec<NavHit> {
    let mut hits: Vec<NavHit> = Vec::new();
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
                    _ => hits.push(NavHit {
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

/// Targets with a hit in `[scroll, scroll + lines)`, in reading order, each once.
pub fn visible_nav_targets(hits: &[NavHit], scroll: usize, lines: usize) -> Vec<NavTarget> {
    let mut out: Vec<NavTarget> = Vec::new();
    for hit in hits
        .iter()
        .filter(|hit| (scroll..scroll + lines).contains(&hit.line))
    {
        if !out.contains(&hit.target) {
            out.push(hit.target);
        }
    }
    out
}

/// Links with a hit in `[scroll, scroll + lines)`, each once.
pub fn visible_links(hits: &[NavHit], scroll: usize, lines: usize) -> Vec<LinkId> {
    visible_nav_targets(hits, scroll, lines)
        .into_iter()
        .filter_map(NavTarget::link_id)
        .collect()
}

/// The link drawn at logical `line`, column `col`, if any.
pub fn link_at(hits: &[NavHit], line: usize, col: usize) -> Option<LinkId> {
    hits.iter()
        .find(|hit| hit.line == line && (hit.x..hit.x + hit.width).contains(&col))
        .and_then(|hit| hit.target.link_id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_style_round_trips_and_rejects_oversized_ids() {
        for target in [
            NavTarget::Link(LinkId(0)),
            NavTarget::Link(LinkId(65_535)),
            NavTarget::Footnote(FootnoteId(7)),
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
        let target = NavTarget::Link(LinkId(3));
        assert_eq!(
            hits,
            [
                NavHit {
                    target,
                    line: 0,
                    x: 1,
                    width: 4
                },
                NavHit {
                    target,
                    line: 1,
                    x: 2,
                    width: 1
                },
            ]
        );
        assert_eq!(link_at(&hits, 0, 4), Some(LinkId(3)));
        assert_eq!(link_at(&hits, 1, 0), None);
        assert_eq!(visible_links(&hits, 1, 1), [LinkId(3)]);
        assert!(visible_nav_targets(&hits, 2, 5).is_empty());
    }
}
