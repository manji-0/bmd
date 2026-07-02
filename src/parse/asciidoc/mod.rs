//! AsciiDoc parser: acdc-parser AST -> DTO.

use acdc_parser::{
    self, Admonition, AttributeValue, Author, Block as AdocBlock, BlockMetadata,
    DelimitedBlockType, Footnote as AdocFootnote, HorizontalAlignment, InlineMacro, InlineNode,
    ListItemCheckedStatus, Options, Source, StemContent, Table, TableColumn, TocEntry,
};

use crate::parse::dto::{
    ParsedAlignment, ParsedBlock, ParsedCodeBlock, ParsedDefinitionItem, ParsedDefinitionList,
    ParsedDocument, ParsedDocumentParts, ParsedFootnoteDefinition, ParsedFrontMatter,
    ParsedFrontMatterKind, ParsedHeading, ParsedInline, ParsedLink, ParsedLinkKind, ParsedList,
    ParsedListItem, ParsedMathBlock, ParsedTable,
};
use crate::parse::error::ParseError;
use crate::parse::format::MarkupFormat;
use crate::parse::slug::{anchor_href, normalize_anchor_slug, slugify_heading};

struct AsciiDocState<'a> {
    parts: ParsedDocumentParts,
    footnotes: Vec<ParsedFootnoteDefinition>,
    footnote_order: Vec<usize>,
    front_matter: Option<ParsedFrontMatter>,
    toc_entries: &'a [TocEntry<'a>],
}

impl<'a> AsciiDocState<'a> {
    fn new(
        footnotes: Vec<ParsedFootnoteDefinition>,
        front_matter: Option<ParsedFrontMatter>,
        toc_entries: &'a [TocEntry<'a>],
    ) -> Self {
        Self {
            parts: ParsedDocumentParts::default(),
            footnotes,
            footnote_order: Vec::new(),
            front_matter,
            toc_entries,
        }
    }

    fn into_document(self, blocks: Vec<ParsedBlock>) -> ParsedDocument {
        ParsedDocument::new(
            blocks,
            self.parts.links,
            self.parts.mermaid_diagrams,
            self.footnotes,
            self.footnote_order,
            self.front_matter,
        )
    }

    fn footnote_display_for(&mut self, footnote_id: usize) -> usize {
        if let Some(pos) = self.footnote_order.iter().position(|&id| id == footnote_id) {
            pos + 1
        } else {
            self.footnote_order.push(footnote_id);
            self.footnote_order.len()
        }
    }
}

/// Parse AsciiDoc into a [`ParsedDocument`].
pub fn parse(content: &str) -> Result<ParsedDocument, ParseError> {
    let options = Options::default();
    let parsed = acdc_parser::parse(content, &options)
        .map_err(|error| ParseError::syntax(MarkupFormat::AsciiDoc, error.to_string()))?;
    let doc = parsed.document();
    let footnotes = convert_document_footnotes(&doc.footnotes);
    let front_matter = build_front_matter(&doc.attributes, doc.header.as_ref());
    let mut state = AsciiDocState::new(footnotes, front_matter, &doc.toc_entries);
    let mut blocks = Vec::new();
    if let Some(header) = doc.header.as_ref() {
        let title = acdc_parser::inlines_to_string(&header.title);
        blocks.push(ParsedBlock::Heading(ParsedHeading {
            level: 1,
            content: map_inlines(&header.title, &mut state),
            anchor: section_anchor(&header.metadata).or_else(|| Some(slugify_heading(&title))),
        }));
        if let Some(subtitle) = &header.subtitle {
            blocks.push(ParsedBlock::Paragraph(map_inlines(subtitle, &mut state)));
        }
        for author in &header.authors {
            blocks.extend(author_blocks(author));
        }
    }
    blocks.extend(map_blocks(&doc.blocks, &mut state)?);
    Ok(state.into_document(blocks))
}

