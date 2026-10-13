//! Read-only Trading 212 account value. No trading operations or relay.

use ansi_renderer::{Glyph, GlyphLibrary};
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

const ENDPOINT: &str = "https://live.trading212.com/api/v0/equity/account/summary";

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PortfolioSettings {
    pub refresh_seconds: u64,
    pub key_env: String,
    pub secret_env: String,
    /// Optional private dotenv file. Process environment takes precedence.
    pub env_file: Option<std::path::PathBuf>,
    /// Explicit sample data for previews; never reads credentials or makes requests.
    pub demo: bool,
}

impl Default for PortfolioSettings {
    fn default() -> Self {
        Self {
            refresh_seconds: 60,
            key_env: "TRADING212_API_KEY".into(),
            secret_env: "TRADING212_API_SECRET".into(),
            env_file: None,
            demo: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PortfolioError {
    #[error("refresh_seconds must be between 30 and 3600")]
    InvalidRefresh,
    #[error("key_env and secret_env must name environment variables")]
    InvalidEnvironment,
    #[error("Check Trading 212 read-only key and secret")]
    Authentication,
    #[error("Trading 212 access denied; check account-read permission and IP restrictions")]
    Forbidden,
    #[error("Trading 212 unavailable")]
    Unavailable,
    #[error("Trading 212 connection failed; check network and TLS certificates")]
    Connection,
    #[error("Trading 212 request timed out")]
    Timeout,
    #[error("Trading 212 rate limited")]
    RateLimited,
    #[error("Invalid Trading 212 account summary")]
    InvalidResponse,
}

#[derive(Deserialize)]
struct Summary {
    currency: String,
    #[serde(rename = "totalValue")]
    total_value: f64,
}

#[derive(Debug, PartialEq)]
pub struct PortfolioValue {
    pub amount: String,
    pub currency: String,
}

pub fn parse_summary(bytes: &[u8]) -> Result<PortfolioValue, PortfolioError> {
    let summary: Summary =
        serde_json::from_slice(bytes).map_err(|_| PortfolioError::InvalidResponse)?;
    if summary.currency.len() != 3
        || !summary.currency.bytes().all(|b| b.is_ascii_uppercase())
        || !summary.total_value.is_finite()
        || !(0.0..=1_000_000_000_000.0).contains(&summary.total_value)
    {
        return Err(PortfolioError::InvalidResponse);
    }
    let decimals = match summary.currency.as_str() {
        "JPY" | "KRW" | "VND" | "CLP" | "ISK" => 0,
        "BHD" | "KWD" | "JOD" | "OMR" | "TND" => 3,
        _ => 2,
    };
    let raw = format!("{:.*}", decimals, summary.total_value);
    let (integer, fraction) = raw
        .split_once('.')
        .map_or((raw.as_str(), None), |(i, f)| (i, Some(f)));
    let mut amount = String::new();
    for (index, ch) in integer.chars().enumerate() {
        if index > 0 && (integer.len() - index).is_multiple_of(3) {
            amount.push(',');
        }
        amount.push(ch);
    }
    if let Some(fraction) = fraction {
        amount.push('.');
        amount.push_str(fraction);
    }
    Ok(PortfolioValue {
        amount,
        currency: summary.currency,
    })
}

fn read_credential(
    name: &str,
    file: &std::collections::HashMap<String, String>,
) -> Result<String, PortfolioError> {
    std::env::var(name)
        .ok()
        .or_else(|| file.get(name).cloned())
        .filter(|value| !value.trim().is_empty())
        .ok_or(PortfolioError::Authentication)
}

fn fetch(settings: &PortfolioSettings) -> Result<PortfolioValue, PortfolioError> {
    let mut values = std::collections::HashMap::new();
    if let (Ok(key), Ok(secret)) = (
        read_credential(&settings.key_env, &values),
        read_credential(&settings.secret_env, &values),
    ) {
        return fetch_from(ENDPOINT, &key, &secret);
    }
    if let Some(path) = &settings.env_file {
        let file = std::fs::File::open(path).map_err(|_| PortfolioError::Authentication)?;
        for entry in dotenvy::from_read_iter(file) {
            let (name, value) = entry.map_err(|_| PortfolioError::Authentication)?;
            values.insert(name, value);
        }
    }
    let key = read_credential(&settings.key_env, &values)?;
    let secret = read_credential(&settings.secret_env, &values)?;
    fetch_from(ENDPOINT, &key, &secret)
}

// URL is fixed in production. Tests use an ephemeral loopback server.
fn fetch_from(endpoint: &str, key: &str, secret: &str) -> Result<PortfolioValue, PortfolioError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| PortfolioError::Unavailable)?;
    let response = client
        .get(endpoint)
        .basic_auth(key, Some(secret))
        .header("User-Agent", "pi-display")
        .send()
        .map_err(|error| {
            if error.is_timeout() {
                PortfolioError::Timeout
            } else if error.is_connect() {
                PortfolioError::Connection
            } else {
                PortfolioError::Unavailable
            }
        })?;
    match response.status().as_u16() {
        200 => {}
        401 => return Err(PortfolioError::Authentication),
        403 => return Err(PortfolioError::Forbidden),
        429 => return Err(PortfolioError::RateLimited),
        _ => return Err(PortfolioError::Unavailable),
    }
    let mut bytes = Vec::new();
    response
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| PortfolioError::Unavailable)?;
    if bytes.len() > 65536 {
        return Err(PortfolioError::InvalidResponse);
    }
    parse_summary(&bytes)
}

