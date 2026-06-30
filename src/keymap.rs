//! Vim-style key mapping.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

/// User command produced by a key event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    Quit,
    None,
}

/// Map a crossterm event to a command.
pub fn map_event(event: Event) -> Command {
    match event {
        Event::Key(key) => map_key(key),
        _ => Command::None,
    }
}

fn map_key(key: KeyEvent) -> Command {
    if std::env::var("BMD_DEBUG").is_ok() {
        eprintln!("[bmd debug] key event: {:?}", key);
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
        _ => Command::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vim_bindings() {
        assert_eq!(map_key(key('j')), Command::ScrollDown);
        assert_eq!(map_key(key('k')), Command::ScrollUp);
        assert_eq!(map_key(key('d')), Command::HalfPageDown);
        assert_eq!(map_key(key('u')), Command::HalfPageUp);
        assert_eq!(map_key(key('g')), Command::JumpToTop);
        assert_eq!(map_key(shift('G')), Command::JumpToBottom);
        assert_eq!(map_key(key('n')), Command::NextLink);
        assert_eq!(map_key(shift('N')), Command::PrevLink);
        assert_eq!(map_key(key('o')), Command::OpenLink);
        assert_eq!(map_key(key('q')), Command::Quit);
    }

    fn key(c: char) -> KeyEvent {
        KeyEvent::from(KeyCode::Char(c))
    }

    fn shift(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT)
    }
}
