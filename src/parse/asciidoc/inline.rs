//! AsciiDoc inline mapping and macros.

use acdc_parser::{self, DelimitedBlockType, InlineMacro, InlineNode, Source};

use crate::domain::anchor_href;
use crate::parse::dto::{ParsedInline, ParsedLink, ParsedLinkKind};

use super::AsciiDocState;

pub(super) fn map_verbatim_inlines(
    inlines: &[InlineNode<'_>],
    state: &mut AsciiDocState<'_>,
) -> Vec<ParsedInline> {
    let mut out = Vec::new();
    for inline in inlines {
        match inline {
            InlineNode::LineBreak(_) => out.push(ParsedInline::HardBreak),
            _ => out.extend(map_inline(inline, state)),
        }
    }
    out
}

pub(super) fn map_inlines(
    inlines: &[InlineNode<'_>],
    state: &mut AsciiDocState<'_>,
) -> Vec<ParsedInline> {
    inlines
        .iter()
        .flat_map(|inline| map_inline(inline, state))
        .collect()
}

pub(super) fn map_inline_without_state(inline: &InlineNode<'_>) -> Vec<ParsedInline> {
    match inline {
        InlineNode::PlainText(plain) => vec![ParsedInline::Text(plain.content.to_string())],
        InlineNode::RawText(raw) => vec![ParsedInline::Text(raw.content.to_string())],
        InlineNode::VerbatimText(verbatim) => {
            vec![ParsedInline::Code(verbatim.content.to_string())]
        }
        InlineNode::BoldText(bold) => {
            let children = map_inline_children_without_state(&bold.content);
            if is_line_through_role(bold.role) {
                vec![ParsedInline::Strikethrough(children)]
            } else {
                vec![ParsedInline::Strong(children)]
            }
        }
        InlineNode::ItalicText(italic) => {
            let children = map_inline_children_without_state(&italic.content);
            if is_line_through_role(italic.role) {
                vec![ParsedInline::Strikethrough(children)]
            } else {
                vec![ParsedInline::Emphasis(children)]
            }
        }
        InlineNode::MonospaceText(mono) => {
            vec![ParsedInline::Code(acdc_parser::inlines_to_string(
                &mono.content,
            ))]
        }
        InlineNode::HighlightText(node) => {
            let children = map_inline_children_without_state(&node.content);
            if is_line_through_role(node.role) {
                vec![ParsedInline::Strikethrough(children)]
            } else {
                children
            }
        }
        InlineNode::SubscriptText(node) => {
            vec![ParsedInline::Subscript(map_inline_children_without_state(
                &node.content,
            ))]
        }
        InlineNode::SuperscriptText(node) => {
            vec![ParsedInline::Superscript(
                map_inline_children_without_state(&node.content),
            )]
        }
        InlineNode::CurvedQuotationText(node) => map_inline_children_without_state(&node.content),
        InlineNode::CurvedApostropheText(node) => map_inline_children_without_state(&node.content),
        InlineNode::StandaloneCurvedApostrophe(_) => vec![ParsedInline::Text("'".into())],
        InlineNode::LineBreak(_) => vec![ParsedInline::HardBreak],
        InlineNode::Macro(macro_node) => map_inline_macro_without_state(macro_node),
        InlineNode::CalloutRef(callout) => {
            vec![ParsedInline::Text(format!("<{}>", callout.number))]
        }
        _ => Vec::new(),
    }
}

fn map_inline(inline: &InlineNode<'_>, state: &mut AsciiDocState<'_>) -> Vec<ParsedInline> {
    match inline {
        InlineNode::PlainText(plain) => vec![ParsedInline::Text(plain.content.to_string())],
        InlineNode::RawText(raw) => vec![ParsedInline::Text(raw.content.to_string())],
        InlineNode::VerbatimText(verbatim) => {
            vec![ParsedInline::Code(verbatim.content.to_string())]
        }
        InlineNode::BoldText(bold) => styled_inlines(&bold.content, bold.role, state, |children| {
            vec![ParsedInline::Strong(children)]
        }),
        InlineNode::ItalicText(italic) => {
            styled_inlines(&italic.content, italic.role, state, |children| {
                vec![ParsedInline::Emphasis(children)]
            })
        }
        InlineNode::MonospaceText(mono) => {
            vec![ParsedInline::Code(acdc_parser::inlines_to_string(
                &mono.content,
            ))]
        }
        InlineNode::LineBreak(_) => vec![ParsedInline::HardBreak],
        InlineNode::HighlightText(node) => {
            styled_inlines(&node.content, node.role, state, |children| {
                if is_line_through_role(node.role) {
                    vec![ParsedInline::Strikethrough(children)]
                } else {
                    children
                }
            })
        }
        InlineNode::SubscriptText(node) => {
            vec![ParsedInline::Subscript(map_inlines(&node.content, state))]
        }
        InlineNode::SuperscriptText(node) => {
            vec![ParsedInline::Superscript(map_inlines(&node.content, state))]
        }
        InlineNode::CurvedQuotationText(node) => map_inlines(&node.content, state),
        InlineNode::CurvedApostropheText(node) => map_inlines(&node.content, state),
        InlineNode::StandaloneCurvedApostrophe(_) => vec![ParsedInline::Text("'".into())],
        InlineNode::Macro(macro_node) => map_inline_macro(macro_node, state),
        InlineNode::InlineAnchor(anchor) => anchor_inlines(anchor.id, anchor.xreflabel, state),
        InlineNode::CalloutRef(callout) => {
            vec![ParsedInline::Text(format!("<{}>", callout.number))]
        }
        _ => Vec::new(),
    }
}

fn map_inline_children_without_state(inlines: &[InlineNode<'_>]) -> Vec<ParsedInline> {
    inlines.iter().flat_map(map_inline_without_state).collect()
}

fn styled_inlines<F>(
    content: &[InlineNode<'_>],
    role: Option<&str>,
    state: &mut AsciiDocState<'_>,
    wrap: F,
) -> Vec<ParsedInline>
where
    F: FnOnce(Vec<ParsedInline>) -> Vec<ParsedInline>,
{
    let children = map_inlines(content, state);
    if is_line_through_role(role) {
        vec![ParsedInline::Strikethrough(children)]
    } else {
        wrap(children)
    }
}

fn is_line_through_role(role: Option<&str>) -> bool {
    role.is_some_and(|value| {
        value.eq_ignore_ascii_case("line-through") || value.eq_ignore_ascii_case("line_through")
    })
}

fn anchor_inlines(
    id: &str,
    xreflabel: Option<&str>,
    state: &mut AsciiDocState<'_>,
) -> Vec<ParsedInline> {
    let link_id = state.parts.push_link(ParsedLink::new(
        anchor_href(id),
        None,
        ParsedLinkKind::Anchor,
    ));
    let label = xreflabel.unwrap_or(id);
    vec![ParsedInline::Link {
        link_id,
        children: vec![ParsedInline::Text(label.to_string())],
    }]
}

fn map_inline_macro(
    macro_node: &InlineMacro<'_>,
    state: &mut AsciiDocState<'_>,
) -> Vec<ParsedInline> {
    match macro_node {
        InlineMacro::Link(link) => {
            let parsed = classify_link_target(source_to_string(&link.target));
            let link_id = state.parts.push_link(parsed);
            vec![ParsedInline::Link {
                link_id,
                children: map_inlines(&link.text, state),
            }]
        }
        InlineMacro::Url(url) => {
            let link = classify_link_target(source_to_string(&url.target));
            let display = link.url.clone();
            let link_id = state.parts.push_link(link);
            vec![ParsedInline::Link {
                link_id,
                children: vec![ParsedInline::Text(display)],
            }]
        }
        InlineMacro::Mailto(mailto) => {
            let target =
                ParsedLink::from_url(format!("mailto:{}", source_to_string(&mailto.target)), None);
            let link_id = state.parts.push_link(target);
            vec![ParsedInline::Link {
                link_id,
                children: map_inlines(&mailto.text, state),
            }]
        }
        InlineMacro::Autolink(autolink) => {
            let url = source_to_string(&autolink.url);
            let link_id = state
                .parts
                .push_link(ParsedLink::from_url(url.clone(), None));
            vec![ParsedInline::Link {
                link_id,
                children: vec![ParsedInline::Text(url)],
            }]
        }
        InlineMacro::CrossReference(xref) => {
            let link_id = state.parts.push_link(ParsedLink::new(
                anchor_href(xref.target),
                None,
                ParsedLinkKind::Anchor,
            ));
            vec![ParsedInline::Link {
                link_id,
                children: map_inlines(&xref.text, state),
            }]
        }
        InlineMacro::Image(image) => {
            let url = source_to_string(&image.source);
            let link_id = state
                .parts
                .push_link(ParsedLink::new(url, None, ParsedLinkKind::Image));
            vec![ParsedInline::Link {
                link_id,
                children: map_inlines(&image.title, state),
            }]
        }
        InlineMacro::Footnote(footnote) => {
            let footnote_id = footnote.number.saturating_sub(1) as usize;
            let display = state.footnote_display_for(footnote_id);
            vec![ParsedInline::FootnoteReference {
                footnote_id,
                display,
            }]
        }
        InlineMacro::Pass(pass) => pass
            .text
            .map(|text| vec![ParsedInline::Text(text.to_string())])
            .unwrap_or_default(),
        InlineMacro::Stem(stem) => vec![ParsedInline::Math(stem.content.to_string())],
        InlineMacro::Icon(icon) => vec![ParsedInline::Text(format!(
            "[icon:{}]",
            source_to_string(&icon.target)
        ))],
        InlineMacro::Keyboard(keyboard) => vec![ParsedInline::Code(keyboard.keys.join("+"))],
        InlineMacro::Button(button) => vec![ParsedInline::Strong(vec![ParsedInline::Text(
            button.label.to_string(),
        )])],
        InlineMacro::Menu(menu) => {
            let mut path = vec![menu.target];
            path.extend(menu.items.iter().copied());
            vec![ParsedInline::Text(path.join(" > "))]
        }
        InlineMacro::IndexTerm(index_term) => match &index_term.kind {
            acdc_parser::IndexTermKind::Flow(term) => vec![ParsedInline::Text(term.to_string())],
            acdc_parser::IndexTermKind::Concealed { .. } | _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn map_inline_macro_without_state(macro_node: &InlineMacro<'_>) -> Vec<ParsedInline> {
    match macro_node {
        InlineMacro::Footnote(footnote) => {
            let footnote_id = footnote.number.saturating_sub(1) as usize;
            vec![ParsedInline::FootnoteReference {
                footnote_id,
                display: footnote.number as usize,
            }]
        }
        InlineMacro::Pass(pass) => pass
            .text
            .map(|text| vec![ParsedInline::Text(text.to_string())])
            .unwrap_or_default(),
        InlineMacro::Stem(stem) => vec![ParsedInline::Math(stem.content.to_string())],
        _ => Vec::new(),
    }
}

fn classify_link_target(url: String) -> ParsedLink {
    if url.starts_with('#') {
        ParsedLink::new(anchor_href(&url), None, ParsedLinkKind::Anchor)
    } else {
        ParsedLink::from_url(url, None)
    }
}

pub(super) fn verbatim_content(inner: &DelimitedBlockType<'_>) -> String {
    match inner {
        DelimitedBlockType::DelimitedListing(inlines)
        | DelimitedBlockType::DelimitedLiteral(inlines)
        | DelimitedBlockType::DelimitedPass(inlines)
        | DelimitedBlockType::DelimitedVerse(inlines) => acdc_parser::inlines_to_string(inlines),
        _ => String::new(),
    }
}

pub(super) fn source_to_string(source: &Source<'_>) -> String {
    source.to_string()
}

pub(super) fn mermaid_link_label(source: &str) -> String {
    let first_line = source.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        "[mermaid diagram]".to_string()
    } else {
        format!("[mermaid: {first_line}]")
    }
}
