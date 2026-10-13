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

## App-only screenshots

Capture the native window without including other desktop applications:

```sh
cargo run -p player -- --preview --capture /tmp/clock-preview.png
```

The player opens normally, saves a PNG after its first rendered frames, and exits.
The PNG includes the preview toolbar but excludes desktop window decorations.
This needs a Wayland or X11 session; it is not a headless renderer.

Run the opt-in native rendering checks with `bun run smoke`. These open and close
three native windows, checking portrait size, 2× scaling, and clock-format
rotation. Screenshots are saved to the system temporary directory, or to
`PI_DISPLAY_TEST_ARTIFACT_DIR` when set.

The player disables Slint 1.18.1's newer software-renderer text-layout path.
Native screenshots reproduced missing digits after switching from 24-hour to
12-hour time; the legacy path renders the transition correctly. The smoke test
checks that those digits remain visible so this workaround can be revisited
when upgrading Slint.
