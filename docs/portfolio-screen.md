# Trading 212 portfolio value

The `portfolio` screen displays one current account value with its currency
symbol, such as `£8,543.80`. There is no brand heading, day/week/lifetime delta,
or percentage return. A small eight-segment strip in the bottom-right drains
until the next refresh. During a request, one segment moves along the strip.
No numeric countdown is shown. Retry backoff uses its actual longer interval.
The shared `screen-widgets` crate supplies this indicator for both ANSI and Slint.

It makes a read-only GET directly to:

```text
https://live.trading212.com/api/v0/equity/account/summary
```

The displayed amount is the response's `totalValue`, not cost basis,
unrealised profit, or the `investments.currentValue` field. No amounts are
summed locally. Account balances are displayed in the returned currency.
This integration supports Trading 212 Invest and Stocks ISA, not CFD accounts.

## Credentials

Create a Trading 212 API key and secret with only the account-read permission.
Do not enable order placement or cancellation permissions for this display.
The player sends HTTP Basic authentication over HTTPS. It never places orders
and does not require OpenCode, Codex, or a relay.

By default the screen accepts these variables in the player's environment:

- `TRADING212_API_KEY`
- `TRADING212_API_SECRET`

Your bundled playlist is configured for the private file
`screens/portfolio/.env` with `TRADING_212_KEY_ID` and `TRADING_212_SECRET`.
The file is ignored by Git. Process environment values take precedence over
the file, and the file is read again on every refresh without modifying the
process environment. Relative paths are relative to the player's working
directory, so run the documented commands from the repository root.

Keep credentials in a private environment file outside the repository, or
your service manager's secret configuration. Do not paste them into chat,
commit them, or include them in command-line arguments. This screen reads
the environment but never stores credentials. No interactive login or
token refresh flow runs on the Pi.

```sh
cargo run -p player -- --preview --playlist config/portfolio.toml \
  --display profiles/displays/dfrobot-fit0820-480x320.toml
```

Settings accept environment variable names, never the credentials themselves:

```toml
[[screens]]
module = "portfolio"
duration_seconds = 15
renderer = "ansi" # or "native"
glyphs = "ascii"

[screens.settings]
refresh_seconds = 60
key_env = "TRADING212_API_KEY"
secret_env = "TRADING212_API_SECRET"
# Optional private dotenv file. Defaults to no file.
# env_file = "/secure/trading212.env"
```

The default playlist includes this screen after the clock and usage screen.
Missing credentials display `--` with a key-check message. Failed requests
clear old values. Refresh intervals must be 30–3600 seconds; the default is
60. A rate-limited request backs off for at least five minutes. Requests run
in a background thread, with a 10-second timeout, no redirects, and a 64 KiB
response limit. Error messages never include credentials or response bodies.
Hidden screens do not start requests; validation does not read credentials.

HTTP 401 reports a credential error. HTTP 403 reports denied access; check
the key's account-read permission and any IP restrictions. Trading 212's
access filters can also reject a request. A 403 alone does not identify which
of these caused it. The Linux client uses system TLS trust roots.

## Offline preview

```sh
cargo run -p player -- --preview --playlist config/portfolio-demo.toml \
  --display profiles/displays/dfrobot-fit0820-480x320.toml
```

This explicit demo shows £12,345.67. It is sample data, makes no network
requests, and does not read credentials. Use `--renderer native` for a plain
text amount instead of ASCII character art.

Reference: [official account-summary API](https://docs.trading212.com/api/accounts/getaccountsummary).
