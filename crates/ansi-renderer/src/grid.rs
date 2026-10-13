use vte::{Params, Perform};

use crate::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub foreground: Color,
    pub background: Color,
    pub bold: bool,
    pub inverse: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            foreground: Color::WHITE,
            background: Color::BLACK,
            bold: false,
            inverse: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub character: char,
    pub style: Style,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            character: ' ',
            style: Style::default(),
        }
    }
}

pub(crate) struct Grid {
    pub columns: usize,
    pub rows: usize,
    pub cells: Vec<Cell>,
    column: usize,
    row: usize,
    saved_cursor: (usize, usize),
    style: Style,
}

impl Grid {
    pub fn new(columns: usize, rows: usize) -> Self {
        Self {
            columns,
            rows,
            cells: vec![Cell::default(); columns * rows],
            column: 0,
            row: 0,
            saved_cursor: (0, 0),
            style: Style::default(),
        }
    }

    pub fn reset(&mut self) {
        self.cells.fill(Cell::default());
        self.column = 0;
        self.row = 0;
        self.saved_cursor = (0, 0);
        self.style = Style::default();
    }

    fn blank(&self) -> Cell {
        Cell {
            character: ' ',
            style: self.style,
        }
    }

    fn newline(&mut self) {
        if self.row + 1 < self.rows {
            self.row += 1;
        } else {
            self.cells.copy_within(self.columns.., 0);
            let blank = self.blank();
            self.cells[(self.rows - 1) * self.columns..].fill(blank);
        }
    }

    fn sgr(&mut self, params: &[u16]) {
        let mut index = 0;
        while index < params.len() {
            match params[index] {
                0 => self.style = Style::default(),
                1 => self.style.bold = true,
                22 => self.style.bold = false,
                7 => self.style.inverse = true,
                27 => self.style.inverse = false,
                30..=37 => self.style.foreground = Color::indexed((params[index] - 30) as u8),
                40..=47 => self.style.background = Color::indexed((params[index] - 40) as u8),
                90..=97 => self.style.foreground = Color::indexed((params[index] - 90 + 8) as u8),
                100..=107 => {
                    self.style.background = Color::indexed((params[index] - 100 + 8) as u8)
                }
                39 => self.style.foreground = Style::default().foreground,
                49 => self.style.background = Style::default().background,
                38 | 48 => {
                    let foreground = params[index] == 38;
                    let tail = &params[index + 1..];
                    let (color, consumed) = match tail {
                        [5, color, ..] => (u8::try_from(*color).ok().map(Color::indexed), 2),
                        [2, r, g, b, ..] => (
                            u8::try_from(*r)
                                .ok()
                                .zip(u8::try_from(*g).ok())
                                .zip(u8::try_from(*b).ok())
                                .map(|((r, g), b)| Color(r, g, b)),
                            4,
                        ),
                        _ => return,
                    };
                    if let Some(color) = color {
                        if foreground {
                            self.style.foreground = color;
                        } else {
                            self.style.background = color;
                        }
                    }
                    index += consumed;
                }
                _ => {}
            }
            index += 1;
        }
    }
}

impl Perform for Grid {
    fn print(&mut self, character: char) {
        if self.column == self.columns {
            self.column = 0;
            self.newline();
        }
        self.cells[self.row * self.columns + self.column] = Cell {
            character,
            style: self.style,
        };
        self.column += 1;
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' | 0x0b | 0x0c => {
                self.column = self.column.min(self.columns - 1);
                self.newline();
            }
            b'\r' => self.column = 0,
            b'\t' => self.column = ((self.column / 8 + 1) * 8).min(self.columns - 1),
            0x08 => self.column = self.column.saturating_sub(1),
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        if ignore || !intermediates.is_empty() {
            return;
        }
        // Colon-style SGR subparameters are outside this renderer's supported subset.
        if params.iter().any(|group| group.len() != 1) {
            return;
        }
        let params: Vec<u16> = params.iter().map(|group| group[0]).collect();
        let first = params.first().copied().unwrap_or(0) as usize;
        let amount = first.max(1);
        if matches!(
            action,
            'A' | 'B' | 'C' | 'D' | 'G' | 'H' | 'f' | 'J' | 'K' | 's' | 'u'
        ) {
            self.column = self.column.min(self.columns - 1);
        }
        let blank = self.blank();
        let cursor = self.row * self.columns + self.column.min(self.columns - 1);
        match action {
            'A' => self.row = self.row.saturating_sub(amount),
            'B' => self.row = self.row.saturating_add(amount).min(self.rows - 1),
            'C' => self.column = self.column.saturating_add(amount).min(self.columns - 1),
            'D' => self.column = self.column.saturating_sub(amount),
            'G' => self.column = first.saturating_sub(1).min(self.columns - 1),
            'H' | 'f' => {
                self.row = first.saturating_sub(1).min(self.rows - 1);
                self.column = params.get(1).copied().unwrap_or(1).saturating_sub(1) as usize;
                self.column = self.column.min(self.columns - 1);
            }
            'J' => match first {
                0 => self.cells[cursor..].fill(blank),
                1 => self.cells[..=cursor].fill(blank),
                2 => self.cells.fill(blank),
                _ => {}
            },
            'K' => {
                let start = self.row * self.columns;
                match first {
                    0 => self.cells[cursor..start + self.columns].fill(blank),
                    1 => self.cells[start..=cursor].fill(blank),
                    2 => self.cells[start..start + self.columns].fill(blank),
                    _ => {}
                }
            }
            'm' => self.sgr(&params),
            's' => self.saved_cursor = (self.column, self.row),
            'u' => (self.column, self.row) = self.saved_cursor,
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        if ignore || !intermediates.is_empty() {
            return;
        }
        match byte {
            b'7' => self.saved_cursor = (self.column.min(self.columns - 1), self.row),
            b'8' => (self.column, self.row) = self.saved_cursor,
            b'c' => self.reset(),
            _ => {}
        }
    }
}
