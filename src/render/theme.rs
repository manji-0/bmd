//! Visual theme and built-in color presets.

use ratatui::style::{Color, Modifier, Style};

use crate::error::AppError;

/// Canonical name of the preset used when config does not select one.
pub const DEFAULT_PRESET: &str = "cursor-midnight";

/// Names of built-in theme presets (stable identifiers for config).
pub const PRESET_NAMES: &[&str] = &[
    "dark",
    "light",
    "solarized-dark",
    "solarized-light",
    "nord",
    "gruvbox-dark",
    "dracula",
    "tokyo-night",
    "hackerman-omarchy",
    "cursor-midnight",
];

/// Visual theme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub text: Style,
    pub h1: Style,
    pub h1_prefix: Style,
    pub h2: Style,
    pub h2_prefix: Style,
    pub h3: Style,
    pub h3_prefix: Style,
    pub h4: Style,
    pub h4_prefix: Style,
    pub h5: Style,
    pub h5_prefix: Style,
    pub h6: Style,
    pub h6_prefix: Style,
    pub code_inline: Style,
    pub code_block: Style,
    pub code_block_language: Style,
    pub blockquote: Style,
    pub list_marker: Style,
    pub link: Style,
    pub link_selected: Style,
    pub image_link: Style,
    pub image_link_selected: Style,
    pub rule: Style,
    pub table_header: Style,
    pub table_cell: Style,
    pub table_border: Style,
    pub mermaid_placeholder: Style,
    pub math: Style,
    pub search_match: Style,
    pub search_match_selected: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self::from_preset(DEFAULT_PRESET).expect("built-in default preset must exist")
    }
}

impl Theme {
    /// Resolve a preset name to a full theme.
    pub fn from_preset(name: &str) -> Result<Self, AppError> {
        let Some(canonical) = canonical_preset_name(name) else {
            return Err(AppError::UnsupportedInput(format!(
                "unknown theme preset '{name}' (available: {})",
                PRESET_NAMES.join(", ")
            )));
        };
        Ok(builtin_preset(canonical).build())
    }
}

fn canonical_preset_name(name: &str) -> Option<&'static str> {
    match name.trim().to_ascii_lowercase().as_str() {
        "dark" => Some("dark"),
        "default" => Some(DEFAULT_PRESET),
        "light" => Some("light"),
        "solarized-dark" | "solarized_dark" => Some("solarized-dark"),
        "solarized-light" | "solarized_light" => Some("solarized-light"),
        "nord" => Some("nord"),
        "gruvbox-dark" | "gruvbox_dark" | "gruvbox" => Some("gruvbox-dark"),
        "dracula" => Some("dracula"),
        "tokyo-night" | "tokyo_night" | "tokyonight" => Some("tokyo-night"),
        "hackerman-omarchy" | "hackerman_omarchy" | "hackerman" | "omarchy-hackerman" => {
            Some("hackerman-omarchy")
        }
        "cursor-midnight" | "cursor_midnight" | "cursor-dark-midnight" => Some("cursor-midnight"),
        _ => None,
    }
}

