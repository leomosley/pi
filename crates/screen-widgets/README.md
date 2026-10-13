# Screen widgets

Reusable presentation widgets for native Slint and ANSI screens.

## Refresh indicator

An eight-segment strip drains as the next refresh approaches. While a request
is in flight, one segment moves along the strip. No numeric countdown is shown.
The widget owns no network requests or timers: screens supply the actual
remaining time and retry interval, so backoff is represented honestly.

```rust
use std::time::Duration;
use screen_widgets::RefreshIndicator;

let indicator = RefreshIndicator::waiting(Duration::from_secs(30), Duration::from_secs(60));
let overlay = indicator.ansi_bottom_right(60, 40);
let busy = RefreshIndicator::refreshing(Duration::from_millis(450));
```

For ANSI, append `ansi_bottom_right` after your screen frame. It resets colour
and restores the cursor, with one-cell margins. Tiny grids omit the widget.
For Slint, import `ui/refresh-indicator.slint` and bind its `mask` property to
the same Rust indicator's mask. Position the component within the screen's
viewport, not the preview controls. Both renderers use identical segment state.
