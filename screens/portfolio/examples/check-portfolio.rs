//! Read-only connection check. Never prints credentials or response bodies.
use screen_portfolio::{PortfolioScreen, PortfolioSettings};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = PortfolioSettings {
        key_env: "TRADING_212_KEY_ID".into(),
        secret_env: "TRADING_212_SECRET".into(),
        env_file: Some("screens/portfolio/.env".into()),
        ..Default::default()
    };
    let value = PortfolioScreen::check_connection(&settings)?;
    println!(
        "Trading 212 connection succeeded: {} {}",
        value.currency, value.amount
    );
    Ok(())
}
