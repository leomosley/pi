//! Two Codex subscription usage bars. Network work never runs on the UI thread.

use screen_api::Screen;
use serde::Deserialize;
use std::{
    fmt::Write,
    fs,
    io::Read,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

const ENDPOINT: &str = "https://chatgpt.com/backend-api/wham/usage";

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UsageSettings {
    pub refresh_seconds: u64,
    /// Optional local Codex auth.json or OpenCode auth.json. Never a token.
    pub auth_file: Option<PathBuf>,
    /// Offline preview only. Does not load credentials or make requests.
    pub demo: bool,
}

impl Default for UsageSettings {
    fn default() -> Self {
        Self {
            refresh_seconds: 60,
            auth_file: None,
            demo: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UsageError {
    #[error("refresh_seconds must be between 30 and 3600")]
    InvalidRefresh,
    #[error("Sign in with ChatGPT in OpenCode or Codex")]
    Authentication,
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
    rate_limit: Option<Limits>,
}
#[derive(Deserialize)]
struct Limits {
    primary_window: Option<Window>,
    secondary_window: Option<Window>,
}
#[derive(Deserialize)]
struct Window {
    used_percent: f64,
    limit_window_seconds: u64,
}

pub fn parse_usage(bytes: &[u8]) -> Result<Usage, UsageError> {
    let payload: Payload =
        serde_json::from_slice(bytes).map_err(|_| UsageError::InvalidResponse)?;
    let mut usage = Usage {
        five_hour: None,
        weekly: None,
    };
    if let Some(limits) = payload.rate_limit {
        for window in [limits.primary_window, limits.secondary_window]
            .into_iter()
            .flatten()
        {
            if !window.used_percent.is_finite() || !(0.0..=100.0).contains(&window.used_percent) {
                return Err(UsageError::InvalidResponse);
            }
            match window.limit_window_seconds {
                18000 => usage.five_hour = Some(window.used_percent),
                604800 => usage.weekly = Some(window.used_percent),
                _ => {}
            }
        }
    }
    Ok(usage)
}

// Deliberately no Debug implementation: tokens must never appear in diagnostics.
struct Credentials {
    access: String,
    account: Option<String>,
}

fn parse_credentials(bytes: &[u8]) -> Result<Credentials, UsageError> {
    let data: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| UsageError::Authentication)?;
    let (access, account) = if let Some(tokens) = data.get("tokens") {
        (tokens.get("access_token"), tokens.get("account_id"))
    } else {
        let openai = data.get("openai").ok_or(UsageError::Authentication)?;
        if openai.get("type").and_then(|v| v.as_str()) != Some("oauth") {
            return Err(UsageError::Authentication);
        }
        (openai.get("access"), openai.get("accountId"))
    };
    let access = access
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or(UsageError::Authentication)?
        .to_owned();
    Ok(Credentials {
        access,
        account: account.and_then(|v| v.as_str()).map(str::to_owned),
    })
}

fn credentials(settings: &UsageSettings) -> Result<Credentials, UsageError> {
    let paths = if let Some(path) = &settings.auth_file {
        vec![path.clone()]
    } else {
        let home = PathBuf::from(std::env::var_os("HOME").ok_or(UsageError::Authentication)?);
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"));
        let codex = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex"));
        vec![data.join("opencode/auth.json"), codex.join("auth.json")]
    };
    for path in paths {
        if let Ok(bytes) = fs::read(path)
            && let Ok(credentials) = parse_credentials(&bytes)
        {
            return Ok(credentials);
        }
    }
    Err(UsageError::Authentication)
}

fn fetch(settings: &UsageSettings) -> Result<Usage, UsageError> {
    let auth = credentials(settings)?;
    // Fixed HTTPS host, no redirects, no credential writes or token refresh races.
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| UsageError::Unavailable)?;
    let mut request = client
        .get(ENDPOINT)
        .bearer_auth(&auth.access)
        .header("User-Agent", "codex-cli");
    if let Some(account) = auth.account {
        request = request.header("ChatGPT-Account-Id", account);
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
                        self.message = "Sign in with ChatGPT in OpenCode or Codex";
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
                .name("codex-usage".into())
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
        serde_json::to_vec(&json!({"rate_limit": {
            "primary_window": {"used_percent": primary, "limit_window_seconds": 18000},
            "secondary_window": {"used_percent": secondary, "limit_window_seconds": 604800}
        }}))
        .unwrap()
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
    fn maps_windows_by_duration_not_position() {
        let bytes = br#"{"rate_limit":{"secondary_window":{"used_percent":40,"limit_window_seconds":18000}}}"#;
        assert_eq!(parse_usage(bytes).unwrap().five_hour, Some(40.0));
    }
    #[test]
    fn absent_limits_are_unknown_not_zero() {
        assert_eq!(
            parse_usage(br#"{"rate_limit":null}"#).unwrap(),
            Usage {
                five_hour: None,
                weekly: None
            }
        );
    }
    #[test]
    fn unrelated_window_is_not_mislabeled() {
        let bytes =
            br#"{"rate_limit":{"primary_window":{"used_percent":10,"limit_window_seconds":3600}}}"#;
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
    fn reads_codex_credentials() {
        let auth =
            parse_credentials(br#"{"tokens":{"access_token":"test","account_id":"workspace"}}"#)
                .unwrap();
        assert_eq!(auth.account.as_deref(), Some("workspace"));
    }
    #[test]
    fn reads_opencode_oauth_credentials() {
        let auth = parse_credentials(
            br#"{"openai":{"type":"oauth","access":"test","accountId":"workspace"}}"#,
        )
        .unwrap();
        assert_eq!(auth.access, "test");
    }
    #[test]
    fn rejects_api_key_auth() {
        assert!(parse_credentials(br#"{"openai":{"type":"api","key":"test"}}"#).is_err());
    }
    #[test]
    fn credential_errors_do_not_include_input() {
        let err = parse_credentials(b"very-secret-token").err().unwrap();
        assert!(!err.to_string().contains("very-secret-token"));
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
