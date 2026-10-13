# Creating a screen

Screens are trusted Rust code compiled into the player, not runtime-loaded
plugins. Each module may own its data sources, worker threads, and state.

1. Create `screens/<name>` with a Cargo library, `package.json`, and `ui/<name>.slint`.
2. Add the crate to the Cargo workspace and shared dependencies.
3. Declare the player-to-screen and screen-to-API dependencies in both Cargo
   and Bun manifests. Bun uses `workspace:*`; Cargo uses local paths.
4. Implement `screen_api::Screen` for the screen's Rust state.
5. Add the module to `ScreenInstance` in `apps/player/src/registry.rs`, including
   construction, lifecycle dispatch, and property updates in `render`.
6. Import its Slint component into `apps/player/ui/player.slint`. Add an active
   screen property and conditional views when there are multiple screen types.
7. Add a playlist entry with its module name, duration, and settings.
8. Run `bun install`, `cargo generate-lockfile`, and `bun run check`.

The clock is the reference implementation. Its Slint layout is compiled through
the player's build script, while its Rust logic lives in an independent crate.
The shared API handles behaviour; it does not pretend Slint has a generic
runtime widget insertion API.

## ANSI screens

The `ansi-renderer` crate turns ANSI strings into a bounded grid of 8×8 bitmap
cells and opaque RGBA pixels. It has no Slint dependency and can be reused in
other applications. See [its API and supported sequences](../crates/ansi-renderer/README.md).

A screen can implement `ansi_frame(columns, rows) -> String`, then use the
player's `AnsiSurface` to parse, cache, and present that frame. The clock is the
reference implementation. ANSI screens use the shared image view and do not
need their own Slint component. Register their Rust state and ANSI frame call
in `registry.rs` as with the clock.

Select `renderer = "ansi"` or `"native"` per playlist entry. A screen offering
both may use either; future ANSI-only modules should reject native mode at
configuration validation. ANSI itself does not limit the module's Rust logic.

## Lifecycle

- `activate`: prepare the visible screen and populate its initial state.
- `tick`: update state while visible. Currently called every 100 milliseconds.
- `deactivate`: stop subscriptions or timers when hidden or shutting down.

All these calls run on the UI thread. Keep them short. Perform network requests
and expensive work on workers, deliver results through channels, and consume
them during `tick`. Shut down workers cleanly when they are no longer needed.

A module has the player's permissions and can crash the process. There is no
sandbox. Video, browser content, and custom GPU rendering require their own
integration rather than automatically becoming Slint widgets.
