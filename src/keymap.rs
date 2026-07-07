//! Vim-style key mapping per UI mode.

use std::collections::HashMap;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::config::{KeyBindingValue, command_from_name, parse_binding_specs};
use crate::domain::{NormalSearch, UiMode};
use crate::error::AppError;

/// User command produced by a key event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    ScrollDown,
    ScrollUp,
    HalfPageDown,
    HalfPageUp,
    JumpToTop,
    JumpToBottom,
    NextLink,
    PrevLink,
    NextHeading,
    PrevHeading,
    OpenLink,
    ClosePreview,
    PreviewZoomIn,
    PreviewZoomOut,
    PreviewZoomReset,
    StartSearchForward,
    StartSearchBackward,
    SearchConfirm,
    SearchCancel,
    SearchInput(char),
    SearchBackspace,
    ToggleHelp,
    CloseHelp,
    ToggleChecklist,
    CopySelection,
    ClearSelection,
    NavBack,
    NavReset,
    Quit,
    None,
}

/// Parsed key binding from config or defaults.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeySpec {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeySpec {
    pub fn parse(raw: &str) -> Result<Self, AppError> {
        let mut modifiers = KeyModifiers::empty();
        let mut rest = raw.trim();
        while let Some((head, tail)) = rest.split_once('-') {
            if !is_modifier_token(head) {
                break;
            }
            modifiers |= modifier_from_token(head)?;
            rest = tail;
        }

        let code = match rest {
            "down" | "Down" => KeyCode::Down,
            "up" | "Up" => KeyCode::Up,
            "left" | "Left" => KeyCode::Left,
            "right" | "Right" => KeyCode::Right,
            "enter" | "Enter" | "return" | "Return" => KeyCode::Enter,
            "esc" | "Esc" | "escape" | "Escape" => KeyCode::Esc,
            "tab" | "Tab" => KeyCode::Tab,
            "backtab" | "BackTab" => KeyCode::BackTab,
            "pagedown" | "PageDown" | "pgdn" | "PgDn" => KeyCode::PageDown,
            "pageup" | "PageUp" | "pgup" | "PgUp" => KeyCode::PageUp,
            "home" | "Home" => KeyCode::Home,
            "end" | "End" => KeyCode::End,
            "backspace" | "Backspace" => KeyCode::Backspace,
            single if single.chars().count() == 1 => {
                KeyCode::Char(single.chars().next().expect("single char"))
            }
            other => {
                return Err(AppError::UnsupportedInput(format!(
                    "unknown key '{other}' in '{raw}'"
                )));
            }
        };
        Ok(Self { code, modifiers })
    }
}

fn is_modifier_token(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "c" | "ctrl" | "control" | "s" | "shift" | "a" | "alt"
    )
}

fn modifier_from_token(token: &str) -> Result<KeyModifiers, AppError> {
    match token.to_ascii_lowercase().as_str() {
        "c" | "ctrl" | "control" => Ok(KeyModifiers::CONTROL),
        "s" | "shift" => Ok(KeyModifiers::SHIFT),
        "a" | "alt" => Ok(KeyModifiers::ALT),
        other => Err(AppError::UnsupportedInput(format!(
            "unknown key modifier '{other}'"
        ))),
    }
}

