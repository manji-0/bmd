//! Inline HTML token parsing.

/// Parsed inline HTML token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InlineHtmlToken {
    Br,
    A,
    B,
    Strong,
    I,
    Em,
    Code,
    Del,
    S,
    Unknown,
}

impl InlineHtmlToken {
    /// Parse a self-closing or opening tag like `<br>`, `<br/>`, `<a href="...">`.
    /// Returns the token, its kind, and, for opening `<a>`, the href attribute if present.
    pub(crate) fn parse_tag(html: &str) -> (Self, InlineHtmlKind, Option<String>) {
        let trimmed = html.trim();
        let after_open = trimmed.strip_prefix('<').unwrap_or(trimmed);
        let is_closing = after_open.starts_with('/');
        let tag_body = if is_closing {
            after_open.strip_prefix('/').unwrap_or(after_open)
        } else {
            after_open
        };
        let mut iter = tag_body.splitn(2, '>');
        let inner = iter.next().unwrap_or("").trim();
        if inner.is_empty() {
            return (
                Self::Unknown,
                if is_closing {
                    InlineHtmlKind::Close
                } else {
                    InlineHtmlKind::Open
                },
                None,
            );
        }
        let mut parts = inner.split_whitespace();
        let mut tag_name = parts.next().unwrap_or("");
        let rest: &str = inner[tag_name.len()..].trim();
        let is_self_closing = rest.ends_with('/') || (!rest.is_empty() && tag_name.ends_with('/'));
        if tag_name.ends_with('/') {
            tag_name = &tag_name[..tag_name.len() - 1];
        }
        let rest = if is_self_closing {
            rest[..rest.len().saturating_sub(1)].trim_end()
        } else {
            rest
        };
        let href = if tag_name.eq_ignore_ascii_case("a") && !is_closing {
            extract_href(rest).map(String::from)
        } else {
            None
        };
        let token = match tag_name.to_ascii_lowercase().as_str() {
            "br" => Self::Br,
            "a" => Self::A,
            "b" | "big" => Self::B,
            "strong" => Self::Strong,
            "i" | "cite" | "dfn" => Self::I,
            "em" => Self::Em,
            "code" => Self::Code,
            "del" => Self::Del,
            "s" | "strike" => Self::S,
            _ => Self::Unknown,
        };
        let kind = if is_closing {
            InlineHtmlKind::Close
        } else if is_self_closing || token == Self::Br {
            InlineHtmlKind::SelfClosing
        } else {
            InlineHtmlKind::Open
        };
        (token, kind, href)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InlineHtmlKind {
    Open,
    Close,
    SelfClosing,
}

pub(crate) fn extract_href(rest: &str) -> Option<&str> {
    // rest is the attribute substring, e.g. `href="https://x" target="_blank"`
    // We look for a `href` attribute that is not part of a longer attribute name.
    // Attribute names are matched case-insensitively.
    let lower = rest.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(pos) = lower[search_from..].find("href") {
        let abs = search_from + pos;
        let prefix = &rest[abs..];
        // Because `lower` matched "href" at `abs`, `prefix` must begin with
        // `href` or `HREF` in some ASCII case. Use the lower-cased prefix to
        // strip the attribute name without extra branching.
        let after = prefix
            .strip_prefix("href")
            .or_else(|| prefix.strip_prefix("HREF"))?;
        // Ensure `href` is a complete attribute name: preceding char must be
        // whitespace or start of string, and next non-space char must be `=`.
        let prev_ok = abs == 0 || rest[..abs].ends_with(|c: char| c.is_ascii_whitespace());
        let after_ws = after.trim_start_matches(|c: char| c.is_ascii_whitespace());
        if prev_ok && after_ws.starts_with('=') {
            let value = after_ws.strip_prefix('=').unwrap_or(after_ws).trim_start();
            let quote = value.chars().next()?;
            if quote != '\"' && quote != '\'' {
                return None;
            }
            let after_quote = &value[1..];
            return after_quote.find(quote).map(|end| &after_quote[..end]);
        }
        search_from = abs + 4;
    }
    None
}
