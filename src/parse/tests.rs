use super::parse;
use crate::domain::{Block, Inline};

#[test]
fn parse_simple_paragraph() {
    let doc = parse("Hello **world**!").unwrap();
    assert_eq!(doc.blocks.len(), 1);
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert_eq!(inlines.len(), 3);
}

#[test]
fn parse_mermaid_block() {
    let doc = parse("```mermaid\ngraph TD; A-->B;\n```").unwrap();
    assert_eq!(doc.blocks.len(), 1);
    assert!(matches!(doc.blocks[0], Block::Mermaid(_)));
}

#[test]
fn parse_table() {
    let doc = parse("| a | b |\n|---|---|\n| 1 | 2 |").unwrap();
    assert_eq!(doc.blocks.len(), 1);
    let Block::Table(table) = &doc.blocks[0] else {
        panic!("expected table");
    };
    assert_eq!(table.headers.len(), 2);
    assert_eq!(table.rows.len(), 1);
}

#[test]
fn parse_link_collects_url() {
    let doc = parse("[text](https://example.com)").unwrap();
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].url.as_str(), "https://example.com");
}

#[test]
fn parse_headings_all_levels() {
    let doc = parse("# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6").unwrap();
    assert_eq!(doc.blocks.len(), 6);
    for (i, block) in doc.blocks.iter().enumerate() {
        let Block::Heading(heading) = block else {
            panic!("expected heading at {i}");
        };
        assert_eq!(heading.level.as_u8(), i as u8 + 1);
    }
}

#[test]
fn parse_blockquote() {
    let doc = parse("> quoted").unwrap();
    assert_eq!(doc.blocks.len(), 1);
    let Block::BlockQuote(children) = &doc.blocks[0] else {
        panic!("expected blockquote");
    };
    assert_eq!(children.len(), 1);
    assert!(matches!(children[0], Block::Paragraph(_)));
}

#[test]
fn parse_unordered_list() {
    let doc = parse("- alpha\n- beta").unwrap();
    assert_eq!(doc.blocks.len(), 1);
    let Block::List(list) = &doc.blocks[0] else {
        panic!("expected list");
    };
    assert!(!list.ordered);
    assert_eq!(list.items.len(), 2);
}

#[test]
fn parse_ordered_list() {
    let doc = parse("1. first\n2. second").unwrap();
    assert_eq!(doc.blocks.len(), 1);
    let Block::List(list) = &doc.blocks[0] else {
        panic!("expected list");
    };
    assert!(list.ordered);
    assert_eq!(list.items.len(), 2);
}

#[test]
fn parse_nested_list() {
    let doc = parse("- outer\n  - inner").unwrap();
    let Block::List(outer) = &doc.blocks[0] else {
        panic!("expected outer list");
    };
    assert_eq!(outer.items.len(), 1);
    let nested = outer.items[0]
        .content
        .iter()
        .find(|b| matches!(b, Block::List(_)))
        .expect("expected a nested list");
    let Block::List(inner) = nested else {
        unreachable!();
    };
    assert_eq!(inner.items.len(), 1);
}

#[test]
fn parse_fenced_code_block_with_language() {
    let doc = parse("```rust\nfn main() {}\n```").unwrap();
    assert_eq!(doc.blocks.len(), 1);
    let Block::CodeBlock(cb) = &doc.blocks[0] else {
        panic!("expected code block");
    };
    assert_eq!(cb.language.as_deref(), Some("rust"));
    assert!(cb.content.contains("fn main"));
}

#[test]
fn parse_fenced_code_block_language_is_case_insensitive() {
    let doc = parse("```MERMAID\ngraph TD;\n```").unwrap();
    assert!(matches!(doc.blocks[0], Block::Mermaid(_)));
}

#[test]
fn parse_indented_code_block() {
    let doc = parse("    line one\n    line two").unwrap();
    let Block::CodeBlock(cb) = &doc.blocks[0] else {
        panic!("expected code block");
    };
    assert!(cb.language.is_none());
    assert_eq!(cb.content.lines().count(), 2);
}

#[test]
fn parse_image_is_represented_as_link_placeholder() {
    let doc = parse("![alt text](diagram.png)").unwrap();
    assert_eq!(doc.links.len(), 1);
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(matches!(inlines[0], Inline::Link(_, _)));
}

#[test]
fn parse_emphasis_and_strong() {
    let doc = parse("*emphasis* **strong**").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(matches!(inlines[0], Inline::Emphasis(_)));
    assert!(matches!(inlines[1], Inline::Text(_)));
    assert!(matches!(inlines[2], Inline::Strong(_)));
}

#[test]
fn parse_inline_code() {
    let doc = parse("`code`").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(matches!(inlines[0], Inline::Code(_)));
}

