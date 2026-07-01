//! Markdown document and block model.

use unicode_width::UnicodeWidthStr;

use super::link::{DocumentError, Link, LinkId};

/// A parsed markdown document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub blocks: Vec<Block>,
    pub links: Vec<Link>,
    pub mermaid_diagrams: Vec<MermaidDiagram>,
}

impl Document {
    /// Build a document after validating that link references are in bounds.
    ///
    /// # Errors
    ///
    /// Returns `DocumentError::DanglingLink` if an inline references a link id that does not exist.
    pub fn new(
        blocks: Vec<Block>,
        links: Vec<Link>,
        mermaid_diagrams: Vec<MermaidDiagram>,
    ) -> Result<Self, DocumentError> {
        let doc = Self {
            blocks,
            links,
            mermaid_diagrams,
        };
        doc.validate_links()?;
        Ok(doc)
    }

    fn validate_links(&self) -> Result<(), DocumentError> {
        let count = self.links.len();
        for (block_idx, block) in self.blocks.iter().enumerate() {
            Self::validate_block_links(block, block_idx, count)?;
        }
        Ok(())
    }

    fn validate_block_links(
        block: &Block,
        block_idx: usize,
        link_count: usize,
    ) -> Result<(), DocumentError> {
        match block {
            Block::Paragraph(inlines)
            | Block::Heading(Heading {
                content: inlines, ..
            }) => {
                Self::validate_inlines_links(inlines, block_idx, link_count)?;
            }
            Block::CodeBlock(_) | Block::Rule => {}
            Block::BlockQuote(blocks) => {
                for child in blocks {
                    Self::validate_block_links(child, block_idx, link_count)?;
                }
            }
            Block::List(list) => {
                for item in &list.items {
                    for child in &item.content {
                        Self::validate_block_links(child, block_idx, link_count)?;
                    }
                }
            }
            Block::Table(table) => {
                for cell in &table.headers {
                    Self::validate_inlines_links(cell, block_idx, link_count)?;
                }
                for row in &table.rows {
                    for cell in row {
                        Self::validate_inlines_links(cell, block_idx, link_count)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_inlines_links(
        inlines: &[Inline],
        block_idx: usize,
        link_count: usize,
    ) -> Result<(), DocumentError> {
        for inline in inlines {
            match inline {
                Inline::Link(id, children) => {
                    if id.0 >= link_count {
                        return Err(DocumentError::DanglingLink {
                            block_index: block_idx,
                            link_id: *id,
                        });
                    }
                    Self::validate_inlines_links(children, block_idx, link_count)?;
                }
                Inline::Strong(children) | Inline::Emphasis(children) => {
                    Self::validate_inlines_links(children, block_idx, link_count)?;
                }
                Inline::Text(_) | Inline::Code(_) | Inline::HardBreak | Inline::SoftBreak => {}
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Heading(Heading),
    Paragraph(Vec<Inline>),
    CodeBlock(CodeBlock),
    BlockQuote(Vec<Block>),
    List(List),
    Table(Table),
    Rule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HeadingLevel {
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
}

impl HeadingLevel {
    pub fn from_u8(level: u8) -> Option<Self> {
        match level {
            1 => Some(Self::H1),
            2 => Some(Self::H2),
            3 => Some(Self::H3),
            4 => Some(Self::H4),
            5 => Some(Self::H5),
            6 => Some(Self::H6),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            Self::H1 => 1,
            Self::H2 => 2,
            Self::H3 => 3,
            Self::H4 => 4,
            Self::H5 => 5,
            Self::H6 => 6,
        }
    }

    /// Returns the textual marker used for this heading level (e.g. "## ").
    pub fn prefix(self) -> &'static str {
        match self {
            Self::H1 => "# ",
            Self::H2 => "## ",
            Self::H3 => "### ",
            Self::H4 => "#### ",
            Self::H5 => "##### ",
            Self::H6 => "###### ",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    pub level: HeadingLevel,
    pub content: Vec<Inline>,
    /// Explicit anchor slug; when absent, derived from heading text at jump time.
    pub anchor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeBlock {
    pub language: Option<String>,
    pub content: String,
}

impl CodeBlock {
    /// Logical height of the code block: one row for the language label plus
    /// the number of content lines.
    pub fn logical_height(&self) -> usize {
        let line_count = self.content.matches('\n').count() + 1;
        line_count + 1
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct List {
    pub ordered: bool,
    pub items: Vec<ListItem>,
}

impl List {
    pub fn is_task_list(&self) -> bool {
        self.items.iter().any(|item| item.checklist_id.is_some())
    }
}

impl Heading {
    /// Returns the textual marker used to prefix this heading in the terminal.
    pub fn prefix(&self) -> &'static str {
        self.level.prefix()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem {
    pub checklist_id: Option<super::checklist::ChecklistId>,
    pub checked: bool,
    pub content: Vec<Block>,
}

impl ListItem {
    pub fn plain(content: Vec<Block>) -> Self {
        Self {
            checklist_id: None,
            checked: false,
            content,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub headers: Vec<Vec<Inline>>,
    pub rows: Vec<Vec<Vec<Inline>>>,
    pub alignments: Vec<Alignment>,
}

impl Table {
    /// Number of columns, derived from headers and first row.
    pub fn column_count(&self) -> usize {
        self.headers
            .len()
            .max(self.rows.first().map(|r| r.len()).unwrap_or(0))
    }

    /// Compute column widths within the given total terminal width.
    ///
    /// The returned widths are content-only; the rendered frame adds
    /// `3 * col_count + 1` columns for vertical borders and per-cell padding.
    pub fn allocate_column_widths(&self, total_width: usize) -> Vec<usize> {
        let col_count = self.column_count();
        if col_count == 0 {
            return Vec::new();
        }

        let frame_overhead = table_frame_overhead(col_count);
        let available = total_width.saturating_sub(frame_overhead).max(col_count);

        let mut ideal = vec![0usize; col_count];
        let mut min = vec![0usize; col_count];

        for (i, header) in self.headers.iter().enumerate() {
            ideal[i] = ideal[i].max(Inline::text_width(header));
            min[i] = min[i].max(Inline::min_word_width(header));
        }
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                ideal[i] = ideal[i].max(Inline::text_width(cell));
                min[i] = min[i].max(Inline::min_word_width(cell));
            }
        }

        let total_ideal: usize = ideal.iter().sum();
        let mut widths = if total_ideal <= available {
            ideal
        } else if min.iter().sum::<usize>() >= available {
            distribute_table_width(available, &min, &min)
        } else {
            let total_min: usize = min.iter().sum();
            let extra = available - total_min;
            let desire: Vec<usize> = ideal.iter().zip(&min).map(|(i, m)| i - m).collect();
            let mut widths = min.clone();
            let total_desire: usize = desire.iter().sum();
            if total_desire > 0 {
                for i in 0..col_count {
                    widths[i] += (extra * desire[i]) / total_desire;
                }
                let assigned: usize = widths.iter().sum();
                let mut remainder = available.saturating_sub(assigned);
                let mut idx = 0usize;
                while remainder > 0 {
                    widths[idx % col_count] += 1;
                    remainder -= 1;
                    idx += 1;
                }
            } else {
                widths = distribute_table_width(available, &min, &min);
            }
            trim_column_widths(&mut widths, available);
            widths
        };

        if total_ideal > available {
            expand_column_widths(&mut widths, available);
        }
        widths
    }

    /// Rendered table width in terminal columns for `widths`.
    pub fn table_frame_width(widths: &[usize]) -> usize {
        table_frame_overhead(widths.len()) + widths.iter().sum::<usize>()
    }
}

/// Vertical borders plus per-cell `space + content + space` padding.
fn table_frame_overhead(col_count: usize) -> usize {
    3 * col_count + 1
}

fn trim_column_widths(widths: &mut [usize], available: usize) {
    while widths.iter().sum::<usize>() > available {
        let Some(max_idx) = widths
            .iter()
            .enumerate()
            .max_by_key(|(_, v)| *v)
            .map(|(i, _)| i)
        else {
            break;
        };
        if widths[max_idx] > 1 {
            widths[max_idx] -= 1;
        } else {
            break;
        }
    }
}

fn expand_column_widths(widths: &mut [usize], available: usize) {
    let sum: usize = widths.iter().sum();
    if sum >= available {
        return;
    }
    let mut extra = available - sum;
    let n = widths.len();
    for (i, width) in widths.iter_mut().enumerate() {
        if extra == 0 {
            break;
        }
        let remaining = n - i;
        let add = extra / remaining;
        *width += add;
        extra -= add;
    }
}

fn distribute_table_width(available: usize, weights: &[usize], floors: &[usize]) -> Vec<usize> {
    let total_weight: usize = weights.iter().sum();
    if total_weight == 0 {
        return floors.iter().map(|_| 1usize).collect();
    }
    let mut out = Vec::with_capacity(weights.len());
    for (w, floor) in weights.iter().zip(floors) {
        let v = (available * w).div_ceil(total_weight).max(*floor).max(1);
        out.push(v);
    }
    // Trim if rounding pushed us over.
    while out.iter().sum::<usize>() > available {
        if let Some(max_idx) = out
            .iter()
            .enumerate()
            .max_by_key(|(_, v)| *v)
            .map(|(i, _)| i)
        {
            if out[max_idx] > 1 {
                out[max_idx] -= 1;
            } else {
                break;
            }
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    None,
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MermaidDiagram {
    pub source: String,
}

impl MermaidDiagram {
    /// Estimate the rendered width of the diagram in terminal columns.
    ///
    /// This uses a simple heuristic based on the number of lines and
    /// average node length, clamped to a reasonable range.
    pub fn estimated_width(&self) -> u16 {
        let lines: Vec<&str> = self.source.lines().collect();
        let max_line_len = lines.iter().map(|l| l.len()).max().unwrap_or(0);
        let avg_node_len = if lines.is_empty() {
            0
        } else {
            self.source.len() / lines.len()
        };
        let estimate = max_line_len.max(avg_node_len).min(200);
        estimate.clamp(20, 160) as u16
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    Code(String),
    Link(LinkId, Vec<Inline>),
    HardBreak,
    SoftBreak,
}

impl Inline {
    /// Width of the inline content in terminal columns.
    pub fn text_width(inlines: &[Inline]) -> usize {
        inlines
            .iter()
            .map(|i| match i {
                Inline::Text(t) | Inline::Code(t) => t.width(),
                Inline::Strong(c) | Inline::Emphasis(c) | Inline::Link(_, c) => Self::text_width(c),
                Inline::HardBreak | Inline::SoftBreak => 1,
            })
            .sum()
    }

    /// Maximum width of any single whitespace-separated word in the inlines.
    pub fn min_word_width(inlines: &[Inline]) -> usize {
        inlines
            .iter()
            .map(|i| match i {
                Inline::Text(t) | Inline::Code(t) => {
                    t.split_whitespace().map(|w| w.width()).max().unwrap_or(0)
                }
                Inline::Strong(c) | Inline::Emphasis(c) | Inline::Link(_, c) => {
                    Self::min_word_width(c)
                }
                Inline::HardBreak | Inline::SoftBreak => 0,
            })
            .max()
            .unwrap_or(0)
    }

    /// Extract plain text from inline children, preserving a single space for breaks.
    pub(crate) fn plain_text(inlines: &[Inline]) -> String {
        let mut out = String::new();
        for (i, inline) in inlines.iter().enumerate() {
            match inline {
                Inline::Text(t) | Inline::Code(t) => out.push_str(t),
                Inline::Strong(c) | Inline::Emphasis(c) | Inline::Link(_, c) => {
                    out.push_str(&Self::plain_text(c));
                }
                Inline::HardBreak | Inline::SoftBreak => {
                    if i > 0 {
                        out.push(' ');
                    }
                }
            }
        }
        out
    }
}
