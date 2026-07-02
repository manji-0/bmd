//! Tight subscript/superscript expansion (`H~2~O`, `x^2^y`) shared across parsers.

use crate::parse::dto::ParsedInline;

pub(crate) fn expand_tight_sub_sup_text(text: &str) -> Vec<ParsedInline> {
    let with_sub = expand_delimited(text, '~', ParsedInline::Subscript);
    with_sub
        .into_iter()
        .flat_map(|inline| match inline {
            ParsedInline::Text(value) => expand_delimited(&value, '^', ParsedInline::Superscript),
            other => vec![other],
        })
        .collect()
}

pub(crate) fn normalize_tight_sub_sup(inlines: Vec<ParsedInline>) -> Vec<ParsedInline> {
    inlines.into_iter().flat_map(normalize_inline).collect()
}

fn normalize_inline(inline: ParsedInline) -> Vec<ParsedInline> {
    match inline {
        ParsedInline::Text(text) => expand_tight_sub_sup_text(&text),
        ParsedInline::Strong(children) => {
            vec![ParsedInline::Strong(normalize_tight_sub_sup(children))]
        }
        ParsedInline::Emphasis(children) => {
            vec![ParsedInline::Emphasis(normalize_tight_sub_sup(children))]
        }
        ParsedInline::Strikethrough(children) => vec![ParsedInline::Strikethrough(
            normalize_tight_sub_sup(children),
        )],
        ParsedInline::Subscript(children) => {
            vec![ParsedInline::Subscript(normalize_tight_sub_sup(children))]
        }
        ParsedInline::Superscript(children) => {
            vec![ParsedInline::Superscript(normalize_tight_sub_sup(children))]
        }
        ParsedInline::Link { link_id, children } => vec![ParsedInline::Link {
            link_id,
            children: normalize_tight_sub_sup(children),
        }],
        other => vec![other],
    }
}

fn expand_delimited(
    text: &str,
    marker: char,
    wrap: fn(Vec<ParsedInline>) -> ParsedInline,
) -> Vec<ParsedInline> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(marker) {
        if start > 0 {
            out.push(ParsedInline::Text(rest[..start].to_string()));
        }
        rest = &rest[start + marker.len_utf8()..];
        let Some(end) = rest.find(marker) else {
            out.push(ParsedInline::Text(format!("{marker}{rest}")));
            return out;
        };
        if end == 0 {
            out.push(ParsedInline::Text(format!("{marker}{marker}")));
            rest = &rest[marker.len_utf8()..];
            continue;
        }
        out.push(wrap(vec![ParsedInline::Text(rest[..end].to_string())]));
        rest = &rest[end + marker.len_utf8()..];
    }
    if !rest.is_empty() {
        out.push(ParsedInline::Text(rest.to_string()));
    }
    out
}
