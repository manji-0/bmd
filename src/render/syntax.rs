//! Syntect assets.

use syntect::{
    highlighting::{Theme as SyntectTheme, ThemeSet},
    parsing::SyntaxSet,
};

/// Shared syntect resources.
pub struct SyntaxAssets {
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
}

impl SyntaxAssets {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    pub fn theme(&self) -> &SyntectTheme {
        self.theme_set
            .themes
            .get("base16-ocean.dark")
            .unwrap_or_else(|| &self.theme_set.themes["InspiredGitHub"])
    }
}

impl Default for SyntaxAssets {
    fn default() -> Self {
        Self::new()
    }
}
