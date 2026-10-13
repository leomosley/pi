# Pi Display

A native Rust and Slint screen player for Linux and Raspberry Pi. Bun manages
workspace tooling, Turborepo runs tasks, and Cargo compiles the application.

## Workspace

- `apps/player`: native player and preview controls.
- `crates/screen-api`: shared screen lifecycle.
- `screens/clock`: clock logic and Slint layout.
- `config`: playlists and per-screen settings.
- `profiles/displays`: target resolution and UI scale.
- `profiles/devices`: informational deployment metadata, not hardware emulation.

## Development

Install Bun 1.4.3 and Rust through rustup. The repository selects Rust 1.95.0.

```sh
bun install
bun run build
bun run test
bun run lint
bun run format
```

Cargo caches compilation. Turbo caching is disabled because all Rust packages
share `target/`. Turbo runs Cargo tasks serially to avoid competing build locks.

Slint has licensing conditions, including special considerations for embedded
products. Review https://slint.dev/pricing before distributing this on a device.
