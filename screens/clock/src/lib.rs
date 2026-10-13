//! Native clock logic, independent of the player's window.

use std::time::{Duration, Instant};

use ansi_renderer::GlyphLibrary;
use chrono::{DateTime, Local, TimeZone};
use screen_api::Screen;
use serde::Deserialize;

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
    /// Duration of ANSI digit transitions. Zero disables animation.
    pub transition_ms: u16,
}

impl Default for ClockSettings {
    fn default() -> Self {
        Self {
            format: ClockFormat::TwentyFourHour,
            show_seconds: true,
            transition_ms: 320,
        }
    }
}

pub struct ClockScreen {
    settings: ClockSettings,
    active: bool,
    time: String,
    previous_time: Option<String>,
    transition_started: Instant,
}

#[derive(Debug, thiserror::Error)]
pub enum ClockError {
    #[error("clock transition_ms must be between 0 and 900 milliseconds")]
    InvalidTransitionDuration,
}

impl ClockScreen {
    pub fn new(settings: ClockSettings) -> Result<Self, ClockError> {
        if settings.transition_ms > 900 {
            return Err(ClockError::InvalidTransitionDuration);
        }
        Ok(Self {
            settings,
            active: false,
            time: String::new(),
            previous_time: None,
            transition_started: Instant::now(),
        })
    }

    pub fn time(&self) -> &str {
        &self.time
    }

    /// ANSI frame for any renderer or terminal using the same character-grid size.
    pub fn ansi_frame(&self, columns: usize, rows: usize, glyphs: &GlyphLibrary) -> String {
        self.ansi_frame_at(columns, rows, glyphs, Instant::now())
    }

    fn ansi_frame_at(
        &self,
        columns: usize,
        rows: usize,
        glyphs: &GlyphLibrary,
        now: Instant,
    ) -> String {
        if let Some(previous) = &self.previous_time {
            let duration = Duration::from_millis(u64::from(self.settings.transition_ms));
            let progress =
                now.duration_since(self.transition_started).as_secs_f32() / duration.as_secs_f32();
            glyphs.render_rolling(previous, &self.time, progress, columns, rows)
        } else {
            glyphs.render_centered(&self.time, columns, rows)
        }
    }

    fn update_time(&mut self, time: String, now: Instant) {
        if self.time != time {
            let previous = std::mem::replace(&mut self.time, time);
            self.previous_time =
                (!previous.is_empty() && self.settings.transition_ms > 0).then_some(previous);
            self.transition_started = now;
        }
        if now.duration_since(self.transition_started)
            >= Duration::from_millis(u64::from(self.settings.transition_ms))
        {
            self.previous_time = None;
        }
    }
}

impl Screen for ClockScreen {
    fn activate(&mut self) {
        self.active = true;
        self.time = format_time(&Local::now(), self.settings);
        self.previous_time = None;
    }

    fn tick(&mut self) {
        if self.active {
            self.update_time(format_time(&Local::now(), self.settings), Instant::now());
        }
    }

    fn deactivate(&mut self) {
        self.active = false;
        self.previous_time = None;
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
            ..ClockSettings::default()
        };
        assert_eq!(format_time(&now, settings), "12:04 AM");
    }

    #[test]
    fn hidden_clock_does_not_update() {
        let mut clock = ClockScreen::new(ClockSettings::default()).unwrap();
        clock.tick();
        assert_eq!(clock.time(), "");
    }

    #[test]
    fn activating_clock_populates_time_immediately() {
        let mut clock = ClockScreen::new(ClockSettings::default()).unwrap();
        clock.activate();
        assert!(!clock.time().is_empty());
    }

    #[test]
    fn first_visible_time_does_not_animate() {
        let mut clock = ClockScreen::new(ClockSettings::default()).unwrap();
        clock.activate();
        assert!(clock.previous_time.is_none());
    }

    #[test]
    fn changed_time_starts_a_transition() {
        let mut clock = ClockScreen::new(ClockSettings::default()).unwrap();
        let now = Instant::now();
        clock.update_time("12:59:59".into(), now);
        clock.update_time("13:00:00".into(), now + Duration::from_secs(1));
        assert_eq!(clock.previous_time.as_deref(), Some("12:59:59"));
    }

    #[test]
    fn animation_produces_a_distinct_middle_frame() {
        let mut clock = ClockScreen::new(ClockSettings::default()).unwrap();
        let glyphs = ansi_renderer::GlyphLibrary::preset(ansi_renderer::GlyphSet::Ascii);
        let now = Instant::now();
        clock.update_time("12:59:59".into(), now);
        clock.update_time("13:00:00".into(), now + Duration::from_secs(1));
        let start = clock.ansi_frame_at(60, 40, &glyphs, now + Duration::from_secs(1));
        let middle = clock.ansi_frame_at(60, 40, &glyphs, now + Duration::from_millis(1160));
        let end = clock.ansi_frame_at(60, 40, &glyphs, now + Duration::from_millis(1320));
        assert!(middle != start && middle != end);
    }

    #[test]
    fn completed_transition_is_removed() {
        let mut clock = ClockScreen::new(ClockSettings::default()).unwrap();
        let now = Instant::now();
        clock.update_time("12:34:56".into(), now);
        clock.update_time("12:34:57".into(), now + Duration::from_secs(1));
        clock.update_time("12:34:57".into(), now + Duration::from_millis(1320));
        assert!(clock.previous_time.is_none());
    }

    #[test]
    fn zero_duration_disables_animation() {
        let settings = ClockSettings {
            transition_ms: 0,
            ..ClockSettings::default()
        };
        let mut clock = ClockScreen::new(settings).unwrap();
        let now = Instant::now();
        clock.update_time("12:34:56".into(), now);
        clock.update_time("12:34:57".into(), now + Duration::from_secs(1));
        assert!(clock.previous_time.is_none());
    }

    #[test]
    fn excessive_transition_duration_is_rejected() {
        let settings = ClockSettings {
            transition_ms: 1000,
            ..ClockSettings::default()
        };
        assert!(ClockScreen::new(settings).is_err());
    }

    #[test]
    fn hiding_clock_clears_transition_state() {
        let mut clock = ClockScreen::new(ClockSettings::default()).unwrap();
        let now = Instant::now();
        clock.update_time("12:34:56".into(), now);
        clock.update_time("12:34:57".into(), now + Duration::from_secs(1));
        clock.deactivate();
        assert!(clock.previous_time.is_none());
    }
}
