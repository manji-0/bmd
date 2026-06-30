//! Visual theme.

use ratatui::style::{Color, Modifier, Style};

/// Visual theme.
#[derive(Clone, Debug)]
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
    pub rule: Style,
    pub table_header: Style,
    pub table_cell: Style,
    pub table_border: Style,
    pub mermaid_placeholder: Style,
    pub search_match: Style,
    pub search_match_selected: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            text: Style::default().fg(Color::White),
            h1: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
            h1_prefix: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            h2: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            h2_prefix: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            h3: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            h3_prefix: Style::default().fg(Color::Yellow),
            h4: Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
            h4_prefix: Style::default().fg(Color::DarkGray),
            h5: Style::default().fg(Color::Gray),
            h5_prefix: Style::default().fg(Color::DarkGray),
            h6: Style::default().fg(Color::DarkGray),
            h6_prefix: Style::default().fg(Color::DarkGray),
            code_inline: Style::default().fg(Color::Yellow).bg(Color::Black),
            code_block: Style::default().fg(Color::White).bg(Color::Black),
            code_block_language: Style::default().fg(Color::Black).bg(Color::Yellow),
            blockquote: Style::default().fg(Color::Gray).italic(),
            list_marker: Style::default().fg(Color::Cyan),
            link: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
            link_selected: Style::default()
                .fg(Color::Black)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            search_match: Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            search_match_selected: Style::default()
                .fg(Color::White)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
            rule: Style::default().fg(Color::DarkGray),
            table_header: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            table_cell: Style::default(),
            table_border: Style::default().fg(Color::DarkGray),
            mermaid_placeholder: Style::default().fg(Color::Yellow),
        }
    }
}
