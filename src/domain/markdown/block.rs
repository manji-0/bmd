//! Block-level markdown nodes.

use unicode_width::UnicodeWidthStr;

use super::super::callout::Callout;
use super::super::checklist::ChecklistStyle;
use super::inline::Inline;
use super::table::Table;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Heading(Heading),
    Paragraph(Vec<Inline>),
    CodeBlock(CodeBlock),
    MathBlock(MathBlock),
    BlockQuote(Vec<Block>),
    Callout(Callout),
    List(List),
    DefinitionList(DefinitionList),
    Table(Table),
    Rule,
}

impl Block {
    /// Natural unwrapped width in terminal columns.
    pub fn ideal_content_width(&self) -> usize {
        match self {
            Block::Paragraph(inlines) => Inline::text_width(inlines),
            Block::Heading(h) => h.level.prefix().width() + Inline::text_width(&h.content),
            Block::CodeBlock(cb) => {
                let label = cb
                    .language
                    .as_ref()
                    .map(|l| format!(" {l} "))
                    .unwrap_or_else(|| " code ".to_string());
                let label_w = label.width();
                let code_w = cb
                    .content
                    .lines()
                    .map(UnicodeWidthStr::width)
                    .max()
                    .unwrap_or(0);
                label_w.max(code_w)
            }
            Block::MathBlock(math) => math
                .content
                .lines()
                .map(UnicodeWidthStr::width)
                .max()
                .unwrap_or(1),
            Block::BlockQuote(blocks) => blocks
                .iter()
                .map(Self::ideal_content_width)
                .max()
                .unwrap_or(0)
                .saturating_add(2),
            Block::Callout(callout) => callout.frame_width(usize::MAX),
            Block::List(list) => list_ideal_content_width(list),
            Block::DefinitionList(list) => {
                let mut max_w = 0usize;
                for item in &list.items {
                    if !item.term.is_empty() {
                        max_w = max_w.max(Inline::text_width(&item.term));
                    }
                    for definition in &item.definitions {
                        for block in definition {
                            max_w = max_w.max(2 + Self::ideal_content_width(block));
                        }
                    }
                }
                max_w
            }
            Block::Table(table) => {
                Table::table_frame_width(&table.allocate_column_widths(usize::MAX))
            }
            Block::Rule => 3,
        }
    }
}

fn list_ideal_content_width(list: &List) -> usize {
    let checklist_marker = ChecklistStyle::Unicode.marker_width();
    list.items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let marker_w = if item.checklist_id.is_some() {
                checklist_marker
            } else if list.ordered {
                format!("{}. ", i + 1).width()
            } else {
                2
            };
            let content_w = item
                .content
                .iter()
                .map(Block::ideal_content_width)
                .max()
                .unwrap_or(0);
            marker_w + content_w
        })
        .max()
        .unwrap_or(0)
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MathBlock {
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
    pub checklist_id: Option<super::super::checklist::ChecklistId>,
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
pub struct DefinitionList {
    pub items: Vec<DefinitionItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefinitionItem {
    pub term: Vec<Inline>,
    /// Each entry is the block content of one definition (`<dd>`).
    pub definitions: Vec<Vec<Block>>,
}
