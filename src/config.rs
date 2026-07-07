//! User configuration loaded from `~/.config/bmd/config.toml`.
//!
//! Theme resolution: pick a named [`crate::render::DEFAULT_PRESET`] or `[theme] preset`,
//! then apply each `[theme.<role>]` section as field-level overrides on that preset.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;

use crate::error::AppError;
use crate::keymap::{Command, KeySpec, Keymap};
use crate::render::{DEFAULT_PRESET, Theme};

const CONFIG_RELATIVE: &str = ".config/bmd/config.toml";

/// Application configuration with optional theme and keymap overrides.
#[derive(Clone, Debug)]
pub struct Config {
    pub theme: Theme,
    pub keymap: Keymap,
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    theme: Option<ThemeSection>,
    keymap: Option<KeymapSection>,
}

#[derive(Debug, Default, Deserialize)]
struct ThemeSection {
    preset: Option<String>,
    text: Option<StyleSection>,
    h1: Option<StyleSection>,
    h1_prefix: Option<StyleSection>,
    h2: Option<StyleSection>,
    h2_prefix: Option<StyleSection>,
    h3: Option<StyleSection>,
    h3_prefix: Option<StyleSection>,
    h4: Option<StyleSection>,
    h4_prefix: Option<StyleSection>,
    h5: Option<StyleSection>,
    h5_prefix: Option<StyleSection>,
    h6: Option<StyleSection>,
    h6_prefix: Option<StyleSection>,
    code_inline: Option<StyleSection>,
    code_block: Option<StyleSection>,
    code_block_language: Option<StyleSection>,
    blockquote: Option<StyleSection>,
    list_marker: Option<StyleSection>,
    link: Option<StyleSection>,
    link_selected: Option<StyleSection>,
    image_link: Option<StyleSection>,
    image_link_selected: Option<StyleSection>,
    rule: Option<StyleSection>,
    table_header: Option<StyleSection>,
    table_cell: Option<StyleSection>,
    table_border: Option<StyleSection>,
    mermaid_placeholder: Option<StyleSection>,
    search_match: Option<StyleSection>,
    search_match_selected: Option<StyleSection>,
}

#[derive(Debug, Default, Deserialize)]
struct StyleSection {
    fg: Option<String>,
    bg: Option<String>,
    bold: Option<bool>,
    italic: Option<bool>,
    underlined: Option<bool>,
    dim: Option<bool>,
    reversed: Option<bool>,
    crossed_out: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct KeymapSection {
    normal: Option<HashMap<String, KeyBindingValue>>,
    preview: Option<HashMap<String, KeyBindingValue>>,
    search: Option<HashMap<String, KeyBindingValue>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum KeyBindingValue {
    One(String),
    Many(Vec<String>),
}

impl Config {
    /// Load configuration from the default path, falling back to built-in defaults.
    pub fn load() -> Result<Self, AppError> {
        let path = default_config_path();
        if let Some(path) = path
            && path.is_file()
        {
            return Self::load_from_path(&path);
        }
        Ok(Self::default())
    }

    /// Load configuration from an explicit path.
    pub fn load_from_path(path: &Path) -> Result<Self, AppError> {
        let raw = fs::read_to_string(path).map_err(AppError::Io)?;
        let file: ConfigFile = toml::from_str(&raw).map_err(|e| {
            AppError::UnsupportedInput(format!("invalid config {}: {e}", path.display()))
        })?;
        file.into_config()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: Theme::from_preset(DEFAULT_PRESET).expect("built-in default preset must exist"),
            keymap: Keymap::default(),
        }
    }
}

impl ConfigFile {
    fn into_config(self) -> Result<Config, AppError> {
        let mut config = Config::default();
        if let Some(theme) = self.theme {
            config.theme = theme.into_theme()?;
        }
        if let Some(keymap) = self.keymap {
            config.keymap = keymap.apply_to(Keymap::default())?;
        }
        Ok(config)
    }
}

impl ThemeSection {
    fn into_theme(self) -> Result<Theme, AppError> {
        let base = match self.preset.as_deref() {
            Some(name) => Theme::from_preset(name)?,
            None => Theme::from_preset(DEFAULT_PRESET)?,
        };
        self.apply_overrides(base)
    }

