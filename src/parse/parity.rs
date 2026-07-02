//! Cross-format parity tests: equivalent markup should yield similar domain shapes.

#[cfg(test)]
mod tests {
    use crate::domain::{Alignment, Block, Inline};
    use crate::parse::{MarkupFormat, parse_dto};

    fn domain(format: MarkupFormat, source: &str) -> crate::domain::Document {
        parse_dto(format, source).unwrap().into_domain().unwrap()
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
        assert!(inlines.iter().any(|inline| matches!(inline, Inline::Emphasis(_))));
        assert!(inlines.iter().any(|inline| matches!(inline, Inline::Strong(_))));
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
            assert!(doc.blocks.iter().any(|block| matches!(block, Block::DefinitionList(_))));
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
            assert!(inlines.iter().any(|inline| matches!(inline, Inline::Code(c) if c == "code")));
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
}