fn resolve_key(key: &KeyEvent) -> ResolvedKey {
    let mut modifiers = key.modifiers;
    if let KeyCode::Char(c) = key.code
        && modifiers.contains(KeyModifiers::SHIFT)
    {
        modifiers -= KeyModifiers::SHIFT;
        return ResolvedKey {
            code: KeyCode::Char(c),
            modifiers,
        };
    }
    ResolvedKey {
        code: key.code,
        modifiers,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ResolvedKey {
    code: KeyCode,
    modifiers: KeyModifiers,
}

impl From<&KeySpec> for ResolvedKey {
    fn from(spec: &KeySpec) -> Self {
        Self {
            code: spec.code,
            modifiers: spec.modifiers,
        }
    }
}

#[derive(Clone, Debug)]
struct ModeBindings {
    map: HashMap<ResolvedKey, Command>,
    scroll_down: Vec<ResolvedKey>,
    scroll_up: Vec<ResolvedKey>,
    single_press: Vec<ResolvedKey>,
    quit: Vec<ResolvedKey>,
}

impl ModeBindings {
    fn empty() -> Self {
        Self {
            map: HashMap::new(),
            scroll_down: Vec::new(),
            scroll_up: Vec::new(),
            single_press: Vec::new(),
            quit: Vec::new(),
        }
    }

    fn bind(&mut self, spec: KeySpec, command: Command) {
        let key = ResolvedKey::from(&spec);
        self.map.insert(key, command);
        match command {
            Command::ScrollDown => {
                self.scroll_down.push(key);
                self.single_press.push(key);
            }
            Command::ScrollUp => {
                self.scroll_up.push(key);
                self.single_press.push(key);
            }
            Command::HalfPageDown | Command::HalfPageUp => {
                self.single_press.push(key);
            }
            Command::Quit => self.quit.push(key),
            _ => {}
        }
    }
}

/// Runtime keymap for all UI modes.
#[derive(Clone, Debug)]
pub struct Keymap {
    normal: ModeBindings,
    preview: ModeBindings,
    search: ModeBindings,
}

impl Default for Keymap {
    fn default() -> Self {
        let mut normal = ModeBindings::empty();
        for (spec, command) in default_normal_bindings() {
            normal.bind(spec, command);
        }

        let mut preview = ModeBindings::empty();
        for (spec, command) in default_preview_bindings() {
            preview.bind(spec, command);
        }

        let mut search = ModeBindings::empty();
        for (spec, command) in default_search_bindings() {
            search.bind(spec, command);
        }

        Self {
            normal,
            preview,
            search,
        }
    }
}

impl Keymap {
    pub(crate) const MODE_NORMAL: &str = "normal";
    pub(crate) const MODE_PREVIEW: &str = "preview";
    pub(crate) const MODE_SEARCH: &str = "search";

    pub(crate) fn apply_overrides(
        &mut self,
        mode: &str,
        overrides: HashMap<String, KeyBindingValue>,
    ) -> Result<(), AppError> {
        let bindings = match mode {
            Self::MODE_NORMAL => &mut self.normal,
            Self::MODE_PREVIEW => &mut self.preview,
            Self::MODE_SEARCH => &mut self.search,
            other => {
                return Err(AppError::UnsupportedInput(format!(
                    "unknown keymap mode '{other}'"
                )));
            }
        };

        for (name, value) in overrides {
            let Some(command) = command_from_name(&name) else {
                return Err(AppError::UnsupportedInput(format!(
                    "unknown keymap command '{name}'"
                )));
            };
            let specs = parse_binding_specs(value.into_specs())?;
            bindings.remove_command(command);
            for spec in specs {
                bindings.bind(spec, command);
            }
        }
        Ok(())
    }

    pub fn map_event(&self, event: Event, mode: &UiMode, normal_search: &NormalSearch) -> Command {
        match event {
            Event::Key(key) => match mode {
                UiMode::Normal => self.map_normal_key(key, normal_search),
                UiMode::SearchInput { .. } => self.map_search_input_key(key),
                UiMode::Preview { .. } => self.map_preview_key(key),
            },
            _ => Command::None,
        }
    }

    pub fn normal_command(&self, key: &KeyEvent) -> Command {
        Self::lookup_command(&self.normal, key)
    }

    fn lookup_command(bindings: &ModeBindings, key: &KeyEvent) -> Command {
        bindings
            .map
            .get(&resolve_key(key))
            .copied()
            .unwrap_or(Command::None)
    }

    pub fn is_line_scroll_key(&self, key: &KeyEvent) -> bool {
        let resolved = resolve_key(key);
        self.normal.scroll_down.contains(&resolved) || self.normal.scroll_up.contains(&resolved)
    }

    pub fn is_single_press_key(&self, key: &KeyEvent) -> bool {
        self.normal.single_press.contains(&resolve_key(key))
    }

    pub fn line_scroll_command(&self, key: &KeyEvent) -> Command {
        let resolved = resolve_key(key);
        if self.normal.scroll_up.contains(&resolved) {
            Command::ScrollUp
        } else {
            Command::ScrollDown
        }
    }

    fn map_normal_key(&self, key: KeyEvent, normal_search: &NormalSearch) -> Command {
        if std::env::var("BMD_DEBUG").is_ok() {
            eprintln!("[bmd debug] key event: {:?}", key);
        }

        if normal_search.is_active() && key.code == KeyCode::Esc && key.modifiers.is_empty() {
            return Command::SearchCancel;
        }

        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            return Command::NavReset;
        }

        let resolved = resolve_key(&key);
        if self.normal.quit.contains(&resolved) {
            return Command::Quit;
        }

        self.normal_command(&key)
    }

    fn map_preview_key(&self, key: KeyEvent) -> Command {
        if std::env::var("BMD_DEBUG").is_ok() {
            eprintln!("[bmd debug] preview key event: {:?}", key);
        }

        let resolved = resolve_key(&key);
        if self.preview.quit.contains(&resolved) {
            return Command::Quit;
        }
        Self::lookup_command(&self.preview, &key)
    }

    fn map_search_input_key(&self, key: KeyEvent) -> Command {
        if std::env::var("BMD_DEBUG").is_ok() {
            eprintln!("[bmd debug] search key event: {:?}", key);
        }

        let command = Self::lookup_command(&self.search, &key);
        if command != Command::None {
            return command;
        }

        if let KeyCode::Char(c) = key.code {
            return Command::SearchInput(c);
        }
        Command::None
    }
}

impl ModeBindings {
    fn remove_command(&mut self, command: Command) {
        self.map.retain(|_, mapped| *mapped != command);
        self.rebuild_derived();
    }

    fn rebuild_derived(&mut self) {
        self.scroll_down.clear();
        self.scroll_up.clear();
        self.single_press.clear();
        self.quit.clear();
        for (key, mapped) in &self.map {
            match mapped {
                Command::ScrollDown => {
                    self.scroll_down.push(*key);
                    self.single_press.push(*key);
                }
                Command::ScrollUp => {
                    self.scroll_up.push(*key);
                    self.single_press.push(*key);
                }
                Command::HalfPageDown | Command::HalfPageUp => {
                    self.single_press.push(*key);
                }
                Command::Quit => self.quit.push(*key),
                _ => {}
            }
        }
    }
}

fn default_normal_bindings() -> Vec<(KeySpec, Command)> {
    fn k(raw: &str, command: Command) -> (KeySpec, Command) {
        (KeySpec::parse(raw).expect("default key binding"), command)
    }

    vec![
        k("j", Command::ScrollDown),
        k("down", Command::ScrollDown),
        k("k", Command::ScrollUp),
        k("up", Command::ScrollUp),
        k("d", Command::HalfPageDown),
        k("pagedown", Command::HalfPageDown),
        k("u", Command::HalfPageUp),
        k("pageup", Command::HalfPageUp),
        k("g", Command::JumpToTop),
        k("G", Command::JumpToBottom),
        k("n", Command::NextLink),
        k("tab", Command::NextLink),
        k("N", Command::PrevLink),
        k("backtab", Command::PrevLink),
        k("p", Command::PrevLink),
        k("[", Command::PrevHeading),
        k("]", Command::NextHeading),
        k("o", Command::OpenLink),
        k("enter", Command::OpenLink),
        k("O", Command::NavBack),
        k("/", Command::StartSearchForward),
        k("?", Command::StartSearchBackward),
        k("h", Command::ToggleHelp),
        k("H", Command::CloseHelp),
        k("x", Command::ToggleChecklist),
        k("y", Command::CopySelection),
        k("q", Command::Quit),
        k("C-c", Command::Quit),
    ]
}

fn default_preview_bindings() -> Vec<(KeySpec, Command)> {
    fn k(raw: &str, command: Command) -> (KeySpec, Command) {
        (KeySpec::parse(raw).expect("default key binding"), command)
    }

    vec![
        k("esc", Command::ClosePreview),
        k("o", Command::ClosePreview),
        k("+", Command::PreviewZoomIn),
        k("=", Command::PreviewZoomIn),
        k("-", Command::PreviewZoomOut),
        k("0", Command::PreviewZoomReset),
        k("q", Command::Quit),
        k("C-c", Command::Quit),
    ]
}

fn default_search_bindings() -> Vec<(KeySpec, Command)> {
    fn k(raw: &str, command: Command) -> (KeySpec, Command) {
        (KeySpec::parse(raw).expect("default key binding"), command)
    }

    vec![
        k("enter", Command::SearchConfirm),
        k("esc", Command::SearchCancel),
        k("backspace", Command::SearchBackspace),
    ]
}

/// Backwards-compatible helper for tests.
pub fn map_event(event: Event, mode: &UiMode, normal_search: &NormalSearch) -> Command {
    Keymap::default().map_event(event, mode, normal_search)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::UiMode;

    #[test]
    fn vim_bindings() {
        assert_eq!(map(key('j')), Command::ScrollDown);
        assert_eq!(map(key('k')), Command::ScrollUp);
        assert_eq!(map(key('d')), Command::HalfPageDown);
        assert_eq!(map(key('u')), Command::HalfPageUp);
        assert_eq!(map(key('g')), Command::JumpToTop);
        assert_eq!(map(shift('G')), Command::JumpToBottom);
        assert_eq!(map(key('n')), Command::NextLink);
        assert_eq!(map(shift('N')), Command::PrevLink);
        assert_eq!(map(key('p')), Command::PrevLink);
        assert_eq!(map(key('o')), Command::OpenLink);
        assert_eq!(map(key('q')), Command::Quit);
    }

    #[test]
    fn heading_bindings_use_brackets() {
        assert_eq!(map(key('[')), Command::PrevHeading);
        assert_eq!(map(key(']')), Command::NextHeading);
    }

    #[test]
    fn help_close_uses_shift_h() {
        assert_eq!(map(shift('H')), Command::CloseHelp);
    }

    #[test]
    fn search_bindings() {
        assert_eq!(map(key('/')), Command::StartSearchForward);
        assert_eq!(map(shift('?')), Command::StartSearchBackward);
    }

    #[test]
    fn search_mode_maps_input() {
        let mode = UiMode::SearchInput {
            direction: crate::domain::SearchDirection::Forward,
            query: String::new(),
        };
        assert_eq!(
            map_event(Event::Key(key('a')), &mode, &NormalSearch::inactive()),
            Command::SearchInput('a')
        );
        assert_eq!(
            map_event(
                Event::Key(KeyEvent::from(KeyCode::Backspace)),
                &mode,
                &NormalSearch::inactive()
            ),
            Command::SearchBackspace
        );
        assert_eq!(
            map_event(
                Event::Key(KeyEvent::from(KeyCode::Enter)),
                &mode,
                &NormalSearch::inactive()
            ),
            Command::SearchConfirm
        );
        assert_eq!(
            map_event(
                Event::Key(KeyEvent::from(KeyCode::Esc)),
                &mode,
                &NormalSearch::inactive()
            ),
            Command::SearchCancel
        );
    }

    #[test]
    fn search_mode_ignores_normal_commands() {
        let mode = UiMode::SearchInput {
            direction: crate::domain::SearchDirection::Forward,
            query: String::new(),
        };
        assert_eq!(
            map_event(Event::Key(key('q')), &mode, &NormalSearch::inactive()),
            Command::SearchInput('q')
        );
    }

    #[test]
    fn active_search_esc_clears_search_instead_of_quitting() {
        let active = NormalSearch::Active {
            direction: crate::domain::SearchDirection::Forward,
            query: crate::domain::SearchQuery::new("foo".to_string()).unwrap(),
            matches: vec![],
            current_index: 0,
        };
        assert_eq!(
            map_event(
                Event::Key(KeyEvent::from(KeyCode::Esc)),
                &UiMode::Normal,
                &active
            ),
            Command::SearchCancel
        );
    }

    #[test]
    fn preview_esc_closes_overlay() {
        assert_eq!(
            map_event(
                Event::Key(KeyEvent::from(KeyCode::Esc)),
                &UiMode::Preview {
                    link_id: crate::domain::LinkId(0)
                },
                &NormalSearch::inactive()
            ),
            Command::ClosePreview
        );
    }

    #[test]
    fn inactive_search_esc_resets_navigation() {
        assert_eq!(
            map_event(
                Event::Key(KeyEvent::from(KeyCode::Esc)),
                &UiMode::Normal,
                &NormalSearch::inactive()
            ),
            Command::NavReset
        );
    }

    #[test]
    fn shift_o_navigates_back() {
        assert_eq!(map(shift('O')), Command::NavBack);
    }

    #[test]
    fn arrow_and_page_keys_scroll() {
        assert_eq!(map(KeyEvent::from(KeyCode::Down)), Command::ScrollDown);
        assert_eq!(map(KeyEvent::from(KeyCode::Up)), Command::ScrollUp);
        assert_eq!(
            map(KeyEvent::from(KeyCode::PageDown)),
            Command::HalfPageDown
        );
        assert_eq!(map(KeyEvent::from(KeyCode::PageUp)), Command::HalfPageUp);
    }

    #[test]
    fn enter_opens_link() {
        assert_eq!(
            map_event(
                Event::Key(KeyEvent::from(KeyCode::Enter)),
                &UiMode::Normal,
                &NormalSearch::inactive()
            ),
            Command::OpenLink
        );
    }

    #[test]
    fn ctrl_c_quits_from_normal_mode() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(
            map_event(Event::Key(key), &UiMode::Normal, &NormalSearch::inactive()),
            Command::Quit
        );
    }

    #[test]
    fn preview_o_closes_overlay() {
        assert_eq!(
            map_event(
                Event::Key(key('o')),
                &UiMode::Preview {
                    link_id: crate::domain::LinkId(0)
                },
                &NormalSearch::inactive()
            ),
            Command::ClosePreview
        );
    }

    #[test]
    fn preview_q_quits() {
        assert_eq!(
            map_event(
                Event::Key(key('q')),
                &UiMode::Preview {
                    link_id: crate::domain::LinkId(0)
                },
                &NormalSearch::inactive()
            ),
            Command::Quit
        );
    }

    #[test]
    fn non_key_events_map_to_none() {
        assert_eq!(
            map_event(
                Event::Resize(80, 24),
                &UiMode::Normal,
                &NormalSearch::inactive()
            ),
            Command::None
        );
    }

    fn map(key: KeyEvent) -> Command {
        map_event(Event::Key(key), &UiMode::Normal, &NormalSearch::inactive())
    }

    fn key(c: char) -> KeyEvent {
        KeyEvent::from(KeyCode::Char(c))
    }

    fn shift(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT)
    }
}
