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
