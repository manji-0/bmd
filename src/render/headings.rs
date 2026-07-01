//! Heading position discovery for navigation.

use crate::domain::{Block, Document, Heading, HeadingLevel, Inline};

use super::context::RenderContext;
use super::measure::measure_block_height;

/// Cached heading offsets for repeated j/k navigation.
#[derive(Clone, Default)]
pub struct HeadingOffsetCache {
    key: Option<HeadingOffsetCacheKey>,
    headings: Vec<(usize, HeadingLevel)>,
}

#[derive(Clone, PartialEq, Eq)]
struct HeadingOffsetCacheKey {
    document_revision: u64,
    width: u16,
    checklist_revision: u64,
}

impl HeadingOffsetCache {
    pub fn get_or_collect(
        &mut self,
        document_revision: u64,
        width: u16,
        checklist_revision: u64,
        document: &Document,
        ctx: &RenderContext,
    ) -> &[(usize, HeadingLevel)] {
        let key = HeadingOffsetCacheKey {
            document_revision,
            width,
            checklist_revision,
        };
        if self.key.as_ref() != Some(&key) {
            self.headings = collect_heading_offsets(document, width, ctx);
            self.key = Some(key);
        }
        &self.headings
    }
}

/// Collect logical line offsets of each heading in document order.
pub fn collect_heading_offsets(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
) -> Vec<(usize, HeadingLevel)> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut line_offset = 0usize;
    for (block_idx, block) in document.blocks.iter().enumerate() {
        let gap = if block_idx == 0 { 0 } else { 1 };
        if let Block::Heading(h) = block {
            out.push((line_offset, h.level));
        }
        line_offset += measure_block_height(block, block_idx, width, ctx) + gap;
    }
    out
}

/// Next heading line strictly after `scroll`.
pub fn next_heading_line(headings: &[(usize, HeadingLevel)], scroll: usize) -> Option<usize> {
    headings
        .iter()
        .find(|(offset, _)| *offset > scroll)
        .map(|(offset, _)| *offset)
}

/// Previous heading line strictly before `scroll`, or the first heading when at the top.
pub fn prev_heading_line(headings: &[(usize, HeadingLevel)], scroll: usize) -> Option<usize> {
    if scroll == 0 {
        return headings.first().map(|(offset, _)| *offset);
    }
    headings
        .iter()
        .rfind(|(offset, _)| *offset < scroll)
        .map(|(offset, _)| *offset)
}

/// Find a heading line offset matching a markdown anchor slug (`#section`).
pub fn find_heading_line_by_anchor(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
    anchor: &str,
) -> Option<usize> {
    if width == 0 || anchor.is_empty() {
        return None;
    }
    let target = crate::parse::normalize_anchor_slug(anchor);
    let mut line_offset = 0usize;
    for (block_idx, block) in document.blocks.iter().enumerate() {
        let gap = if block_idx == 0 { 0 } else { 1 };
        if let Block::Heading(h) = block
            && heading_anchor_slug(h) == target
        {
            return Some(line_offset);
        }
        line_offset += measure_block_height(block, block_idx, width, ctx) + gap;
    }
    None
}

fn heading_anchor_slug(heading: &Heading) -> String {
    heading
        .anchor
        .as_ref()
        .map(|anchor| crate::parse::normalize_anchor_slug(anchor))
        .unwrap_or_else(|| slugify_heading(&Inline::plain_text(&heading.content)))
}

/// GitHub-compatible heading slug: lowercase words separated by hyphens.
pub fn slugify_heading(text: &str) -> String {
    crate::parse::slugify_heading(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_offset_cache_reuses_collected_offsets() {
        use crate::domain::TerminalSize;
        use crate::render::{
            HeadingOffsetCache, RenderContext, RenderedDocument, SyntaxAssets, Theme,
        };

        let document = Document {
            blocks: vec![
                crate::domain::Block::Heading(crate::domain::Heading {
                    level: crate::domain::HeadingLevel::H1,
                    content: vec![crate::domain::Inline::Text("One".into())],
                    anchor: None,
                }),
                crate::domain::Block::Heading(crate::domain::Heading {
                    level: crate::domain::HeadingLevel::H2,
                    content: vec![crate::domain::Inline::Text("Two".into())],
                    anchor: None,
                }),
            ],
            links: vec![],
            mermaid_diagrams: vec![],
            footnotes: vec![],
            footnote_order: vec![],
            front_matter: None,
        };
        let rendered = RenderedDocument::new(
            &document,
            &ratatui_image::picker::Picker::halfblocks(),
            TerminalSize::new(80, 24).unwrap(),
            None,
        )
        .unwrap();
        let view_state = crate::domain::ViewState::new(TerminalSize::new(80, 24).unwrap());
        let checklist_state =
            crate::domain::ChecklistState::new(crate::domain::ChecklistStyle::Unicode);
        let theme = Theme::default();
        let syntax_assets = SyntaxAssets::new();
        let ctx = RenderContext::new(
            &theme,
            &syntax_assets,
            &rendered,
            &document.links,
            &view_state,
            true,
            &checklist_state,
        );
        let mut cache = HeadingOffsetCache::default();
        let first = cache
            .get_or_collect(0, 80, checklist_state.revision(), &document, &ctx)
            .to_vec();
        let second = cache
            .get_or_collect(0, 80, checklist_state.revision(), &document, &ctx)
            .to_vec();
        assert_eq!(first, second);
        assert_eq!(first.len(), 2);
    }

    #[test]
    fn slugify_heading_matches_github_style() {
        assert_eq!(slugify_heading("Hello World"), "hello-world");
        assert_eq!(slugify_heading("  Foo: Bar!  "), "foo-bar");
    }
}
