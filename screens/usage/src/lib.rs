//! Two subscription usage bars fed by a user-owned relay.

use screen_api::Screen;
use serde::Deserialize;
use std::{
    fmt::Write,
    io::Read,
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UsageSettings {
    pub refresh_seconds: u64,
    pub endpoint: String,
    /// Name of an environment variable holding the relay's bearer key.
    #[serde(default)]
    pub token_env: Option<String>,
    /// Offline preview only. Does not load credentials or make requests.
    pub demo: bool,
}

impl Default for UsageSettings {
    fn default() -> Self {
        Self {
            refresh_seconds: 60,
            endpoint: "https://api.mosly.dev/usage".into(),
            token_env: Some("PI_USAGE_RELAY_KEY".into()),
            demo: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UsageError {
    #[error("refresh_seconds must be between 30 and 3600")]
    InvalidRefresh,
    #[error("Check usage endpoint key")]
    Authentication,
    #[error(
        "usage endpoint must be HTTPS, or HTTP on loopback, with no embedded credentials or fragment"
    )]
    InvalidEndpoint,
    #[error("token_env must name an environment variable")]
    InvalidTokenEnv,
    #[error("Usage unavailable")]
    Unavailable,
    #[error("Unsupported usage response")]
    InvalidResponse,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Usage {
    pub five_hour: Option<f64>,
    pub weekly: Option<f64>,
}

#[derive(Deserialize)]
struct Payload {
    five_hour_used_percent: Option<f64>,
    weekly_used_percent: Option<f64>,
}

pub fn parse_usage(bytes: &[u8]) -> Result<Usage, UsageError> {
    let payload: Payload =
        serde_json::from_slice(bytes).map_err(|_| UsageError::InvalidResponse)?;
    for percent in [payload.five_hour_used_percent, payload.weekly_used_percent]
        .into_iter()
        .flatten()
    {
        if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
            return Err(UsageError::InvalidResponse);
        }
    }
    Ok(Usage {
        five_hour: payload.five_hour_used_percent,
        weekly: payload.weekly_used_percent,
    })
}

fn validate_endpoint(endpoint: &str) -> Result<(), UsageError> {
    let url = reqwest::Url::parse(endpoint).map_err(|_| UsageError::InvalidEndpoint)?;
    let loopback = url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    if !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(UsageError::InvalidEndpoint);
    }
    Ok(())
}

fn fetch(settings: &UsageSettings) -> Result<Usage, UsageError> {
    let token = settings
        .token_env
        .as_ref()
        .map(|name| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .ok_or(UsageError::Authentication)
        })
        .transpose()?;
    fetch_with_token(settings, token.as_deref())
}

fn fetch_with_token(settings: &UsageSettings, token: Option<&str>) -> Result<Usage, UsageError> {
    validate_endpoint(&settings.endpoint)?;
    // Never forward the relay key to a redirect destination or log response bodies.
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| UsageError::Unavailable)?;
    let mut request = client
        .get(&settings.endpoint)
        .header("User-Agent", "pi-display");
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().map_err(|_| UsageError::Unavailable)?;
    match response.status().as_u16() {
        401 | 403 => return Err(UsageError::Authentication),
        200 => {}
        _ => return Err(UsageError::Unavailable),
    }
    let mut bytes = Vec::new();
    response
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| UsageError::Unavailable)?;
    if bytes.len() > 65536 {
        return Err(UsageError::InvalidResponse);
    }
    parse_usage(&bytes)
}

pub struct UsageScreen {
    settings: UsageSettings,
    active: bool,
    usage: Option<Usage>,
    message: &'static str,
    pending: Option<Receiver<Result<Usage, UsageError>>>,
    next_refresh: Instant,
}

