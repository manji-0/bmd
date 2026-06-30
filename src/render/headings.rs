//! Heading position discovery for navigation.

use crate::domain::{Block, Document, Heading, HeadingLevel, Inline};

use super::context::RenderContext;
use super::measure::measure_block_height;

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
    let target = anchor.to_ascii_lowercase();
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
    slugify_heading(&Inline::plain_text(&heading.content))
}

/// GitHub-compatible heading slug: lowercase words separated by hyphens.
pub fn slugify_heading(text: &str) -> String {
    let mut slug = String::new();
    let mut prev_hyphen = false;
    for c in text.trim().to_lowercase().chars() {
        if c.is_alphanumeric() {
            slug.push(c);
            prev_hyphen = false;
        } else if !prev_hyphen && !slug.is_empty() {
            slug.push('-');
            prev_hyphen = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_heading_matches_github_style() {
        assert_eq!(slugify_heading("Hello World"), "hello-world");
        assert_eq!(slugify_heading("  Foo: Bar!  "), "foo-bar");
    }
}
