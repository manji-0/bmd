//! Runtime checklist toggle state (not persisted to disk).

use std::collections::HashMap;

use unicode_width::UnicodeWidthStr;

use super::markdown::ListItem;

/// Stable id assigned at parse time for each task-list item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChecklistId(pub u32);

/// Visual style for task-list markers.
///
/// Selection order:
/// 1. `BMD_CHECKLIST_STYLE=unicode` or `emoji` — explicit override.
/// 2. `BMD_CHECKLIST_STYLE=auto` or unset — use [`ChecklistStyle::detect`].
/// 3. Any other value — fall back to Unicode box glyphs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChecklistStyle {
    /// U+2610 BALLOT BOX / U+2611 BALLOT BOX WITH CHECK.
    Unicode,
    /// Emoji pair for terminals known to render color emoji reliably.
    Emoji,
}

impl ChecklistStyle {
    pub fn from_env() -> Self {
        match std::env::var("BMD_CHECKLIST_STYLE") {
            Ok(value) if value.eq_ignore_ascii_case("emoji") => Self::Emoji,
            Ok(value) if value.eq_ignore_ascii_case("unicode") => Self::Unicode,
            Ok(value) if value.eq_ignore_ascii_case("auto") => Self::detect(),
            Ok(_) => Self::Unicode,
            Err(_) => Self::detect(),
        }
    }

    /// Conservative auto-detection: emoji only when the host terminal is identifiable.
    pub fn detect() -> Self {
        if terminal_likely_supports_emoji() {
            Self::Emoji
        } else {
            Self::Unicode
        }
    }

    pub fn unchecked_marker(self) -> &'static str {
        match self {
            Self::Unicode => "\u{2610} ",
            Self::Emoji => "⬜ ",
        }
    }

    pub fn checked_marker(self) -> &'static str {
        match self {
            Self::Unicode => "\u{2611} ",
            Self::Emoji => "✅ ",
        }
    }

    pub fn marker_width(self) -> usize {
        self.unchecked_marker()
            .width()
            .max(self.checked_marker().width())
    }
}

/// Returns true when common terminal metadata indicates color-emoji support.
fn terminal_likely_supports_emoji() -> bool {
    if std::env::var("KITTY_WINDOW_ID").is_ok() {
        return true;
    }
    let Ok(term_program) = std::env::var("TERM_PROGRAM") else {
        return false;
    };
    matches!(
        term_program.as_str(),
        "Apple_Terminal" | "iTerm.app" | "WezTerm" | "vscode" | "ghostty" | "kitty"
    )
}

/// In-memory overrides for checklist items toggled during the session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChecklistState {
    style: ChecklistStyle,
    overrides: HashMap<ChecklistId, bool>,
    revision: u64,
}

impl ChecklistState {
    pub fn new(style: ChecklistStyle) -> Self {
        Self {
            style,
            overrides: HashMap::new(),
            revision: 0,
        }
    }

    pub fn style(&self) -> ChecklistStyle {
        self.style
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn checked(&self, item: &ListItem) -> bool {
        let Some(id) = item.checklist_id else {
            return false;
        };
        self.overrides.get(&id).copied().unwrap_or(item.checked)
    }

    pub fn toggle(&mut self, item: &ListItem) -> bool {
        let Some(id) = item.checklist_id else {
            return false;
        };
        let next = !self.checked(item);
        self.overrides.insert(id, next);
        self.revision = self.revision.wrapping_add(1);
        next
    }
}

impl Default for ChecklistState {
    fn default() -> Self {
        Self::new(ChecklistStyle::Unicode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_markers_are_two_columns_wide() {
        let style = ChecklistStyle::Unicode;
        assert_eq!(style.unchecked_marker(), "☐ ");
        assert_eq!(style.checked_marker(), "☑ ");
        assert_eq!(style.marker_width(), 2);
    }

    #[test]
    fn emoji_markers_use_display_width() {
        let style = ChecklistStyle::Emoji;
        assert!(style.marker_width() >= 2);
    }
}
