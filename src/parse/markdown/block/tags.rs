//! Markdown start/end tag handling.

use pulldown_cmark::{Tag, TagEnd};

use crate::parse::dto::{
    ParsedBlock, ParsedCodeBlock, ParsedDefinitionItem, ParsedDefinitionList, ParsedFrontMatter,
    ParsedHeading, ParsedInline, ParsedLink, ParsedLinkKind, ParsedList, ParsedListItem,
    ParsedMermaidDiagram, ParsedTable,
};
use crate::parse::error::ParseError;

use super::super::callout;
use super::super::inline::InlineParser;
use super::super::syntax_error;
use super::{
    BlockFrame, ParserState, PendingStandaloneImage, heading_level_to_u8, map_alignment,
    map_metadata_kind, mermaid_link_label, toc_marker_link_id,
};

impl<'a> ParserState<'a> {
    pub(super) fn start_tag(&mut self, tag: Tag<'a>) -> Result<(), ParseError> {
        match tag {
            Tag::Paragraph => self.stack.push(BlockFrame::Paragraph(InlineParser::new())),
            Tag::Heading { id, .. } => {
                self.stack.push(BlockFrame::Heading {
                    parser: InlineParser::new(),
                    anchor: id.map(|s| s.into_string()).filter(|s| !s.trim().is_empty()),
                });
            }
            Tag::BlockQuote(kind) => self.stack.push(BlockFrame::BlockQuote {
                kind,
                blocks: Vec::new(),
            }),
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        if lang.is_empty() {
                            None
                        } else {
                            Some(lang.into_string())
                        }
                    }
                    pulldown_cmark::CodeBlockKind::Indented => None,
                };
                let is_mermaid = language
                    .as_ref()
                    .and_then(|l: &String| l.split_whitespace().next())
                    .map(|l| l.eq_ignore_ascii_case("mermaid"))
                    .unwrap_or(false);
                self.stack.push(BlockFrame::CodeBlock {
                    language,
                    content: String::new(),
                    is_mermaid,
                });
            }
            Tag::List(start_number) => self.stack.push(BlockFrame::List {
                ordered: start_number.is_some(),
                items: Vec::new(),
                current_item: Vec::new(),
            }),
            Tag::Item => self.stack.push(BlockFrame::ListItem {
                blocks: Vec::new(),
                checked: false,
                checklist_id: None,
                pending_inline: None,
            }),
            Tag::Table(alignments) => self.stack.push(BlockFrame::Table {
                alignments: alignments.into_iter().map(map_alignment).collect(),
                headers: Vec::new(),
                rows: Vec::new(),
            }),
            Tag::TableHead => self.stack.push(BlockFrame::TableHead(Vec::new())),
            Tag::TableRow => self.stack.push(BlockFrame::TableRow(Vec::new())),
            Tag::TableCell => self.stack.push(BlockFrame::TableCell(InlineParser::new())),
            Tag::Emphasis => self.with_inline_parser(|p| p.start_emphasis()),
            Tag::Strong => self.with_inline_parser(|p| p.start_strong()),
            Tag::Strikethrough => self.with_inline_parser(|p| p.start_strikethrough()),
            Tag::Subscript => self.with_inline_parser(|p| p.start_subscript()),
            Tag::Superscript => self.with_inline_parser(|p| p.start_superscript()),
            Tag::Link {
                dest_url, title, ..
            } => {
                let dest = dest_url.into_string();
                let title = title.into_string();
                let kind = ParsedLinkKind::classify_url(&dest);
                if let Some(p) = Self::inline_parser_from_stack(&mut self.stack) {
                    p.start_link(&mut self.links, dest, title, kind);
                }
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                let dest = dest_url.into_string();
                let title = if title.is_empty() {
                    None
                } else {
                    Some(title.into_string())
                };
                if let Some(BlockFrame::Paragraph(parser)) = self.stack.last()
                    && parser.is_empty()
                {
                    self.paragraph_standalone_image = Some(PendingStandaloneImage {
                        src: dest,
                        title,
                        alt: String::new(),
                    });
                } else if let Some(p) = Self::inline_parser_from_stack(&mut self.stack) {
                    p.start_link(
                        &mut self.links,
                        dest,
                        title.unwrap_or_default(),
                        ParsedLinkKind::Image,
                    );
                }
            }
            Tag::FootnoteDefinition(label) => {
                let label = label.into_string();
                let footnote_id = self.footnote_id_for_label(&label);
                self.stack.push(BlockFrame::FootnoteDefinition {
                    footnote_id,
                    blocks: Vec::new(),
                });
            }
            Tag::MetadataBlock(kind) => {
                self.stack.push(BlockFrame::MetadataBlock {
                    kind: map_metadata_kind(kind),
                    content: String::new(),
                });
            }
            Tag::DefinitionList => self.stack.push(BlockFrame::DefinitionList {
                items: Vec::new(),
                current_term: None,
            }),
            Tag::DefinitionListTitle => {
                self.stack.push(BlockFrame::Paragraph(InlineParser::new()));
            }
            Tag::DefinitionListDefinition => {
                self.stack.push(BlockFrame::DefinitionListDefinition {
                    blocks: Vec::new(),
                    pending_inline: None,
                })
            }
            Tag::HtmlBlock => {}
        }
        Ok(())
    }

    pub(super) fn end_tag(&mut self, tag_end: TagEnd) -> Result<(), ParseError> {
        match tag_end {
            TagEnd::Paragraph => {
                let frame = self.pop_frame("paragraph")?;
                if let BlockFrame::Paragraph(parser) = frame {
                    if let Some(pending) = self.paragraph_standalone_image.take() {
                        let label = if pending.alt.is_empty() {
                            pending.src.clone()
                        } else {
                            pending.alt
                        };
                        if pending.src.trim().is_empty() {
                            self.finish_block(ParsedBlock::Paragraph(vec![ParsedInline::Text(
                                label,
                            )]));
                        } else {
                            let link_id = self.links.len();
                            self.links.push(ParsedLink {
                                url: pending.src,
                                title: pending.title,
                                kind: ParsedLinkKind::Image,
                            });
                            self.finish_block(ParsedBlock::Paragraph(vec![ParsedInline::Link {
                                link_id,
                                children: vec![ParsedInline::Text(label)],
                            }]));
                        }
                    } else {
                        let inlines = parser.into_inlines(&mut self.links);
                        if let Some(link_id) = toc_marker_link_id(&inlines, &self.links) {
                            self.links[link_id] =
                                ParsedLink::new("bmd:toc".into(), None, ParsedLinkKind::Toc);
                            self.finish_block(ParsedBlock::Paragraph(vec![ParsedInline::Link {
                                link_id,
                                children: vec![ParsedInline::Text("[table of contents]".into())],
                            }]));
                        } else {
                            self.finish_block(ParsedBlock::Paragraph(inlines));
                        }
                    }
                }
            }
            TagEnd::Heading(level) => {
                let frame = self.pop_frame("heading")?;
                if let BlockFrame::Heading { parser, anchor } = frame {
                    let content = parser.into_inlines(&mut self.links);
                    self.finish_block(ParsedBlock::Heading(ParsedHeading {
                        level: heading_level_to_u8(level),
                        content,
                        anchor,
                    }));
                }
            }
            TagEnd::BlockQuote(_) => {
                let frame = self.pop_frame("blockquote")?;
                if let BlockFrame::BlockQuote { kind, blocks } = frame {
                    self.finish_block(callout::normalize_blockquote(kind, blocks));
                }
            }
            TagEnd::CodeBlock => {
                let frame = self.pop_frame("code block")?;
                if let BlockFrame::CodeBlock {
                    language,
                    content,
                    is_mermaid,
                } = frame
                {
                    if is_mermaid {
                        let diagram_idx = self.mermaid_diagrams.len();
                        self.mermaid_diagrams
                            .push(ParsedMermaidDiagram { source: content });
                        let label = mermaid_link_label(&self.mermaid_diagrams[diagram_idx].source);
                        let link_id = self.links.len();
                        self.links.push(ParsedLink {
                            url: format!("bmd:mermaid:{diagram_idx}"),
                            title: None,
                            kind: ParsedLinkKind::Mermaid,
                        });
                        self.finish_block(ParsedBlock::Paragraph(vec![ParsedInline::Link {
                            link_id,
                            children: vec![ParsedInline::Text(label)],
                        }]));
                    } else {
                        self.finish_block(ParsedBlock::CodeBlock(ParsedCodeBlock {
                            language,
                            content,
                        }));
                    }
                }
            }
            TagEnd::List(_) => {
                let frame = self.pop_frame("list")?;
                if let BlockFrame::List {
                    ordered,
                    items,
                    current_item,
                } = frame
                {
                    if !current_item.is_empty() {
                        return Err(syntax_error("list ended with unclosed item"));
                    }
                    self.finish_block(ParsedBlock::List(ParsedList { ordered, items }));
                }
            }
            TagEnd::Item => {
                let frame = self.pop_frame("list item")?;
                if let BlockFrame::ListItem {
                    mut blocks,
                    checked,
                    checklist_id,
                    mut pending_inline,
                } = frame
                {
                    Self::flush_pending_inline(&mut pending_inline, &mut blocks, &mut self.links);
                    if let Some(BlockFrame::List { items, .. }) = self.stack.last_mut() {
                        items.push(ParsedListItem {
                            checklist_id,
                            checked,
                            content: blocks,
                        });
                    } else {
                        return Err(syntax_error("list item without parent list"));
                    }
                }
            }
            TagEnd::Table => {
                let frame = self.pop_frame("table")?;
                if let BlockFrame::Table {
                    alignments,
                    headers,
                    rows,
                } = frame
                {
                    self.finish_block(ParsedBlock::Table(ParsedTable {
                        headers,
                        rows,
                        alignments,
                    }));
                }
            }
            TagEnd::TableHead => {
                let frame = self.pop_frame("table head")?;
                if let BlockFrame::TableHead(cells) = frame {
                    if let Some(BlockFrame::Table { headers, .. }) = self.stack.last_mut() {
                        *headers = cells;
                    } else {
                        return Err(syntax_error("table head without parent table"));
                    }
                }
            }
            TagEnd::TableRow => {
                let frame = self.pop_frame("table row")?;
                if let BlockFrame::TableRow(cells) = frame {
                    if let Some(BlockFrame::Table { rows, .. }) = self.stack.last_mut() {
                        rows.push(cells);
                    } else {
                        return Err(syntax_error("table row without parent table"));
                    }
                }
            }
            TagEnd::TableCell => {
                let frame = self.pop_frame("table cell")?;
                if let BlockFrame::TableCell(parser) = frame {
                    let cells = match self.stack.last_mut() {
                        Some(BlockFrame::TableRow(row)) => row,
                        Some(BlockFrame::TableHead(head_cells)) => head_cells,
                        _ => {
                            return Err(syntax_error("table cell without parent row or head"));
                        }
                    };
                    cells.push(parser.into_inlines(&mut self.links));
                }
            }
            TagEnd::Emphasis => self.with_inline_parser(|p| {
                p.end_emphasis().ok();
            }),
            TagEnd::Strong => self.with_inline_parser(|p| {
                p.end_strong().ok();
            }),
            TagEnd::Strikethrough => self.with_inline_parser(|p| {
                p.end_strikethrough().ok();
            }),
            TagEnd::Subscript => self.with_inline_parser(|p| {
                p.end_subscript().ok();
            }),
            TagEnd::Superscript => self.with_inline_parser(|p| {
                p.end_superscript().ok();
            }),
            TagEnd::Link => self.with_inline_parser(|p| {
                p.end_link().ok();
            }),
            TagEnd::Image => {
                if self.paragraph_standalone_image.is_none() {
                    self.with_inline_parser(|p| {
                        p.end_link().ok();
                    });
                }
            }
            TagEnd::FootnoteDefinition => {
                let frame = self.pop_frame("footnote definition")?;
                if let BlockFrame::FootnoteDefinition {
                    footnote_id,
                    blocks,
                } = frame
                    && let Some(def) = self.footnotes.get_mut(footnote_id)
                {
                    def.blocks = blocks;
                }
            }
            TagEnd::MetadataBlock(_) => {
                let frame = self.pop_frame("metadata block")?;
                if let BlockFrame::MetadataBlock { kind, content } = frame {
                    let raw = content.trim_end().to_string();
                    if !raw.is_empty() && self.front_matter.is_none() {
                        self.front_matter = Some(ParsedFrontMatter { kind, raw });
                    }
                }
            }
            TagEnd::DefinitionList => {
                let frame = self.pop_frame("definition list")?;
                if let BlockFrame::DefinitionList {
                    items,
                    current_term,
                } = frame
                {
                    let mut items = items;
                    if let Some(term) = current_term {
                        items.push(ParsedDefinitionItem {
                            term,
                            definitions: Vec::new(),
                        });
                    }
                    self.finish_block(ParsedBlock::DefinitionList(ParsedDefinitionList { items }));
                }
            }
            TagEnd::DefinitionListTitle => {
                let frame = self.pop_frame("definition list title")?;
                if let BlockFrame::Paragraph(parser) = frame
                    && let Some(BlockFrame::DefinitionList { current_term, .. }) =
                        self.stack.last_mut()
                {
                    *current_term = Some(parser.into_inlines(&mut self.links));
                }
            }
            TagEnd::DefinitionListDefinition => {
                let frame = self.pop_frame("definition list definition")?;
                if let BlockFrame::DefinitionListDefinition {
                    mut blocks,
                    mut pending_inline,
                } = frame
                {
                    Self::flush_pending_inline(&mut pending_inline, &mut blocks, &mut self.links);
                    self.finish_definition_list_definition(blocks);
                }
            }
            TagEnd::HtmlBlock => {}
        }
        Ok(())
    }
}
