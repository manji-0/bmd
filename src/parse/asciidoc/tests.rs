//! AsciiDoc parser tests.

use super::*;
use crate::domain::{Block, Inline, LinkKind};
use crate::parse::error::ParseError;
use crate::parse::format::MarkupFormat;

#[test]
fn parse_asciidoc_heading_and_emphasis() {
    let dto = parse("= Title\n\nHello *world*.\n").unwrap();
    let doc = dto.into_domain().unwrap();
    assert!(matches!(doc.blocks[0], Block::Heading(_)));
    assert!(matches!(doc.blocks[1], Block::Paragraph(_)));
}

#[test]
fn parse_asciidoc_mermaid_block() {
    let dto = parse("[mermaid]\n....\ngraph TD; A-->B;\n....\n").unwrap();
    let doc = dto.into_domain().unwrap();
    assert_eq!(doc.links[0].kind, LinkKind::Mermaid);
}

#[test]
fn parse_asciidoc_admonition_as_blockquote() {
    let dto = parse("NOTE: Remember this.\n").unwrap();
    assert!(matches!(dto.blocks[0], ParsedBlock::BlockQuote(_)));
}

#[test]
fn parse_asciidoc_xref_uses_github_slug() {
    let dto = parse("= Doc\n\n== Hello World\n\nxref:hello-world[Jump]\n").unwrap();
    let doc = dto.into_domain().unwrap();
    assert_eq!(doc.links[0].url.as_str(), "#hello-world");
    assert_eq!(doc.links[0].kind, LinkKind::Anchor);
}

#[test]
fn parse_asciidoc_rejects_heading_level_beyond_h6() {
    assert!(matches!(
        section_heading_level(6),
        Err(ParseError::InvalidHeadingLevel {
            format: MarkupFormat::AsciiDoc,
            level: 7,
        })
    ));
}

#[test]
fn parse_asciidoc_description_list_orders_term_before_colon() {
    let dto = parse("= Doc\n\nname:: value\n").unwrap();
    let ParsedBlock::DefinitionList(list) = &dto.blocks[1] else {
        panic!("expected description list, got {:?}", dto.blocks);
    };
    assert_eq!(list.items.len(), 1);
    assert!(matches!(&list.items[0].term[0], ParsedInline::Text(t) if t == "name"));
    let ParsedBlock::Paragraph(inlines) = &list.items[0].definitions[0][0] else {
        panic!("expected principal paragraph");
    };
    assert!(matches!(&inlines[0], ParsedInline::Text(t) if t == "value"));
}

#[test]
fn parse_asciidoc_delimited_table() {
    let dto = parse("|===\n|Name |Value\n\n|alpha |1\n|beta |2\n|===\n").unwrap();
    let doc = dto.into_domain().unwrap();
    let Block::Table(table) = &doc.blocks[0] else {
        panic!("expected table");
    };
    assert_eq!(table.headers.len(), 2);
    assert_eq!(table.rows.len(), 2);
}

#[test]
fn parse_asciidoc_footnote_reference_and_definition() {
    let dto = parse("= Doc\n\nText footnote:[Body here].\n").unwrap();
    assert_eq!(dto.footnotes.len(), 1);
    assert_eq!(dto.footnote_order, vec![0]);
    let doc = dto.into_domain().unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[1] else {
        panic!("expected paragraph");
    };
    assert!(matches!(
        &inlines[1],
        Inline::FootnoteReference(crate::domain::FootnoteId(0), 1)
    ));
}

#[test]
fn parse_asciidoc_line_through_highlight() {
    let dto = parse("= Doc\n\n[line-through]#removed#\n").unwrap();
    let doc = dto.into_domain().unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[1] else {
        panic!("expected paragraph");
    };
    assert!(matches!(&inlines[0], Inline::Strikethrough(_)));
}

#[test]
fn parse_asciidoc_document_attributes_as_front_matter() {
    let dto = parse("= Doc\n:toc: left\n:revnumber: 1.0\n\nBody.\n").unwrap();
    let front_matter = dto.front_matter.expect("front matter");
    assert!(front_matter.raw.contains("toc: left"));
    assert!(front_matter.raw.contains("revnumber: 1.0"));
}

#[test]
fn parse_asciidoc_subtitle_and_author_in_header() {
    let dto = parse("= Doc Title\nAuthor Name <author@example.com>\n:toc:\n\nBody.\n").unwrap();
    assert!(matches!(dto.blocks[0], ParsedBlock::Heading(_)));
    assert!(matches!(dto.blocks[1], ParsedBlock::Paragraph(_)));
    assert!(matches!(dto.blocks[2], ParsedBlock::Paragraph(_)));
}

