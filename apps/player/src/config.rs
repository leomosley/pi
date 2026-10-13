use std::{fs, path::Path};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, de::DeserializeOwned};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayProfile {
    pub name: String,
    pub width_px: u32,
    pub height_px: u32,
    pub scale_factor: f32,
}

impl DisplayProfile {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (240..=8192).contains(&self.width_px) && (240..=8192).contains(&self.height_px),
            "display dimensions must be between 240 and 8192 pixels"
        );
        ensure!(
            self.scale_factor.is_finite() && (0.5..=4.0).contains(&self.scale_factor),
            "display scale_factor must be between 0.5 and 4.0"
        );
        ensure!(
            self.width_px as f32 / self.scale_factor >= 240.0,
            "display must have at least 240 logical pixels of width for preview controls"
        );
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaylistConfig {
    pub screens: Vec<ScreenEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenEntry {
    pub module: String,
    pub duration_seconds: u64,
    #[serde(default)]
    pub settings: toml::Table,
}

impl PlaylistConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.screens.is_empty(), "playlist must contain a screen");
        for entry in &self.screens {
            ensure!(
                (1..=86400).contains(&entry.duration_seconds),
                "screen '{}' duration_seconds must be between 1 and 86400",
                entry.module
            );
        }
        Ok(())
    }
}

pub fn read_config<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let source =
        fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;
    toml::from_str(&source).with_context(|| format!("invalid configuration in {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_display_profile_is_valid() {
        let display: DisplayProfile = toml::from_str(include_str!(
            "../../../profiles/displays/example-800x480.toml"
        ))
        .unwrap();
        assert!(display.validate().is_ok());
    }

    #[test]
    fn zero_sized_display_is_rejected() {
        let display = DisplayProfile {
            name: "Invalid".into(),
            width_px: 0,
            height_px: 480,
            scale_factor: 1.0,
        };
        assert!(display.validate().is_err());
    }

    #[test]
    fn empty_playlist_is_rejected() {
        assert!(PlaylistConfig { screens: vec![] }.validate().is_err());
    }

    #[test]
    fn non_finite_scale_is_rejected() {
        let display = DisplayProfile {
            name: "Invalid".into(),
            width_px: 800,
            height_px: 480,
            scale_factor: f32::NAN,
        };
        assert!(display.validate().is_err());
    }

    #[test]
    fn bundled_portrait_display_is_valid() {
        let display: DisplayProfile = toml::from_str(include_str!(
            "../../../profiles/displays/example-480x800.toml"
        ))
        .unwrap();
        assert!(display.validate().is_ok());
    }

    #[test]
    fn dfrobot_touchscreen_profile_is_valid() {
        let display: DisplayProfile = toml::from_str(include_str!(
            "../../../profiles/displays/dfrobot-fit0820-480x320.toml"
        ))
        .unwrap();
        assert!(display.validate().is_ok());
    }

    #[test]
    fn zero_duration_is_rejected() {
        let config: PlaylistConfig =
            toml::from_str("[[screens]]\nmodule='clock'\nduration_seconds=0").unwrap();
        assert!(config.validate().is_err());
    }
}
