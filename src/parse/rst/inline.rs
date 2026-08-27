//! reStructuredText inline mapping and text expansions.

use std::collections::HashMap;

use parserst::Inline as RstInline;

use crate::domain::anchor_href;
use crate::parse::dto::{ParsedInline, ParsedLink, ParsedLinkKind};

use super::RestState;

pub(super) fn map_inlines(inlines: &[RstInline], state: &mut RestState) -> Vec<ParsedInline> {
    let mapped = inlines
        .iter()
        .flat_map(|inline| map_inline(inline, state))
        .collect::<Vec<_>>();
    normalize_rst_inline_patterns(mapped)
}

pub(super) fn map_inline(inline: &RstInline, state: &mut RestState) -> Vec<ParsedInline> {
    match inline {
        RstInline::Text(text) => expand_rst_text(text, state),
        RstInline::Em(children) => vec![ParsedInline::Emphasis(map_inlines(children, state))],
        RstInline::Strong(children) => vec![ParsedInline::Strong(map_inlines(children, state))],
        RstInline::Code(code) => vec![ParsedInline::Code(code.clone())],
        RstInline::Link { text, url } => {
            let (url, kind) = if url.starts_with('#') {
                (anchor_href(url), ParsedLinkKind::Anchor)
            } else {
                (url.clone(), ParsedLinkKind::classify_url(url))
            };
            let link_id = state.parts.push_link(ParsedLink::new(url, None, kind));
            vec![ParsedInline::Link {
                link_id,
                children: map_inlines(text, state),
            }]
        }
    }
}

pub(super) fn expand_rst_text(text: &str, state: &mut RestState) -> Vec<ParsedInline> {
    let mut out = Vec::new();
    for (index, segment) in text.split("\\\n").enumerate() {
        if index > 0 {
            out.push(ParsedInline::HardBreak);
        }
        for inline in expand_footnote_refs(segment, state) {
            match inline {
                ParsedInline::Text(value) => {
                    for part in expand_strikethrough(&value) {
                        match part {
                            ParsedInline::Text(text) => {
                                for subpart in
                                    crate::parse::subsup::expand_tight_sub_sup_text(&text)
                                {
                                    match subpart {
                                        ParsedInline::Text(t) => {
                                            out.extend(expand_image_substitutions_text(&t, state));
                                        }
                                        other => out.push(other),
                                    }
                                }
                            }
                            other => out.push(other),
                        }
                    }
                }
                other => out.push(other),
            }
        }
    }
    out
}

pub(super) fn expand_strikethrough(text: &str) -> Vec<ParsedInline> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("~~") {
        if start > 0 {
            out.push(ParsedInline::Text(rest[..start].to_string()));
        }
        rest = &rest[start + 2..];
        let Some(end) = rest.find("~~") else {
            out.push(ParsedInline::Text(format!("~~{rest}")));
            return out;
        };
        out.push(ParsedInline::Strikethrough(vec![ParsedInline::Text(
            rest[..end].to_string(),
        )]));
        rest = &rest[end + 2..];
    }
    if !rest.is_empty() {
        out.push(ParsedInline::Text(rest.to_string()));
    }
    out
}

pub(super) fn normalize_rst_inline_patterns(inlines: Vec<ParsedInline>) -> Vec<ParsedInline> {
    let mut out = Vec::new();
    let mut iter = inlines.into_iter();
    while let Some(inline) = iter.next() {
        let Some((before, role)) = parse_inline_role_prefix(&inline) else {
            out.push(inline);
            continue;
        };
        let Some(ParsedInline::Code(content)) = iter.next() else {
            out.push(inline);
            continue;
        };
        if !before.is_empty() {
            out.push(ParsedInline::Text(before));
        }
        out.push(match role {
            InlineRole::Math => ParsedInline::Math(content),
            InlineRole::Strike => ParsedInline::Strikethrough(vec![ParsedInline::Text(content)]),
        });
    }
    out
}

#[derive(Clone, Copy)]
pub(super) enum InlineRole {
    Math,
    Strike,
}

pub(super) fn parse_inline_role_prefix(inline: &ParsedInline) -> Option<(String, InlineRole)> {
    let ParsedInline::Text(prefix) = inline else {
        return None;
    };
    for (marker, role) in [(":math:", InlineRole::Math), (":m:", InlineRole::Math)] {
        if let Some(before) = prefix.strip_suffix(marker) {
            return Some((before.to_string(), role));
        }
    }
    if let Some(before) = prefix.strip_suffix(":strike:") {
        return Some((before.to_string(), InlineRole::Strike));
    }
    None
}

pub(super) fn extract_image_substitutions(content: &str) -> HashMap<String, String> {
    let mut substitutions = HashMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix(".. ") else {
            continue;
        };
        if let Some((name, url)) = parse_image_substitution_directive(rest) {
            substitutions.insert(name, url);
        }
    }
    substitutions
}

pub(super) fn parse_image_substitution_directive(rest: &str) -> Option<(String, String)> {
    let rest = rest.strip_prefix('|')?;
    let (name, after) = rest.split_once('|')?;
    let after = after.trim_start();
    if !after.starts_with("image::") {
        return None;
    }
    let url = after.strip_prefix("image::")?.trim();
    if name.is_empty() || url.is_empty() {
        return None;
    }
    Some((name.to_string(), url.to_string()))
}

