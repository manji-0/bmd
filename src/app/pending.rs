//! Two-key pending input for marks and yank.

/// Operator waiting for a second keystroke in normal mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PendingInput {
    #[default]
    None,
    /// After `m` — next `a`–`z` sets a mark.
    SetMark,
    /// After `'` — next `a`–`z` jumps to a mark.
    JumpMark,
    /// After `y` with no selection — next `l`/`h`/`c`/`y` yanks.
    Yank,
}

impl PendingInput {
    pub(crate) const fn is_active(self) -> bool {
        !matches!(self, Self::None)
    }

    pub(crate) const fn prompt(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::SetMark => Some("m — mark a-z"),
            Self::JumpMark => Some("' — jump a-z"),
            Self::Yank => Some("yank: l link  h heading  c code  y selection"),
        }
    }
}
