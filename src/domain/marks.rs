//! Named scroll bookmarks (`a`–`z`), vim-style.

/// A validated mark name in `a`..=`z`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MarkName(char);

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MarkNameError {
    #[error("mark name must be a lowercase letter a-z")]
    Invalid,
}

impl MarkName {
    /// # Errors
    ///
    /// Returns [`MarkNameError::Invalid`] when `c` is not `a`..=`z`.
    pub fn new(c: char) -> Result<Self, MarkNameError> {
        if !c.is_ascii_lowercase() {
            return Err(MarkNameError::Invalid);
        }
        Ok(Self(c))
    }

    pub fn as_char(self) -> char {
        self.0
    }

    fn index(self) -> usize {
        (self.0 as u8 - b'a') as usize
    }
}

/// Session scroll marks for the current document.
///
/// Values are logical layout-line offsets at the wrap width when the mark was
/// set. A later wrap or outline-width change can land the same offset on a
/// different heading; jumps clamp to the current max scroll. Reload clears
/// marks because the document identity changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marks {
    slots: [Option<usize>; 26],
}

impl Default for Marks {
    fn default() -> Self {
        Self::new()
    }
}

impl Marks {
    pub const fn new() -> Self {
        Self { slots: [None; 26] }
    }

    /// Set mark `name` to `offset`, consuming `self`.
    pub fn set(mut self, name: MarkName, offset: usize) -> Self {
        self.slots[name.index()] = Some(offset);
        self
    }

    pub fn get(&self, name: MarkName) -> Option<usize> {
        self.slots[name.index()]
    }

    pub fn clear(self) -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_name_rejects_non_lowercase() {
        assert!(MarkName::new('A').is_err());
        assert!(MarkName::new('0').is_err());
        assert!(MarkName::new('a').is_ok());
        assert!(MarkName::new('z').is_ok());
    }

    #[test]
    fn marks_set_and_get() {
        let name = MarkName::new('b').unwrap();
        let marks = Marks::new().set(name, 42);
        assert_eq!(marks.get(name), Some(42));
        assert_eq!(marks.get(MarkName::new('a').unwrap()), None);
    }
}
