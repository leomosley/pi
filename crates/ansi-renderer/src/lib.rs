//! ANSI text to a bounded cell grid and crisp RGBA bitmap, without a GUI dependency.
//! This is a screen renderer, not a full terminal emulator. See the crate README.

mod color;
mod grid;
mod raster;

pub use color::Color;
pub use grid::{Cell, Style};
pub use raster::Frame;

pub const CELL_WIDTH: u32 = 8;
pub const CELL_HEIGHT: u32 = 8;
const MAX_CELLS: usize = 262_144;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RenderError {
    #[error("grid dimensions must be nonzero and contain at most {MAX_CELLS} cells")]
    InvalidDimensions,
}

/// Stateful ANSI parser. `feed` accepts partial sequences across calls.
pub struct Terminal {
    parser: vte::Parser,
    grid: grid::Grid,
}

impl Terminal {
    pub fn new(columns: usize, rows: usize) -> Result<Self, RenderError> {
        let count = columns
            .checked_mul(rows)
            .ok_or(RenderError::InvalidDimensions)?;
        if columns == 0 || rows == 0 || count > MAX_CELLS {
            return Err(RenderError::InvalidDimensions);
        }
        Ok(Self {
            parser: vte::Parser::new(),
            grid: grid::Grid::new(columns, rows),
        })
    }

    pub fn columns(&self) -> usize {
        self.grid.columns
    }

    pub fn rows(&self) -> usize {
        self.grid.rows
    }

    pub fn cell(&self, column: usize, row: usize) -> Option<&Cell> {
        if column >= self.columns() || row >= self.rows() {
            return None;
        }
        self.grid.cells.get(row * self.columns() + column)
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.grid, bytes);
    }

    /// Begin an independent screen frame, clearing parser, cursor, and cell state.
    pub fn reset(&mut self) {
        self.parser = vte::Parser::new();
        self.grid.reset();
    }

    /// One 8×8 bitmap glyph per cell, with opaque pixels and no anti-aliasing.
    pub fn rasterize(&self) -> Frame {
        raster::render(&self.grid)
    }
}
