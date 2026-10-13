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
