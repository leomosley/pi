use anyhow::{Context, Result, bail};
use screen_api::Screen;
use screen_clock::{ClockScreen, ClockSettings};

use crate::{PlayerWindow, config::ScreenEntry};

pub enum ScreenInstance {
    Clock(ClockScreen),
}

impl ScreenInstance {
    pub fn build(entry: &ScreenEntry) -> Result<Self> {
        match entry.module.as_str() {
            "clock" => {
                let settings: ClockSettings = entry
                    .settings
                    .clone()
                    .try_into()
                    .context("invalid clock settings")?;
                Ok(Self::Clock(ClockScreen::new(settings)))
            }
            module => bail!("unknown screen module '{module}'; registered modules: clock"),
        }
    }

    pub fn render(&self, window: &PlayerWindow) {
        match self {
            Self::Clock(clock) => window.set_clock_time(clock.time().into()),
        }
    }
}

impl Screen for ScreenInstance {
    fn activate(&mut self) {
        match self {
            Self::Clock(screen) => screen.activate(),
        }
    }

    fn tick(&mut self) {
        match self {
            Self::Clock(screen) => screen.tick(),
        }
    }

    fn deactivate(&mut self) {
        match self {
            Self::Clock(screen) => screen.deactivate(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_module_is_rejected() {
        let entry = ScreenEntry {
            module: "missing".into(),
            duration_seconds: 10,
            settings: toml::Table::new(),
        };
        assert!(ScreenInstance::build(&entry).is_err());
    }

    #[test]
    fn invalid_clock_format_is_rejected() {
        let entry: ScreenEntry =
            toml::from_str("module='clock'\nduration_seconds=10\n[settings]\nformat='invalid'")
                .unwrap();
        assert!(ScreenInstance::build(&entry).is_err());
    }
}
