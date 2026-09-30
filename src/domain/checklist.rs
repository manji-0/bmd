//! Runtime checklist toggle state (not persisted to disk).

use std::collections::HashMap;

use unicode_width::UnicodeWidthStr;

use super::markdown::ListItem;

/// Stable id assigned at parse time for each task-list item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChecklistId(pub u32);

/// Visual style for task-list markers.
///
/// Selection order (see [`ChecklistStyle::resolve`]):
/// 1. `BMD_CHECKLIST_STYLE=unicode`, `emoji`, `ascii`, or `auto` — explicit override.
///    Any other value falls back to Unicode box glyphs.
/// 2. The `[view] checklist` config value, when set.
/// 3. Otherwise [`ChecklistStyle::detect`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChecklistStyle {
    /// U+2610 BALLOT BOX / U+2611 BALLOT BOX WITH CHECK.
    Unicode,
    /// Emoji pair for terminals known to render color emoji reliably.
    Emoji,
    /// `[ ]` / `[x]`, for fonts or terminals without reliable box glyphs.
    Ascii,
}

impl ChecklistStyle {
    /// Parse a style name (`unicode`, `emoji`, `ascii`, or `auto`), case-insensitively.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "unicode" => Some(Self::Unicode),
            "emoji" => Some(Self::Emoji),
            "ascii" => Some(Self::Ascii),
            "auto" => Some(Self::detect()),
            _ => None,
        }
    }

    /// Style from `BMD_CHECKLIST_STYLE`, else `configured`, else auto-detection.
    pub fn resolve(configured: Option<Self>) -> Self {
        match std::env::var("BMD_CHECKLIST_STYLE") {
            Ok(value) => Self::from_name(&value).unwrap_or(Self::Unicode),
            Err(_) => configured.unwrap_or_else(Self::detect),
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
            Self::Ascii => "[ ] ",
        }
    }

    pub fn checked_marker(self) -> &'static str {
        match self {
            Self::Unicode => "\u{2611} ",
            Self::Emoji => "✅ ",
            Self::Ascii => "[x] ",
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