pub struct PortfolioScreen {
    settings: PortfolioSettings,
    active: bool,
    value: Option<PortfolioValue>,
    message: &'static str,
    pending: Option<Receiver<Result<PortfolioValue, PortfolioError>>>,
    next_refresh: Instant,
    refresh_interval: Duration,
    request_started: Instant,
}

impl PortfolioScreen {
    /// One read-only request for connection diagnostics, outside the GUI thread.
    pub fn check_connection(
        settings: &PortfolioSettings,
    ) -> Result<PortfolioValue, PortfolioError> {
        fetch(settings)
    }
    pub fn new(settings: PortfolioSettings) -> Result<Self, PortfolioError> {
        if !(30..=3600).contains(&settings.refresh_seconds) {
            return Err(PortfolioError::InvalidRefresh);
        }
        for name in [&settings.key_env, &settings.secret_env] {
            if name.is_empty()
                || !name.chars().enumerate().all(|(i, ch)| {
                    ch == '_' || ch.is_ascii_alphabetic() || (i > 0 && ch.is_ascii_digit())
                })
            {
                return Err(PortfolioError::InvalidEnvironment);
            }
        }
        let value = if settings.demo {
            Some(parse_summary(
                br#"{"currency":"GBP","totalValue":12345.67}"#,
            )?)
        } else {
            None
        };
        let message = if value.is_some() { "" } else { "Loading" };
        let now = Instant::now();
        let refresh_interval = Duration::from_secs(settings.refresh_seconds);
        let next_refresh = if settings.demo {
            now + refresh_interval
        } else {
            now
        };
        Ok(Self {
            settings,
            active: false,
            value,
            message,
            pending: None,
            next_refresh,
            refresh_interval,
            request_started: now,
        })
    }

    pub fn amount(&self) -> &str {
        self.value.as_ref().map_or("--", |v| v.amount.as_str())
    }
    pub fn currency(&self) -> &str {
        self.value.as_ref().map_or("", |v| v.currency.as_str())
    }
    pub fn message(&self) -> &str {
        self.message
    }

    pub fn display_amount(&self) -> String {
        if self.value.is_none() {
            return "--".into();
        }
        let symbol = match self.currency() {
            "GBP" => "£",
            "EUR" => "€",
            "USD" => "$",
            "JPY" => "¥",
            currency => currency,
        };
        if symbol.chars().count() == 1 {
            format!("{symbol}{}", self.amount())
        } else {
            format!("{symbol} {}", self.amount())
        }
    }

    pub fn refresh_indicator(&self) -> RefreshIndicator {
        self.refresh_indicator_at(Instant::now())
    }