impl UsageScreen {
    pub fn new(settings: UsageSettings) -> Result<Self, UsageError> {
        if !(30..=3600).contains(&settings.refresh_seconds) {
            return Err(UsageError::InvalidRefresh);
        }
        validate_endpoint(&settings.endpoint)?;
        if let Some(name) = &settings.token_env
            && (name.is_empty()
                || !name.chars().enumerate().all(|(index, ch)| {
                    ch == '_' || ch.is_ascii_alphabetic() || (index > 0 && ch.is_ascii_digit())
                }))
        {
            return Err(UsageError::InvalidTokenEnv);
        }
        let usage = settings.demo.then_some(Usage {
            five_hour: Some(37.0),
            weekly: Some(62.0),
        });
        Ok(Self {
            settings,
            active: false,
            usage,
            message: if usage.is_some() { "" } else { "Loading" },
            pending: None,
            next_refresh: Instant::now(),
        })
    }

    pub fn usage(&self) -> Option<Usage> {
        self.usage
    }
    pub fn message(&self) -> &str {
        self.message
    }

    pub fn ansi_frame(&self, columns: usize, rows: usize) -> String {
        let width = columns.saturating_sub(8).clamp(4, 44);
        let left = columns.saturating_sub(width + 2) / 2 + 1;
        let top = rows.saturating_sub(10) / 2 + 1;
        let mut output = String::from("\x1b[0m\x1b[2J\x1b[H\x1b[97m");
        for (index, label, percent) in [
            (0, "5-hour", self.usage.and_then(|u| u.five_hour)),
            (1, "Weekly", self.usage.and_then(|u| u.weekly)),
        ] {
            let value = percent.map_or_else(|| "--".into(), |p| format!("{p:.0}% used"));
            let _ = write!(
                output,
                "\x1b[{};{}H{label}  {value}\x1b[{};{}H[",
                top + index * 6,
                left,
                top + index * 6 + 2,
                left
            );
            let filled = percent.map_or(0, |p| (p / 100.0 * width as f64).round() as usize);
            output.extend(std::iter::repeat_n('#', filled));
            output.extend(std::iter::repeat_n('-', width - filled));
            output.push(']');
        }
        if !self.message.is_empty() {
            let text: String = self
                .message
                .chars()
                .take(columns.saturating_sub(2))
                .collect();
            let _ = write!(output, "\x1b[{};2H{text}", rows.saturating_sub(2).max(1));
        }
        output.push_str("\x1b[0m");
        output
    }
}

