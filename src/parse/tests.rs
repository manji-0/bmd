use super::parse;
use super::{MarkupFormat, parse_document};
use crate::domain::{Block, Inline, LinkId, LinkKind};

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
fn parse_mermaid_block_becomes_preview_link() {
    let doc = parse("```mermaid\ngraph TD; A-->B;\n```").unwrap();
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].kind, LinkKind::Mermaid);
    assert_eq!(doc.mermaid_diagrams.len(), 1);
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(matches!(inlines[0], Inline::Link(LinkId(0), _)));
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
fn parse_table_inline_link() {
    let doc = parse("| A | B |\n|---|---|\n| [link](https://example.com) | text |").unwrap();
    let Block::Table(table) = &doc.blocks[0] else {
        panic!("expected table, got {:?}", doc.blocks);
    };
    assert_eq!(doc.links.len(), 1);
    let has_link = table
        .rows
        .iter()
        .flatten()
        .any(|cell| cell.iter().any(|inline| matches!(inline, Inline::Link(_, _))));
    assert!(has_link);
}

#[test]
fn parse_link_collects_url() {
    let doc = parse("[text](https://example.com)").unwrap();
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].url.as_str(), "https://example.com");
}

#[test]
fn parse_anchor_link_classified() {
    let doc = parse("[section](#bottom-section)").unwrap();
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].kind, LinkKind::Anchor);
    assert_eq!(doc.links[0].url.as_str(), "#bottom-section");
}

#[test]
fn parse_local_document_link_classified() {
    let doc = parse("[guide](./guide.md)").unwrap();
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].kind, LinkKind::Document);
    assert_eq!(doc.links[0].url.as_str(), "./guide.md");
}

#[test]
fn parse_heading_explicit_id() {
    let doc = parse("# Hello World {#custom-anchor}\n").unwrap();
    let Block::Heading(heading) = &doc.blocks[0] else {
        panic!("expected heading");
    };
    assert_eq!(heading.anchor.as_deref(), Some("custom-anchor"));
    assert!(matches!(&heading.content[0], Inline::Text(t) if t == "Hello World"));
}

#[test]
fn parse_heading_explicit_id_normalizes_at_domain_boundary() {
    let doc = parse("# Title {#_Hello_World}\n").unwrap();
    let Block::Heading(heading) = &doc.blocks[0] else {
        panic!("expected heading");
    };
    assert_eq!(heading.anchor.as_deref(), Some("hello-world"));
}

#[test]
fn parse_yaml_front_matter() {
    let doc = parse("---\ntitle: My Doc\n---\n\n# Body\n").unwrap();
    let fm = doc.front_matter.as_ref().expect("front matter");
    assert_eq!(fm.kind, crate::domain::FrontMatterKind::Yaml);
    assert_eq!(fm.title().as_deref(), Some("My Doc"));
    assert_eq!(doc.blocks.len(), 1);
    let Block::Heading(heading) = &doc.blocks[0] else {
        panic!("expected heading");
    };
    assert!(matches!(&heading.content[0], Inline::Text(t) if t == "Body"));
}

