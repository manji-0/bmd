//! GitHub-compatible heading and anchor slug helpers.

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

/// Normalize an explicit anchor id or cross-reference target to a GitHub-style slug.
pub fn normalize_anchor_slug(raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches('#').trim_start_matches('_');
    slugify_heading(trimmed)
}

/// Build an in-document anchor link destination (`#slug`).
pub fn anchor_href(raw: &str) -> String {
    format!("#{}", normalize_anchor_slug(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_heading_matches_github_style() {
        assert_eq!(slugify_heading("Hello World"), "hello-world");
        assert_eq!(slugify_heading("  Foo: Bar!  "), "foo-bar");
    }

    #[test]
    fn normalize_anchor_slug_strips_prefixes() {
        assert_eq!(normalize_anchor_slug("_Section_Title"), "section-title");
        assert_eq!(normalize_anchor_slug("#Hello World"), "hello-world");
    }
}
