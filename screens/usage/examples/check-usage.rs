//! Print only quota percentages or a sanitized error, never authentication data.
use screen_api::Screen;
use screen_usage::{UsageScreen, UsageSettings};
use std::{
    thread,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut screen = UsageScreen::new(UsageSettings::default())?;
    screen.activate();
    let deadline = Instant::now() + Duration::from_secs(12);
    while Instant::now() < deadline {
        screen.tick();
        if let Some(usage) = screen.usage() {
            println!(
                "5-hour used: {:?}%; weekly used: {:?}%",
                usage.five_hour, usage.weekly
            );
            return Ok(());
        }
        if screen.message() != "Loading" {
            return Err(screen.message().into());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("Usage request timed out".into())
}
