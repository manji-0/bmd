//! GFM / Obsidian-style alert callouts.

use unicode_width::UnicodeWidthStr;

use super::markdown::Block;

/// Kind of alert callout (GitHub alert syntax).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CalloutKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl CalloutKind {
    /// Uppercase label shown in the callout header row.
    pub fn label_upper(self) -> &'static str {
        match self {
            Self::Note => "NOTE",
            Self::Tip => "TIP",
            Self::Important => "IMPORTANT",
            Self::Warning => "WARNING",
            Self::Caution => "CAUTION",
        }
    }
}

/// A boxed alert panel with optional inline title text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Callout {
    pub kind: CalloutKind,
    pub title: Option<String>,
    pub body: Vec<Block>,
}

impl Callout {
    /// Header text for the top border row.
    pub fn header_label(&self) -> String {
        match self
            .title
            .as_ref()
            .map(|title| title.trim())
            .filter(|title| !title.is_empty())
        {
            Some(title) => format!("{} · {title}", self.kind.label_upper()),
            None => self.kind.label_upper().to_string(),
        }
    }

    /// Ideal inner width (between vertical borders) from unwrapped content.
    pub fn ideal_inner_width(&self) -> usize {
        let header_width = self.header_label().width();
        let body_width = self
            .body
            .iter()
            .map(Block::ideal_content_width)
            .max()
            .unwrap_or(0);
        header_width.max(body_width).max(1)
    }

    /// Inner width capped to fit within `total_width` terminal columns.
    pub fn allocate_inner_width(&self, total_width: usize) -> usize {
        let available = total_width.saturating_sub(2).max(1);
        let ideal = self.ideal_inner_width();
        if ideal <= available { ideal } else { available }
    }

    /// Rendered callout frame width in terminal columns.
    pub fn frame_width(&self, total_width: usize) -> usize {
        let frame = self.allocate_inner_width(total_width).saturating_add(2);
        frame.min(total_width.max(3)).max(3)
    }
}
