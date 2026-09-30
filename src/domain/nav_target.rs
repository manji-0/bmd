//! In-document navigation targets (links; footnotes via click/selection only).

use super::{FootnoteId, LinkId};

/// A selectable in-document navigation target.
///
/// Keyboard `n`/`N` cycling uses links only. Footnotes remain a `NavTarget` so
/// click selection and status labeling can still identify them.
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
