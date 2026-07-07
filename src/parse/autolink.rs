//! Bare URL autolinking for Markdown inline text.

use crate::parse::dto::{ParsedInline, ParsedLink, ParsedLinkKind};

/// Apply bare URL autolinking to inline nodes.
pub(crate) fn apply_autolinks(
    inlines: Vec<ParsedInline>,
    links: &mut Vec<ParsedLink>,
) -> Vec<ParsedInline> {
    inlines
        .into_iter()
        .flat_map(|inline| apply_autolink_inline(inline, links, false))
        .collect()
}

/// Split a plain text node into text and autolinked URL nodes.
pub(crate) fn split_text_autolinks(text: &str, links: &mut Vec<ParsedLink>) -> Vec<ParsedInline> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some((start, end)) = find_bare_url(text, pos) {
        if start > pos {
            out.push(ParsedInline::Text(text[pos..start].to_string()));
        }
        let url = text[start..end].to_string();
        let link_id = links.len();
        links.push(ParsedLink::new(url.clone(), None, ParsedLinkKind::Web));
        out.push(ParsedInline::Link {
            link_id,
            children: vec![ParsedInline::Text(url)],
        });
        pos = end;
    }
    if pos < text.len() {
        out.push(ParsedInline::Text(text[pos..].to_string()));
    }
    if out.is_empty() && !text.is_empty() {
        out.push(ParsedInline::Text(text.to_string()));
    }
    out
}

fn apply_autolink_inline(
    inline: ParsedInline,
    links: &mut Vec<ParsedLink>,
    in_link: bool,
) -> Vec<ParsedInline> {
    match inline {
        ParsedInline::Text(text) if !in_link => split_text_autolinks(&text, links),
        ParsedInline::Strong(children) => {
            vec![ParsedInline::Strong(apply_autolinks(children, links))]
        }
        ParsedInline::Emphasis(children) => {
            vec![ParsedInline::Emphasis(apply_autolinks(children, links))]
        }
        ParsedInline::Strikethrough(children) => vec![ParsedInline::Strikethrough(
            apply_autolinks(children, links),
        )],
        ParsedInline::Subscript(children) => {
            vec![ParsedInline::Subscript(apply_autolinks(children, links))]
        }
        ParsedInline::Superscript(children) => {
            vec![ParsedInline::Superscript(apply_autolinks(children, links))]
        }
        ParsedInline::Link { link_id, children } => vec![ParsedInline::Link {
            link_id,
            children: children
                .into_iter()
                .flat_map(|child| apply_autolink_inline(child, links, true))
                .collect(),
        }],
        other => vec![other],
    }
}

fn find_bare_url(text: &str, from: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut i = from;
    while i < text.len() {
        if i + 8 <= text.len() && &bytes[i..i + 8] == b"https://" {
            let end = scan_url_end(text, i)?;
            return Some((i, end));
        }
        if i + 7 <= text.len() && &bytes[i..i + 7] == b"http://" {
            let end = scan_url_end(text, i)?;
            return Some((i, end));
        }
        i += 1;
    }
    None
}

fn scan_url_end(text: &str, start: usize) -> Option<usize> {
    let scheme_end = if text[start..].starts_with("https://") {
        start + 8
    } else if text[start..].starts_with("http://") {
        start + 7
    } else {
        return None;
    };
    if scheme_end >= text.len() {
        return None;
    }

    let mut end = scheme_end;
    let bytes = text.as_bytes();
    while end < text.len() {
        match bytes[end] {
            b'a'..=b'z'
            | b'A'..=b'Z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b':'
            | b'/'
            | b'?'
            | b'#'
            | b'['
            | b']'
            | b'@'
            | b'!'
            | b'$'
            | b'&'
            | b'\''
            | b'('
            | b')'
            | b'*'
            | b'+'
            | b','
            | b';'
            | b'='
            | b'%' => end += 1,
            _ => break,
        }
    }
    if end == scheme_end {
        return None;
    }

    while end > scheme_end {
        match bytes[end - 1] {
            b'?' | b'!' | b'.' | b',' | b':' | b'*' | b'_' | b'~' | b')' | b']' => end -= 1,
            _ => break,
        }
    }
    Some(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn links_for(text: &str) -> (Vec<ParsedInline>, Vec<ParsedLink>) {
        let mut links = Vec::new();
        let inlines = split_text_autolinks(text, &mut links);
        (inlines, links)
    }

    #[test]
    fn splits_standalone_https_url() {
        let (inlines, links) = links_for("https://github.com");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://github.com");
        assert_eq!(inlines.len(), 1);
        assert!(matches!(
            &inlines[0],
            ParsedInline::Link { link_id: 0, children }
            if children == &vec![ParsedInline::Text("https://github.com".into())]
        ));
    }

    #[test]
    fn splits_url_in_sentence() {
        let (inlines, links) =
            links_for("スタンドアロン URL も自動リンク化されます: https://github.com");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://github.com");
        assert_eq!(inlines.len(), 2);
        assert!(matches!(&inlines[0], ParsedInline::Text(t) if t.ends_with(": ")));
        assert!(matches!(&inlines[1], ParsedInline::Link { .. }));
    }

    #[test]
    fn strips_trailing_punctuation() {
        let (inlines, links) = links_for("(https://example.com/standalone)");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.com/standalone");
        assert!(matches!(&inlines[0], ParsedInline::Text(t) if t == "("));
        assert!(matches!(&inlines[2], ParsedInline::Text(t) if t == ")"));
    }

    #[test]
    fn leaves_plain_text_without_urls() {
        let (inlines, links) = links_for("no links here");
        assert!(links.is_empty());
        assert_eq!(inlines, vec![ParsedInline::Text("no links here".into())]);
    }
}
