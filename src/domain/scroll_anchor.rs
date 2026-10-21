//! Scroll positions that survive a change of wrap width.

/// A scroll position named by the top-level block it falls in, not by its row.
///
/// Row offsets depend on the wrap width, so an offset saved before a terminal
/// resize or an outline toggle lands somewhere else afterwards. The anchor keeps
/// the block index, the row inside that block, and the block's height when the
/// anchor was taken; at another width the row is scaled to the block's new
/// height. Rows past the block's height sit in the gap after it and keep their
/// distance from its end. Block index `blocks.len()` stands for the footnotes
/// section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollAnchor {
    block: usize,
    line: usize,
    rows: usize,
}

impl ScrollAnchor {
    /// `line` rows into `block`, whose height is `rows` at the current width.
    pub const fn new(block: usize, line: usize, rows: usize) -> Self {
        Self { block, line, rows }
    }

    pub const fn block(self) -> usize {
        self.block
    }

    /// Row inside the block once the block is `rows` tall.
    ///
    /// Returns the stored row unchanged when the height did not change.
    pub fn line_in(self, rows: usize) -> usize {
        if self.line >= self.rows {
            return rows + (self.line - self.rows);
        }
        self.line * rows / self.rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_height_keeps_line() {
        let anchor = ScrollAnchor::new(3, 4, 10);
        assert_eq!(anchor.line_in(10), 4);
    }

    #[test]
    fn line_scales_with_block_height() {
        let anchor = ScrollAnchor::new(0, 5, 10);
        assert_eq!(anchor.line_in(20), 10);
        assert_eq!(anchor.line_in(5), 2);
    }

    #[test]
    fn gap_rows_keep_distance_from_block_end() {
        let anchor = ScrollAnchor::new(0, 10, 10);
        assert_eq!(anchor.line_in(4), 4);
    }

    #[test]
    fn empty_block_keeps_offset_past_it() {
        let anchor = ScrollAnchor::new(2, 3, 0);
        assert_eq!(anchor.line_in(0), 3);
    }
}