#[test]
fn parse_toml_front_matter() {
    let doc = parse("+++\ntitle = \"Guide\"\n+++\n\nParagraph.\n").unwrap();
    let fm = doc.front_matter.as_ref().expect("front matter");
    assert_eq!(fm.kind, crate::domain::FrontMatterKind::Toml);
    assert_eq!(fm.title().as_deref(), Some("Guide"));
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(matches!(&inlines[0], Inline::Text(t) if t == "Paragraph."));
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
fn parse_markdown_callout_note() {
    let doc = parse("> [!NOTE]\n> Remember this.\n").unwrap();
    let Block::BlockQuote(children) = &doc.blocks[0] else {
        panic!("expected blockquote");
    };
    assert_eq!(children.len(), 2);
    let Block::Paragraph(label) = &children[0] else {
        panic!("expected callout label");
    };
    assert!(matches!(label[0], Inline::Strong(_)));
    let Block::Paragraph(body) = &children[1] else {
        panic!("expected callout body");
    };
    assert!(matches!(&body[0], Inline::Text(t) if t == "Remember this."));
}

#[test]
fn parse_markdown_obsidian_callout_with_inline_title() {
    let doc = parse("> [!INFO] Extra context\n").unwrap();
    let Block::BlockQuote(children) = &doc.blocks[0] else {
        panic!("expected blockquote");
    };
    let Block::Paragraph(label) = &children[0] else {
        panic!("expected callout label");
    };
    let Inline::Strong(inlines) = &label[0] else {
        panic!("expected strong label");
    };
    assert!(matches!(
        &inlines[0],
        Inline::Text(t) if t == "note: Extra context"
    ));
}

#[test]
fn parse_markdown_callout_multiline_body() {
    let doc = parse("> [!WARNING]\n> Be careful.\n> Always verify.\n").unwrap();
    let Block::BlockQuote(children) = &doc.blocks[0] else {
        panic!("expected blockquote");
    };
    assert_eq!(children.len(), 2);
    let Block::Paragraph(label) = &children[0] else {
        panic!("expected callout label");
    };
    let Inline::Strong(inlines) = &label[0] else {
        panic!("expected strong label");
    };
    assert!(matches!(&inlines[0], Inline::Text(t) if t == "warning:"));
    let Block::Paragraph(body) = &children[1] else {
        panic!("expected callout body");
    };
    assert!(
        body.iter()
            .any(|i| matches!(i, Inline::Text(t) if t.contains("Be careful")))
    );
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
fn parse_task_list() {
    let doc = parse("- [ ] todo\n- [x] done").unwrap();
    assert_eq!(doc.blocks.len(), 1);
    let Block::List(list) = &doc.blocks[0] else {
        panic!("expected list");
    };
    assert!(list.is_task_list());
    assert_eq!(list.items.len(), 2);
    assert!(list.items[0].checklist_id.is_some());
    assert!(!list.items[0].checked);
    assert!(list.items[1].checklist_id.is_some());
    assert!(list.items[1].checked);
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
    assert_eq!(doc.links[0].kind, LinkKind::Mermaid);
    assert!(matches!(doc.blocks[0], Block::Paragraph(_)));
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
fn parse_standalone_image_becomes_image_link_paragraph() {
    let doc = parse("![alt text](diagram.png)").unwrap();
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].kind, LinkKind::Image);
    assert_eq!(doc.links[0].url.as_str(), "diagram.png");
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    let Inline::Link(LinkId(0), children) = &inlines[0] else {
        panic!("expected image link");
    };
    assert_eq!(children[0], Inline::Text("alt text".to_string()));
}

#[test]
fn parse_inline_image_in_mixed_paragraph_stays_as_link() {
    let doc = parse("before ![alt](diagram.png) after").unwrap();
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].kind, LinkKind::Image);
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(matches!(inlines[1], Inline::Link(_, _)));
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
fn parse_strikethrough() {
    let doc = parse("~~deleted~~").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(
        matches!(&inlines[0], Inline::Strikethrough(c) if c == &[Inline::Text("deleted".into())])
    );
}

#[test]
fn parse_subscript_and_superscript() {
    let doc = parse("H ~2~ O and x ^2^ y").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(inlines.iter().any(|inline| {
        matches!(inline, Inline::Subscript(c) if c == &[Inline::Text("2".into())])
    }));
    assert!(inlines.iter().any(|inline| {
        matches!(inline, Inline::Superscript(c) if c == &[Inline::Text("2".into())])
    }));
}