    fn apply_overrides(self, mut base: Theme) -> Result<Theme, AppError> {
        base.text = override_style(base.text, self.text)?;
        base.h1 = override_style(base.h1, self.h1)?;
        base.h1_prefix = override_style(base.h1_prefix, self.h1_prefix)?;
        base.h2 = override_style(base.h2, self.h2)?;
        base.h2_prefix = override_style(base.h2_prefix, self.h2_prefix)?;
        base.h3 = override_style(base.h3, self.h3)?;
        base.h3_prefix = override_style(base.h3_prefix, self.h3_prefix)?;
        base.h4 = override_style(base.h4, self.h4)?;
        base.h4_prefix = override_style(base.h4_prefix, self.h4_prefix)?;
        base.h5 = override_style(base.h5, self.h5)?;
        base.h5_prefix = override_style(base.h5_prefix, self.h5_prefix)?;
        base.h6 = override_style(base.h6, self.h6)?;
        base.h6_prefix = override_style(base.h6_prefix, self.h6_prefix)?;
        base.code_inline = override_style(base.code_inline, self.code_inline)?;
        base.code_block = override_style(base.code_block, self.code_block)?;
        base.code_block_language =
            override_style(base.code_block_language, self.code_block_language)?;
        base.blockquote = override_style(base.blockquote, self.blockquote)?;
        base.list_marker = override_style(base.list_marker, self.list_marker)?;
        base.link = override_style(base.link, self.link)?;
        base.link_selected = override_style(base.link_selected, self.link_selected)?;
        base.image_link = override_style(base.image_link, self.image_link)?;
        base.image_link_selected =
            override_style(base.image_link_selected, self.image_link_selected)?;
        base.rule = override_style(base.rule, self.rule)?;
        base.table_header = override_style(base.table_header, self.table_header)?;
        base.table_cell = override_style(base.table_cell, self.table_cell)?;
        base.table_border = override_style(base.table_border, self.table_border)?;
        base.mermaid_placeholder =
            override_style(base.mermaid_placeholder, self.mermaid_placeholder)?;
        base.search_match = override_style(base.search_match, self.search_match)?;
        base.search_match_selected =
            override_style(base.search_match_selected, self.search_match_selected)?;
        Ok(base)
    }
}

/// Apply config fields onto a preset style. Unset fields keep the preset value.
fn override_style(base: Style, section: Option<StyleSection>) -> Result<Style, AppError> {
    let Some(section) = section else {
        return Ok(base);
    };
    let mut style = base;
    if let Some(fg) = section.fg {
        style.fg = Some(parse_color(&fg)?);
    }
    if let Some(bg) = section.bg {
        style.bg = Some(parse_color(&bg)?);
    }
    if let Some(enabled) = section.bold {
        style.add_modifier = set_modifier(style.add_modifier, Modifier::BOLD, enabled);
    }
    if let Some(enabled) = section.italic {
        style.add_modifier = set_modifier(style.add_modifier, Modifier::ITALIC, enabled);
    }
    if let Some(enabled) = section.underlined {
        style.add_modifier = set_modifier(style.add_modifier, Modifier::UNDERLINED, enabled);
    }
    if let Some(enabled) = section.dim {
        style.add_modifier = set_modifier(style.add_modifier, Modifier::DIM, enabled);
    }
    if let Some(enabled) = section.reversed {
        style.add_modifier = set_modifier(style.add_modifier, Modifier::REVERSED, enabled);
    }
    if let Some(enabled) = section.crossed_out {
        style.add_modifier = set_modifier(style.add_modifier, Modifier::CROSSED_OUT, enabled);
    }
    Ok(style)
}

fn set_modifier(modifiers: Modifier, flag: Modifier, enabled: bool) -> Modifier {
    if enabled {
        modifiers | flag
    } else {
        modifiers - flag
    }
}

fn parse_color(name: &str) -> Result<Color, AppError> {
    let lower = name.to_ascii_lowercase();
    let color = match lower.as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        "white" => Color::White,
        "reset" => Color::Reset,
        hex if hex.starts_with('#') && hex.len() == 7 => {
            let r = u8::from_str_radix(&hex[1..3], 16).map_err(invalid_color)?;
            let g = u8::from_str_radix(&hex[3..5], 16).map_err(invalid_color)?;
            let b = u8::from_str_radix(&hex[5..7], 16).map_err(invalid_color)?;
            Color::Rgb(r, g, b)
        }
        other => {
            return Err(AppError::UnsupportedInput(format!(
                "unknown color '{other}'"
            )));
        }
    };
    Ok(color)
}

fn invalid_color<E: std::fmt::Display>(err: E) -> AppError {
    AppError::UnsupportedInput(format!("invalid color hex: {err}"))
}

impl KeymapSection {
    fn apply_to(self, base: Keymap) -> Result<Keymap, AppError> {
        let mut keymap = base;
        if let Some(normal) = self.normal {
            keymap.apply_overrides(Keymap::MODE_NORMAL, normal)?;
        }
        if let Some(preview) = self.preview {
            keymap.apply_overrides(Keymap::MODE_PREVIEW, preview)?;
        }
        if let Some(search) = self.search {
            keymap.apply_overrides(Keymap::MODE_SEARCH, search)?;
        }
        Ok(keymap)
    }
}

impl KeyBindingValue {
    pub(crate) fn into_specs(self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
    }
}

/// Default config file path: `$XDG_CONFIG_HOME/bmd/config.toml` or `~/.config/bmd/config.toml`.
pub fn default_config_path() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(dir).join("bmd/config.toml"));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(CONFIG_RELATIVE))
}

