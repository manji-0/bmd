//! GitHub-style Markdown callouts (`> [!NOTE]`, etc.).

use pulldown_cmark::BlockQuoteKind;

use crate::parse::dto::{ParsedBlock, ParsedCallout, ParsedCalloutKind, ParsedInline};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CalloutKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl CalloutKind {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "note" | "info" => Some(Self::Note),
            "tip" | "hint" | "success" => Some(Self::Tip),
            "important" | "todo" => Some(Self::Important),
            "warning" | "failure" | "bug" | "attention" => Some(Self::Warning),
            "caution" | "danger" | "error" => Some(Self::Caution),
            "question" | "example" | "quote" => Some(Self::Note),
            _ => None,
        }
    }

    fn from_gfm(kind: BlockQuoteKind) -> Self {
        match kind {
            BlockQuoteKind::Note => Self::Note,
            BlockQuoteKind::Tip => Self::Tip,
            BlockQuoteKind::Important => Self::Important,
            BlockQuoteKind::Warning => Self::Warning,
            BlockQuoteKind::Caution => Self::Caution,
        }
    }

    fn to_parsed(self) -> ParsedCalloutKind {
        match self {
            Self::Note => ParsedCalloutKind::Note,
            Self::Tip => ParsedCalloutKind::Tip,
            Self::Important => ParsedCalloutKind::Important,
            Self::Warning => ParsedCalloutKind::Warning,
            Self::Caution => ParsedCalloutKind::Caution,
        }
    }
}

/// Normalize a blockquote that may be a GFM alert or Obsidian-style callout.
pub(crate) fn normalize_blockquote(
    gfm_kind: Option<BlockQuoteKind>,
    blocks: Vec<ParsedBlock>,
) -> ParsedBlock {
    if let Some(kind) = gfm_kind {
        return ParsedBlock::Callout(ParsedCallout {
            kind: CalloutKind::from_gfm(kind).to_parsed(),
            title: None,
            body: blocks,
        });
    }
    if let Some((kind, title, rest)) = split_callout(&blocks) {
        return ParsedBlock::Callout(ParsedCallout {
            kind: kind.to_parsed(),
            title,
            body: rest,
        });
    }
    ParsedBlock::BlockQuote(blocks)
}

fn split_callout(
    blocks: &[ParsedBlock],
) -> Option<(CalloutKind, Option<String>, Vec<ParsedBlock>)> {
    let ParsedBlock::Paragraph(inlines) = blocks.first()? else {
        return None;
    };
    let (kind, title) = parse_callout_marker(&ParsedInline::plain_text(inlines))?;
    Some((kind, title, blocks[1..].to_vec()))
}

fn parse_callout_marker(text: &str) -> Option<(CalloutKind, Option<String>)> {
    let text = text.trim();
    let rest = text.strip_prefix("[!")?;
    let (kind_raw, after) = rest.split_once(']')?;
    let kind = CalloutKind::parse(kind_raw)?;
    let title = after.trim();
    let title = if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    };
    Some((kind, title))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_callout_marker_with_title() {
        let (kind, title) = parse_callout_marker("[!TIP] Helpful advice").unwrap();
        assert_eq!(kind, CalloutKind::Tip);
        assert_eq!(title.as_deref(), Some("Helpful advice"));
    }

    #[test]
    fn parse_callout_marker_without_title() {
        let (kind, title) = parse_callout_marker("[!NOTE]").unwrap();
        assert_eq!(kind, CalloutKind::Note);
        assert!(title.is_none());
    }

    #[test]
    fn parse_callout_marker_rejects_unknown_kind() {
        assert!(parse_callout_marker("[!CUSTOM] x").is_none());
    }

    #[test]
    fn normalize_blockquote_uses_gfm_kind() {
        let block = normalize_blockquote(
            Some(BlockQuoteKind::Warning),
            vec![ParsedBlock::Paragraph(vec![ParsedInline::Text(
                "Be careful.".into(),
            )])],
        );
        let ParsedBlock::Callout(callout) = block else {
            panic!("expected callout");
        };
        assert_eq!(callout.kind, ParsedCalloutKind::Warning);
        assert_eq!(callout.body.len(), 1);
    }

    #[test]
    fn normalize_blockquote_parses_obsidian_marker_fallback() {
        let block = normalize_blockquote(
            None,
            vec![ParsedBlock::Paragraph(vec![ParsedInline::Text(
                "[!INFO] Extra context".into(),
            )])],
        );
        let ParsedBlock::Callout(callout) = block else {
            panic!("expected callout");
        };
        assert_eq!(callout.kind, ParsedCalloutKind::Note);
        assert_eq!(callout.title.as_deref(), Some("Extra context"));
    }

    #[test]
    fn normalize_blockquote_leaves_regular_quotes_untouched() {
        let input = vec![ParsedBlock::Paragraph(vec![ParsedInline::Text(
            "plain quote".into(),
        )])];
        let block = normalize_blockquote(None, input.clone());
        assert_eq!(block, ParsedBlock::BlockQuote(input));
    }
}
