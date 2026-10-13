use std::{path::Path, process::Command};

fn player() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_player"));
    command.current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    command
}

#[test]
fn validates_bundled_configuration_without_a_display() {
    let output = player()
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .arg("--validate")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn missing_playlist_returns_an_error() {
    let output = player()
        .args(["--validate", "--playlist", "missing-playlist.toml"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn rejects_conflicting_display_modes() {
    let output = player()
        .args(["--preview", "--fullscreen", "--validate"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
