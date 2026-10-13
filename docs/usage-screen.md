# Codex usage screen

Two bars show **percentage used**, labeled `5-hour` and `Weekly`. The default
playlist rotates between the clock and usage screen. No reset countdown,
account details, or additional dashboard elements appear.

```sh
cargo run -p player -- --preview --playlist config/usage.toml \
  --display profiles/displays/dfrobot-fit0820-480x320.toml
```

The default renderer uses ASCII `#` fill and `-` empty cells. Use
`--renderer native` for plain Slint percentage bars. Glyph presets apply to
character art in the clock; usage bars always use ordinary ASCII characters.

## Authentication

The screen makes a read-only request to
`https://chatgpt.com/backend-api/wham/usage`, the endpoint used by Codex.
This is an internal subscription endpoint, not a supported public API. Its
schema or availability may change. These quotas concern Codex, not ChatGPT
conversations or OpenAI API-key billing. OpenCode usage counts against them
when it uses the same ChatGPT subscription account.

On Linux it reads the existing OpenAI OAuth entry from
`$XDG_DATA_HOME/opencode/auth.json`, defaulting to
`~/.local/share/opencode/auth.json`. If unavailable, it tries
`$CODEX_HOME/auth.json`, defaulting to `~/.codex/auth.json`. It supports
OpenCode's file-based `openai` OAuth entry and Codex's `tokens` entry.
Other credential stores, including keyring-only login, are not supported.

An explicit path overrides discovery:

```toml
[[screens]]
module = "usage"
duration_seconds = 15
renderer = "ansi"

[screens.settings]
refresh_seconds = 60
auth_file = "/secure/local/auth.json"
```

Keep authentication files outside the repository. Do not put tokens in a
playlist. The player does not modify login files, refresh tokens, or print
authentication data. Renew an expired login in OpenCode or Codex. The next
refresh reads the updated file. On a Pi, sign in there or provision the
authentication file securely for the account running the player.

## Refresh and unavailable data

Requests run on a background thread with a 10-second timeout, no redirects,
and a bounded response size. Only one request per screen instance can run at
a time. The interval is 60 seconds by default and must be 30–3600 seconds.
Hidden screens do not start requests. Configuration validation does not read
credentials or start requests.

Unavailable windows display `--`, not 0%. A rejected login shows a sign-in
message. Failed refreshes clear old percentages so stale values do not look
current. Window labels follow the reported durations rather than assuming
primary always means 5 hours.

For an offline visual test, use `config/usage-demo.toml`. It deliberately uses
sample values of 37% and 62% and never loads credentials or requests data.
Tests use this mode, not your account.

Check the actual connection without opening a window:

```sh
cargo run -p screen-usage --example check-usage
```