fn builtin_preset(name: &str) -> Palette {
    match name {
        "dark" => Palette {
            text: Color::White,
            heading: Color::White,
            heading_prefix: Color::Yellow,
            heading_muted: Color::Gray,
            heading_faint: Color::DarkGray,
            code_fg: Color::Yellow,
            code_bg: Color::Black,
            code_label_fg: Color::Black,
            code_label_bg: Color::Yellow,
            blockquote: Color::Gray,
            list_marker: Color::Cyan,
            link: Color::Blue,
            link_selected_fg: Color::Black,
            link_selected_bg: Color::Blue,
            image_link: Color::Magenta,
            image_selected_fg: Color::Black,
            image_selected_bg: Color::Magenta,
            search_match_fg: Color::Black,
            search_match_bg: Color::Yellow,
            search_selected_fg: Color::White,
            search_selected_bg: Color::Magenta,
            rule: Color::DarkGray,
            table_header: Color::White,
            table_border: Color::DarkGray,
            mermaid: Color::Yellow,
        },
        "light" => Palette {
            text: rgb(0x1e, 0x1e, 0x1e),
            heading: rgb(0x11, 0x11, 0x11),
            heading_prefix: rgb(0x5c, 0x5c, 0x5c),
            heading_muted: rgb(0x4a, 0x4a, 0x4a),
            heading_faint: rgb(0x6e, 0x6e, 0x6e),
            code_fg: rgb(0x0f, 0x4c, 0x81),
            code_bg: rgb(0xef, 0xef, 0xef),
            code_label_fg: rgb(0xef, 0xef, 0xef),
            code_label_bg: rgb(0x5c, 0x5c, 0x5c),
            blockquote: rgb(0x5c, 0x5c, 0x5c),
            list_marker: rgb(0x0e, 0x63, 0x8c),
            link: rgb(0x05, 0x63, 0xc1),
            link_selected_fg: Color::White,
            link_selected_bg: rgb(0x05, 0x63, 0xc1),
            image_link: rgb(0x6f, 0x42, 0xc1),
            image_selected_fg: Color::White,
            image_selected_bg: rgb(0x6f, 0x42, 0xc1),
            search_match_fg: rgb(0x1e, 0x1e, 0x1e),
            search_match_bg: rgb(0xff, 0xe0, 0x66),
            search_selected_fg: Color::White,
            search_selected_bg: rgb(0xc7, 0x00, 0x39),
            rule: rgb(0xb0, 0xb0, 0xb0),
            table_header: rgb(0x11, 0x11, 0x11),
            table_border: rgb(0xc0, 0xc0, 0xc0),
            mermaid: rgb(0x0e, 0x63, 0x8c),
        },
        "solarized-dark" => Palette {
            text: rgb(0x83, 0x94, 0x96),
            heading: rgb(0x93, 0xa1, 0xa1),
            heading_prefix: rgb(0xb5, 0x89, 0x00),
            heading_muted: rgb(0x65, 0x7b, 0x83),
            heading_faint: rgb(0x58, 0x6e, 0x75),
            code_fg: rgb(0x2a, 0xa1, 0x98),
            code_bg: rgb(0x00, 0x2b, 0x36),
            code_label_fg: rgb(0x00, 0x2b, 0x36),
            code_label_bg: rgb(0xb5, 0x89, 0x00),
            blockquote: rgb(0x65, 0x7b, 0x83),
            list_marker: rgb(0x2a, 0xa1, 0x98),
            link: rgb(0x26, 0x8b, 0xd2),
            link_selected_fg: rgb(0x00, 0x2b, 0x36),
            link_selected_bg: rgb(0x26, 0x8b, 0xd2),
            image_link: rgb(0xd3, 0x36, 0x82),
            image_selected_fg: rgb(0x00, 0x2b, 0x36),
            image_selected_bg: rgb(0xd3, 0x36, 0x82),
            search_match_fg: rgb(0x00, 0x2b, 0x36),
            search_match_bg: rgb(0xb5, 0x89, 0x00),
            search_selected_fg: rgb(0xfd, 0xf6, 0xe3),
            search_selected_bg: rgb(0x6c, 0x71, 0xc4),
            rule: rgb(0x58, 0x6e, 0x75),
            table_header: rgb(0x93, 0xa1, 0xa1),
            table_border: rgb(0x58, 0x6e, 0x75),
            mermaid: rgb(0xb5, 0x89, 0x00),
        },
        "solarized-light" => Palette {
            text: rgb(0x65, 0x7b, 0x83),
            heading: rgb(0x58, 0x6e, 0x75),
            heading_prefix: rgb(0xcb, 0x4b, 0x16),
            heading_muted: rgb(0x83, 0x94, 0x96),
            heading_faint: rgb(0x93, 0xa1, 0xa1),
            code_fg: rgb(0x07, 0x89, 0x6f),
            code_bg: rgb(0xee, 0xe8, 0xd5),
            code_label_fg: rgb(0xfd, 0xf6, 0xe3),
            code_label_bg: rgb(0xcb, 0x4b, 0x16),
            blockquote: rgb(0x83, 0x94, 0x96),
            list_marker: rgb(0x07, 0x89, 0x6f),
            link: rgb(0x26, 0x8b, 0xd2),
            link_selected_fg: rgb(0xfd, 0xf6, 0xe3),
            link_selected_bg: rgb(0x26, 0x8b, 0xd2),
            image_link: rgb(0xd3, 0x36, 0x82),
            image_selected_fg: rgb(0xfd, 0xf6, 0xe3),
            image_selected_bg: rgb(0xd3, 0x36, 0x82),
            search_match_fg: rgb(0xfd, 0xf6, 0xe3),
            search_match_bg: rgb(0xcb, 0x4b, 0x16),
            search_selected_fg: rgb(0xfd, 0xf6, 0xe3),
            search_selected_bg: rgb(0x6c, 0x71, 0xc4),
            rule: rgb(0x93, 0xa1, 0xa1),
            table_header: rgb(0x58, 0x6e, 0x75),
            table_border: rgb(0x93, 0xa1, 0xa1),
            mermaid: rgb(0xcb, 0x4b, 0x16),
        },
        "nord" => Palette {
            text: rgb(0xd8, 0xde, 0xe9),
            heading: rgb(0xe5, 0xe9, 0xf0),
            heading_prefix: rgb(0x88, 0xc0, 0xd0),
            heading_muted: rgb(0xa3, 0xbe, 0x8c),
            heading_faint: rgb(0x81, 0xa1, 0xc1),
            code_fg: rgb(0x8f, 0xbc, 0xbb),
            code_bg: rgb(0x2e, 0x34, 0x40),
            code_label_fg: rgb(0x2e, 0x34, 0x40),
            code_label_bg: rgb(0x5e, 0x81, 0xac),
            blockquote: rgb(0x81, 0xa1, 0xc1),
            list_marker: rgb(0x88, 0xc0, 0xd0),
            link: rgb(0x81, 0xa1, 0xc1),
            link_selected_fg: rgb(0x2e, 0x34, 0x40),
            link_selected_bg: rgb(0x5e, 0x81, 0xac),
            image_link: rgb(0xb4, 0x8e, 0xad),
            image_selected_fg: rgb(0x2e, 0x34, 0x40),
            image_selected_bg: rgb(0xb4, 0x8e, 0xad),
            search_match_fg: rgb(0x2e, 0x34, 0x40),
            search_match_bg: rgb(0xeb, 0xcb, 0x8b),
            search_selected_fg: rgb(0x2e, 0x34, 0x40),
            search_selected_bg: rgb(0xbf, 0x61, 0x6a),
            rule: rgb(0x4c, 0x56, 0x6a),
            table_header: rgb(0xe5, 0xe9, 0xf0),
            table_border: rgb(0x4c, 0x56, 0x6a),
            mermaid: rgb(0x88, 0xc0, 0xd0),
        },
        "gruvbox-dark" => Palette {
            text: rgb(0xeb, 0xdb, 0xb2),
            heading: rgb(0xfb, 0xf1, 0xc7),
            heading_prefix: rgb(0xfe, 0x80, 0x19),
            heading_muted: rgb(0xd5, 0xc4, 0xa1),
            heading_faint: rgb(0xa8, 0x99, 0x84),
            code_fg: rgb(0x8e, 0xc0, 0x7c),
            code_bg: rgb(0x28, 0x28, 0x28),
            code_label_fg: rgb(0x28, 0x28, 0x28),
            code_label_bg: rgb(0xd7, 0x99, 0x21),
            blockquote: rgb(0xa8, 0x99, 0x84),
            list_marker: rgb(0x83, 0xa5, 0x98),
            link: rgb(0x83, 0xa5, 0x98),
            link_selected_fg: rgb(0x28, 0x28, 0x28),
            link_selected_bg: rgb(0x83, 0xa5, 0x98),
            image_link: rgb(0xd3, 0x86, 0x9b),
            image_selected_fg: rgb(0x28, 0x28, 0x28),
            image_selected_bg: rgb(0xd3, 0x86, 0x9b),
            search_match_fg: rgb(0x28, 0x28, 0x28),
            search_match_bg: rgb(0xfa, 0xbd, 0x2f),
            search_selected_fg: rgb(0x28, 0x28, 0x28),
            search_selected_bg: rgb(0xfb, 0x49, 0x34),
            rule: rgb(0x50, 0x50, 0x50),
            table_header: rgb(0xfb, 0xf1, 0xc7),
            table_border: rgb(0x50, 0x50, 0x50),
            mermaid: rgb(0xfe, 0x80, 0x19),
        },
        "dracula" => Palette {
            text: rgb(0xf8, 0xf8, 0xf2),
            heading: rgb(0xff, 0xff, 0xff),
            heading_prefix: rgb(0xff, 0x79, 0xc6),
            heading_muted: rgb(0xbd, 0x93, 0xf9),
            heading_faint: rgb(0x62, 0x72, 0xa4),
            code_fg: rgb(0x50, 0xfa, 0x7b),
            code_bg: rgb(0x21, 0x22, 0x2c),
            code_label_fg: rgb(0x28, 0x2a, 0x36),
            code_label_bg: rgb(0xf1, 0xfa, 0x8c),
            blockquote: rgb(0x62, 0x72, 0xa4),
            list_marker: rgb(0x8b, 0xe9, 0xfd),
            link: rgb(0x8b, 0xe9, 0xfd),
            link_selected_fg: rgb(0x28, 0x2a, 0x36),
            link_selected_bg: rgb(0x8b, 0xe9, 0xfd),
            image_link: rgb(0xff, 0x79, 0xc6),
            image_selected_fg: rgb(0x28, 0x2a, 0x36),
            image_selected_bg: rgb(0xff, 0x79, 0xc6),
            search_match_fg: rgb(0x28, 0x2a, 0x36),
            search_match_bg: rgb(0xf1, 0xfa, 0x8c),
            search_selected_fg: rgb(0xf8, 0xf8, 0xf2),
            search_selected_bg: rgb(0xff, 0x55, 0x55),
            rule: rgb(0x44, 0x47, 0x5a),
            table_header: rgb(0xff, 0xff, 0xff),
            table_border: rgb(0x44, 0x47, 0x5a),
            mermaid: rgb(0xbd, 0x93, 0xf9),
        },
        "tokyo-night" => Palette {
            text: rgb(0xc0, 0xca, 0xf5),
            heading: rgb(0xd9, 0xe2, 0xff),
            heading_prefix: rgb(0x7a, 0xa2, 0xf7),
            heading_muted: rgb(0x9a, 0xa5, 0xce),
            heading_faint: rgb(0x56, 0x5f, 0x89),
            code_fg: rgb(0x9e, 0xce, 0x6a),
            code_bg: rgb(0x1a, 0x1b, 0x26),
            code_label_fg: rgb(0x1a, 0x1b, 0x26),
            code_label_bg: rgb(0xe0, 0xaf, 0x68),
            blockquote: rgb(0x56, 0x5f, 0x89),
            list_marker: rgb(0x7d, 0xcf, 0xff),
            link: rgb(0x7a, 0xa2, 0xf7),
            link_selected_fg: rgb(0x1a, 0x1b, 0x26),
            link_selected_bg: rgb(0x7a, 0xa2, 0xf7),
            image_link: rgb(0xbb, 0x9a, 0xf7),
            image_selected_fg: rgb(0x1a, 0x1b, 0x26),
            image_selected_bg: rgb(0xbb, 0x9a, 0xf7),
            search_match_fg: rgb(0x1a, 0x1b, 0x26),
            search_match_bg: rgb(0xe0, 0xaf, 0x68),
            search_selected_fg: rgb(0x1a, 0x1b, 0x26),
            search_selected_bg: rgb(0xf7, 0x76, 0x8e),
            rule: rgb(0x3b, 0x40, 0x60),
            table_header: rgb(0xd9, 0xe2, 0xff),
            table_border: rgb(0x3b, 0x40, 0x60),
            mermaid: rgb(0x7d, 0xcf, 0xff),
        },
        // Omarchy Hackerman — colors from basecamp/omarchy themes/hackerman/colors.toml
        "hackerman-omarchy" => Palette {
            text: rgb(0xdd, 0xf7, 0xff),
            heading: rgb(0xdd, 0xf7, 0xff),
            heading_prefix: rgb(0x82, 0xfb, 0x9c),
            heading_muted: rgb(0x85, 0xe1, 0xfb),
            heading_faint: rgb(0x6a, 0x6e, 0x95),
            code_fg: rgb(0x7c, 0xf8, 0xf7),
            code_bg: rgb(0x0b, 0x0c, 0x16),
            code_label_fg: rgb(0x0b, 0x0c, 0x16),
            code_label_bg: rgb(0x82, 0xfb, 0x9c),
            blockquote: rgb(0x6a, 0x6e, 0x95),
            list_marker: rgb(0x50, 0xf7, 0xd4),
            link: rgb(0x82, 0x9d, 0xd4),
            link_selected_fg: rgb(0x0b, 0x0c, 0x16),
            link_selected_bg: rgb(0x82, 0x9d, 0xd4),
            image_link: rgb(0x86, 0xa7, 0xdf),
            image_selected_fg: rgb(0x0b, 0x0c, 0x16),
            image_selected_bg: rgb(0x86, 0xa7, 0xdf),
            search_match_fg: rgb(0x0b, 0x0c, 0x16),
            search_match_bg: rgb(0x50, 0xf8, 0x72),
            search_selected_fg: rgb(0x0b, 0x0c, 0x16),
            search_selected_bg: rgb(0x50, 0xf7, 0xd4),
            rule: rgb(0x3e, 0x40, 0x58),
            table_header: rgb(0xdd, 0xf7, 0xff),
            table_border: rgb(0x3e, 0x40, 0x58),
            mermaid: rgb(0x82, 0xfb, 0x9c),
        },
        // Cursor Dark Midnight — from Cursor IDE theme-cursor (VS Code JSON sources)
        "cursor-midnight" => Palette {
            text: rgb(0xd8, 0xde, 0xe9),
            heading: rgb(0x88, 0xc0, 0xd0),
            heading_prefix: rgb(0x81, 0xa1, 0xc1),
            heading_muted: rgb(0x8f, 0xbc, 0xbb),
            heading_faint: rgb(0x7b, 0x88, 0xa1),
            code_fg: rgb(0x8f, 0xbc, 0xbb),
            code_bg: rgb(0x1e, 0x21, 0x27),
            code_label_fg: rgb(0x19, 0x1c, 0x22),
            code_label_bg: rgb(0x88, 0xc0, 0xd0),
            blockquote: rgb(0x4c, 0x56, 0x6a),
            list_marker: rgb(0x81, 0xa1, 0xc1),
            link: rgb(0x8f, 0xbc, 0xbb),
            link_selected_fg: rgb(0x19, 0x1c, 0x22),
            link_selected_bg: rgb(0x88, 0xc0, 0xd0),
            image_link: rgb(0xb4, 0x8e, 0xad),
            image_selected_fg: rgb(0x19, 0x1c, 0x22),
            image_selected_bg: rgb(0xb4, 0x8e, 0xad),
            search_match_fg: rgb(0x1e, 0x21, 0x27),
            search_match_bg: rgb(0x88, 0xc0, 0xd0),
            search_selected_fg: rgb(0x19, 0x1c, 0x22),
            search_selected_bg: rgb(0x81, 0xa1, 0xc1),
            rule: rgb(0x43, 0x4c, 0x5e),
            table_header: rgb(0xec, 0xef, 0xf4),
            table_border: rgb(0x43, 0x4c, 0x5e),
            mermaid: rgb(0x88, 0xc0, 0xd0),
        },
        other => panic!("unknown built-in preset '{other}'"),
    }
}

