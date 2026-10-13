# Usage screen

Two bars show **percentage used**, labeled `5-hour` and `Weekly`. The Pi reads
a user-owned endpoint. It does not run OpenCode or Codex, load their login files,
contact ChatGPT, or refresh OAuth tokens. You provide the real relay separately.

## Local mock

Start the mock endpoint in one terminal:

```sh
bun run mock:usage
```

Then start the screen in another:

```sh
cargo run -p player -- --preview --playlist config/usage.toml \
  --display profiles/displays/dfrobot-fit0820-480x320.toml
```

The mock binds only to `127.0.0.1:8787` and serves `GET /usage`. It reads
`config/usage-mock.json` on each request. Edit that file to change the bars on
the next refresh. These are sample values, not actual account usage. The mock
is development tooling, not a service required on the Pi.

The default playlist rotates between the clock and this endpoint-backed screen.
`config/usage-demo.toml` remains an offline visual fixture that makes no requests.

## Endpoint contract

Return HTTP 200 with JSON:

```json
{
  "five_hour_used_percent": 37,
  "weekly_used_percent": 62
}
```

Values are numbers between 0 and 100 and mean **used**, not remaining.
Missing or `null` values mean unavailable and display `--`. Extra fields are
ignored. Invalid percentages, malformed JSON, and failed requests clear the
old values rather than showing stale numbers as current. The relay should
return null or an error when its upstream data is stale.

## Connect your real endpoint

```toml
[[screens]]
module = "usage"
duration_seconds = 15
renderer = "ansi"

[screens.settings]
endpoint = "https://your-relay.example/usage"
refresh_seconds = 60
token_env = "PI_USAGE_RELAY_KEY"
```

`token_env` is optional. If configured, the player reads that environment
variable and sends `Authorization: Bearer <key>`. Provision the key outside
the repository, such as in a private service environment file. Never put it
in the URL or playlist. Missing keys and HTTP 401/403 show a key-check message.
The mock needs no key. There is no login flow on the Pi.

Remote endpoints must use HTTPS. HTTP is allowed only on loopback for local
development. Redirects and embedded URL credentials are rejected. The relay
key and response body never appear in error diagnostics.

Requests run in the background with a 10-second timeout and 64 KiB response
limit. The refresh interval must be 30–3600 seconds. Hidden screens do not
start requests. Validation does not read keys or start network requests.

The ANSI renderer uses `#` fill and `-` empty cells. `--renderer native` selects
plain Slint bars. Clock glyph presets do not affect the usage bars.

With the local mock running, check the HTTP path without opening a window:

```sh
cargo run -p screen-usage --example check-usage
```
