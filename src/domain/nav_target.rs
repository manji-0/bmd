//! In-document navigation targets (links and footnote references).

use super::{FootnoteId, LinkId};

/// A selectable in-document navigation target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NavTarget {
    Link(LinkId),
    Footnote(FootnoteId),
}

impl NavTarget {
    pub fn link_id(self) -> Option<LinkId> {
        match self {
            Self::Link(id) => Some(id),
            Self::Footnote(_) => None,
        }
    }

    pub fn footnote_id(self) -> Option<FootnoteId> {
        match self {
            Self::Link(_) => None,
            Self::Footnote(id) => Some(id),
        }
    }
}
