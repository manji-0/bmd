//! Sub-line scroll via Unicode block-element compositing.

use ratatui::buffer::Cell;
use ratatui::style::Color;

/// Fractional scroll below this snaps to the integer row blit path.
pub(crate) const SUBPIXEL_SNAP: f32 = 0.001;

/// Lower block glyphs at 1/8-line steps (empty → full).
const LOWER_BLOCKS: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

/// Default terminal background when a cell leaves `bg` at reset.
const DEFAULT_BG: Color = Color::Black;

/// Default terminal foreground when a cell leaves `fg` at reset.
const DEFAULT_FG: Color = Color::White;

fn effective_fg(cell: &Cell) -> Color {
    if cell.fg == Color::Reset {
        DEFAULT_FG
    } else {
        cell.fg
    }
}

fn effective_bg(cell: &Cell) -> Color {
    if cell.bg == Color::Reset {
        DEFAULT_BG
    } else {
        cell.bg
    }
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
            cell.set_fg(effective_fg(bottom));
            cell.set_bg(effective_bg(top));
            cell.modifier = top.modifier;
            cell
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn cell(ch: char, fg: Color, bg: Color) -> Cell {
        let mut c = Cell::default();
        c.set_char(ch);
        c.set_fg(fg);
        c.set_bg(bg);
        c
    }

    #[test]
    fn compose_at_extremes_returns_source_cells() {
        let top = cell('A', Color::Red, Color::Black);
        let bottom = cell('B', Color::Blue, Color::Black);
        assert_eq!(compose_cells_vertical(&top, &bottom, 0.0).symbol(), "A");
        assert_eq!(compose_cells_vertical(&top, &bottom, 1.0).symbol(), "B");
    }

    #[test]
    fn compose_mid_fraction_uses_block_glyph() {
        let top = cell('A', Color::Red, Color::Black);
        let bottom = cell('B', Color::Blue, Color::Black);
        let blended = compose_cells_vertical(&top, &bottom, 0.5);
        assert_eq!(blended.symbol(), "▄");
        assert_eq!(blended.fg, Color::Blue);
        assert_eq!(blended.bg, Color::Black);
    }

    #[test]
    fn compose_uses_background_not_foreground_for_blend() {
        let top = cell('A', Color::White, Color::Black);
        let bottom = cell('B', Color::White, Color::Black);
        let blended = compose_cells_vertical(&top, &bottom, 0.5);
        assert_eq!(blended.bg, Color::Black);
        assert_eq!(blended.fg, Color::White);
    }
}