impl Screen for UsageScreen {
    fn activate(&mut self) {
        self.active = true;
    }
    fn deactivate(&mut self) {
        self.active = false;
    }
    fn tick(&mut self) {
        if !self.active || self.settings.demo {
            return;
        }
        if let Some(receiver) = &self.pending {
            let result = match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(UsageError::Unavailable)),
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                // A failed refresh must not present old numbers as current usage.
                match result {
                    Ok(usage) => {
                        self.usage = Some(usage);
                        self.message = "";
                    }
                    Err(UsageError::Authentication) => {
                        self.usage = None;
                        self.message = "Check usage endpoint key";
                    }
                    Err(_) => {
                        self.usage = None;
                        self.message = "Usage unavailable";
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
            match thread::Builder::new()
                .name("usage-relay".into())
                .spawn(move || {
                    let _ = sender.send(fetch(&settings));
                }) {
                Ok(_) => self.pending = Some(receiver),
                Err(_) => {
                    self.usage = None;
                    self.message = "Usage unavailable";
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
    use serde_json::json;

    fn payload(primary: f64, secondary: f64) -> Vec<u8> {
        serde_json::to_vec(
            &json!({"five_hour_used_percent": primary, "weekly_used_percent": secondary}),
        )
        .unwrap()
    }

    #[test]
    fn defaults_to_authenticated_mosly_api() {
        let settings = UsageSettings::default();
        assert_eq!(settings.endpoint, "https://api.mosly.dev/usage");
        assert_eq!(settings.token_env.as_deref(), Some("PI_USAGE_RELAY_KEY"));
    }
    #[test]
    fn mock_configuration_does_not_require_a_key() {
        let settings: UsageSettings = serde_json::from_value(json!({
            "endpoint": "http://127.0.0.1:8787/usage"
        }))
        .unwrap();
        assert!(settings.token_env.is_none());
    }
    #[test]
    fn parses_mosly_api_metadata_without_changing_percentages() {
        let bytes = br#"{
            "five_hour_used_percent":37,
            "weekly_used_percent":62,
            "five_hour_reset_at":null,
            "weekly_reset_at":null,
            "plan_type":"plus",
            "allowed":true,
            "limit_reached":false,
            "observed_at":"2026-10-13T19:00:00Z"
        }"#;
        assert_eq!(
            parse_usage(bytes).unwrap(),
            Usage {
                five_hour: Some(37.0),
                weekly: Some(62.0)
            }
        );
    }
    #[test]
    fn parses_percent_used_without_inverting_to_remaining() {
        assert_eq!(
            parse_usage(&payload(37.0, 62.0)).unwrap(),
            Usage {
                five_hour: Some(37.0),
                weekly: Some(62.0)
            }
        );
    }
    #[test]
    fn accepts_a_single_available_window() {
        let bytes = br#"{"five_hour_used_percent":40}"#;
        assert_eq!(parse_usage(bytes).unwrap().five_hour, Some(40.0));
    }
    #[test]
    fn absent_limits_are_unknown_not_zero() {
        assert_eq!(
            parse_usage(br#"{"five_hour_used_percent":null,"weekly_used_percent":null}"#).unwrap(),
            Usage {
                five_hour: None,
                weekly: None
            }
        );
    }
    #[test]
    fn missing_window_is_unknown() {
        let bytes = br#"{"weekly_used_percent":10}"#;
        assert_eq!(parse_usage(bytes).unwrap().five_hour, None);
    }
    #[test]
    fn rejects_out_of_range_percent() {
        assert!(parse_usage(&payload(101.0, 50.0)).is_err());
    }
    #[test]
    fn rejects_negative_percent() {
        assert!(parse_usage(&payload(-1.0, 50.0)).is_err());
    }
    #[test]
    fn rejects_invalid_json() {
        assert!(parse_usage(b"not json").is_err());
    }
    #[test]
    fn accepts_https_endpoints() {
        assert!(validate_endpoint("https://usage.example.com/usage").is_ok());
    }
    #[test]
    fn rejects_plaintext_remote_endpoints() {
        assert!(validate_endpoint("http://192.168.1.2/usage").is_err());
    }
    #[test]
    fn rejects_embedded_credentials() {
        assert!(validate_endpoint("https://secret@example.com/usage").is_err());
    }
    #[test]
    fn accepts_loopback_http_for_mock() {
        assert!(validate_endpoint("http://[::1]:8787/usage").is_ok());
    }

    fn mock_http(status: &str, body: &str, token: Option<&str>) -> Result<Usage, UsageError> {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/usage", listener.local_addr().unwrap());
        let status = status.to_owned();
        let body = body.to_owned();
        let expected_token = token.map(str::to_owned);
        let handle = thread::spawn(move || {
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
            let request = String::from_utf8(request).unwrap().to_lowercase();
            assert!(request.starts_with("get /usage http/1.1"));
            if let Some(token) = expected_token {
                assert!(
                    request.contains(&format!("authorization: bearer {}", token.to_lowercase()))
                );
            } else {
                assert!(!request.contains("authorization:"));
            }
            write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\nLocation: https://example.invalid/redirect\r\n\r\n{body}", body.len()).unwrap();
        });
        let result = fetch_with_token(
            &UsageSettings {
                endpoint,
                ..Default::default()
            },
            token,
        );
        handle.join().unwrap();
        result
    }

    #[test]
    fn fetches_relay_json_over_http() {
        let usage = mock_http(
            "200 OK",
            r#"{"five_hour_used_percent":37,"weekly_used_percent":62}"#,
            None,
        )
        .unwrap();
        assert_eq!(
            usage,
            Usage {
                five_hour: Some(37.0),
                weekly: Some(62.0)
            }
        );
    }
    #[test]
    fn sends_optional_bearer_key() {
        assert!(
            mock_http(
                "200 OK",
                r#"{"five_hour_used_percent":37}"#,
                Some("test-only-key")
            )
            .is_ok()
        );
    }
    #[test]
    fn rejected_key_returns_sanitized_error() {
        assert!(matches!(
            mock_http("401 Unauthorized", "secret in response", None),
            Err(UsageError::Authentication)
        ));
    }
    #[test]
    fn does_not_follow_redirects() {
        assert!(matches!(
            mock_http("302 Found", "", Some("test-only-key")),
            Err(UsageError::Unavailable)
        ));
    }
    #[test]
    fn oversized_response_is_rejected() {
        assert!(matches!(
            mock_http("200 OK", &" ".repeat(65537), None),
            Err(UsageError::InvalidResponse)
        ));
    }
    #[test]
    fn invalid_token_environment_name_is_rejected() {
        assert!(
            UsageScreen::new(UsageSettings {
                token_env: Some("key=value".into()),
                ..Default::default()
            })
            .is_err()
        );
    }
    #[test]
    fn rejects_aggressive_refresh_interval() {
        assert!(
            UsageScreen::new(UsageSettings {
                refresh_seconds: 1,
                ..UsageSettings::default()
            })
            .is_err()
        );
    }
    #[test]
    fn inactive_screen_does_not_start_fetch() {
        let mut screen = UsageScreen::new(UsageSettings::default()).unwrap();
        screen.tick();
        assert!(screen.pending.is_none());
    }
    #[test]
    fn activation_does_not_read_credentials_or_start_network() {
        let mut screen = UsageScreen::new(UsageSettings::default()).unwrap();
        screen.activate();
        assert!(screen.pending.is_none());
    }
    #[test]
    fn demo_never_starts_network() {
        let mut screen = UsageScreen::new(UsageSettings {
            demo: true,
            ..UsageSettings::default()
        })
        .unwrap();
        screen.activate();
        screen.tick();
        assert!(screen.pending.is_none());
    }
    #[test]
    fn failed_refresh_removes_old_percentages() {
        let mut screen = UsageScreen::new(UsageSettings::default()).unwrap();
        screen.usage = Some(Usage {
            five_hour: Some(37.0),
            weekly: Some(62.0),
        });
        let (sender, receiver) = mpsc::channel();
        sender.send(Err(UsageError::Unavailable)).unwrap();
        screen.pending = Some(receiver);
        screen.activate();
        screen.tick();
        assert_eq!(screen.usage(), None);
    }
    #[test]
    fn successful_refresh_is_not_repeated_every_tick() {
        let mut screen = UsageScreen::new(UsageSettings::default()).unwrap();
        let (sender, receiver) = mpsc::channel();
        sender
            .send(Ok(Usage {
                five_hour: Some(37.0),
                weekly: Some(62.0),
            }))
            .unwrap();
        screen.pending = Some(receiver);
        screen.activate();
        screen.tick();
        screen.tick();
        assert!(screen.pending.is_none());
    }
    #[test]
    fn ascii_bars_have_correct_fill_lengths() {
        let screen = UsageScreen::new(UsageSettings {
            demo: true,
            ..UsageSettings::default()
        })
        .unwrap();
        let frame = screen.ansi_frame(60, 40);
        assert!(frame.contains(&format!("[{}{}]", "#".repeat(16), "-".repeat(28))));
    }
    #[test]
    fn unknown_usage_displays_dashes_instead_of_fake_percentage() {
        let screen = UsageScreen::new(UsageSettings::default()).unwrap();
        let frame = screen.ansi_frame(60, 40);
        assert!(frame.contains("5-hour  --") && !frame.contains("0%"));
    }
    #[test]
    fn ansi_labels_fit_in_smallest_supported_display() {
        let screen = UsageScreen::new(UsageSettings {
            demo: true,
            ..UsageSettings::default()
        })
        .unwrap();
        let mut terminal = ansi_renderer::Terminal::new(30, 30).unwrap();
        terminal.feed(screen.ansi_frame(30, 30).as_bytes());
        let text: String = (0..30)
            .flat_map(|y| (0..30).map(move |x| (x, y)))
            .map(|(x, y)| terminal.cell(x, y).unwrap().character)
            .collect();
        assert!(text.contains("5-hour  37% used") && text.contains("Weekly  62% used"));
    }
}
