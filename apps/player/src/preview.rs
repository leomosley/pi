use std::{env, process::Command};

use anyhow::{Context, Result};

use crate::config::DisplayProfile;

/// Slint reads the scale override when initializing its backend. Re-exec with a
/// child environment rather than mutating process-global environment in Rust.
pub fn launch_with_scale(display: &DisplayProfile) -> Result<Option<std::process::ExitCode>> {
    let scale = display.scale_factor.to_string();
    if env::var("SLINT_SCALE_FACTOR").as_deref() == Ok(scale.as_str()) {
        return Ok(None);
    }
    let status = Command::new(env::current_exe().context("could not locate player executable")?)
        .args(env::args_os().skip(1))
        .env("SLINT_SCALE_FACTOR", scale)
        .status()
        .context("could not launch player with display scaling")?;
    Ok(Some(std::process::ExitCode::from(
        status.code().unwrap_or(1) as u8,
    )))
}
