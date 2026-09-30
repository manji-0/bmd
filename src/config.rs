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
use crate::keymap::{KeyBindingValue, Keymap};
use crate::render::{DEFAULT_PRESET, Theme};

const CONFIG_RELATIVE: &str = ".config/bmd/config.toml";

/// Application configuration with optional theme, keymap, and view overrides.
#[derive(Clone, Debug)]
pub struct Config {
    pub theme: Theme,
    pub keymap: Keymap,
    pub view: ViewConfig,
}

/// `[view]` settings for how documents are laid out and marked up.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewConfig {
    /// Task-list marker style; `None` leaves it to `BMD_CHECKLIST_STYLE` or detection.
    pub checklist: Option<ChecklistMarkers>,
}

/// Task-list marker choice from `[view] checklist`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChecklistMarkers {
    /// Detect from the terminal (same as leaving the setting out).
    Auto,
    /// `☐` / `☑`.
    Unicode,
    /// `⬜` / `✅`.
    Emoji,
    /// `[ ]` / `[x]`.
    Ascii,
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    theme: Option<ThemeSection>,
    keymap: Option<KeymapSection>,
    view: Option<ViewSection>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewSection {
    checklist: Option<ChecklistMarkers>,
}

#[derive(Debug, Default, Deserialize)]
struct ThemeSection {
    preset: Option<String>,
    /// `[theme.<role>]` overrides keyed by [`Theme::role_mut`] names.
    #[serde(flatten)]
    roles: HashMap<String, StyleSection>,
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
            view: ViewConfig::default(),
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
        if let Some(view) = self.view {
            config.view = ViewConfig {
                checklist: view.checklist.filter(|m| *m != ChecklistMarkers::Auto),
            };
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
        for (role, section) in self.roles {
            let style = base.role_mut(&role).ok_or_else(|| {
                AppError::UnsupportedInput(format!("unknown theme role '{role}'"))
            })?;
            *style = override_style(*style, section)?;
        }
        Ok(base)
    }
}

/// Apply config fields onto a preset style. Unset fields keep the preset value.
fn override_style(base: Style, section: StyleSection) -> Result<Style, AppError> {
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
    name.parse()
        .map_err(|_| AppError::UnsupportedInput(format!("unknown color '{name}'")))
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

/// Default config file path: `$XDG_CONFIG_HOME/bmd/config.toml` or `~/.config/bmd/config.toml`.
pub fn default_config_path() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(dir).join("bmd/config.toml"));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(CONFIG_RELATIVE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::Command;
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn default_config_uses_builtin_defaults() {
        let config = Config::default();
        assert_eq!(
            config.keymap.normal_command(&test_key(KeyCode::Char('N'))),
            Command::PrevLink
        );
        assert_eq!(
            config.keymap.normal_command(&test_key(KeyCode::Char('p'))),
            Command::None
        );
    }

    #[test]
    fn theme_preset_selects_base_palette() {
        let toml = r#"
[theme]
preset = "dark"
"#;
        let file: ConfigFile = toml::from_str(toml).unwrap();
        let config = file.into_config().unwrap();
        assert_eq!(config.theme, Theme::from_preset("dark").unwrap());
    }

    #[test]
    fn theme_preset_with_override_replaces_specified_fields() {
        let toml = r#"
[theme]
preset = "light"

[theme.link]
fg = "cyan"
"#;
        let file: ConfigFile = toml::from_str(toml).unwrap();
        let config = file.into_config().unwrap();
        let preset = Theme::from_preset("light").unwrap();
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

    #[test]
    fn theme_override_rejects_unknown_role_and_color() {
        for toml in ["[theme.nope]\nfg = \"red\"", "[theme.link]\nfg = \"#12\""] {
            let file: ConfigFile = toml::from_str(toml).unwrap();
            assert!(file.into_config().is_err(), "{toml}");
        }
        let file: ConfigFile = toml::from_str("[theme.math]\nfg = \"#0a0b0c\"").unwrap();
        let theme = file.into_config().unwrap().theme;
        assert_eq!(theme.math.fg, Some(Color::Rgb(10, 11, 12)));
    }

    fn test_key(code: KeyCode) -> crossterm::event::KeyEvent {
        crossterm::event::KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn view_checklist_parses_marker_names() {
        let file: ConfigFile = toml::from_str("[view]\nchecklist = \"ascii\"\n").unwrap();
        let config = file.into_config().unwrap();
        assert_eq!(config.view.checklist, Some(ChecklistMarkers::Ascii));

        let file: ConfigFile = toml::from_str("[view]\nchecklist = \"auto\"\n").unwrap();
        assert_eq!(file.into_config().unwrap().view.checklist, None);

        assert!(toml::from_str::<ConfigFile>("[view]\nchecklist = \"boxes\"\n").is_err());
        assert!(toml::from_str::<ConfigFile>("[view]\nunknown = 1\n").is_err());
    }
}
