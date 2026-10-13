use std::{env, fs::File, io::BufWriter, path::Path, process::Command};

use anyhow::{Context, Result};
use slint::ComponentHandle;

use crate::config::DisplayProfile;

pub fn capture(window: &crate::PlayerWindow, path: &Path) -> Result<()> {
    let snapshot = window.window().take_snapshot()?;
    let file = File::create(path)
        .with_context(|| format!("could not create screenshot {}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), snapshot.width(), snapshot.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(snapshot.as_bytes())?;
    writer.finish()?;
    println!(
        "Saved {}x{} app screenshot to {}",
        snapshot.width(),
        snapshot.height(),
        path.display()
    );
    Ok(())
}

/// Slint reads the scale override when initializing its backend. Re-exec with a
/// child environment rather than mutating process-global environment in Rust.
pub fn launch_with_scale(display: &DisplayProfile) -> Result<Option<std::process::ExitCode>> {
    let scale = display.scale_factor.to_string();
    if env::var("SLINT_SCALE_FACTOR").as_deref() == Ok(scale.as_str())
        && env::var("SLINT_SOFTWARE_RENDERER_PARLEY_DISABLED").as_deref() == Ok("1")
    {
        return Ok(None);
    }
    let status = Command::new(env::current_exe().context("could not locate player executable")?)
        .args(env::args_os().skip(1))
        .env("SLINT_SCALE_FACTOR", scale)
        // Slint 1.18.1's Parley path can drop reused digits after a clock format
        // transition. The legacy software text path passes our native smoke test.
        .env("SLINT_SOFTWARE_RENDERER_PARLEY_DISABLED", "1")
        .status()
        .context("could not launch player with display scaling")?;
    Ok(Some(std::process::ExitCode::from(
        status.code().unwrap_or(1) as u8,
    )))
}
