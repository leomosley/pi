# Previewing

```sh
bun run dev
```

This runs the real native player, not a browser or hardware emulator. The default
display profile is 800 × 480 pixels with scale factor 1.

## Display profiles

Profiles specify physical output pixels and UI scaling:

```toml
name = "My screen"
width_px = 800
height_px = 480
scale_factor = 1.0
```

For portrait orientation swap width and height. At scale factor 2 an 800 × 480
screen has a 400 × 240 logical UI area. The player starts its Slint backend with
that scale override rather than inheriting your desktop's scale.

The DFRobot FIT0820 3.5-inch SPI touchscreen sold by The Pi Hut has a bundled
480 × 320 landscape profile:

```sh
cargo run -p player -- --preview \
  --display profiles/displays/dfrobot-fit0820-480x320.toml
```

This matches the pixel layout, not its 3.5-inch physical size on your monitor.
The panel uses resistive touch and requires hardware-specific display drivers;
verify that its vendor setup supports your Raspberry Pi OS version before buying.

The preview window is fixed-size. Its control strip adds 48 logical pixels below
the target content, plus the desktop's normal window decorations. Choose a
profile that fits on your monitor; automatic zoom-to-fit is not implemented.

This does not simulate physical inches, panel colours, touch hardware, CPU speed,
RAM limits, or Pi graphics drivers. The device profile is deployment metadata
only and is not consumed by the player. Test compatibility and performance on
the actual Pi.

## Playlist

Each entry defines a registered module, its visible duration in seconds, and
module-specific settings. Multiple entries can use the same module with different
settings. Rotation wraps to the first entry.

The default playlist has only one clock, so rotation returns to the same clock.
To see a visible transition with the existing module, add a second clock entry
with `format = "12h"` and a short duration.

Pause freezes playlist timing; screen content continues updating. Previous and
Next work while paused. Configuration changes require restarting the player.

Validate a playlist without opening a GUI:

```sh
cargo run -p player -- --validate --playlist config/playlist.toml
```

## Renderer selection

The bundled clock uses `renderer = "ansi"` and `glyphs = "ascii"`. Its Rust code
composes digits from `#` characters and emits ANSI cursor and colour sequences,
which the custom crate rasterizes without anti-aliasing. On a 480 × 320 display
the grid is 60 columns × 40 rows, with
8×8 physical-pixel cells. This is a deliberately pixelated presentation, not a
simulation of the TFT's pixel density or panel sharpness.

```toml
[[screens]]
module = "clock"
duration_seconds = 30
renderer = "ansi" # or "native" for the original Slint clock
glyphs = "ascii"  # "ascii", "blocks", or "segments"; used only by ANSI

[screens.settings]
format = "24h"
show_seconds = true
```

Override the entire playlist from the command line:

```sh
cargo run -p player -- --preview --renderer ansi --glyphs ascii \
  --display profiles/displays/dfrobot-fit0820-480x320.toml
```

Each frame is cached until its ANSI source changes. Presentation uses nearest-
neighbour image sampling. Choose display dimensions divisible by 8 for an exact
cell-to-pixel match; other dimensions fit the smaller grid into the viewport.

Use `--glyphs blocks` for the original solid-block presentation or
`--glyphs segments` for compact `_`/`|` character art. These choose the glyph
library, not the rasterizer. See the crate README for adding custom glyphs.

## App-only screenshots

Capture the native window without including other desktop applications:

```sh
cargo run -p player -- --preview --capture /tmp/clock-preview.png
```

The player opens normally, saves a PNG after its first rendered frames, and exits.
The PNG includes the preview toolbar but excludes desktop window decorations.
This needs a Wayland or X11 session; it is not a headless renderer.

Run the opt-in native rendering checks with `bun run smoke`. These open and close
five native windows, checking portrait size, 2× scaling, native clock-format
rotation, exact solid-block pixels, and actual ASCII character pixels.
Screenshots are saved to the system temporary directory, or to
`PI_DISPLAY_TEST_ARTIFACT_DIR` when set.

The player disables Slint 1.18.1's newer software-renderer text-layout path.
Native screenshots reproduced missing digits after switching from 24-hour to
12-hour time; the legacy path renders the transition correctly. The smoke test
checks that those digits remain visible so this workaround can be revisited
when upgrading Slint.
