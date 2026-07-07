//! GFM / Obsidian-style alert callouts.

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
}
