# Pi Display

A native Rust and Slint screen player for Linux and Raspberry Pi. Bun manages
workspace tooling, Turborepo runs tasks, and Cargo compiles the application.

## Workspace

- `apps/player`: native player and preview controls.
- `crates/screen-api`: shared screen lifecycle.
- `crates/ansi-renderer`: reusable ANSI cell grid, colours, and bitmap output.
- `screens/clock`: clock logic and Slint layout.
- `config`: playlists and per-screen settings.
- `profiles/displays`: target resolution and UI scale.
- `profiles/devices`: informational deployment metadata, not hardware emulation.

## Development

Install Bun 1.4.3 and Rust through rustup. The repository selects Rust 1.95.0.

```sh
bun install
bun run dev
bun run build
bun run test
bun run lint
bun run format
bun run check
# Optional: opens real windows and checks rendered pixels.
bun run smoke
```

`bun run dev` opens a native window titled **Pi Display | Clock preview** on your
Linux desktop. The black content area is exactly 800 × 480 physical pixels at
scale 1. The toolbar sits below that area and is not part of the target screen.
Pause stops playlist rotation, not the live clock. Previous and Next restart
the selected entry's duration. Closing the window stops the application.

Run the player directly from the repository root to choose another profile:

```sh
cargo run -p player -- --preview \
  --display profiles/displays/example-480x800.toml \
  --playlist config/playlist.toml

# Check configuration without needing a desktop session.
cargo run -p player -- --validate
```

The default clock uses the ANSI renderer with ASCII `#` character-art glyphs.
Choose `glyphs = "ascii"`, `"blocks"`, or `"segments"` alongside `renderer = "ansi"`
in the playlist, or override the library with `--glyphs`. Set `renderer = "native"`
on a playlist entry to use its original Slint interface, or override every entry
with `cargo run -p player -- --renderer native`.

The clock uses the operating system's local timezone. Set `format = "12h"` or
`"24h"` and `show_seconds = true` or `false` in `config/playlist.toml`.
Configuration is read at startup; restart the player after editing it.

See [creating a screen](docs/creating-a-screen.md),
[previewing](docs/previewing.md), and [running on the Pi](docs/raspberry-pi.md).

Cargo caches compilation. Turbo caching is disabled because all Rust packages
share `target/`. Turbo runs Cargo tasks serially to avoid competing build locks.

Slint has licensing conditions, including special considerations for embedded
products. Review https://slint.dev/pricing before distributing this on a device.
