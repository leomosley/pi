use std::fmt::Write;

fn glyph(character: char) -> [&'static str; 7] {
    match character {
        '0' => [
            "01110", "10001", "10011", "10101", "11001", "10001", "01110",
        ],
        '1' => [
            "00100", "01100", "00100", "00100", "00100", "00100", "01110",
        ],
        '2' => [
            "01110", "10001", "00001", "00010", "00100", "01000", "11111",
        ],
        '3' => [
            "11110", "00001", "00001", "01110", "00001", "00001", "11110",
        ],
        '4' => [
            "00010", "00110", "01010", "10010", "11111", "00010", "00010",
        ],
        '5' => [
            "11111", "10000", "10000", "11110", "00001", "00001", "11110",
        ],
        '6' => [
            "01110", "10000", "10000", "11110", "10001", "10001", "01110",
        ],
        '7' => [
            "11111", "00001", "00010", "00100", "01000", "01000", "01000",
        ],
        '8' => [
            "01110", "10001", "10001", "01110", "10001", "10001", "01110",
        ],
        '9' => [
            "01110", "10001", "10001", "01111", "00001", "00001", "01110",
        ],
        'A' => [
            "01110", "10001", "10001", "11111", "10001", "10001", "10001",
        ],
        'P' => [
            "11110", "10001", "10001", "11110", "10000", "10000", "10000",
        ],
        'M' => [
            "10001", "11011", "10101", "10101", "10001", "10001", "10001",
        ],
        ':' => ["0", "1", "1", "0", "1", "1", "0"],
        _ => ["000", "000", "000", "000", "000", "000", "000"],
    }
}

pub(crate) fn clock_frame(time: &str, columns: usize, rows: usize) -> String {
    let mut output = String::from("\x1b[0m\x1b[2J\x1b[H\x1b[97m");
    let glyphs: Vec<_> = time.chars().map(glyph).collect();
    let width = glyphs
        .iter()
        .map(|glyph| glyph[0].len() + 1)
        .sum::<usize>()
        .saturating_sub(1);
    if width > columns || rows < 7 {
        let column = columns.saturating_sub(time.len()) / 2;
        // Writing to a String is infallible; clipping happens in the receiving grid.
        let _ = write!(output, "\x1b[{};{}H{}", rows / 2 + 1, column + 1, time);
    } else {
        let left = (columns - width) / 2;
        let top = (rows - 7) / 2;
        for row in 0..7 {
            let _ = write!(output, "\x1b[{};{}H", top + row + 1, left + 1);
            for (index, glyph) in glyphs.iter().enumerate() {
                if index > 0 {
                    output.push(' ');
                }
                for pixel in glyph[row].bytes() {
                    output.push(if pixel == b'1' { '█' } else { ' ' });
                }
            }
        }
    }
    output.push_str("\x1b[0m");
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_grid_uses_block_glyphs() {
        assert!(clock_frame("12:34:56", 60, 40).contains('█'));
    }

    #[test]
    fn small_grid_falls_back_to_regular_text() {
        assert!(clock_frame("12:34:56", 30, 20).contains("12:34:56"));
    }

    #[test]
    fn frame_begins_by_resetting_and_clearing() {
        assert!(clock_frame("12:34:56", 60, 40).starts_with("\x1b[0m\x1b[2J\x1b[H"));
    }
}
