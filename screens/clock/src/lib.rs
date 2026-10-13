//! Native clock logic, independent of the player's window.

use chrono::{DateTime, Local, TimeZone};
use screen_api::Screen;
use serde::Deserialize;

mod ansi;

#[derive(Debug, Default, Deserialize, Clone, Copy)]
pub enum ClockFormat {
    #[default]
    #[serde(rename = "24h")]
    TwentyFourHour,
    #[serde(rename = "12h")]
    TwelveHour,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(default, deny_unknown_fields)]
pub struct ClockSettings {
    pub format: ClockFormat,
    pub show_seconds: bool,
}

impl Default for ClockSettings {
    fn default() -> Self {
        Self {
            format: ClockFormat::TwentyFourHour,
            show_seconds: true,
        }
    }
}

pub struct ClockScreen {
    settings: ClockSettings,
    active: bool,
    time: String,
}

impl ClockScreen {
    pub fn new(settings: ClockSettings) -> Self {
        Self {
            settings,
            active: false,
            time: String::new(),
        }
    }

    pub fn time(&self) -> &str {
        &self.time
    }

    /// ANSI frame for any renderer or terminal using the same character-grid size.
    pub fn ansi_frame(&self, columns: usize, rows: usize) -> String {
        ansi::clock_frame(&self.time, columns, rows)
    }
}

impl Screen for ClockScreen {
    fn activate(&mut self) {
        self.active = true;
        self.tick();
    }

    fn tick(&mut self) {
        if self.active {
            self.time = format_time(&Local::now(), self.settings);
        }
    }

    fn deactivate(&mut self) {
        self.active = false;
    }
}

pub fn format_time<T: TimeZone>(now: &DateTime<T>, settings: ClockSettings) -> String
where
    T::Offset: std::fmt::Display,
{
    let pattern = match (settings.format, settings.show_seconds) {
        (ClockFormat::TwentyFourHour, true) => "%H:%M:%S",
        (ClockFormat::TwentyFourHour, false) => "%H:%M",
        (ClockFormat::TwelveHour, true) => "%I:%M:%S %p",
        (ClockFormat::TwelveHour, false) => "%I:%M %p",
    };
    now.format(pattern).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn formats_twenty_four_hour_time_with_seconds() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 13, 4, 5).unwrap();
        assert_eq!(format_time(&now, ClockSettings::default()), "13:04:05");
    }

    #[test]
    fn formats_twelve_hour_time_without_seconds() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 4, 5).unwrap();
        let settings = ClockSettings {
            format: ClockFormat::TwelveHour,
            show_seconds: false,
        };
        assert_eq!(format_time(&now, settings), "12:04 AM");
    }

    #[test]
    fn hidden_clock_does_not_update() {
        let mut clock = ClockScreen::new(ClockSettings::default());
        clock.tick();
        assert_eq!(clock.time(), "");
    }

    #[test]
    fn activating_clock_populates_time_immediately() {
        let mut clock = ClockScreen::new(ClockSettings::default());
        clock.activate();
        assert!(!clock.time().is_empty());
    }
}
