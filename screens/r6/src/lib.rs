//! r6fetch's public ANSI response, adapted to a small native display.
use ansi_renderer::{Cell, Terminal};
use screen_api::Screen;
use screen_widgets::RefreshIndicator;
use serde::Deserialize;
use std::{
    fmt::Write,
    io::Read,
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

const LABELS: [&str; 11] = [
    "Level",
    "Current Rank",
    "Season Peak",
    "All-Time Peak",
    "K/D",
    "Win Rate",
    "Kills",
    "Deaths",
    "Wins / Losses",
    "Headshot %",
    "Top Operator",
];

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct R6Settings {
    pub platform: String,
    pub username: String,
    pub refresh_seconds: u64,
    pub demo: bool,
}
impl Default for R6Settings {
    fn default() -> Self {
        Self {
            platform: "pc".into(),
            username: "mosly".into(),
            refresh_seconds: 300,
            demo: false,
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum R6Error {
    #[error(
        "r6 platform must be pc, ps, or xbox; username must have 1-100 characters and no controls"
    )]
    InvalidPlayer,
    #[error("r6 refresh_seconds must be between 60 and 3600")]
    InvalidRefresh,
    #[error("r6fetch unavailable")]
    Unavailable,
    #[error("r6fetch player not found")]
    NotFound,
    #[error("Invalid r6fetch ANSI response")]
    InvalidResponse,
}

pub struct Profile {
    art: Vec<Vec<Cell>>,
    stats: Vec<(&'static str, String)>,
}

impl Profile {
    fn display_stats(&self) -> [(&'static str, &str); 4] {
        let value = |label| {
            self.stats
                .iter()
                .find(|(name, _)| *name == label)
                .map_or("--", |(_, value)| value.as_str())
        };
        let rp = value("Current Rank")
            .split('/')
            .map(str::trim)
            .find(|part| part.ends_with(" RP"))
            .map_or("--", |part| part.trim_end_matches(" RP"));
        [
            ("RP", rp),
            ("K/D", value("K/D")),
            ("Win rate", value("Win Rate")),
            ("Headshot", value("Headshot %")),
        ]
    }
}

fn is_braille(ch: char) -> bool {
    ('\u{2800}'..='\u{28ff}').contains(&ch)
}

/// Parse into bounded cells first. Only extracted text and emblem colours are
/// composed into the final frame; remote cursor/OSC commands are never replayed.
pub fn parse_profile(source: &[u8]) -> Result<Profile, R6Error> {
    if source.len() > 262144 {
        return Err(R6Error::InvalidResponse);
    }
    let source = std::str::from_utf8(source).map_err(|_| R6Error::InvalidResponse)?;
    let mut lines = Vec::new();
    for line in source.lines().take(65) {
        let mut terminal = Terminal::new(160, 1).map_err(|_| R6Error::InvalidResponse)?;
        terminal.feed(line.as_bytes());
        lines.push(
            (0..160)
                .filter_map(|x| terminal.cell(x, 0).copied())
                .collect::<Vec<_>>(),
        );
    }
    if lines.len() > 64 {
        return Err(R6Error::InvalidResponse);
    }
    let mut bounds: Option<(usize, usize, usize, usize)> = None;
    for (y, line) in lines.iter().enumerate() {
        for (x, cell) in line.iter().enumerate() {
            if is_braille(cell.character) {
                bounds = Some(bounds.map_or((x, y, x, y), |(left, top, right, bottom)| {
                    (left.min(x), top.min(y), right.max(x), bottom.max(y))
                }));
            }
        }
    }
    let (left, top, right, bottom) = bounds.ok_or(R6Error::InvalidResponse)?;
    if right - left + 1 > 32 || bottom - top + 1 > 24 {
        return Err(R6Error::InvalidResponse);
    }
    let art = lines[top..=bottom]
        .iter()
        .map(|line| line[left..=right].to_vec())
        .collect();
    let mut stats = Vec::new();
    for label in LABELS {
        for line in &lines {
            let text: String = line[right + 1..].iter().map(|c| c.character).collect();
            if let Some(value) = text.trim().strip_prefix(label)
                && value.starts_with(' ')
                && !value.trim().is_empty()
            {
                stats.push((label, value.trim().replace('·', "/")));
                break;
            }
        }
    }
    if !["Current Rank", "K/D", "Win Rate"]
        .iter()
        .all(|label| stats.iter().any(|(name, _)| name == label))
    {
        return Err(R6Error::InvalidResponse);
    }
    Ok(Profile { art, stats })
}

fn fetch(settings: &R6Settings) -> Result<Profile, R6Error> {
    let mut url = reqwest::Url::parse("https://r6fetch.cc").map_err(|_| R6Error::Unavailable)?;
    url.path_segments_mut()
        .map_err(|_| R6Error::InvalidPlayer)?
        .extend([settings.platform.as_str(), settings.username.as_str()]);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| R6Error::Unavailable)?;
    let response = client
        .get(url)
        .header("Accept", "text/plain")
        .header("User-Agent", "pi-display")
        .send()
        .map_err(|_| R6Error::Unavailable)?;
    match response.status().as_u16() {
        200 => {}
        404 => return Err(R6Error::NotFound),
        _ => return Err(R6Error::Unavailable),
    }
    let mut bytes = Vec::new();
    response
        .take(262145)
        .read_to_end(&mut bytes)
        .map_err(|_| R6Error::Unavailable)?;
    parse_profile(&bytes)
}

fn text_at(output: &mut String, text: &str, x: usize, y: usize, width: usize, grey: bool) {
    let text: String = text
        .chars()
        .filter(|c| !c.is_control())
        .take(width)
        .collect();
    let color = if grey { 160 } else { 230 };
    let _ = write!(
        output,
        "\x1b[{};{}H\x1b[0m\x1b[38;2;{color};{color};{color}m{text}",
        y + 1,
        x + 1
    );
}

pub struct R6Screen {
    settings: R6Settings,
    active: bool,
    profile: Option<Profile>,
    message: &'static str,
    pending: Option<Receiver<Result<Profile, R6Error>>>,
    next_refresh: Instant,
    request_started: Instant,
}
impl R6Screen {
    /// Read-only live endpoint check for diagnostics, outside the GUI thread.
    pub fn check_connection(settings: &R6Settings) -> Result<(), R6Error> {
        fetch(settings).map(|_| ())
    }
    pub fn new(settings: R6Settings) -> Result<Self, R6Error> {
        if !["pc", "ps", "xbox"].contains(&settings.platform.as_str())
            || settings.username.is_empty()
            || settings.username.chars().count() > 100
            || settings.username.chars().any(char::is_control)
        {
            return Err(R6Error::InvalidPlayer);
        }
        if !(60..=3600).contains(&settings.refresh_seconds) {
            return Err(R6Error::InvalidRefresh);
        }
        let profile = if settings.demo {
            Some(parse_profile(include_bytes!("../fixtures/mosly.ansi"))?)
        } else {
            None
        };
        let message = if profile.is_some() {
            ""
        } else {
            "Loading r6fetch"
        };
        let now = Instant::now();
        let next_refresh = if settings.demo {
            now + Duration::from_secs(settings.refresh_seconds)
        } else {
            now
        };
        Ok(Self {
            settings,
            active: false,
            profile,
            message,
            pending: None,
            next_refresh,
            request_started: now,
        })
    }

    pub fn ansi_frame(&self, columns: usize, rows: usize) -> String {
        let mut output = String::from("\x1b[0m\x1b[2J\x1b[H");
        if columns < 54 || rows < 30 {
            text_at(
                &mut output,
                "R6 needs at least 54 x 30 cells",
                0,
                rows / 2,
                columns,
                false,
            );
            return output;
        }
        if let Some(profile) = &self.profile {
            // r6fetch targets tall terminal cells. Double emblem rows when they
            // fit, preserving its aspect ratio on our square 8x8 bitmap grid.
            let repeat = if rows >= profile.art.len() * 2 + 6 {
                2
            } else {
                1
            };
            let top = (rows - profile.art.len() * repeat) / 2;
            for (y, line) in profile.art.iter().enumerate() {
                for offset in 0..repeat {
                    let _ = write!(output, "\x1b[{};3H", top + y * repeat + offset + 1);
                    for cell in line {
                        let color = cell.style.foreground;
                        let _ = write!(
                            output,
                            "\x1b[0m\x1b[38;2;{};{};{}m{}",
                            color.0, color.1, color.2, cell.character
                        );
                    }
                }
            }
            let x = 2 + profile.art[0].len() + 3;
            let width = columns.saturating_sub(x + 2);
            let mut y = rows.saturating_sub(20) / 2;
            for (label, value) in profile.display_stats() {
                text_at(&mut output, label, x, y, width, true);
                text_at(&mut output, value, x, y + 1, width, false);
                y += 5;
            }
        }
        if !self.message.is_empty() {
            text_at(&mut output, self.message, 2, rows - 4, columns - 4, true);
        }
        let now = Instant::now();
        let indicator = if self.pending.is_some() {
            RefreshIndicator::refreshing(now.saturating_duration_since(self.request_started))
        } else {
            RefreshIndicator::waiting(
                self.next_refresh.saturating_duration_since(now),
                Duration::from_secs(self.settings.refresh_seconds),
            )
        };
        output.push_str(&indicator.ansi_bottom_right(columns, rows));
        output.push_str("\x1b[0m");
        output
    }
}

impl Screen for R6Screen {
    fn activate(&mut self) {
        self.active = true;
    }
    fn deactivate(&mut self) {
        self.active = false;
    }
    fn tick(&mut self) {
        if !self.active {
            return;
        }
        if self.settings.demo {
            if Instant::now() >= self.next_refresh {
                self.next_refresh =
                    Instant::now() + Duration::from_secs(self.settings.refresh_seconds);
            }
            return;
        }
        if let Some(receiver) = &self.pending {
            let result = match receiver.try_recv() {
                Ok(value) => Some(value),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(R6Error::Unavailable)),
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                match result {
                    Ok(profile) => {
                        self.profile = Some(profile);
                        self.message = "";
                    }
                    Err(error) => {
                        self.profile = None;
                        self.message = match error {
                            R6Error::NotFound => "R6 player not found",
                            _ => "r6fetch unavailable",
                        };
                    }
                }
                self.pending = None;
                self.next_refresh =
                    Instant::now() + Duration::from_secs(self.settings.refresh_seconds);
            }
        }
        if self.pending.is_none() && Instant::now() >= self.next_refresh {
            let (sender, receiver) = mpsc::channel();
            let settings = self.settings.clone();
            self.request_started = Instant::now();
            match thread::Builder::new()
                .name("r6fetch".into())
                .spawn(move || {
                    let _ = sender.send(fetch(&settings));
                }) {
                Ok(_) => self.pending = Some(receiver),
                Err(_) => {
                    self.profile = None;
                    self.message = "r6fetch unavailable";
                    self.next_refresh =
                        Instant::now() + Duration::from_secs(self.settings.refresh_seconds);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &[u8] = include_bytes!("../fixtures/mosly.ansi");
    #[test]
    fn extracts_the_live_emblem_dimensions() {
        let profile = parse_profile(FIXTURE).unwrap();
        assert_eq!((profile.art[0].len(), profile.art.len()), (24, 16));
    }
    #[test]
    fn preserves_original_emblem_rgb_colors() {
        let profile = parse_profile(FIXTURE).unwrap();
        assert_eq!(
            profile.art[0][1].style.foreground,
            ansi_renderer::Color(29, 23, 39)
        );
    }
    #[test]
    fn extracts_current_rank_and_rp() {
        let profile = parse_profile(FIXTURE).unwrap();
        assert_eq!(
            profile
                .stats
                .iter()
                .find(|(label, _)| *label == "Current Rank")
                .unwrap()
                .1,
            "Diamond V / 4,090 RP"
        );
    }
    #[test]
    fn extracts_kd_and_win_rate() {
        let profile = parse_profile(FIXTURE).unwrap();
        assert!(
            profile.stats.contains(&("K/D", "0.82".into()))
                && profile.stats.contains(&("Win Rate", "47%".into()))
        );
    }
    #[test]
    fn rejects_html_or_missing_art() {
        assert!(parse_profile(b"<html>unavailable</html>").is_err());
    }
    #[test]
    fn rejects_invalid_utf8() {
        assert!(parse_profile(&[255]).is_err());
    }
    #[test]
    fn rejects_oversized_response() {
        assert!(parse_profile(&vec![b' '; 262145]).is_err());
    }
    #[test]
    fn rejects_excessive_lines() {
        assert!(parse_profile("\n".repeat(66).as_bytes()).is_err());
    }
    #[test]
    fn validates_platform() {
        assert!(
            R6Screen::new(R6Settings {
                platform: "wrong".into(),
                ..Default::default()
            })
            .is_err()
        );
    }
    #[test]
    fn prevents_control_sequences_in_username() {
        assert!(
            R6Screen::new(R6Settings {
                username: "mosly\x1b[0m".into(),
                ..Default::default()
            })
            .is_err()
        );
    }
    #[test]
    fn rejects_excessively_frequent_refreshes() {
        assert!(
            R6Screen::new(R6Settings {
                refresh_seconds: 1,
                ..Default::default()
            })
            .is_err()
        );
    }
    #[test]
    fn activation_does_not_fetch_during_validation() {
        let mut screen = R6Screen::new(Default::default()).unwrap();
        screen.activate();
        assert!(screen.pending.is_none());
    }
    #[test]
    fn demo_never_starts_network() {
        let mut screen = R6Screen::new(R6Settings {
            demo: true,
            ..Default::default()
        })
        .unwrap();
        screen.activate();
        screen.tick();
        assert!(screen.pending.is_none());
    }
    #[test]
    fn failure_clears_old_profile() {
        let mut screen = R6Screen::new(Default::default()).unwrap();
        screen.profile = Some(parse_profile(FIXTURE).unwrap());
        let (sender, receiver) = mpsc::channel();
        sender.send(Err(R6Error::Unavailable)).unwrap();
        screen.pending = Some(receiver);
        screen.activate();
        screen.tick();
        assert!(screen.profile.is_none());
    }
    #[test]
    fn emitted_frame_contains_readable_stats_and_braille() {
        let screen = R6Screen::new(R6Settings {
            demo: true,
            ..Default::default()
        })
        .unwrap();
        let frame = screen.ansi_frame(60, 40);
        assert!(frame.chars().any(is_braille) && frame.contains("K/D") && frame.contains("4,090"));
    }
    #[test]
    fn small_grid_shows_help_instead_of_scrolling() {
        let screen = R6Screen::new(R6Settings {
            demo: true,
            ..Default::default()
        })
        .unwrap();
        assert!(screen.ansi_frame(30, 30).contains("R6 needs at least"));
    }

    #[test]
    fn displays_only_the_four_requested_stats() {
        let profile = parse_profile(FIXTURE).unwrap();
        assert_eq!(
            profile.display_stats(),
            [
                ("RP", "4,090"),
                ("K/D", "0.82"),
                ("Win rate", "47%"),
                ("Headshot", "60.2%")
            ]
        );
    }
    #[test]
    fn frame_omits_rank_name_and_extra_statistics() {
        let screen = R6Screen::new(R6Settings {
            demo: true,
            ..Default::default()
        })
        .unwrap();
        let frame = screen.ansi_frame(60, 40);
        for removed in [
            "Diamond",
            "Level",
            "Season Peak",
            "All-Time Peak",
            "Kills",
            "Deaths",
            "Wins / Losses",
        ] {
            assert!(!frame.contains(removed));
        }
    }
    #[test]
    fn missing_headshot_is_unknown_not_zero() {
        let mut profile = parse_profile(FIXTURE).unwrap();
        profile.stats.retain(|(label, _)| *label != "Headshot %");
        assert_eq!(profile.display_stats()[3], ("Headshot", "--"));
    }
    #[test]
    fn unranked_without_rp_is_unknown_not_fake_value() {
        let mut profile = parse_profile(FIXTURE).unwrap();
        profile
            .stats
            .iter_mut()
            .find(|(label, _)| *label == "Current Rank")
            .unwrap()
            .1 = "Unranked".into();
        assert_eq!(profile.display_stats()[0], ("RP", "--"));
    }
}