#[test]
fn parse_tight_subscript_and_superscript_without_spaces() {
    let doc = parse("H~2~O and x^2^y").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(inlines.iter().any(|inline| {
        matches!(inline, Inline::Subscript(c) if c == &[Inline::Text("2".into())])
    }));
    assert!(inlines.iter().any(|inline| {
        matches!(inline, Inline::Superscript(c) if c == &[Inline::Text("2".into())])
    }));
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
fn parse_inline_html_del_is_strikethrough() {
    let doc = parse("<del>removed</del>").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(
        matches!(&inlines[0], Inline::Strikethrough(c) if c == &[Inline::Text("removed".into())])
    );
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

#[test]
fn parse_footnote_reference_and_definition() {
    let doc = parse("Text with footnote[^note].\n\n[^note]: Footnote body.\n").unwrap();
    assert_eq!(doc.footnotes.len(), 1);
    assert_eq!(doc.footnote_order, vec![crate::domain::FootnoteId(0)]);
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(inlines.iter().any(|inline| matches!(
        inline,
        Inline::FootnoteReference(crate::domain::FootnoteId(0), 1)
    )));
    assert_eq!(doc.footnotes[0].label, "note");
    assert_eq!(doc.footnotes[0].content.len(), 1);
}

#[test]
fn parse_footnote_display_number_follows_first_reference_order() {
    let doc = parse("First[^b] and second[^a].\n\n[^a]: A def.\n\n[^b]: B def.\n").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    let refs: Vec<_> = inlines
        .iter()
        .filter_map(|inline| match inline {
            Inline::FootnoteReference(id, display) => Some((*id, *display)),
            _ => None,
        })
        .collect();
    assert_eq!(
        refs,
        vec![
            (crate::domain::FootnoteId(0), 1),
            (crate::domain::FootnoteId(1), 2)
        ]
    );
}

#[test]
fn parse_inline_math() {
    let doc = parse("Energy is $E = mc^2$.").unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(matches!(&inlines[1], Inline::Math(content) if content == "E = mc^2"));
}

#[test]
fn parse_display_math() {
    let doc = parse("$$\n\\frac{a}{b}\n$$").unwrap();
    let Block::MathBlock(math) = &doc.blocks[0] else {
        panic!("expected math block");
    };
    assert_eq!(math.content.trim(), "\\frac{a}{b}");
}

#[test]
fn parse_definition_list() {
    let doc = parse(
        "apple\n:   red fruit\n\norange\n:   orange fruit\n",
    )
    .unwrap();
    let Block::DefinitionList(list) = &doc.blocks[0] else {
        panic!("expected definition list");
    };
    assert_eq!(list.items.len(), 2);
    assert!(matches!(&list.items[0].term[0], Inline::Text(t) if t == "apple"));
    assert_eq!(list.items[0].definitions.len(), 1);
    let Block::Paragraph(inlines) = &list.items[0].definitions[0][0] else {
        panic!("expected definition paragraph");
    };
    assert!(matches!(&inlines[0], Inline::Text(t) if t == "red fruit"));
}

#[test]
fn parse_toc_marker_creates_toc_link() {
    let doc = parse("# Title\n\n[[TOC]]\n\n## Section A\n").unwrap();
    assert_eq!(doc.links.iter().filter(|l| l.kind == LinkKind::Toc).count(), 1);
    assert_eq!(doc.links.iter().find(|l| l.kind == LinkKind::Toc).unwrap().url.as_str(), "bmd:toc");
    let has_toc_inline = doc.blocks.iter().any(|b| {
        matches!(b, Block::Paragraph(inlines) if inlines.iter().any(|i| matches!(i, Inline::Link(_, _))))
    });
    assert!(has_toc_inline, "expected paragraph with TOC link");
}

#[test]
fn parse_toc_marker_case_insensitive() {
    let doc = parse("# H1\n\n[[toc]]\n\n## H2\n").unwrap();
    assert_eq!(doc.links.iter().filter(|l| l.kind == LinkKind::Toc).count(), 1);
}

#[test]
fn parse_toc_without_headings_still_creates_link() {
    let doc = parse("[[TOC]]\n\nJust a paragraph.\n").unwrap();
    assert_eq!(doc.links.iter().filter(|l| l.kind == LinkKind::Toc).count(), 1);
}

#[test]
fn parse_rst_contents_directive_creates_toc_link() {
    let doc = parse_document(
        MarkupFormat::Rest,
        "Title\n=====\n\n.. contents::\n\nSection\n-------\n\nBody.\n",
    )
    .unwrap();
    assert_eq!(doc.links.iter().filter(|l| l.kind == LinkKind::Toc).count(), 1);
    assert_eq!(doc.links.iter().find(|l| l.kind == LinkKind::Toc).unwrap().url.as_str(), "bmd:toc");
}
