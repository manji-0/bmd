//! Cross-format parity tests: equivalent markup should yield similar domain shapes.

#[cfg(test)]
mod tests {
    use crate::domain::{Alignment, Block, Inline, LinkKind};
    use crate::parse::{MarkupFormat, parse_dto};

    fn domain(format: MarkupFormat, source: &str) -> crate::domain::Document {
        parse_dto(format, source).unwrap().into_domain().unwrap()
    }

    fn first_paragraph(doc: &crate::domain::Document) -> &[Inline] {
        let block = doc
            .blocks
            .iter()
            .find(|block| matches!(block, Block::Paragraph(_)))
            .unwrap_or_else(|| panic!("expected paragraph in {:?}", doc.blocks));
        let Block::Paragraph(inlines) = block else {
            unreachable!();
        };
        inlines
    }

    fn first_list(doc: &crate::domain::Document) -> &crate::domain::List {
        let block = doc
            .blocks
            .iter()
            .find(|block| matches!(block, Block::List(_)))
            .unwrap_or_else(|| panic!("expected list in {:?}", doc.blocks));
        let Block::List(list) = block else {
            unreachable!();
        };
        list
    }

    fn block_kinds(doc: &crate::domain::Document) -> Vec<&'static str> {
        doc.blocks
            .iter()
            .map(|block| match block {
                Block::Heading(_) => "heading",
                Block::Paragraph(_) => "paragraph",
                Block::CodeBlock(_) => "code",
                Block::MathBlock(_) => "math",
                Block::BlockQuote(_) => "quote",
                Block::Callout(_) => "callout",
                Block::List(_) => "list",
                Block::DefinitionList(_) => "definition_list",
                Block::Table(_) => "table",
                Block::Rule => "rule",
            })
            .collect()
    }

    fn paragraph_has_emphasis_and_strong(doc: &crate::domain::Document) {
        let Block::Paragraph(inlines) = &doc.blocks[1] else {
            panic!("expected paragraph at index 1, got {:?}", doc.blocks);
        };
        assert!(
            inlines
                .iter()
                .any(|inline| matches!(inline, Inline::Emphasis(_)))
        );
        assert!(
            inlines
                .iter()
                .any(|inline| matches!(inline, Inline::Strong(_)))
        );
    }

    #[test]
    fn parity_heading_and_emphasis() {
        let markdown = domain(MarkupFormat::Markdown, "Title\n=====\n\n*em* **strong**");
        let asciidoc = domain(MarkupFormat::AsciiDoc, "= Title\n\n_em_ *strong*");
        let rest = domain(MarkupFormat::Rest, "Title\n=====\n\n*em* **strong**");

        assert_eq!(block_kinds(&markdown), vec!["heading", "paragraph"]);
        assert_eq!(block_kinds(&asciidoc), vec!["heading", "paragraph"]);
        assert_eq!(block_kinds(&rest), vec!["heading", "paragraph"]);
        paragraph_has_emphasis_and_strong(&markdown);
        paragraph_has_emphasis_and_strong(&asciidoc);
        paragraph_has_emphasis_and_strong(&rest);
    }

    #[test]
    fn parity_fenced_code_block() {
        let markdown = domain(MarkupFormat::Markdown, "```rust\nfn main() {}\n```");
        let asciidoc = domain(
            MarkupFormat::AsciiDoc,
            "[source,rust]\n----\nfn main() {}\n----\n",
        );
        let rest = domain(
            MarkupFormat::Rest,
            ".. code-block:: rust\n\n    fn main() {}\n",
        );

        for doc in [&markdown, &asciidoc, &rest] {
            assert_eq!(block_kinds(doc), vec!["code"]);
            let Block::CodeBlock(code) = &doc.blocks[0] else {
                panic!("expected code block");
            };
            assert_eq!(code.language.as_deref(), Some("rust"));
            assert!(code.content.contains("fn main()"));
        }
    }

    #[test]
    fn parity_display_math_block() {
        let markdown = domain(MarkupFormat::Markdown, "$$x^2$$");
        let asciidoc = domain(MarkupFormat::AsciiDoc, "[stem]\n++++\nx^2\n++++\n");
        let rest = domain(MarkupFormat::Rest, ".. math::\n\n    x^2\n");

        for doc in [&markdown, &asciidoc, &rest] {
            assert!(matches!(doc.blocks.first(), Some(Block::MathBlock(_))));
            let Block::MathBlock(math) = &doc.blocks[0] else {
                panic!("expected math block");
            };
            assert!(math.content.contains("x^2"));
        }
    }

    #[test]
    fn parity_unordered_list() {
        let markdown = domain(MarkupFormat::Markdown, "- alpha\n- beta");
        let asciidoc = domain(MarkupFormat::AsciiDoc, "* alpha\n* beta");
        let rest = domain(MarkupFormat::Rest, "- alpha\n- beta");

        for doc in [&markdown, &asciidoc, &rest] {
            assert_eq!(block_kinds(doc), vec!["list"]);
            let Block::List(list) = &doc.blocks[0] else {
                panic!("expected list");
            };
            assert!(!list.ordered);
            assert_eq!(list.items.len(), 2);
        }
    }

    #[test]
    fn parity_horizontal_rule_between_paragraphs() {
        let markdown = domain(MarkupFormat::Markdown, "Before.\n\n---\n\nAfter.");
        let asciidoc = domain(MarkupFormat::AsciiDoc, "Before.\n\n'''\n\nAfter.");
        let rest = domain(MarkupFormat::Rest, "Before.\n\n----\n\nAfter.");

        for doc in [&markdown, &asciidoc, &rest] {
            assert_eq!(block_kinds(doc), vec!["paragraph", "rule", "paragraph"]);
        }
    }

    #[test]
    fn parity_definition_list() {
        let markdown = domain(MarkupFormat::Markdown, "term\n: definition");
        let asciidoc = domain(MarkupFormat::AsciiDoc, "term:: definition");
        let rest = domain(MarkupFormat::Rest, "Intro.\n\n:term: definition");

        for doc in [&markdown, &asciidoc, &rest] {
            assert!(
                doc.blocks
                    .iter()
                    .any(|block| matches!(block, Block::DefinitionList(_)))
            );
            let block = doc
                .blocks
                .iter()
                .find(|block| matches!(block, Block::DefinitionList(_)))
                .unwrap();
            let Block::DefinitionList(list) = block else {
                panic!("expected definition list");
            };
            assert_eq!(list.items.len(), 1);
        }
    }

    #[test]
    fn parity_inline_code_in_paragraph() {
        let markdown = domain(MarkupFormat::Markdown, "Use `code` here.");
        let asciidoc = domain(MarkupFormat::AsciiDoc, "Use `code` here.");
        let rest = domain(MarkupFormat::Rest, "Use ``code`` here.");

        for doc in [&markdown, &asciidoc, &rest] {
            assert_eq!(block_kinds(doc), vec!["paragraph"]);
            let Block::Paragraph(inlines) = &doc.blocks[0] else {
                panic!("expected paragraph");
            };
            assert!(
                inlines
                    .iter()
                    .any(|inline| matches!(inline, Inline::Code(c) if c == "code"))
            );
        }
    }

    #[test]
    fn parity_checklist_items() {
        let markdown = domain(MarkupFormat::Markdown, "- [ ] Todo\n- [x] Done");
        let asciidoc = domain(MarkupFormat::AsciiDoc, "* [ ] Todo\n* [x] Done");
        let rest = domain(MarkupFormat::Rest, "- [ ] Todo\n- [x] Done");

        for doc in [&markdown, &asciidoc, &rest] {
            let Block::List(list) = &doc.blocks[0] else {
                panic!("expected list");
            };
            assert!(list.items[0].checklist_id.is_some());
            assert!(!list.items[0].checked);
            assert!(list.items[1].checklist_id.is_some());
            assert!(list.items[1].checked);
        }
    }

    #[test]
    fn parity_table_column_alignments() {
        let markdown = domain(
            MarkupFormat::Markdown,
            "| Left | Right |\n|------|------:|\n| A | B |",
        );
        let asciidoc = domain(
            MarkupFormat::AsciiDoc,
            "[cols=\"<,>\"]\n|===\n|Left |Right\n\n|A |B\n|===\n",
        );
        let rest = domain(
            MarkupFormat::Rest,
            "=====  =====\nLeft   Right\n=====  =====\nA      B\n-----  ------:\nC       D\n=====  =====\n",
        );

        for doc in [&markdown, &asciidoc, &rest] {
            let block = doc
                .blocks
                .iter()
                .find(|block| matches!(block, Block::Table(_)))
                .unwrap_or_else(|| panic!("expected table in {:?}", doc.blocks));
            let Block::Table(table) = block else {
                unreachable!();
            };
            assert_eq!(table.alignments.last().copied(), Some(Alignment::Right));
        }
    }

    #[test]
    fn parity_grid_table_column_alignments() {
        let rest = domain(
            MarkupFormat::Rest,
            "+-------+--------+\n| Left  | Right  |\n+=======+=======:+\n| A     |      B |\n+-------+--------+\n",
        );
        let Block::Table(table) = &rest.blocks[0] else {
            panic!("expected table");
        };
        assert_eq!(table.alignments, vec![Alignment::Left, Alignment::Right]);
    }

    #[test]
    fn parity_block_image_link() {
        let markdown = domain(MarkupFormat::Markdown, "![logo](/img/logo.png)");
        let asciidoc = domain(MarkupFormat::AsciiDoc, "image::/img/logo.png[]");
        let rest = domain(MarkupFormat::Rest, ".. image:: /img/logo.png\n");

        for doc in [&markdown, &asciidoc, &rest] {
            assert_eq!(doc.links.len(), 1);
            assert_eq!(doc.links[0].kind, crate::domain::LinkKind::Image);
            assert_eq!(doc.links[0].url.as_str(), "/img/logo.png");
        }
    }

    #[test]
    fn parity_table_inline_link() {
        let cases = [
            (
                MarkupFormat::Markdown,
                "| A | B |\n|---|---|\n| [link](https://example.com) | text |",
            ),
            (
                MarkupFormat::AsciiDoc,
                "|===\n|A |B\n\n|https://example.com[link] |text\n|===\n",
            ),
            (
                MarkupFormat::Rest,
                "=====  =====\nLeft   Right\n=====  =====\n`link <https://example.com>`_  text\n-----  -----\n=====  =====\n",
            ),
        ];

        for (format, source) in cases {
            let doc = domain(format, source);
            assert_eq!(doc.links.len(), 1, "{format:?} links");
            assert_eq!(
                doc.links[0].url.as_str(),
                "https://example.com",
                "{format:?} url"
            );
            let block = doc
                .blocks
                .iter()
                .find(|block| matches!(block, Block::Table(_)))
                .unwrap_or_else(|| panic!("{format:?} expected table in {:?}", doc.blocks));
            let Block::Table(table) = block else {
                unreachable!();
            };
            let has_link = table.rows.iter().flatten().any(|cell| {
                cell.iter()
                    .any(|inline| matches!(inline, Inline::Link(_, _)))
            });
            assert!(has_link, "{format:?} expected link inline in table cell");
        }
    }

    #[test]
    fn parity_inline_math_in_paragraph() {
        let markdown = domain(MarkupFormat::Markdown, "$x^2$ inline.");
        let asciidoc = domain(MarkupFormat::AsciiDoc, "stem:[x^2] inline.");
        let rest = domain(MarkupFormat::Rest, ":math:`x^2` inline.");

        for doc in [&markdown, &asciidoc, &rest] {
            let Block::Paragraph(inlines) = &doc.blocks[0] else {
                panic!("expected paragraph, got {:?}", doc.blocks);
            };
            assert!(
                inlines.iter().any(
                    |inline| matches!(inline, Inline::Math(content) if content.contains("x^2"))
                )
            );
        }
    }

    #[test]
    fn parity_inline_subscript_and_superscript() {
        let markdown = domain(MarkupFormat::Markdown, "H~2~O and x^2^y");
        let asciidoc = domain(MarkupFormat::AsciiDoc, "= Doc\n\nH~2~O and x^2^y");
        let rest = domain(MarkupFormat::Rest, "H~2~O and x^2^y");

        for doc in [&markdown, &asciidoc, &rest] {
            let block = doc
                .blocks
                .iter()
                .find(|block| matches!(block, Block::Paragraph(_)))
                .unwrap_or_else(|| panic!("expected paragraph in {:?}", doc.blocks));
            let Block::Paragraph(inlines) = block else {
                unreachable!();
            };
            assert!(
                inlines
                    .iter()
                    .any(|inline| matches!(inline, Inline::Subscript(_))),
                "expected subscript in {doc:?}"
            );
            assert!(
                inlines
                    .iter()
                    .any(|inline| matches!(inline, Inline::Superscript(_))),
                "expected superscript in {doc:?}"
            );
        }
    }

    #[test]
    fn parity_ordered_list() {
        let markdown = domain(MarkupFormat::Markdown, "1. first\n2. second");
        let asciidoc = domain(MarkupFormat::AsciiDoc, ". first\n. second");
        let rest = domain(MarkupFormat::Rest, "1. first\n2. second");

        for doc in [&markdown, &asciidoc, &rest] {
            assert_eq!(block_kinds(doc), vec!["list"]);
            let list = first_list(doc);
            assert!(list.ordered, "expected ordered list in {doc:?}");
            assert_eq!(list.items.len(), 2);
        }
    }

    #[test]
    fn parity_blockquote() {
        let markdown = domain(MarkupFormat::Markdown, "> quoted text");
        let asciidoc = domain(MarkupFormat::AsciiDoc, "____\nquoted text\n____");
        let rest = domain(MarkupFormat::Rest, "Intro.\n\n    quoted text\n");

        for doc in [&markdown, &asciidoc, &rest] {
            let quote = doc
                .blocks
                .iter()
                .find(|block| matches!(block, Block::BlockQuote(_)))
                .unwrap_or_else(|| panic!("expected blockquote in {doc:?}"));
            let Block::BlockQuote(children) = quote else {
                unreachable!();
            };
            assert!(
                children
                    .iter()
                    .any(|block| matches!(block, Block::Paragraph(_))),
                "expected quoted paragraph in {doc:?}"
            );
        }
    }

    #[test]
    fn parity_footnote_reference() {
        let markdown = domain(
            MarkupFormat::Markdown,
            "Text with footnote[^note].\n\n[^note]: Footnote body.\n",
        );
        let asciidoc = domain(
            MarkupFormat::AsciiDoc,
            "= Doc\n\nText footnote:[Footnote body].\n",
        );
        let rest = domain(
            MarkupFormat::Rest,
            "Text [#note]_.\n\n.. [#note] Footnote body.\n",
        );

        for doc in [&markdown, &asciidoc, &rest] {
            assert_eq!(doc.footnotes.len(), 1, "footnotes in {doc:?}");
            assert!(
                first_paragraph(doc)
                    .iter()
                    .any(|inline| matches!(inline, Inline::FootnoteReference(_, 1))),
                "expected footnote reference in {doc:?}"
            );
        }
    }

    #[test]
    fn parity_strikethrough() {
        let markdown = domain(MarkupFormat::Markdown, "~~deleted~~");
        let asciidoc = domain(MarkupFormat::AsciiDoc, "[line-through]#deleted#");
        let rest = domain(MarkupFormat::Rest, "~~deleted~~");

        for doc in [&markdown, &asciidoc, &rest] {
            let inlines = first_paragraph(doc);
            assert!(
                inlines.iter().any(|inline| {
                    matches!(
                        inline,
                        Inline::Strikethrough(children) if children == &[Inline::Text("deleted".into())]
                    )
                }),
                "expected strikethrough in {doc:?}"
            );
        }
    }

    #[test]
    fn parity_anchor_link() {
        let markdown = domain(
            MarkupFormat::Markdown,
            "## Hello World\n\nSee [jump](#hello-world).",
        );
        let asciidoc = domain(
            MarkupFormat::AsciiDoc,
            "= Doc\n\n[#hello-world]\n\nSee <<hello-world>>.",
        );
        let rest = domain(
            MarkupFormat::Rest,
            "Hello World\n===========\n\nSee `jump <#hello-world>`_ now.",
        );

        for doc in [&markdown, &asciidoc, &rest] {
            assert_eq!(doc.links.len(), 1, "links in {doc:?}");
            assert_eq!(doc.links[0].kind, LinkKind::Anchor, "anchor in {doc:?}");
            assert!(
                doc.links[0].url.as_str().contains("hello-world"),
                "url in {doc:?}"
            );
            assert!(
                first_paragraph(doc)
                    .iter()
                    .any(|inline| matches!(inline, Inline::Link(_, _))),
                "expected link inline in {doc:?}"
            );
        }
    }

    #[test]
    fn parity_inline_image_link_in_paragraph() {
        let markdown = domain(MarkupFormat::Markdown, "See ![logo](/img/logo.png) here.");
        let asciidoc = domain(
            MarkupFormat::AsciiDoc,
            "See image:/img/logo.png[logo] here.",
        );
        let rest = domain(
            MarkupFormat::Rest,
            ".. |logo| image:: /img/logo.png\n\nSee |logo| here.\n",
        );

        for doc in [&markdown, &asciidoc, &rest] {
            assert_eq!(doc.links.len(), 1, "{doc:?}");
            assert_eq!(doc.links[0].kind, LinkKind::Image);
            assert_eq!(doc.links[0].url.as_str(), "/img/logo.png");
            assert!(
                first_paragraph(doc)
                    .iter()
                    .any(|inline| matches!(inline, Inline::Link(_, _))),
                "expected inline image link in {doc:?}"
            );
        }
    }

    #[test]
    fn parity_asciidoc_table_of_contents() {
        let doc = domain(
            MarkupFormat::AsciiDoc,
            "= Doc\n\n== Section One\n\nBody.\n\ntoc::[]\n",
        );
        let list = first_list(&doc);
        assert!(!list.items.is_empty());
        assert!(list.items.iter().any(|item| {
            matches!(
                &item.content[0],
                Block::Paragraph(inlines)
                    if inlines.iter().any(|inline| matches!(inline, Inline::Link(_, _)))
            )
        }));
        assert!(
            doc.links
                .iter()
                .any(|link| link.kind == LinkKind::Anchor && link.url.as_str().contains("section"))
        );
    }

    #[test]
    fn parity_asciidoc_media_blocks() {
        let doc = domain(
            MarkupFormat::AsciiDoc,
            "= Doc\n\nvideo::/media/demo.mp4[]\n\naudio::/media/note.mp3[Chime]\n",
        );
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
        let link_paragraphs = doc
            .blocks
            .iter()
            .filter(|block| {
                matches!(
                    block,
                    Block::Paragraph(inlines)
                        if inlines.iter().any(|inline| matches!(inline, Inline::Link(_, _)))
                )
            })
            .count();
        assert_eq!(link_paragraphs, 2);
    }

    #[test]
    fn parity_table_of_contents_link() {
        let markdown = domain(
            MarkupFormat::Markdown,
            "# Title\n\n[[TOC]]\n\n## Section A\n\n## Section B\n",
        );
        let rest = domain(
            MarkupFormat::Rest,
            "Title\n=====\n\n.. contents::\n\nSection A\n---------\n\nSection B\n---------\n",
        );

        for doc in [&markdown, &rest] {
            let toc_links: Vec<_> = doc
                .links
                .iter()
                .filter(|l| l.kind == LinkKind::Toc)
                .collect();
            assert_eq!(toc_links.len(), 1, "expected exactly one TOC link");
            assert_eq!(toc_links[0].url.as_str(), "bmd:toc");
        }
    }
}
