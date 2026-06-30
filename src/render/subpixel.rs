//! Sub-line scroll via Unicode block-element compositing.

use ratatui::buffer::Cell;
use ratatui::style::Color;

/// Fractional scroll below this snaps to the integer row blit path.
pub(crate) const SUBPIXEL_SNAP: f32 = 0.001;

/// Lower block glyphs at 1/8-line steps (empty → full).
const LOWER_BLOCKS: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

fn ink(cell: &Cell) -> Color {
    cell.fg
}

/// Blend two vertically adjacent cache rows into one screen cell.
///
/// `bottom_frac` is the fraction of the composite occupied by `bottom` (0 = all top).
pub(crate) fn compose_cells_vertical(top: &Cell, bottom: &Cell, bottom_frac: f32) -> Cell {
    let level = (bottom_frac * 8.0).round().clamp(0.0, 8.0) as usize;
    match level {
        0 => top.clone(),
        8 => bottom.clone(),
        n => {
            let mut cell = Cell::new(LOWER_BLOCKS[n]);
            cell.set_fg(ink(bottom));
            cell.set_bg(ink(top));
            cell.modifier = top.modifier;
            cell
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn cell(ch: char, fg: Color) -> Cell {
        let mut c = Cell::default();
        c.set_char(ch);
        c.set_fg(fg);
        c
    }

    #[test]
    fn compose_at_extremes_returns_source_cells() {
        let top = cell('A', Color::Red);
        let bottom = cell('B', Color::Blue);
        assert_eq!(compose_cells_vertical(&top, &bottom, 0.0).symbol(), "A");
        assert_eq!(compose_cells_vertical(&top, &bottom, 1.0).symbol(), "B");
    }

    #[test]
    fn compose_mid_fraction_uses_block_glyph() {
        let top = cell('A', Color::Red);
        let bottom = cell('B', Color::Blue);
        let blended = compose_cells_vertical(&top, &bottom, 0.5);
        assert_eq!(blended.symbol(), "▄");
        assert_eq!(blended.fg, Color::Blue);
        assert_eq!(blended.bg, Color::Red);
    }
}
