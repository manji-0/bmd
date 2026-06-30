//! Domain model for the TUI markdown viewer.
//!
//! Invalid states and invalid transitions are modelled out of the type system where practical:
//! - `LinkUrl` cannot be empty.
//! - `TerminalSize` cannot have zero dimensions.
//! - `ViewState` transitions consume `self`, so the old state cannot be reused.

use std::fmt;

use unicode_width::UnicodeWidthStr;

/// A parsed markdown document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub blocks: Vec<Block>,
    pub links: Vec<Link>,
}

impl Document {
    /// Build a document after validating that link references are in bounds.
    ///
    /// # Errors
    ///
    /// Returns `DocumentError::DanglingLink` if an inline references a link id that does not exist.
    pub fn new(blocks: Vec<Block>, links: Vec<Link>) -> Result<Self, DocumentError> {
        let doc = Self { blocks, links };
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
            Block::CodeBlock(_) | Block::Mermaid(_) | Block::Rule => {}
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
    Mermaid(MermaidDiagram),
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

impl Heading {
    /// Returns the textual marker used to prefix this heading in the terminal.
    pub fn prefix(&self) -> &'static str {
        self.level.prefix()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem {
    pub content: Vec<Block>,
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
    /// The returned widths do not include the Unicode border columns; the caller
    /// must add `widths.len() + 1` to get the full table width.
    pub fn allocate_column_widths(&self, total_width: usize) -> Vec<usize> {
        let col_count = self.column_count();
        if col_count == 0 {
            return Vec::new();
        }

        let border_width = col_count + 1; // one vertical border between each column + sides
        let available = total_width.saturating_sub(border_width).max(col_count);

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
        if total_ideal <= available {
            return ideal;
        }

        let total_min: usize = min.iter().sum();
        if total_min >= available {
            // Even minimums don't fit; distribute proportionally to mins, floor at 1.
            return distribute_table_width(available, &min, &min);
        }

        let extra = available - total_min;
        let desire: Vec<usize> = ideal.iter().zip(&min).map(|(i, m)| i - m).collect();
        let mut widths = min.clone();
        let total_desire: usize = desire.iter().sum();
        if total_desire > 0 {
            for i in 0..col_count {
                widths[i] += (extra * desire[i]).div_ceil(total_desire);
            }
        } else {
            widths = distribute_table_width(available, &min, &min);
        }
        widths
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
}

/// Opaque identifier for a link stored in `Document.links`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinkId(pub usize);

impl fmt::Display for LinkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub url: LinkUrl,
    pub title: Option<String>,
}

/// A non-empty URL string.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LinkUrl(String);

impl LinkUrl {
    /// # Errors
    ///
    /// Returns `LinkUrlError::Empty` if the value is empty or whitespace only.
    pub fn new(value: String) -> Result<Self, LinkUrlError> {
        if value.trim().is_empty() {
            return Err(LinkUrlError::Empty);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LinkUrlError {
    #[error("link URL cannot be empty")]
    Empty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DocumentError {
    #[error("dangling link {link_id} in block {block_index}")]
    DanglingLink { block_index: usize, link_id: LinkId },
}

/// Terminal dimensions with the invariant that neither dimension is zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalSize {
    width: u16,
    height: u16,
}

impl TerminalSize {
    /// # Errors
    ///
    /// Returns `TerminalSizeError::ZeroDimension` if either dimension is zero.
    pub fn new(width: u16, height: u16) -> Result<Self, TerminalSizeError> {
        if width == 0 || height == 0 {
            return Err(TerminalSizeError::ZeroDimension);
        }
        Ok(Self { width, height })
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TerminalSizeError {
    #[error("terminal dimension cannot be zero")]
    ZeroDimension,
}

/// Scroll offset in logical lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scroll {
    offset: usize,
}

impl Default for Scroll {
    fn default() -> Self {
        Self::new()
    }
}

impl Scroll {
    pub const fn new() -> Self {
        Self { offset: 0 }
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }
}

/// View state with typed transitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewState {
    scroll: Scroll,
    selected_link: Option<LinkId>,
    terminal_size: TerminalSize,
}

impl ViewState {
    pub fn new(terminal_size: TerminalSize) -> Self {
        Self {
            scroll: Scroll::new(),
            selected_link: None,
            terminal_size,
        }
    }

    /// Scroll down by `n` lines, clamped to `max_scroll`.
    pub fn scroll_down(self, n: usize, max_scroll: usize) -> Self {
        let offset = self.scroll.offset.saturating_add(n).min(max_scroll);
        Self {
            scroll: Scroll { offset },
            ..self
        }
    }

    /// Scroll up by `n` lines.
    pub fn scroll_up(self, n: usize) -> Self {
        let offset = self.scroll.offset.saturating_sub(n);
        Self {
            scroll: Scroll { offset },
            ..self
        }
    }

    pub fn half_page_down(self, max_scroll: usize) -> Self {
        let n = (self.terminal_size.height() / 2) as usize;
        self.scroll_down(n, max_scroll)
    }

    pub fn half_page_up(self) -> Self {
        let n = (self.terminal_size.height() / 2) as usize;
        self.scroll_up(n)
    }

    pub fn jump_to_top(self) -> Self {
        Self {
            scroll: Scroll { offset: 0 },
            ..self
        }
    }

    pub fn jump_to_bottom(self, max_scroll: usize) -> Self {
        Self {
            scroll: Scroll { offset: max_scroll },
            ..self
        }
    }

    pub fn resize(self, terminal_size: TerminalSize) -> Self {
        Self {
            terminal_size,
            scroll: Scroll {
                offset: self.scroll.offset,
            },
            ..self
        }
    }

    pub fn select_next_link(self, document: &Document) -> Self {
        if document.links.is_empty() {
            return self;
        }
        let next = match self.selected_link {
            None => Some(LinkId(0)),
            Some(LinkId(i)) => Some(LinkId((i + 1) % document.links.len())),
        };
        Self {
            selected_link: next,
            ..self
        }
    }

    pub fn select_prev_link(self, document: &Document) -> Self {
        if document.links.is_empty() {
            return self;
        }
        let prev = match self.selected_link {
            None => Some(LinkId(document.links.len() - 1)),
            Some(LinkId(i)) => {
                if i == 0 {
                    Some(LinkId(document.links.len() - 1))
                } else {
                    Some(LinkId(i - 1))
                }
            }
        };
        Self {
            selected_link: prev,
            ..self
        }
    }

    pub fn clear_link_selection(self) -> Self {
        Self {
            selected_link: None,
            ..self
        }
    }

    pub fn scroll(&self) -> Scroll {
        self.scroll
    }

    pub fn selected_link(&self) -> Option<LinkId> {
        self.selected_link
    }

    pub fn terminal_size(&self) -> TerminalSize {
        self.terminal_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_url_rejects_empty() {
        assert!(matches!(
            LinkUrl::new("".to_string()),
            Err(LinkUrlError::Empty)
        ));
        assert!(matches!(
            LinkUrl::new("   ".to_string()),
            Err(LinkUrlError::Empty)
        ));
    }

    #[test]
    fn link_url_accepts_non_empty() {
        let url = LinkUrl::new("https://example.com".to_string()).unwrap();
        assert_eq!(url.as_str(), "https://example.com");
    }

    #[test]
    fn terminal_size_rejects_zero() {
        assert!(matches!(
            TerminalSize::new(0, 24),
            Err(TerminalSizeError::ZeroDimension)
        ));
        assert!(matches!(
            TerminalSize::new(80, 0),
            Err(TerminalSizeError::ZeroDimension)
        ));
    }

    #[test]
    fn scroll_down_clamps() {
        let size = TerminalSize::new(80, 24).unwrap();
        let state = ViewState::new(size);
        let state = state.scroll_down(100, 10);
        assert_eq!(state.scroll.offset(), 10);
    }

    #[test]
    fn scroll_up_saturates() {
        let size = TerminalSize::new(80, 24).unwrap();
        let state = ViewState::new(size);
        let state = state.scroll_up(5);
        assert_eq!(state.scroll.offset(), 0);
    }

    #[test]
    fn link_selection_wraps() {
        let doc = Document {
            blocks: vec![],
            links: vec![
                Link {
                    url: LinkUrl::new("a".to_string()).unwrap(),
                    title: None,
                },
                Link {
                    url: LinkUrl::new("b".to_string()).unwrap(),
                    title: None,
                },
            ],
        };
        let size = TerminalSize::new(80, 24).unwrap();
        let state = ViewState::new(size);
        let state = state.select_next_link(&doc);
        assert_eq!(state.selected_link(), Some(LinkId(0)));
        let state = state.select_next_link(&doc);
        assert_eq!(state.selected_link(), Some(LinkId(1)));
        let state = state.select_next_link(&doc);
        assert_eq!(state.selected_link(), Some(LinkId(0)));
    }

    #[test]
    fn heading_level_prefixes() {
        assert_eq!(HeadingLevel::H1.prefix(), "# ");
        assert_eq!(HeadingLevel::H6.prefix(), "###### ");
    }

    #[test]
    fn heading_prefix_delegates_to_level() {
        let h = Heading {
            level: HeadingLevel::H2,
            content: vec![],
        };
        assert_eq!(h.prefix(), "## ");
    }

    #[test]
    fn code_block_logical_height() {
        let cb = CodeBlock {
            language: Some("rust".to_string()),
            content: "line one\nline two".to_string(),
        };
        assert_eq!(cb.logical_height(), 3);
    }

    #[test]
    fn inline_text_width_counts_code_and_text() {
        let inlines = vec![
            Inline::Text("hello".to_string()),
            Inline::Code("world".to_string()),
        ];
        assert_eq!(Inline::text_width(&inlines), 10);
    }

    #[test]
    fn inline_min_word_width_ignores_breaks() {
        let inlines = vec![Inline::Text("a longword".to_string()), Inline::SoftBreak];
        assert_eq!(Inline::min_word_width(&inlines), 8);
    }

    #[test]
    fn table_column_count_derives_from_headers_and_rows() {
        let table = Table {
            headers: vec![vec![], vec![]],
            rows: vec![vec![vec![]]],
            alignments: vec![],
        };
        assert_eq!(table.column_count(), 2);
    }

    #[test]
    fn table_allocate_column_widths_fits_total_width() {
        let table = Table {
            headers: vec![
                vec![Inline::Text("A".to_string())],
                vec![Inline::Text("B".to_string())],
            ],
            rows: vec![vec![
                vec![Inline::Text("wide".to_string())],
                vec![Inline::Text("x".to_string())],
            ]],
            alignments: vec![Alignment::Left, Alignment::Left],
        };
        let widths = table.allocate_column_widths(20);
        let border_width = widths.len() + 1;
        assert!(widths.iter().sum::<usize>() + border_width <= 20);
        assert!(widths.iter().all(|w| *w >= 1));
    }
}