    fn refresh_indicator_at(&self, now: Instant) -> RefreshIndicator {
        if self.pending.is_some() {
            RefreshIndicator::refreshing(now.saturating_duration_since(self.request_started))
        } else {
            RefreshIndicator::waiting(
                self.next_refresh.saturating_duration_since(now),
                self.refresh_interval,
            )
        }
    }

    pub fn ansi_frame(&self, columns: usize, rows: usize, glyphs: &GlyphLibrary) -> String {
        let mut output = glyphs.render_centered(&self.display_amount(), columns, rows);
        for (text, row) in [(self.message, rows.saturating_sub(4))] {
            if !text.is_empty() {
                let text: String = text.chars().take(columns).collect();
                let left = columns.saturating_sub(text.chars().count()) / 2;
                let _ = write!(output, "\x1b[97m\x1b[{};{}H{text}", row.max(1), left + 1);
            }
        }
        output.push_str("\x1b[0m");
        output.push_str(&self.refresh_indicator().ansi_bottom_right(columns, rows));
        output
    }
}

/// Keep punctuation in the selected renderer style without changing clock presets.
pub fn portfolio_glyphs(
    mut library: GlyphLibrary,
) -> Result<GlyphLibrary, ansi_renderer::RenderError> {
    library.insert('.', Glyph::new(&[" ", " ", " ", " ", " ", "#", "#"])?);
    library.insert(
        ',',
        Glyph::new(&["  ", "  ", "  ", "  ", "  ", " #", "# "])?,
    );
    library.insert(
        '£',
        Glyph::new(&[
            "  ###", " #   ", " #   ", "#### ", " #   ", " #   ", "#####",
        ])?,
    );
    library.insert(
        '€',
        Glyph::new(&[
            "  ###", " #   ", "#### ", " #   ", "#### ", " #   ", "  ###",
        ])?,
    );
    library.insert(
        '$',
        Glyph::new(&[
            "  #  ", " ####", "# #  ", " ### ", "  # #", "#### ", "  #  ",
        ])?,
    );
    library.insert(
        '¥',
        Glyph::new(&[
            "#   #", " # # ", "  #  ", "#####", "  #  ", "#####", "  #  ",
        ])?,
    );
    library.insert(
        '-',
        Glyph::new(&["   ", "   ", "   ", "###", "   ", "   ", "   "])?,
    );
    Ok(library)
}

