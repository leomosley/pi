//! Reusable character-art glyphs, distinct from the rasterizer's 8×8 cell font.

use std::{collections::BTreeMap, fmt::Write, str::FromStr};

use serde::Deserialize;

use crate::RenderError;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GlyphSet {
    #[default]
    Ascii,
    Blocks,
    Segments,
}

impl FromStr for GlyphSet {
    type Err = RenderError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "ascii" => Ok(Self::Ascii),
            "blocks" => Ok(Self::Blocks),
            "segments" => Ok(Self::Segments),
            _ => Err(RenderError::UnknownGlyphSet(value.into())),
        }
    }
}

/// A rectangular character-art glyph, measured in terminal cells, not pixels.
#[derive(Debug, Clone)]
pub struct Glyph {
    rows: Vec<String>,
    width: usize,
}

impl Glyph {
    pub fn new(rows: &[&str]) -> Result<Self, RenderError> {
        let width = rows.first().map_or(0, |row| row.chars().count());
        if rows.is_empty()
            || rows.len() > 64
            || width == 0
            || width > 64
            || rows
                .iter()
                .any(|row| row.chars().count() != width || row.chars().any(char::is_control))
        {
            return Err(RenderError::InvalidGlyph);
        }
        Ok(Self {
            rows: rows.iter().map(|row| (*row).into()).collect(),
            width,
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }
    pub fn height(&self) -> usize {
        self.rows.len()
    }
    pub fn rows(&self) -> &[String] {
        &self.rows
    }

    fn builtin(rows: Vec<String>) -> Self {
        Self {
            width: rows[0].chars().count(),
            rows,
        }
    }
}

/// Named presets can be extended or overridden with custom character-art glyphs.
pub struct GlyphLibrary {
    glyphs: BTreeMap<char, Glyph>,
    fallback: Glyph,
}

impl GlyphLibrary {
    pub fn new(fallback: Glyph) -> Self {
        Self {
            glyphs: BTreeMap::new(),
            fallback,
        }
    }

    pub fn preset(set: GlyphSet) -> Self {
        if set == GlyphSet::Segments {
            let mut library = Self::new(Glyph::builtin(vec![
                " _ ".into(),
                " _|".into(),
                " . ".into(),
            ]));
            for (character, rows) in SEGMENTS {
                library.insert(
                    *character,
                    Glyph::builtin(rows.iter().map(|row| (*row).into()).collect()),
                );
            }
            return library;
        }
        let ink = if set == GlyphSet::Ascii { '#' } else { '█' };
        let convert = |rows: &[&str]| {
            Glyph::builtin(
                rows.iter()
                    .map(|row| {
                        row.chars()
                            .map(|value| if value == '1' { ink } else { ' ' })
                            .collect()
                    })
                    .collect(),
            )
        };
        let mut library = Self::new(convert(&[
            "01110", "10001", "00001", "00010", "00100", "00000", "00100",
        ]));
        for (character, rows) in MASKS {
            library.insert(*character, convert(rows));
        }
        library.insert(':', convert(&["0", "1", "1", "0", "1", "1", "0"]));
        library.insert(' ', convert(&["000"; 7]));
        library
    }

    pub fn insert(&mut self, character: char, glyph: Glyph) -> Option<Glyph> {
        self.glyphs.insert(character, glyph)
    }

    pub fn glyph(&self, character: char) -> &Glyph {
        self.glyphs
            .get(&character)
            .or_else(|| self.glyphs.get(&character.to_ascii_uppercase()))
            .unwrap_or(&self.fallback)
    }

    /// Compose a centered, independently-cleared ANSI frame in white on black.
    /// If the art will not fit, use centered normal-sized text clipped to the grid.
    pub fn render_centered(&self, text: &str, columns: usize, rows: usize) -> String {
        self.render(text, columns, rows, None)
    }

    /// Roll only changed glyphs upward, clipping old/new rows inside one glyph slot.
    /// Progress 0 is the old frame; progress 1 is the new frame. Different-sized
    /// glyphs or text fall back to a static frame so surrounding content never moves.
    pub fn render_rolling(
        &self,
        previous: &str,
        text: &str,
        progress: f32,
        columns: usize,
        rows: usize,
    ) -> String {
        if !progress.is_finite() {
            return self.render_centered(text, columns, rows);
        }
        self.render(
            text,
            columns,
            rows,
            Some((previous, progress.clamp(0.0, 1.0))),
        )
    }

    fn render(
        &self,
        text: &str,
        columns: usize,
        rows: usize,
        transition: Option<(&str, f32)>,
    ) -> String {
        let mut output = String::from("\x1b[0m\x1b[2J\x1b[H\x1b[97m");
        if columns == 0 || rows == 0 {
            return output;
        }
        let characters: Vec<char> = text
            .chars()
            .filter(|ch| !ch.is_control())
            .take(columns.min(4096))
            .collect();
        let glyphs: Vec<_> = characters
            .iter()
            .map(|&character| self.glyph(character))
            .collect();
        let width = glyphs
            .iter()
            .map(|glyph| glyph.width + 1)
            .sum::<usize>()
            .saturating_sub(1);
        let height = glyphs.iter().map(|glyph| glyph.height()).max().unwrap_or(0);
        let previous = transition.and_then(|(text, progress)| {
            let previous: Vec<char> = text
                .chars()
                .filter(|ch| !ch.is_control())
                .take(columns.min(4096))
                .collect();
            let compatible = previous.len() == characters.len()
                && previous.iter().zip(&glyphs).all(|(&ch, current)| {
                    let old = self.glyph(ch);
                    old.width() == current.width() && old.height() == current.height()
                });
            compatible.then_some((previous, progress))
        });
        if width > columns || height > rows {
            let left = columns.saturating_sub(characters.len()) / 2;
            let _ = write!(output, "\x1b[{};{}H", rows / 2 + 1, left + 1);
            output.extend(characters);
        } else {
            let left = (columns - width) / 2;
            let top = (rows - height) / 2;
            for row in 0..height {
                let _ = write!(output, "\x1b[{};{}H", top + row + 1, left + 1);
                for (index, glyph) in glyphs.iter().enumerate() {
                    if index > 0 {
                        output.push(' ');
                    }
                    let line = if let Some((previous, progress)) = &previous
                        && previous[index] != characters[index]
                        && row < glyph.height()
                    {
                        let shift = (progress * glyph.height() as f32).floor() as usize;
                        let source_row = row + shift;
                        if source_row < glyph.height() {
                            self.glyph(previous[index]).rows.get(source_row)
                        } else {
                            glyph.rows.get(source_row - glyph.height())
                        }
                    } else {
                        glyph.rows.get(row)
                    };
                    if let Some(line) = line {
                        output.push_str(line);
                    } else {
                        output.extend(std::iter::repeat_n(' ', glyph.width));
                    }
                }
            }
        }
        output.push_str("\x1b[0m");
        output
    }
}

const MASKS: &[(char, [&str; 7])] = &[
    (
        '0',
        [
            "01110", "10001", "10011", "10101", "11001", "10001", "01110",
        ],
    ),
    (
        '1',
        [
            "00100", "01100", "00100", "00100", "00100", "00100", "01110",
        ],
    ),
    (
        '2',
        [
            "01110", "10001", "00001", "00010", "00100", "01000", "11111",
        ],
    ),
    (
        '3',
        [
            "11110", "00001", "00001", "01110", "00001", "00001", "11110",
        ],
    ),
    (
        '4',
        [
            "00010", "00110", "01010", "10010", "11111", "00010", "00010",
        ],
    ),
    (
        '5',
        [
            "11111", "10000", "10000", "11110", "00001", "00001", "11110",
        ],
    ),
    (
        '6',
        [
            "01110", "10000", "10000", "11110", "10001", "10001", "01110",
        ],
    ),
    (
        '7',
        [
            "11111", "00001", "00010", "00100", "01000", "01000", "01000",
        ],
    ),
    (
        '8',
        [
            "01110", "10001", "10001", "01110", "10001", "10001", "01110",
        ],
    ),
    (
        '9',
        [
            "01110", "10001", "10001", "01111", "00001", "00001", "01110",
        ],
    ),
    (
        'A',
        [
            "01110", "10001", "10001", "11111", "10001", "10001", "10001",
        ],
    ),
    (
        'B',
        [
            "11110", "10001", "10001", "11110", "10001", "10001", "11110",
        ],
    ),
    (
        'C',
        [
            "01111", "10000", "10000", "10000", "10000", "10000", "01111",
        ],
    ),
    (
        'D',
        [
            "11110", "10001", "10001", "10001", "10001", "10001", "11110",
        ],
    ),
    (
        'E',
        [
            "11111", "10000", "10000", "11110", "10000", "10000", "11111",
        ],
    ),
    (
        'F',
        [
            "11111", "10000", "10000", "11110", "10000", "10000", "10000",
        ],
    ),
    (
        'G',
        [
            "01111", "10000", "10000", "10111", "10001", "10001", "01111",
        ],
    ),
    (
        'H',
        [
            "10001", "10001", "10001", "11111", "10001", "10001", "10001",
        ],
    ),
    (
        'I',
        [
            "11111", "00100", "00100", "00100", "00100", "00100", "11111",
        ],
    ),
    (
        'J',
        [
            "00111", "00010", "00010", "00010", "10010", "10010", "01100",
        ],
    ),
    (
        'K',
        [
            "10001", "10010", "10100", "11000", "10100", "10010", "10001",
        ],
    ),
    (
        'L',
        [
            "10000", "10000", "10000", "10000", "10000", "10000", "11111",
        ],
    ),
    (
        'M',
        [
            "10001", "11011", "10101", "10101", "10001", "10001", "10001",
        ],
    ),
    (
        'N',
        [
            "10001", "11001", "10101", "10011", "10001", "10001", "10001",
        ],
    ),
    (
        'O',
        [
            "01110", "10001", "10001", "10001", "10001", "10001", "01110",
        ],
    ),
    (
        'P',
        [
            "11110", "10001", "10001", "11110", "10000", "10000", "10000",
        ],
    ),
    (
        'Q',
        [
            "01110", "10001", "10001", "10001", "10101", "10010", "01101",
        ],
    ),
    (
        'R',
        [
            "11110", "10001", "10001", "11110", "10100", "10010", "10001",
        ],
    ),
    (
        'S',
        [
            "01111", "10000", "10000", "01110", "00001", "00001", "11110",
        ],
    ),
    (
        'T',
        [
            "11111", "00100", "00100", "00100", "00100", "00100", "00100",
        ],
    ),
    (
        'U',
        [
            "10001", "10001", "10001", "10001", "10001", "10001", "01110",
        ],
    ),
    (
        'V',
        [
            "10001", "10001", "10001", "10001", "10001", "01010", "00100",
        ],
    ),
    (
        'W',
        [
            "10001", "10001", "10001", "10101", "10101", "10101", "01010",
        ],
    ),
    (
        'X',
        [
            "10001", "10001", "01010", "00100", "01010", "10001", "10001",
        ],
    ),
    (
        'Y',
        [
            "10001", "10001", "01010", "00100", "00100", "00100", "00100",
        ],
    ),
    (
        'Z',
        [
            "11111", "00001", "00010", "00100", "01000", "10000", "11111",
        ],
    ),
];

const SEGMENTS: &[(char, [&str; 3])] = &[
    ('0', [" _ ", "| |", "|_|"]),
    ('1', ["   ", "  |", "  |"]),
    ('2', [" _ ", " _|", "|_ "]),
    ('3', [" _ ", " _|", " _|"]),
    ('4', ["   ", "|_|", "  |"]),
    ('5', [" _ ", "|_ ", " _|"]),
    ('6', [" _ ", "|_ ", "|_|"]),
    ('7', [" _ ", "  |", "  |"]),
    ('8', [" _ ", "|_|", "|_|"]),
    ('9', [" _ ", "|_|", " _|"]),
    ('A', [" _ ", "|_|", "| |"]),
    ('P', [" _ ", "|_|", "|  "]),
    ('M', ["   ", "|V|", "| |"]),
    (':', [" ", ".", "."]),
    (' ', ["   ", "   ", "   "]),
];