#[test]
fn parse_hard_line_break() {
    let doc = parse("line  \\\nnext").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(inlines.iter().any(|i| matches!(i, Inline::HardBreak)));
}

#[test]
fn parse_horizontal_rule() {
    let doc = parse("---").unwrap();
    assert_eq!(doc.blocks.len(), 1);
    assert!(matches!(doc.blocks[0], Block::Rule));
}

#[test]
fn parse_strikethrough_is_treated_as_plain_text() {
    let doc = parse("~~deleted~~").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    // Strikethrough wrapper is ignored; only the text remains.
    assert!(matches!(&inlines[0], Inline::Text(t) if t == "deleted"));
}

#[test]
fn parse_empty_link_url_is_rejected() {
    let doc = parse("[text](  )").unwrap();
    assert!(doc.links.is_empty());
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(matches!(&inlines[0], Inline::Text(t) if t == "text"));
}

#[test]
fn parse_inline_html_br_becomes_hard_break() {
    let doc = parse("hello<br>world").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(matches!(inlines[1], Inline::HardBreak));
}

#[test]
fn parse_inline_html_br_with_slash_becomes_hard_break() {
    let doc = parse("hello <br/> world").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(inlines.iter().any(|i| matches!(i, Inline::HardBreak)));
}

#[test]
fn parse_inline_html_link_collected() {
    let doc = parse(r#"<a href="https://example.com">text</a>"#).unwrap();
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].url.as_str(), "https://example.com");
}

#[test]
fn parse_inline_html_link_closes_before_trailing_text() {
    let doc = parse(r#"<a href="https://example.com">link</a> after"#).unwrap();
    assert_eq!(doc.links.len(), 1);
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    let Inline::Link(_, link_children) = &inlines[0] else {
        panic!("expected link, got {:?}", inlines[0]);
    };
    assert!(
        link_children
            .iter()
            .any(|i| matches!(i, Inline::Text(t) if t == "link"))
    );
    assert!(
        inlines
            .iter()
            .any(|i| matches!(i, Inline::Text(t) if t == " after"))
    );
}

#[test]
fn parse_inline_html_link_without_href_is_ignored() {
    let doc = parse("<a>text</a>").unwrap();
    assert!(doc.links.is_empty());
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(matches!(&inlines[0], Inline::Text(t) if t == "text"));
}

#[test]
fn parse_inline_html_b_becomes_strong_and_closes() {
    let doc = parse("<b>bold</b> normal").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    let Inline::Strong(strong_children) = &inlines[0] else {
        panic!("expected strong");
    };
    assert!(
        strong_children
            .iter()
            .any(|i| matches!(i, Inline::Text(t) if t == "bold"))
    );
    assert!(
        inlines
            .iter()
            .any(|i| matches!(i, Inline::Text(t) if t == " normal"))
    );
}

#[test]
fn parse_inline_html_i_and_em_become_emphasis() {
    let doc = parse("<i>i</i> <em>em</em>").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    let emphasis_count = inlines
        .iter()
        .filter(|i| matches!(i, Inline::Emphasis(_)))
        .count();
    assert_eq!(emphasis_count, 2);
}

#[test]
fn parse_inline_html_code_becomes_inline_code() {
    let doc = parse("<code>x + y</code>").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(matches!(&inlines[0], Inline::Code(t) if t == "x + y"));
}

#[test]
fn parse_inline_html_nested_em_in_strong() {
    let doc = parse("<strong>bold <em>italic</em></strong>").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    let Inline::Strong(strong_children) = &inlines[0] else {
        panic!("expected strong");
    };
    assert!(
        strong_children
            .iter()
            .any(|i| matches!(i, Inline::Emphasis(_)))
    );
}

#[test]
fn parse_inline_html_unknown_tag_is_ignored() {
    let doc = parse("foo <span>bar</span> baz").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    let texts: Vec<&str> = inlines
        .iter()
        .filter_map(|i| match i {
            Inline::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["foo ", "bar", " baz"]);
}

#[test]
fn parse_inline_html_del_is_flattened() {
    let doc = parse("<del>removed</del>").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(matches!(&inlines[0], Inline::Text(t) if t == "removed"));
}

#[test]
fn parse_inline_html_href_ignores_prefixed_attribute() {
    let doc = parse(r#"<a data-href="wrong" href="https://right.example">x</a>"#).unwrap();
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].url.as_str(), "https://right.example");
}

#[test]
fn parse_inline_html_in_heading() {
    let doc = parse("# hello <br> world").unwrap();
    let Block::Heading(heading) = &doc.blocks[0] else {
        panic!("expected heading");
    };
    assert!(
        heading
            .content
            .iter()
            .any(|i| matches!(i, Inline::HardBreak))
    );
}