impl Screen for PortfolioScreen {
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
                self.next_refresh = Instant::now() + self.refresh_interval;
            }
            return;
        }
        if let Some(receiver) = &self.pending {
            let result = match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(PortfolioError::Unavailable)),
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                let mut delay = self.settings.refresh_seconds;
                match result {
                    Ok(value) => {
                        self.value = Some(value);
                        self.message = "";
                    }
                    Err(error) => {
                        self.value = None;
                        self.message = match error {
                            PortfolioError::Authentication => {
                                "Check Trading 212 read-only key and secret"
                            }
                            PortfolioError::Forbidden => "Trading 212 access denied",
                            PortfolioError::RateLimited => {
                                delay = delay.max(300);
                                "Trading 212 rate limited"
                            }
                            _ => "Trading 212 unavailable",
                        };
                    }
                }
                self.pending = None;
                self.refresh_interval = Duration::from_secs(delay);
                self.next_refresh = Instant::now() + Duration::from_secs(delay);
            }
        }
        if self.pending.is_none() && Instant::now() >= self.next_refresh {
            self.request_started = Instant::now();
            let (sender, receiver) = mpsc::channel();
            let settings = self.settings.clone();
            match thread::Builder::new()
                .name("trading212-value".into())
                .spawn(move || {
                    let _ = sender.send(fetch(&settings));
                }) {
                Ok(_) => self.pending = Some(receiver),
                Err(_) => {
                    self.value = None;
                    self.message = "Trading 212 unavailable";
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
    fn mock(status: &str, body: &str) -> Result<PortfolioValue, PortfolioError> {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!(
            "http://{}/api/v0/equity/account/summary",
            listener.local_addr().unwrap()
        );
        let status = status.to_owned();
        let body = body.to_owned();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0);
                request.extend_from_slice(&buffer[..count]);
            }
            let request = String::from_utf8(request).unwrap();
            assert!(request.starts_with("GET /api/v0/equity/account/summary HTTP/1.1"));
            // Basic base64(key:secret). Test credentials only.
            assert!(request.contains("a2V5OnNlY3JldA=="));
            write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nLocation: https://example.invalid\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        });
        let result = fetch_from(&endpoint, "key", "secret");
        worker.join().unwrap();
        result
    }
    #[test]
    fn sends_basic_auth_and_parses_account_summary() {
        assert_eq!(
            mock("200 OK", r#"{"currency":"GBP","totalValue":12345.67}"#)
                .unwrap()
                .amount,
            "12,345.67"
        );
    }
    #[test]
    fn rejects_bad_credentials_without_leaking_response() {
        assert!(matches!(
            mock("401 Unauthorized", "secret"),
            Err(PortfolioError::Authentication)
        ));
    }
    #[test]
    fn refuses_to_forward_credentials_on_redirect() {
        assert!(matches!(
            mock("302 Found", ""),
            Err(PortfolioError::Unavailable)
        ));
    }

    #[test]
    fn distinguishes_forbidden_access_from_bad_credentials() {
        assert!(matches!(
            mock("403 Forbidden", ""),
            Err(PortfolioError::Forbidden)
        ));
    }
    #[test]
    fn handles_rate_limit_as_distinct_error() {
        assert!(matches!(
            mock("429 Too Many Requests", ""),
            Err(PortfolioError::RateLimited)
        ));
    }
    #[test]
    fn response_size_is_bounded() {
        assert!(matches!(
            mock("200 OK", &" ".repeat(65537)),
            Err(PortfolioError::InvalidResponse)
        ));
    }
    #[test]
    fn activating_for_validation_does_not_start_network() {
        let mut screen = PortfolioScreen::new(Default::default()).unwrap();
        screen.activate();
        assert!(screen.pending.is_none());
    }
    #[test]
    fn offline_demo_does_not_start_network() {
        let mut screen = PortfolioScreen::new(PortfolioSettings {
            demo: true,
            ..Default::default()
        })
        .unwrap();
        screen.activate();
        screen.tick();
        assert!(screen.pending.is_none());
    }
    #[test]
    fn failure_clears_old_account_value() {
        let mut screen = PortfolioScreen::new(Default::default()).unwrap();
        screen.value = Some(parse_summary(br#"{"currency":"GBP","totalValue":123}"#).unwrap());
        let (sender, receiver) = mpsc::channel();
        sender.send(Err(PortfolioError::Authentication)).unwrap();
        screen.pending = Some(receiver);
        screen.activate();
        screen.tick();
        assert_eq!(screen.amount(), "--");
    }
    #[test]
    fn rate_limited_refresh_backs_off_for_five_minutes() {
        let mut screen = PortfolioScreen::new(Default::default()).unwrap();
        let (sender, receiver) = mpsc::channel();
        sender.send(Err(PortfolioError::RateLimited)).unwrap();
        screen.pending = Some(receiver);
        screen.activate();
        screen.tick();
        assert!(screen.next_refresh.duration_since(Instant::now()) > Duration::from_secs(299));
    }

    #[test]
    fn indicator_tracks_the_actual_retry_interval() {
        let mut screen = PortfolioScreen::new(Default::default()).unwrap();
        let now = Instant::now();
        screen.refresh_interval = Duration::from_secs(300);
        screen.next_refresh = now + Duration::from_secs(150);
        assert_eq!(screen.refresh_indicator_at(now).mask, 15);
    }

    #[test]
    fn indicator_animates_only_during_request() {
        let mut screen = PortfolioScreen::new(Default::default()).unwrap();
        let (_, receiver) = mpsc::channel();
        screen.pending = Some(receiver);
        let now = Instant::now();
        screen.request_started = now;
        assert_eq!(
            screen
                .refresh_indicator_at(now + Duration::from_millis(450))
                .mask,
            8
        );
    }
}
