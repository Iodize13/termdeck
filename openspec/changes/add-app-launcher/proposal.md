## Why

Launching Android apps from Termux today means typing `am start -n <pkg>/<activity>` by hand. That is slow (~0.36s JVM boot per call) and you have to know component names. termdeck is a touch-friendly TUI inside Termux: it opens on pinned and recent apps, lets you search all installed apps, and launches them in under 0.1s.

## What Changes

- New Rust binary `termdeck` (ratatui + crossterm) that runs inside the Termux app.
- Build an index of launchable apps (package, launcher activity, human-readable label) from `pm` and `aapt`, cached on disk and refreshed incrementally.
- Launch the selected app through `termux-am` (Termux's socket-based am client). Detect failure by parsing its output, because the exit code is always 0.
- Home screen: pinned apps as a grid of filled, sharp-cornered buttons; a recent-apps list below; a search prompt that filters all apps as soon as the user types.
- Full touch support: tap to launch or toggle a pin, swipe to scroll. No long-press interactions, because Termux intercepts long-press.
- Pinned and recent apps persist between runs.

## Capabilities

### New Capabilities
- `app-index`: Discover launchable apps with labels, cache them, and refresh the cache.
- `app-launch`: Start an app's launcher activity and report success or failure.
- `launcher-ui`: Home screen (pinned grid, recent list, search), keyboard and touch input, pin/recent persistence, visual style.

### Modified Capabilities
<!-- none: greenfield project -->

## Impact

- New Cargo project at repo root. Dependencies: `ratatui`, `crossterm`, a fuzzy matcher (e.g. `nucleo-matcher`), `serde`/`serde_json`, `dirs`.
- Runtime dependencies on the device: Termux app ≥ 0.118 with `termux-am` (am-socket), the `aapt` package, and the "Display over other apps" permission granted to Termux.
- Writes state under `$XDG_CACHE_HOME`/`$XDG_DATA_HOME` (Termux home).
- Target: `aarch64-linux-android`, built on the device or cross-compiled on the laptop and pushed over SSH.
