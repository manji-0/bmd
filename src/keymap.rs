//! Vim-style key mapping.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

/// Input mode used when interpreting key events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeymapMode {
    Normal,
    Search,
}

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
    OpenLink,
    StartSearchForward,
    StartSearchBackward,
    SearchConfirm,
    SearchCancel,
    SearchInput(char),
    SearchBackspace,
    Quit,
    None,
}

/// Map a crossterm event to a command.
///
/// When `search_active` is true and the mode is [`KeymapMode::Normal`], `Esc`
/// clears the active search instead of quitting.
pub fn map_event(event: Event, mode: KeymapMode, search_active: bool) -> Command {
    match event {
        Event::Key(key) => match mode {
            KeymapMode::Normal => map_normal_key(key, search_active),
            KeymapMode::Search => map_search_key(key),
        },
        _ => Command::None,
    }
}

fn map_normal_key(key: KeyEvent, search_active: bool) -> Command {
    if std::env::var("BMD_DEBUG").is_ok() {
        eprintln!("[bmd debug] key event: {:?}", key);
    }

    if search_active && key.code == KeyCode::Esc {
        return Command::SearchCancel;
    }

    // Quit commands take priority and are recognized on both Press and Repeat.
    let is_quit = match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => true,
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
        KeyCode::Char('o') | KeyCode::Enter => Command::OpenLink,
        KeyCode::Char('/') => Command::StartSearchForward,
        KeyCode::Char('?') => Command::StartSearchBackward,
        _ => Command::None,
    }
}

fn map_search_key(key: KeyEvent) -> Command {
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
        assert_eq!(
            map_event(Event::Key(key('a')), KeymapMode::Search, false),
            Command::SearchInput('a')
        );
        assert_eq!(
            map_event(
                Event::Key(KeyEvent::from(KeyCode::Backspace)),
                KeymapMode::Search,
                false
            ),
            Command::SearchBackspace
        );
        assert_eq!(
            map_event(
                Event::Key(KeyEvent::from(KeyCode::Enter)),
                KeymapMode::Search,
                false
            ),
            Command::SearchConfirm
        );
        assert_eq!(
            map_event(
                Event::Key(KeyEvent::from(KeyCode::Esc)),
                KeymapMode::Search,
                false
            ),
            Command::SearchCancel
        );
    }

    #[test]
    fn search_mode_ignores_normal_commands() {
        // While typing a query, 'q' should be input rather than quit.
        assert_eq!(
            map_event(Event::Key(key('q')), KeymapMode::Search, false),
            Command::SearchInput('q')
        );
    }

    #[test]
    fn active_search_esc_clears_search_instead_of_quitting() {
        assert_eq!(
            map_event(
                Event::Key(KeyEvent::from(KeyCode::Esc)),
                KeymapMode::Normal,
                true
            ),
            Command::SearchCancel
        );
    }

    #[test]
    fn inactive_search_esc_quits() {
        assert_eq!(
            map_event(
                Event::Key(KeyEvent::from(KeyCode::Esc)),
                KeymapMode::Normal,
                false
            ),
            Command::Quit
        );
    }

    fn map(key: KeyEvent) -> Command {
        map_event(Event::Key(key), KeymapMode::Normal, false)
    }

    fn key(c: char) -> KeyEvent {
        KeyEvent::from(KeyCode::Char(c))
    }

    fn shift(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT)
    }
}