pub(super) fn parse_misparsed_image_substitution(
    name: &str,
    argument: &str,
) -> Option<(String, String)> {
    let rest = name.strip_prefix('|')?;
    let (subst_name, suffix) = rest.split_once('|')?;
    if !suffix.trim().eq_ignore_ascii_case("image") {
        return None;
    }
    let url = argument.trim();
    if subst_name.is_empty() || url.is_empty() {
        return None;
    }
    Some((subst_name.to_string(), url.to_string()))
}

pub(super) fn expand_image_substitutions_text(
    text: &str,
    state: &mut RestState,
) -> Vec<ParsedInline> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('|') {
        if open > 0 {
            out.push(ParsedInline::Text(rest[..open].to_string()));
        }
        rest = &rest[open + 1..];
        let Some(close) = rest.find('|') else {
            out.push(ParsedInline::Text(format!("|{rest}")));
            break;
        };
        let name = &rest[..close];
        rest = &rest[close + 1..];
        if let Some(url) = state.image_substitutions.get(name) {
            let link_id =
                state
                    .parts
                    .push_link(ParsedLink::new(url.clone(), None, ParsedLinkKind::Image));
            out.push(ParsedInline::Link {
                link_id,
                children: vec![ParsedInline::Text(name.to_string())],
            });
        } else {
            out.push(ParsedInline::Text(format!("|{name}|")));
        }
    }
    if !rest.is_empty() {
        out.push(ParsedInline::Text(rest.to_string()));
    }
    out
}

pub(super) fn expand_footnote_refs(text: &str, state: &mut RestState) -> Vec<ParsedInline> {
    let mut out = Vec::new();
    expand_footnote_refs_impl(text, state, &mut out);
    if out.is_empty() && !text.is_empty() {
        out.push(ParsedInline::Text(text.to_string()));
    }
    out
}

pub(super) fn expand_footnote_refs_impl(
    text: &str,
    state: &mut RestState,
    out: &mut Vec<ParsedInline>,
) {
    let mut start = 0;
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < text.len() {
        if bytes[index] != b'[' {
            index += 1;
            continue;
        }
        let Some((label, end)) = read_footnote_reference(text, index) else {
            index += 1;
            continue;
        };
        if start < index {
            out.push(ParsedInline::Text(text[start..index].to_string()));
        }
        let footnote_id = state.footnote_id_for_label(&label);
        let display = state.footnote_display_for(footnote_id);
        out.push(ParsedInline::FootnoteReference {
            footnote_id,
            display,
        });
        index = end;
        start = end;
    }
    if start < text.len() {
        out.push(ParsedInline::Text(text[start..].to_string()));
    }
}

pub(super) fn read_footnote_reference(text: &str, open: usize) -> Option<(String, usize)> {
    let rest = text.get(open + 1..)?;
    let close = rest.find(']')?;
    let label = rest[..close].to_string();
    if label.is_empty() {
        return None;
    }
    let after = open + 1 + close + 1;
    text.as_bytes().get(after).copied().filter(|b| *b == b'_')?;
    Some((label, after + 1))
}

pub(super) fn parse_checklist_item_prefix(
    inlines: &[RstInline],
    state: &mut RestState,
) -> (Vec<RstInline>, Option<u32>, bool) {
    let plain = rst_inline_plain(inlines);
    let Some((checked, rest)) = parse_checkbox_prefix(&plain) else {
        return (inlines.to_vec(), None, false);
    };
    let id = state.parts.next_checklist_id();
    let remaining = if inlines.len() == 1 && matches!(&inlines[0], RstInline::Text(_)) {
        if rest.is_empty() {
            Vec::new()
        } else {
            vec![RstInline::Text(rest)]
        }
    } else {
        vec![RstInline::Text(rest)]
    };
    (remaining, Some(id), checked)
}

pub(super) fn parse_checkbox_prefix(text: &str) -> Option<(bool, String)> {
    let trimmed = text.trim_start();
    if let Some(rest) = trimmed.strip_prefix("[ ]") {
        return Some((false, rest.trim_start().to_string()));
    }
    for prefix in ["[x]", "[X]"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return Some((true, rest.trim_start().to_string()));
        }
    }
    None
}

pub(super) fn paragraph_is_horizontal_rule(inlines: &[RstInline]) -> bool {
    if inlines.len() != 1 {
        return false;
    }
    let RstInline::Text(text) = &inlines[0] else {
        return false;
    };
    is_transition_marker(text.trim())
}

pub(super) fn is_transition_marker(text: &str) -> bool {
    if text.len() < 4 {
        return false;
    }
    let Some(marker) = text.chars().next() else {
        return false;
    };
    if marker == '='
        || !matches!(
            marker,
            '*' | '`' | ':' | '|' | '_' | '-' | '#' | '.' | '^' | '"' | '~' | '+' | '\''
        )
    {
        return false;
    }
    text.chars().all(|ch| ch == marker)
}

pub(super) fn rst_inline_plain(inlines: &[RstInline]) -> String {
    inlines
        .iter()
        .map(|inline| match inline {
            RstInline::Text(text) => text.clone(),
            RstInline::Code(code) => code.clone(),
            RstInline::Em(children) | RstInline::Strong(children) => rst_inline_plain(children),
            RstInline::Link { text, .. } => rst_inline_plain(text),
        })
        .collect()
}

pub(super) fn mermaid_link_label(source: &str) -> String {
    let first_line = source.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        "[mermaid diagram]".to_string()
    } else {
        format!("[mermaid: {first_line}]")
    }
}
