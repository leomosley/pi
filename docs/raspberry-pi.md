# Running on Raspberry Pi 4

Use 64-bit Raspberry Pi OS with a desktop session for this first version. The
player uses Slint's winit desktop backend and software renderer, supporting
Wayland or X11 without a browser. Direct boot-to-display without a desktop is
not configured yet.

Install the Rust toolchain through rustup and native development prerequisites:

```sh
sudo apt install build-essential pkg-config libfontconfig1-dev libxkbcommon-dev
```

Copy or clone the repository to the Pi. From its root, build and launch:

```sh
cargo build --release -p player --locked
./target/release/player --fullscreen \
  --display profiles/displays/example-800x480.toml \
  --playlist config/playlist.toml
```

Bun and Turbo are optional on the device. They coordinate development commands;
they are not dependencies of the resulting player executable.

Launch from the Pi's desktop terminal so the process can access its display
session. Fullscreen uses the connected monitor's actual resolution. Configure
the monitor's output mode in the operating system to match the chosen profile.

The device profile records a 4 GB Pi 4 as an example. Edit it for your hardware;
it does not limit resources or influence rendering. Pi deployment must be tested
on the actual device; a successful desktop build is not evidence of Pi performance.

Slint's distribution licensing needs review for embedded products before shipping.