fn convert_document_footnotes(footnotes: &[AdocFootnote<'_>]) -> Vec<ParsedFootnoteDefinition> {
    footnotes
        .iter()
        .map(|footnote| ParsedFootnoteDefinition {
            label: footnote
                .id
                .map(str::to_string)
                .unwrap_or_else(|| footnote.number.to_string()),
            blocks: vec![ParsedBlock::Paragraph(
                footnote
                    .content
                    .iter()
                    .flat_map(|inline| map_inline_without_state(inline))
                    .collect(),
            )],
        })
        .collect()
}

fn build_front_matter(
    attributes: &acdc_parser::DocumentAttributes<'_>,
    header: Option<&acdc_parser::Header<'_>>,
) -> Option<ParsedFrontMatter> {
    let mut lines = Vec::new();
    for (name, value) in attributes.iter() {
        lines.push(format!("{name}: {}", attribute_value_yaml(value)));
    }
    if let Some(header) = header {
        if let Some(subtitle) = &header.subtitle {
            let text = acdc_parser::inlines_to_string(subtitle);
            if !text.is_empty() {
                lines.push(format!("subtitle: {text}"));
            }
        }
        for author in &header.authors {
            lines.push(format!("author: {}", author_display(author)));
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(ParsedFrontMatter {
        kind: ParsedFrontMatterKind::Yaml,
        raw: lines.join("\n"),
    })
}

fn attribute_value_yaml(value: &AttributeValue<'_>) -> String {
    match value {
        AttributeValue::String(text) => text.to_string(),
        AttributeValue::Bool(true) => "true".to_string(),
        AttributeValue::Bool(false) => "false".to_string(),
        AttributeValue::None => "null".to_string(),
        _ => String::new(),
    }
}

fn author_display(author: &Author<'_>) -> String {
    let mut name = author.first_name.to_string();
    if let Some(middle) = author.middle_name {
        name.push(' ');
        name.push_str(middle);
    }
    name.push(' ');
    name.push_str(author.last_name);
    if let Some(email) = author.email {
        name.push_str(" <");
        name.push_str(email);
        name.push('>');
    }
    name
}

fn author_blocks(author: &Author<'_>) -> Vec<ParsedBlock> {
    vec![ParsedBlock::Paragraph(vec![ParsedInline::Text(
        author_display(author),
    )])]
}

fn map_blocks(
    blocks: &[AdocBlock<'_>],
    state: &mut AsciiDocState<'_>,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let mut out = Vec::new();
    for block in blocks {
        out.extend(map_block(block, state)?);
    }
    Ok(out)
}

fn map_block(
    block: &AdocBlock<'_>,
    state: &mut AsciiDocState<'_>,
) -> Result<Vec<ParsedBlock>, ParseError> {
    Ok(match block {
        AdocBlock::Section(section) => {
            let title = acdc_parser::inlines_to_string(&section.title);
            let level = section_heading_level(section.level)?;
            let mut mapped = vec![ParsedBlock::Heading(ParsedHeading {
                level,
                content: map_inlines(&section.title, state),
                anchor: section_anchor(&section.metadata).or_else(|| Some(slugify_heading(&title))),
            })];
            mapped.extend(map_blocks(&section.content, state)?);
            mapped
        }
        AdocBlock::Paragraph(paragraph) => {
            vec![ParsedBlock::Paragraph(map_inlines(
                &paragraph.content,
                state,
            ))]
        }
        AdocBlock::DelimitedBlock(delimited) => map_delimited_block(delimited, state)?,
        AdocBlock::UnorderedList(list) => vec![ParsedBlock::List(ParsedList {
            ordered: false,
            items: list
                .items
                .iter()
                .map(|item| map_list_item(item, state))
                .collect::<Result<Vec<_>, _>>()?,
        })],
        AdocBlock::OrderedList(list) => vec![ParsedBlock::List(ParsedList {
            ordered: true,
            items: list
                .items
                .iter()
                .map(|item| map_list_item(item, state))
                .collect::<Result<Vec<_>, _>>()?,
        })],
        AdocBlock::ThematicBreak(_) => vec![ParsedBlock::Rule],
        AdocBlock::Image(image) => {
            let alt = acdc_parser::inlines_to_string(&image.title);
            let url = source_to_string(&image.source);
            let link_id = state
                .parts
                .push_link(ParsedLink::new(url, None, ParsedLinkKind::Image));
            vec![ParsedBlock::Paragraph(vec![ParsedInline::Link {
                link_id,
                children: vec![ParsedInline::Text(alt)],
            }])]
        }
        AdocBlock::Admonition(admonition) => vec![map_admonition(admonition, state)?],
        AdocBlock::DescriptionList(list) => map_description_list(list, state)?,
        AdocBlock::DiscreteHeader(header) => {
            let title = acdc_parser::inlines_to_string(&header.title);
            let level = section_heading_level(header.level)?;
            vec![ParsedBlock::Heading(ParsedHeading {
                level,
                content: map_inlines(&header.title, state),
                anchor: section_anchor(&header.metadata).or_else(|| Some(slugify_heading(&title))),
            })]
        }
        AdocBlock::CalloutList(list) => map_callout_list(list, state)?,
        AdocBlock::TableOfContents(_) => map_table_of_contents(state),
        AdocBlock::PageBreak(_) => vec![ParsedBlock::Rule],
        AdocBlock::Audio(audio) => vec![media_block(
            source_to_string(&audio.source),
            acdc_parser::inlines_to_string(&audio.title),
            state,
        )],
        AdocBlock::Video(video) => {
            let url = video
                .sources
                .first()
                .map(source_to_string)
                .unwrap_or_default();
            vec![media_block(
                url,
                acdc_parser::inlines_to_string(&video.title),
                state,
            )]
        }
        AdocBlock::Comment(_) | AdocBlock::DocumentAttribute(_) | _ => Vec::new(),
    })
}

fn media_block(url: String, title: String, state: &mut AsciiDocState<'_>) -> ParsedBlock {
    let link_id = state
        .parts
        .push_link(ParsedLink::new(url, None, ParsedLinkKind::Web));
    let label = if title.is_empty() {
        "[media]".into()
    } else {
        title
    };
    ParsedBlock::Paragraph(vec![ParsedInline::Link {
        link_id,
        children: vec![ParsedInline::Text(label)],
    }])
}

fn map_table_of_contents(state: &mut AsciiDocState<'_>) -> Vec<ParsedBlock> {
    let entries: Vec<(String, usize, String)> = state
        .toc_entries
        .iter()
        .map(|entry| {
            (
                entry.id.to_string(),
                entry.level as usize,
                acdc_parser::inlines_to_string(&entry.title),
            )
        })
        .collect();
    if entries.is_empty() {
        return vec![ParsedBlock::Paragraph(vec![ParsedInline::Text(
            "[table of contents]".into(),
        )])];
    }
    let items = entries
        .into_iter()
        .map(|(id, level, title)| {
            let indent = "  ".repeat(level.saturating_sub(1));
            let link_id = state.parts.push_link(ParsedLink::new(
                anchor_href(&id),
                None,
                ParsedLinkKind::Anchor,
            ));
            ParsedListItem::plain(vec![ParsedBlock::Paragraph(vec![ParsedInline::Link {
                link_id,
                children: vec![ParsedInline::Text(format!("{indent}{title}"))],
            }])])
        })
        .collect();
    vec![ParsedBlock::List(ParsedList {
        ordered: false,
        items,
    })]
}

fn map_admonition(
    admonition: &Admonition<'_>,
    state: &mut AsciiDocState<'_>,
) -> Result<ParsedBlock, ParseError> {
    let mut inner = map_blocks(&admonition.blocks, state)?;
    inner.insert(
        0,
        ParsedBlock::Paragraph(vec![ParsedInline::Strong(vec![ParsedInline::Text(
            format!("{}:", admonition.variant),
        )])]),
    );
    Ok(ParsedBlock::BlockQuote(inner))
}

fn map_callout_list(
    list: &acdc_parser::CalloutList<'_>,
    state: &mut AsciiDocState<'_>,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let items = list
        .items
        .iter()
        .map(|item| {
            let mut inlines = vec![ParsedInline::Text(format!("<{}> ", item.callout.number))];
            inlines.extend(map_inlines(&item.principal, state));
            let mut content = vec![ParsedBlock::Paragraph(inlines)];
            content.extend(map_blocks(&item.blocks, state).unwrap_or_default());
            ParsedListItem::plain(content)
        })
        .collect();
    Ok(vec![ParsedBlock::List(ParsedList {
        ordered: true,
        items,
    })])
}

fn map_description_list(
    list: &acdc_parser::DescriptionList<'_>,
    state: &mut AsciiDocState<'_>,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let items = list
        .items
        .iter()
        .map(|item| {
            let mut definition = Vec::new();
            let principal = map_inlines(&item.principal_text, state);
            if !principal.is_empty() {
                definition.push(ParsedBlock::Paragraph(principal));
            }
            definition.extend(map_blocks(&item.description, state)?);
            Ok(ParsedDefinitionItem {
                term: map_inlines(&item.term, state),
                definitions: vec![definition],
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(vec![ParsedBlock::DefinitionList(ParsedDefinitionList {
        items,
    })])
}

fn map_delimited_block(
    delimited: &acdc_parser::DelimitedBlock<'_>,
    state: &mut AsciiDocState<'_>,
) -> Result<Vec<ParsedBlock>, ParseError> {
    let style = delimited.metadata.style.unwrap_or("");
    if style.eq_ignore_ascii_case("mermaid") {
        let source = verbatim_content(&delimited.inner);
        let (link_id, _url) = state.parts.push_mermaid(source.clone());
        return Ok(vec![ParsedBlock::Paragraph(vec![ParsedInline::Link {
            link_id,
            children: vec![ParsedInline::Text(mermaid_link_label(&source))],
        }])]);
    }

    match &delimited.inner {
        DelimitedBlockType::DelimitedListing(inlines)
        | DelimitedBlockType::DelimitedLiteral(inlines) => {
            let language = delimited
                .metadata
                .attributes
                .iter()
                .next()
                .map(|(name, _)| name.to_string());
            Ok(vec![ParsedBlock::CodeBlock(ParsedCodeBlock {
                language: if style == "source" { language } else { None },
                content: acdc_parser::inlines_to_string(inlines),
            })])
        }
        DelimitedBlockType::DelimitedPass(inlines) => {
            Ok(vec![ParsedBlock::CodeBlock(ParsedCodeBlock {
                language: None,
                content: acdc_parser::inlines_to_string(inlines),
            })])
        }
        DelimitedBlockType::DelimitedVerse(inlines) => Ok(vec![ParsedBlock::Paragraph(
            map_verbatim_inlines(inlines, state),
        )]),
        DelimitedBlockType::DelimitedStem(StemContent { content, .. }) => {
            Ok(vec![ParsedBlock::MathBlock(ParsedMathBlock {
                content: (*content).to_string(),
            })])
        }
        DelimitedBlockType::DelimitedQuote(blocks) => {
            Ok(vec![ParsedBlock::BlockQuote(map_blocks(blocks, state)?)])
        }
        DelimitedBlockType::DelimitedExample(blocks)
        | DelimitedBlockType::DelimitedOpen(blocks)
        | DelimitedBlockType::DelimitedSidebar(blocks) => map_blocks(blocks, state),
        DelimitedBlockType::DelimitedTable(table) => Ok(vec![map_table(table, state)?]),
        DelimitedBlockType::DelimitedComment(_) | _ => Ok(Vec::new()),
    }
}

fn map_verbatim_inlines(
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

fn map_table(table: &Table<'_>, state: &mut AsciiDocState<'_>) -> Result<ParsedBlock, ParseError> {
    let mut grid: Vec<Vec<Option<Vec<ParsedInline>>>> = Vec::new();
    if let Some(header) = &table.header {
        place_table_row(&mut grid, 0, &header.columns, state)?;
    }
    for (row_idx, row) in table.rows.iter().enumerate() {
        let grid_row = if table.header.is_some() {
            row_idx + 1
        } else {
            row_idx
        };
        place_table_row(&mut grid, grid_row, &row.columns, state)?;
    }

    let headers = materialize_table_row(grid.first());
    let body_start = usize::from(table.header.is_some());
    let rows = grid
        .into_iter()
        .skip(body_start)
        .map(|row| materialize_table_row(Some(&row)))
        .collect();
    let alignments = table
        .columns
        .iter()
        .map(|column| map_horizontal_alignment(Some(column.halign)))
        .collect();
    Ok(ParsedBlock::Table(ParsedTable {
        headers,
        rows,
        alignments,
    }))
}

fn materialize_table_row(row: Option<&Vec<Option<Vec<ParsedInline>>>>) -> Vec<Vec<ParsedInline>> {
    row.map(|cells| {
        cells
            .iter()
            .map(|cell| cell.clone().unwrap_or_default())
            .collect()
    })
    .unwrap_or_default()
}

fn place_table_row(
    grid: &mut Vec<Vec<Option<Vec<ParsedInline>>>>,
    row_idx: usize,
    columns: &[TableColumn<'_>],
    state: &mut AsciiDocState<'_>,
) -> Result<(), ParseError> {
    while grid.len() <= row_idx {
        grid.push(Vec::new());
    }
    let mut col_idx = 0;
    for column in columns {
        col_idx = next_free_table_col(&grid[row_idx], col_idx);
        let cell = cell_to_inlines(&column.content, state)?;
        let colspan = column.colspan.max(1);
        let rowspan = column.rowspan.max(1);
        for row_offset in 0..rowspan {
            let target_row = row_idx + row_offset;
            while grid.len() <= target_row {
                grid.push(Vec::new());
            }
            for col_offset in 0..colspan {
                let target_col = col_idx + col_offset;
                while grid[target_row].len() <= target_col {
                    grid[target_row].push(None);
                }
                if row_offset == 0 && col_offset == 0 {
                    grid[target_row][target_col] = Some(cell.clone());
                } else {
                    grid[target_row][target_col] = Some(Vec::new());
                }
            }
        }
        col_idx += colspan;
    }
    Ok(())
}

fn next_free_table_col(row: &[Option<Vec<ParsedInline>>], start: usize) -> usize {
    let mut col = start;
    while col < row.len() && row[col].is_some() {
        col += 1;
    }
    col
}

fn cell_to_inlines(
    blocks: &[AdocBlock<'_>],
    state: &mut AsciiDocState<'_>,
) -> Result<Vec<ParsedInline>, ParseError> {
    let mut paragraphs = Vec::new();
    for block in blocks {
        match block {
            AdocBlock::Paragraph(paragraph) => {
                paragraphs.push(map_inlines(&paragraph.content, state));
            }
            other => {
                for mapped in map_block(other, state)? {
                    if let ParsedBlock::Paragraph(inlines) = mapped {
                        paragraphs.push(inlines);
                    }
                }
            }
        }
    }
    if paragraphs.is_empty() {
        return Ok(Vec::new());
    }
    if paragraphs.len() == 1 {
        return Ok(paragraphs.pop().unwrap());
    }
    let mut out = paragraphs.remove(0);
    for paragraph in paragraphs {
        out.push(ParsedInline::HardBreak);
        out.extend(paragraph);
    }
    Ok(out)
}

fn map_horizontal_alignment(alignment: Option<HorizontalAlignment>) -> ParsedAlignment {
    match alignment {
        Some(HorizontalAlignment::Left) => ParsedAlignment::Left,
        Some(HorizontalAlignment::Center) => ParsedAlignment::Center,
        Some(HorizontalAlignment::Right) => ParsedAlignment::Right,
        _ => ParsedAlignment::None,
    }
}

fn section_heading_level(section_level: u8) -> Result<u8, ParseError> {
    let level = section_level.saturating_add(1);
    ParseError::ensure_heading_level(MarkupFormat::AsciiDoc, level)?;
    Ok(level)
}

fn section_anchor(metadata: &BlockMetadata<'_>) -> Option<String> {
    metadata
        .id
        .as_ref()
        .map(|anchor| normalize_anchor_slug(anchor.id))
        .or_else(|| {
            metadata
                .anchors
                .first()
                .map(|anchor| normalize_anchor_slug(anchor.id))
        })
}

fn map_list_item(
    item: &acdc_parser::ListItem<'_>,
    state: &mut AsciiDocState<'_>,
) -> Result<ParsedListItem, ParseError> {
    let mut content = Vec::new();
    if !item.principal.is_empty() {
        content.push(ParsedBlock::Paragraph(map_inlines(&item.principal, state)));
    }
    content.extend(map_blocks(&item.blocks, state)?);
    let (checklist_id, checked) = match item.checked {
        Some(ListItemCheckedStatus::Checked) => (Some(state.parts.next_checklist_id()), true),
        Some(ListItemCheckedStatus::Unchecked) => (Some(state.parts.next_checklist_id()), false),
        None | Some(_) => (None, false),
    };
    Ok(ParsedListItem {
        checklist_id,
        checked,
        content,
    })
}

fn map_inlines(inlines: &[InlineNode<'_>], state: &mut AsciiDocState<'_>) -> Vec<ParsedInline> {
    inlines
        .iter()
        .flat_map(|inline| map_inline(inline, state))
        .collect()
}

fn map_inline_without_state(inline: &InlineNode<'_>) -> Vec<ParsedInline> {
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

fn verbatim_content(inner: &DelimitedBlockType<'_>) -> String {
    match inner {
        DelimitedBlockType::DelimitedListing(inlines)
        | DelimitedBlockType::DelimitedLiteral(inlines)
        | DelimitedBlockType::DelimitedPass(inlines)
        | DelimitedBlockType::DelimitedVerse(inlines) => acdc_parser::inlines_to_string(inlines),
        _ => String::new(),
    }
}

fn source_to_string(source: &Source<'_>) -> String {
    source.to_string()
}

fn mermaid_link_label(source: &str) -> String {
    let first_line = source.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        "[mermaid diagram]".to_string()
    } else {
        format!("[mermaid: {first_line}]")
    }
}

#[cfg(test)]
mod tests {
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
        let dto =
            parse("= Doc\n\nInline stem:[x^2] here.\n\n[stem]\n++++\nx^2 + y^2\n++++\n").unwrap();
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
}
