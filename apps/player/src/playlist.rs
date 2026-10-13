use std::time::Duration;

use anyhow::Result;

use crate::config::PlaylistConfig;

pub struct Playlist {
    durations: Vec<Duration>,
    index: usize,
    elapsed: Duration,
    paused: bool,
}

impl Playlist {
    pub fn new(config: &PlaylistConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            durations: config
                .screens
                .iter()
                .map(|entry| Duration::from_secs(entry.duration_seconds))
                .collect(),
            index: 0,
            elapsed: Duration::ZERO,
            paused: false,
        })
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn remaining(&self) -> Duration {
        self.durations[self.index].saturating_sub(self.elapsed)
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub fn next(&mut self) {
        self.index = (self.index + 1) % self.durations.len();
        self.elapsed = Duration::ZERO;
    }

    pub fn previous(&mut self) {
        self.index = (self.index + self.durations.len() - 1) % self.durations.len();
        self.elapsed = Duration::ZERO;
    }

    pub fn tick(&mut self, delta: Duration) -> bool {
        if self.paused {
            return false;
        }
        self.elapsed = self.elapsed.saturating_add(delta);
        if self.elapsed >= self.durations[self.index] {
            // A delayed UI tick advances once instead of running an unbounded catch-up loop.
            self.next();
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playlist() -> Playlist {
        let config = toml::from_str(
            "[[screens]]\nmodule='clock'\nduration_seconds=2\n\
             [[screens]]\nmodule='clock'\nduration_seconds=3",
        )
        .unwrap();
        Playlist::new(&config).unwrap()
    }

    #[test]
    fn advances_when_duration_expires() {
        let mut playlist = playlist();
        playlist.tick(Duration::from_secs(2));
        assert_eq!(playlist.index(), 1);
    }

    #[test]
    fn does_not_advance_before_duration_expires() {
        let mut playlist = playlist();
        assert!(!playlist.tick(Duration::from_millis(1999)));
    }

    #[test]
    fn navigation_restarts_the_selected_duration() {
        let mut playlist = playlist();
        playlist.tick(Duration::from_secs(1));
        playlist.next();
        assert_eq!(playlist.remaining(), Duration::from_secs(3));
    }

    #[test]
    fn previous_wraps_to_last_screen() {
        let mut playlist = playlist();
        playlist.previous();
        assert_eq!(playlist.index(), 1);
    }

    #[test]
    fn next_wraps_to_first_screen() {
        let mut playlist = playlist();
        playlist.next();
        playlist.next();
        assert_eq!(playlist.index(), 0);
    }

    #[test]
    fn paused_playlist_preserves_remaining_time() {
        let mut playlist = playlist();
        playlist.tick(Duration::from_secs(1));
        playlist.toggle_pause();
        playlist.tick(Duration::from_secs(10));
        assert_eq!(playlist.remaining(), Duration::from_secs(1));
    }

    #[test]
    fn resumed_playlist_advances_normally() {
        let mut playlist = playlist();
        playlist.toggle_pause();
        playlist.toggle_pause();
        assert!(playlist.tick(Duration::from_secs(2)));
    }
}
