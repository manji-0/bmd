//! Link value objects and validation errors.

use std::fmt;

/// Opaque identifier for a link stored in `Document.links`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinkId(pub usize);

impl fmt::Display for LinkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    Web,
    Anchor,
    Document,
    Image,
    Mermaid,
}

impl LinkKind {
    pub fn is_preview(self) -> bool {
        matches!(self, Self::Image | Self::Mermaid)
    }

    /// Classify a markdown link destination (not image URLs).
    pub fn for_link_dest(dest: &str) -> Self {
        if dest.starts_with('#') {
            Self::Anchor
        } else if super::document_link::is_remote_link_dest(dest) {
            Self::Web
        } else {
            Self::Document
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub url: LinkUrl,
    pub title: Option<String>,
    pub kind: LinkKind,
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

    #[error("mermaid link {link_id} references missing diagram")]
    InvalidMermaidLink { link_id: LinkId },
}
