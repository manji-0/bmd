//! Heading catalog for outline, yank, and in-document heading jumps.

use crate::domain::{
    Block, Document, Heading, HeadingLevel, Inline, normalize_anchor_slug, slugify_heading,
};

use super::context::RenderContext;
use super::measure::measure_block_height;

/// One outline-visible heading: layout offset plus display/jump metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadingEntry {
    pub line_offset: usize,
    pub level: HeadingLevel,
    pub text: String,
    pub slug: String,
}

/// Cached heading catalog. Line offsets depend on wrap width and checklist height.
#[derive(Clone, Default)]
pub struct HeadingCatalogCache {
    key: Option<HeadingCatalogCacheKey>,
    headings: Vec<HeadingEntry>,
}

#[derive(Clone, PartialEq, Eq)]
struct HeadingCatalogCacheKey {
    document_revision: u64,
    width: u16,
    checklist_revision: u64,
}

impl HeadingCatalogCache {
    pub fn refresh(
        &mut self,
        document_revision: u64,
        width: u16,
        checklist_revision: u64,
        document: &Document,
        ctx: &RenderContext,
    ) {
        let key = HeadingCatalogCacheKey {
            document_revision,
            width,
            checklist_revision,
        };
        if self.key.as_ref() != Some(&key) {
            self.headings = collect_heading_catalog(document, width, ctx);
            self.key = Some(key);
        }
    }

    pub fn entries(&self) -> &[HeadingEntry] {
        &self.headings
    }
}

/// Collect outline-visible headings in document order.
///
/// Headings with empty plain text are omitted so outline indices match
/// `[` / `]` / yank / current-section highlighting.
pub fn collect_heading_catalog(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
) -> Vec<HeadingEntry> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut line_offset = 0usize;
    for (block_idx, block) in document.blocks.iter().enumerate() {
        let gap = if block_idx == 0 { 0 } else { 1 };
        if let Block::Heading(h) = block {
            let text = Inline::plain_text(&h.content);
            if !text.is_empty() {
                out.push(HeadingEntry {
                    line_offset,
                    level: h.level,
                    text,
                    slug: heading_anchor_slug(h),
                });
            }
        }
        line_offset += measure_block_height(block, block_idx, width, ctx) + gap;
    }
    out
}

/// Next heading line strictly after `scroll`.
pub fn next_heading_line(headings: &[HeadingEntry], scroll: usize) -> Option<usize> {
    headings
        .iter()
        .find(|heading| heading.line_offset > scroll)
        .map(|heading| heading.line_offset)
}

/// Previous heading line strictly before `scroll`, or the first heading when at the top.
pub fn prev_heading_line(headings: &[HeadingEntry], scroll: usize) -> Option<usize> {
    if scroll == 0 {
        return headings.first().map(|heading| heading.line_offset);
    }
    headings
        .iter()
        .rfind(|heading| heading.line_offset < scroll)
        .map(|heading| heading.line_offset)
}

/// Find a heading line offset matching a markdown anchor slug (`#section`).
///
/// This walks every heading block, including headings whose plain text is empty.
/// The outline catalog omits those, so a fragment can still land on an explicit
/// id that `[` / `]` / yank never see.
pub fn find_heading_line_by_anchor(
    document: &Document,
    width: u16,
    ctx: &RenderContext,
    anchor: &str,
) -> Option<usize> {
    if width == 0 || anchor.is_empty() {
        return None;
    }
    let target = normalize_anchor_slug(anchor);
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

pub(crate) fn heading_anchor_slug(heading: &Heading) -> String {
    heading
        .anchor
        .as_ref()
        .filter(|anchor| !anchor.is_empty())
        .map(|anchor| normalize_anchor_slug(anchor))
        .unwrap_or_else(|| slugify_heading(&Inline::plain_text(&heading.content)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ChecklistState, ChecklistStyle, TerminalSize, ViewState};
    use crate::render::{RenderContext, RenderedDocument, SyntaxAssets, Theme};

    #[test]
    fn heading_catalog_cache_reuses_collected_entries() {
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
        let view_state = ViewState::new(TerminalSize::new(80, 24).unwrap());
        let checklist_state = ChecklistState::new(ChecklistStyle::Unicode);
        let theme = Theme::default();
        let syntax_assets = SyntaxAssets::new();
        let ctx = RenderContext::new(
            &theme,
            &syntax_assets,
            &rendered,
            &document.links,
            &view_state,
            &checklist_state,
        );
        let mut cache = HeadingCatalogCache::default();
        cache.refresh(0, 80, checklist_state.revision(), &document, &ctx);
        let first = cache.entries().to_vec();
        cache.refresh(0, 80, checklist_state.revision(), &document, &ctx);
        let second = cache.entries();
        assert_eq!(first, second);
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].slug, "one");
        assert_eq!(first[1].slug, "two");
    }

    #[test]
    fn catalog_skips_empty_heading_text() {
        let document = Document {
            blocks: vec![
                crate::domain::Block::Heading(crate::domain::Heading {
                    level: crate::domain::HeadingLevel::H1,
                    content: vec![],
                    anchor: Some("empty-id".into()),
                }),
                crate::domain::Block::Heading(crate::domain::Heading {
                    level: crate::domain::HeadingLevel::H2,
                    content: vec![crate::domain::Inline::Text("Kept".into())],
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
        let view_state = ViewState::new(TerminalSize::new(80, 24).unwrap());
        let checklist_state = ChecklistState::new(ChecklistStyle::Unicode);
        let theme = Theme::default();
        let syntax_assets = SyntaxAssets::new();
        let ctx = RenderContext::new(
            &theme,
            &syntax_assets,
            &rendered,
            &document.links,
            &view_state,
            &checklist_state,
        );
        let catalog = collect_heading_catalog(&document, 80, &ctx);
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].text, "Kept");
        assert_eq!(
            find_heading_line_by_anchor(&document, 80, &ctx, "empty-id"),
            Some(0)
        );
    }

    #[test]
    fn slugify_heading_matches_github_style() {
        assert_eq!(slugify_heading("Hello World"), "hello-world");
        assert_eq!(slugify_heading("  Foo: Bar!  "), "foo-bar");
    }
}