/// Map a config command name to the runtime [`Command`].
pub fn command_from_name(name: &str) -> Option<Command> {
    match name {
        "scroll_down" => Some(Command::ScrollDown),
        "scroll_up" => Some(Command::ScrollUp),
        "half_page_down" => Some(Command::HalfPageDown),
        "half_page_up" => Some(Command::HalfPageUp),
        "jump_to_top" => Some(Command::JumpToTop),
        "jump_to_bottom" => Some(Command::JumpToBottom),
        "next_link" => Some(Command::NextLink),
        "prev_link" => Some(Command::PrevLink),
        "next_heading" => Some(Command::NextHeading),
        "prev_heading" => Some(Command::PrevHeading),
        "open_link" => Some(Command::OpenLink),
        "nav_back" => Some(Command::NavBack),
        "start_search_forward" => Some(Command::StartSearchForward),
        "start_search_backward" => Some(Command::StartSearchBackward),
        "toggle_help" => Some(Command::ToggleHelp),
        "close_help" => Some(Command::CloseHelp),
        "toggle_checklist" => Some(Command::ToggleChecklist),
        "copy_selection" => Some(Command::CopySelection),
        "clear_selection" => Some(Command::ClearSelection),
        "quit" => Some(Command::Quit),
        "close_preview" => Some(Command::ClosePreview),
        "preview_zoom_in" => Some(Command::PreviewZoomIn),
        "preview_zoom_out" => Some(Command::PreviewZoomOut),
        "preview_zoom_reset" => Some(Command::PreviewZoomReset),
        "search_confirm" => Some(Command::SearchConfirm),
        "search_cancel" => Some(Command::SearchCancel),
        "search_backspace" => Some(Command::SearchBackspace),
        _ => None,
    }
}

/// Parse a list of key binding strings from config.
pub fn parse_binding_specs(values: Vec<String>) -> Result<Vec<KeySpec>, AppError> {
    values.into_iter().map(|v| KeySpec::parse(&v)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn default_config_uses_builtin_defaults() {
        let config = Config::default();
        assert_eq!(
            config.keymap.normal_command(&test_key(KeyCode::Char('p'))),
            Command::PrevLink
        );
    }

    #[test]
    fn theme_preset_selects_base_palette() {
        let toml = r#"
[theme]
preset = "nord"
"#;
        let file: ConfigFile = toml::from_str(toml).unwrap();
        let config = file.into_config().unwrap();
        assert_eq!(config.theme, Theme::from_preset("nord").unwrap());
    }

    #[test]
    fn theme_preset_with_override_replaces_specified_fields() {
        let toml = r#"
[theme]
preset = "dracula"

[theme.link]
fg = "cyan"
"#;
        let file: ConfigFile = toml::from_str(toml).unwrap();
        let config = file.into_config().unwrap();
        let preset = Theme::from_preset("dracula").unwrap();
        assert_eq!(config.theme.link.fg, Some(Color::Cyan));
        assert_eq!(config.theme.link.bg, preset.link.bg);
        assert_eq!(config.theme.link.add_modifier, preset.link.add_modifier);
        assert_eq!(config.theme.text, preset.text);
    }

    #[test]
    fn theme_override_can_clear_preset_modifier() {
        let toml = r#"
[theme]
preset = "dark"

[theme.link]
underlined = false
"#;
        let file: ConfigFile = toml::from_str(toml).unwrap();
        let config = file.into_config().unwrap();
        assert!(
            !config
                .theme
                .link
                .add_modifier
                .contains(Modifier::UNDERLINED)
        );
    }

    #[test]
    fn theme_override_without_preset_uses_default_preset_as_base() {
        let toml = r#"
[theme.link]
fg = "cyan"
underlined = true
"#;
        let file: ConfigFile = toml::from_str(toml).unwrap();
        let config = file.into_config().unwrap();
        let preset = Theme::from_preset(DEFAULT_PRESET).unwrap();
        assert_eq!(config.theme.link.fg, Some(Color::Cyan));
        assert_eq!(config.theme.link.bg, preset.link.bg);
        assert!(
            config
                .theme
                .link
                .add_modifier
                .contains(Modifier::UNDERLINED)
        );
    }

    #[test]
    fn keymap_override_replaces_command_bindings() {
        let toml = r#"
[keymap.normal]
scroll_down = ["e"]
"#;
        let file: ConfigFile = toml::from_str(toml).unwrap();
        let config = file.into_config().unwrap();
        assert_eq!(
            config.keymap.normal_command(&test_key(KeyCode::Char('e'))),
            Command::ScrollDown
        );
        assert_eq!(
            config.keymap.normal_command(&test_key(KeyCode::Char('j'))),
            Command::None
        );
    }

    fn test_key(code: KeyCode) -> crossterm::event::KeyEvent {
        crossterm::event::KeyEvent::new(code, KeyModifiers::empty())
    }
}