struct Palette {
    text: Color,
    heading: Color,
    heading_prefix: Color,
    heading_muted: Color,
    heading_faint: Color,
    code_fg: Color,
    code_bg: Color,
    code_label_fg: Color,
    code_label_bg: Color,
    blockquote: Color,
    list_marker: Color,
    link: Color,
    link_selected_fg: Color,
    link_selected_bg: Color,
    image_link: Color,
    image_selected_fg: Color,
    image_selected_bg: Color,
    search_match_fg: Color,
    search_match_bg: Color,
    search_selected_fg: Color,
    search_selected_bg: Color,
    rule: Color,
    table_header: Color,
    table_border: Color,
    mermaid: Color,
}

impl Palette {
    fn build(self) -> Theme {
        let bold = Modifier::BOLD;
        let underline = Modifier::UNDERLINED;
        Theme {
            text: fg(self.text),
            h1: fg(self.heading).add_modifier(bold).add_modifier(underline),
            h1_prefix: fg(self.heading_prefix).add_modifier(bold),
            h2: fg(self.heading).add_modifier(bold),
            h2_prefix: fg(self.heading_prefix).add_modifier(bold),
            h3: fg(self.heading).add_modifier(bold),
            h3_prefix: fg(self.heading_prefix),
            h4: fg(self.heading_muted).add_modifier(bold),
            h4_prefix: fg(self.heading_faint),
            h5: fg(self.heading_muted),
            h5_prefix: fg(self.heading_faint),
            h6: fg(self.heading_faint),
            h6_prefix: fg(self.heading_faint),
            code_inline: fg(self.code_fg).bg(self.code_bg),
            code_block: fg(self.code_fg).bg(self.code_bg),
            code_block_language: fg(self.code_label_fg).bg(self.code_label_bg),
            blockquote: fg(self.blockquote).italic(),
            list_marker: fg(self.list_marker),
            link: fg(self.link).add_modifier(underline),
            link_selected: fg(self.link_selected_fg)
                .bg(self.link_selected_bg)
                .add_modifier(bold),
            image_link: fg(self.image_link).add_modifier(underline),
            image_link_selected: fg(self.image_selected_fg)
                .bg(self.image_selected_bg)
                .add_modifier(bold),
            search_match: fg(self.search_match_fg)
                .bg(self.search_match_bg)
                .add_modifier(bold),
            search_match_selected: fg(self.search_selected_fg)
                .bg(self.search_selected_bg)
                .add_modifier(bold)
                .add_modifier(underline),
            rule: fg(self.rule),
            table_header: fg(self.table_header).add_modifier(bold),
            table_cell: Style::default(),
            table_border: fg(self.table_border),
            mermaid_placeholder: fg(self.mermaid),
            math: fg(self.code_fg).italic(),
        }
    }
}

