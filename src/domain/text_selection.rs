//! Mouse text selection anchors in document logical coordinates.

/// A point in the rendered document (logical line + terminal column).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextPoint {
    pub line: usize,
    pub col: usize,
}

impl TextPoint {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

/// Inclusive range between two document points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSelection {
    pub anchor: TextPoint,
    pub cursor: TextPoint,
}

impl TextSelection {
    pub fn new(anchor: TextPoint, cursor: TextPoint) -> Self {
        Self { anchor, cursor }
    }

    pub fn is_empty(self) -> bool {
        self.anchor == self.cursor
    }

    /// Normalize to `(start, end)` with both endpoints inclusive.
    pub fn normalized_inclusive(self) -> (TextPoint, TextPoint) {
        if self.anchor <= self.cursor {
            (self.anchor, self.cursor)
        } else {
            (self.cursor, self.anchor)
        }
    }
}
