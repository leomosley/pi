# ANSI renderer

A reusable ANSI-to-bitmap renderer with no Slint dependency. `vte` handles byte
parsing; this crate owns the bounded cell grid, ANSI actions, palette, and bitmap
rasterizer. `font8x8` supplies the basic 8×8 glyph data.

```rust
use ansi_renderer::Terminal;

let mut screen = Terminal::new(60, 40)?;
screen.feed(b"\x1b[2J\x1b[10;20H\x1b[93mHello\x1b[0m");
let frame = screen.rasterize(); // 480 × 320 opaque RGBA pixels
# Ok::<(), ansi_renderer::RenderError>(())
```

Cells are 8×8 physical pixels. Glyphs have no anti-aliasing. Integer enlargement
with nearest-neighbour sampling preserves their pixels.

The bitmap font also renders all 256 Unicode Braille patterns, the horizontal
box-drawing rule, and the middle dot, supporting r6fetch's coloured rank art.

## Character-art glyph libraries

The rasterizer's 8×8 font draws individual terminal characters. `GlyphLibrary`
composes larger letters and digits out of those characters:

- `ascii`: 5×7 character-art letters and digits made from `#` and spaces.
- `blocks`: the same shapes made from solid `█` characters.
- `segments`: compact 3-row digits made from `_`, `|`, and spaces. Also includes
  the A/P/M suffix used by clocks; other letters use the fallback glyph.

ASCII and blocks include A–Z, 0–9, colon, and space; lowercase uses uppercase
shapes. Unknown characters use a question-mark shape. The composer centers art
in the available grid, or falls back to clipped normal text if the art won't fit.

Custom libraries can extend or replace individual glyphs:

```rust
use ansi_renderer::{Glyph, GlyphLibrary, GlyphSet, Terminal};

let mut glyphs = GlyphLibrary::preset(GlyphSet::Ascii);
glyphs.insert('0', Glyph::new(&[" @@ ", "@  @", "@  @", " @@ "])?);
let mut screen = Terminal::new(60, 40)?;
screen.feed(glyphs.render_centered("12:00", 60, 40).as_bytes());
let pixels = screen.rasterize();
```

Custom glyphs must be rectangular, 1–64 cells wide and high, and contain no
control characters. They are Rust data, not external font files. Applications
can provide additional named libraries by constructing and registering them.

`GlyphLibrary::render_rolling(previous, current, progress, columns, rows)`
animates only changed characters. Progress runs from 0 to 1; old and new rows
are clipped to one fixed glyph slot. Unchanged characters stay stationary.
Transitions between different-sized glyphs or different-length text use the
static target frame instead. The clock uses this method for digit changes.

## Supported ANSI subset

- Cursor movement: CSI `A B C D G H f`, save/restore `s u` and ESC `7 8`.
- Erase: CSI `J K`, including clearing the screen and current line.
- SGR: reset, bold, inverse, default colours, 16 colours, 256-colour palette,
  and semicolon-delimited 24-bit RGB foreground/background.
- CR, LF, backspace, tab, wrapping and full-screen scrolling.
- Basic bitmap characters plus full, upper-half and lower-half block glyphs.
- Incremental input: escape sequences and UTF-8 may span `feed` calls.

Unsupported control sequences, OSC commands, private modes, and SGR subparameters
are ignored. Unsupported glyphs appear as `?`, each consuming one cell. There is
no wide-character/combining-character layout, font shaping, scrollback, shell,
alternate screen, or complete VT terminal emulation. No escape sequence can run
commands or interact with the clipboard.

Grids must contain between 1 and 262,144 cells. Raster output is bounded to
64 MiB. Reset between independent screen frames to clear state and incomplete
sequences.
