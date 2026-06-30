//! Vim-style key mapping per UI mode.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::domain::{NormalSearch, UiMode};

/// User command produced by a key event.
#[derive(Clone, Debug, PartialEq, Eq)]
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
    StartSearchForward,
    StartSearchBackward,
    SearchConfirm,
    SearchCancel,
    SearchInput(char),
    SearchBackspace,
    ToggleHelp,
    ToggleChecklist,
    NavBack,
    NavReset,
    Quit,
    None,
}

/// Map a crossterm event to a command for the current UI mode.
pub fn map_event(event: Event, mode: &UiMode, normal_search: &NormalSearch) -> Command {
    match event {
        Event::Key(key) => match mode {
            UiMode::Normal => map_normal_key(key, normal_search),
            UiMode::SearchInput { .. } => map_search_input_key(key),
            UiMode::Preview { .. } => map_preview_key(key),
        },
        _ => Command::None,
    }
}

fn map_normal_key(key: KeyEvent, normal_search: &NormalSearch) -> Command {
    if std::env::var("BMD_DEBUG").is_ok() {
        eprintln!("[bmd debug] key event: {:?}", key);
    }

    if normal_search.is_active() && key.code == KeyCode::Esc {
        return Command::SearchCancel;
    }

    if key.code == KeyCode::Esc {
        return Command::NavReset;
    }

    // Quit commands take priority and are recognized on both Press and Repeat.
    let is_quit = match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => true,
        _ => false,
    };
    if is_quit {
        return Command::Quit;
    }

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Command::ScrollDown,
        KeyCode::Char('k') | KeyCode::Up => Command::ScrollUp,
        KeyCode::Char('d') | KeyCode::PageDown => Command::HalfPageDown,
        KeyCode::Char('u') | KeyCode::PageUp => Command::HalfPageUp,
        KeyCode::Char('g') => Command::JumpToTop,
        KeyCode::Char('G') => Command::JumpToBottom,
        KeyCode::Char('n') | KeyCode::Tab => Command::NextLink,
        KeyCode::Char('N') | KeyCode::BackTab => Command::PrevLink,
        KeyCode::Char('{') => Command::PrevHeading,
        KeyCode::Char('}') => Command::NextHeading,
        KeyCode::Char('o') | KeyCode::Enter => Command::OpenLink,
        KeyCode::Char('O') => Command::NavBack,
        KeyCode::Char('/') => Command::StartSearchForward,
        KeyCode::Char('?') => Command::StartSearchBackward,
        KeyCode::Char('h') => Command::ToggleHelp,
        KeyCode::Char('x') => Command::ToggleChecklist,
        _ => Command::None,
    }
}

fn map_preview_key(key: KeyEvent) -> Command {
    if std::env::var("BMD_DEBUG").is_ok() {
        eprintln!("[bmd debug] preview key event: {:?}", key);
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('o') => Command::ClosePreview,
        KeyCode::Char('q') | KeyCode::Char('Q') => Command::Quit,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Command::Quit,
        _ => Command::None,
    }
}

fn map_search_input_key(key: KeyEvent) -> Command {
    if std::env::var("BMD_DEBUG").is_ok() {
        eprintln!("[bmd debug] search key event: {:?}", key);
    }

    match key.code {
        KeyCode::Enter => Command::SearchConfirm,
        KeyCode::Esc => Command::SearchCancel,
        KeyCode::Backspace => Command::SearchBackspace,
        KeyCode::Char(c) => Command::SearchInput(c),
        _ => Command::None,
    }
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
        assert_eq!(map(key('o')), Command::OpenLink);
        assert_eq!(map(key('q')), Command::Quit);
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
