# Rainbow Six profile

The `r6` screen fetches `https://r6fetch.cc/pc/mosly` directly, with no API key.
It preserves the public ANSI rank emblem's Braille characters and RGB colours,
then lays out the stats beside it for the 480 × 320 display.
When space permits, emblem rows are doubled to match the taller terminal-cell
aspect ratio on the renderer's square 8 × 8 cells. It includes current
rank and RP, season/all-time peak, K/D, win rate, kills, deaths, wins/losses,
level, and headshot percentage when supplied. Optional stats at the bottom may
be omitted on shorter displays.

```sh
cargo run -p player -- --preview --playlist config/r6.toml \
  --display profiles/displays/dfrobot-fit0820-480x320.toml
```

The default playlist includes this screen. Change `username` or `platform`
in its settings to select another player. Supported platforms are `pc`, `ps`,
and `xbox`. URLs encode the username as one path segment.

Requests run in the background with a 15-second timeout and 256 KiB limit.
Refresh defaults to 300 seconds, matching r6fetch's normal cache lifetime;
settings accept 60–3600 seconds. The shared bottom-right refresh strip clears
left to right and animates during requests. Failed refreshes remove stale
profile data. Hidden screens do not start requests. Validation performs no I/O.

This is an ANSI-only screen displayed inside the native Slint image host.
Even `--renderer native` uses the ANSI bitmap for this screen, preserving the
requested emblem. It requires a grid of at least 54 × 30 cells; smaller grids
show a size hint instead of cutting or wrapping the emblem.

The ANSI response is decoded into bounded cells and reformatted, not blindly
replayed into the output frame. Essential stats and the emblem are validated;
upstream format changes may require updating the adapter. No changes were made
to the separate r6fetch project.

`config/r6-demo.toml` uses the recorded public response in
`screens/r6/fixtures/mosly.ansi`. These are snapshot stats, not a live feed.
It makes no requests. Refresh that fixture deliberately if expectations change.

Live connection check:

```sh
cargo run -p screen-r6 --example check-r6
```