fn fg(color: Color) -> Style {
    Style::default().fg(color)
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_names_resolve() {
        for name in PRESET_NAMES {
            Theme::from_preset(name).expect(name);
        }
    }

    #[test]
    fn unknown_preset_errors() {
        assert!(Theme::from_preset("unknown-scheme").is_err());
    }

    #[test]
    fn default_uses_named_preset() {
        assert_eq!(
            Theme::default(),
            Theme::from_preset(DEFAULT_PRESET).unwrap()
        );
    }

    #[test]
    fn presets_differ_from_default() {
        assert_ne!(Theme::from_preset("nord").unwrap(), Theme::default());
        assert_ne!(Theme::from_preset("light").unwrap(), Theme::default());
    }

    #[test]
    fn cursor_midnight_aliases_resolve() {
        for alias in ["cursor-midnight", "cursor-dark-midnight"] {
            let theme = Theme::from_preset(alias).unwrap();
            assert_eq!(theme.link.fg, Some(rgb(0x8f, 0xbc, 0xbb)));
        }
    }

    #[test]
    fn hackerman_omarchy_aliases_resolve() {
        for alias in ["hackerman-omarchy", "hackerman", "omarchy-hackerman"] {
            let theme = Theme::from_preset(alias).unwrap();
            assert_eq!(theme.text.fg, Some(rgb(0xdd, 0xf7, 0xff)));
        }
    }

    #[test]
    fn preset_table_covers_preset_names() {
        for name in PRESET_NAMES {
            assert_eq!(canonical_preset_name(name), Some(*name));
        }
    }
}