#[test]
fn parse_asciidoc_pass_block_and_inline() {
    let dto = parse("= Doc\n\npass:[Hello]\n\n----\nRaw\n----\n").unwrap();
    let doc = dto.into_domain().unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[1] else {
        panic!("expected inline pass paragraph");
    };
    assert!(matches!(&inlines[0], Inline::Text(t) if t == "Hello"));
    let Block::CodeBlock(code) = &doc.blocks[2] else {
        panic!("expected pass block");
    };
    assert_eq!(code.content.trim_end(), "Raw");
}

#[test]
fn parse_asciidoc_inline_anchor() {
    let dto = parse("= Doc\n\n[#bookmark]\n\nJump <<bookmark>>\n").unwrap();
    let doc = dto.into_domain().unwrap();
    assert!(
        doc.links
            .iter()
            .any(|link| link.url.as_str() == "#bookmark")
    );
}

#[test]
fn parse_asciidoc_callout_marker_and_list() {
    let dto = parse("[source,ruby]\n----\nputs 'hi' <1>\n----\n<1> Prints greeting\n").unwrap();
    let doc = dto.into_domain().unwrap();
    let Block::CodeBlock(code) = &doc.blocks[0] else {
        panic!("expected code block, got {:?}", doc.blocks);
    };
    assert!(code.content.contains("<1>"));
    assert!(matches!(doc.blocks[1], Block::List(_)));
}

#[test]
fn parse_asciidoc_subscript_and_superscript() {
    let dto = parse("= Doc\n\nH~2~O and x^2^\n").unwrap();
    let doc = dto.into_domain().unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[1] else {
        panic!("expected paragraph");
    };
    assert!(
        inlines
            .iter()
            .any(|inline| matches!(inline, Inline::Subscript(_)))
    );
    assert!(
        inlines
            .iter()
            .any(|inline| matches!(inline, Inline::Superscript(_)))
    );
}

#[test]
fn parse_asciidoc_table_colspan() {
    let dto = parse("|===\n|Name |Value\n\n2+| spans\n|===\n").unwrap();
    let doc = dto.into_domain().unwrap();
    let Block::Table(table) = &doc.blocks[0] else {
        panic!("expected table");
    };
    assert_eq!(table.rows[0].len(), 2);
}

#[test]
fn parse_asciidoc_inline_and_block_stem() {
    let dto = parse("= Doc\n\nInline stem:[x^2] here.\n\n[stem]\n++++\nx^2 + y^2\n++++\n").unwrap();
    let doc = dto.into_domain().unwrap();
    let Block::Paragraph(inlines) = &doc.blocks[1] else {
        panic!("expected paragraph");
    };
    assert!(
        inlines
            .iter()
            .any(|inline| matches!(inline, Inline::Math(content) if content == "x^2"))
    );
    let Block::MathBlock(math) = &doc.blocks[2] else {
        panic!("expected math block, got {:?}", doc.blocks[2]);
    };
    assert!(math.content.contains("x^2"));
}

#[test]
fn parse_asciidoc_table_inline_link() {
    let dto = parse("|===\n|A |B\n\n|https://example.com[link] |text\n|===\n").unwrap();
    let doc = dto.into_domain().unwrap();
    let Block::Table(table) = &doc.blocks[0] else {
        panic!("expected table, got {:?}", doc.blocks);
    };
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].url.as_str(), "https://example.com");
    let has_link = table.rows.iter().flatten().any(|cell| {
        cell.iter()
            .any(|inline| matches!(inline, Inline::Link(_, _)))
    });
    assert!(has_link);
}

#[test]
fn parse_asciidoc_table_of_contents_macro() {
    let dto = parse("= Doc\n\n== Section\n\nContent.\n\ntoc::[]\n").unwrap();
    let doc = dto.into_domain().unwrap();
    let Block::List(list) = doc
        .blocks
        .iter()
        .find(|block| matches!(block, Block::List(_)))
        .expect("expected TOC list")
    else {
        panic!("expected list block");
    };
    assert!(!list.items.is_empty());
    assert!(
        list.items
            .iter()
            .any(|item| matches!(&item.content[0], Block::Paragraph(inlines) if inlines.iter().any(|inline| matches!(inline, Inline::Link(_, _)))))
    );
}

#[test]
fn parse_asciidoc_video_and_audio_blocks() {
    let dto =
        parse("= Doc\n\nvideo::/media/demo.mp4[]\n\naudio::/media/note.mp3[Chime]\n").unwrap();
    let doc = dto.into_domain().unwrap();
    assert_eq!(doc.links.len(), 2);
    assert!(
        doc.links
            .iter()
            .any(|link| link.url.as_str() == "/media/demo.mp4")
    );
    assert!(
        doc.links
            .iter()
            .any(|link| link.url.as_str() == "/media/note.mp3")
    );
    let link_blocks: Vec<_> = doc
        .blocks
        .iter()
        .filter(|block| matches!(block, Block::Paragraph(inlines) if inlines.iter().any(|inline| matches!(inline, Inline::Link(_, _)))))
        .collect();
    assert_eq!(link_blocks.len(), 2);
}
