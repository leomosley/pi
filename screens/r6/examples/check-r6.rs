use screen_r6::{R6Screen, R6Settings};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    R6Screen::check_connection(&R6Settings::default())?;
    println!("Fetched and parsed r6fetch.cc/pc/mosly with rank emblem, K/D, RP, and other stats");
    Ok(())
}
