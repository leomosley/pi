use font8x8::{BASIC_FONTS, UnicodeFonts};

use crate::{CELL_HEIGHT, CELL_WIDTH, grid::Grid};

/// Opaque RGBA8 pixels. The caller can present them in any native UI or encode PNG.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

fn glyph(character: char) -> [u8; 8] {
    match character {
        '\u{2800}'..='\u{28ff}' => {
            let dots = character as u32 - 0x2800;
            let mut bitmap = [0u8; 8];
            for (bit, x, y) in [
                (0, 1, 0),
                (1, 1, 2),
                (2, 1, 4),
                (3, 5, 0),
                (4, 5, 2),
                (5, 5, 4),
                (6, 1, 6),
                (7, 5, 6),
            ] {
                if dots & (1 << bit) != 0 {
                    bitmap[y] |= 3 << x;
                    bitmap[y + 1] |= 3 << x;
                }
            }
            bitmap
        }
        '─' => [0, 0, 0, 255, 0, 0, 0, 0],
        '·' => [0, 0, 0, 0x18, 0x18, 0, 0, 0],
        '█' => [255; 8],
        '▀' => [255, 255, 255, 255, 0, 0, 0, 0],
        '▄' => [0, 0, 0, 0, 255, 255, 255, 255],
        _ => BASIC_FONTS
            .get(character)
            .unwrap_or([0x3c, 0x66, 0x60, 0x30, 0x18, 0, 0x18, 0]),
    }
}

pub(crate) fn render(grid: &Grid) -> Frame {
    let width = grid.columns * CELL_WIDTH as usize;
    let height = grid.rows * CELL_HEIGHT as usize;
    // Grid construction bounds this allocation to 64 MiB, independently of input length.
    let mut pixels = vec![0; width * height * 4];
    for (index, cell) in grid.cells.iter().enumerate() {
        let bitmap = glyph(cell.character);
        let (foreground, background) = if cell.style.inverse {
            (cell.style.background.rgba(), cell.style.foreground.rgba())
        } else {
            (cell.style.foreground.rgba(), cell.style.background.rgba())
        };
        let origin_x = index % grid.columns * CELL_WIDTH as usize;
        let origin_y = index / grid.columns * CELL_HEIGHT as usize;
        for (y, bits) in bitmap.into_iter().enumerate() {
            for x in 0..8 {
                let ink = bits & (1 << x) != 0
                    || (cell.style.bold && x > 0 && bits & (1 << (x - 1)) != 0);
                let offset = ((origin_y + y) * width + origin_x + x) * 4;
                pixels[offset..offset + 4].copy_from_slice(if ink {
                    &foreground
                } else {
                    &background
                });
            }
        }
    }
    Frame {
        width: width as u32,
        height: height as u32,
        pixels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_braille_has_no_pixels() {
        assert_eq!(glyph('\u{2800}'), [0; 8]);
    }
    #[test]
    fn braille_dot_order_matches_unicode() {
        assert_eq!(glyph('⠁'), [6, 6, 0, 0, 0, 0, 0, 0]);
        assert_eq!(glyph('⠈'), [96, 96, 0, 0, 0, 0, 0, 0]);
        assert_eq!(glyph('⡀'), [0, 0, 0, 0, 0, 0, 6, 6]);
        assert_eq!(glyph('⢀'), [0, 0, 0, 0, 0, 0, 96, 96]);
    }
    #[test]
    fn full_braille_has_all_eight_dots() {
        assert_eq!(glyph('⣿'), [102; 8]);
    }
}
