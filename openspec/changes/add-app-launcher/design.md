## Context

Greenfield repo. The target is a Pixel 5a (aarch64, Android 14) running Termux. On-device spikes (2026-10-10/11) established the constraints below. See proposal.md for motivation and the specs for requirements.

- `/system/bin/pm` and `cmd` fail with `Failed transaction (2147483646)` whenever **any** of stdin/stdout/stderr is a pty. With all three fds redirected away from the pty, `pm` works.
- `cmd activity ...` is unusable from the Termux uid (exit 255, no output, even for `help`).
- `pm query-activities` requires `--user 0`, otherwise it throws a `SecurityException`. With it, all 96 launcher activities come back in about 50 ms.
- `aapt dump badging <base.apk>` reads labels; APKs are world-readable. All 96 apps took 4.6 s with 4 in parallel, with 0 failures.
- `$PREFIX/bin/am` boots a JVM via `app_process` (about 355 ms). `termux-am` (am-socket 1.5.0, a bash client) takes about 90 ms, but needs the overlay permission, and its exit code is 0 even on failure.
- `XDG_*` variables are unset on the device.
- The laptop has Rust (x86_64 only, no NDK). The phone has no Rust installed.

## Goals / Non-Goals

**Goals:**
- A single static binary that can be copied to the phone, with no runtime dependencies other than tools Termux provides.
- A fast edit→run loop from the laptop (build there, push over SSH, drive through tmux on the phone).

**Non-Goals:**
- Being the Android HOME app, widgets, notifications, app icons.
- Multi-user or work-profile apps (user 0 only).
- Talking to the am socket directly (see Open Questions).

## Decisions

### D1: Build `aarch64-unknown-linux-musl` static binary via `cargo-zigbuild`
A static musl binary needs no dynamic loader, so bionic vs glibc does not matter. Zig provides the cross linker without installing the Android NDK.
- *Alternative: `aarch64-linux-android` with NDK.* This is the native target, but it needs a ~1 GB NDK and linker configuration.
- *Alternative: `pkg install rust` on the phone.* No cross-compiling, but slow builds on the device, which defeats the purpose of building on the laptop.
- Consequence: termdeck's own `execve` calls bypass termux-exec's `LD_PRELOAD`. This is fine because every tool we spawn uses an absolute Termux shebang or is an ELF linked for Android, and those children still inherit `LD_PRELOAD`. Spawn by name using the inherited `PATH`.

### D2: All subprocesses use non-pty stdio
Every `Command` uses `stdin(Stdio::null())` and captures stdout/stderr (`Stdio::piped()`). This avoids the pty-related binder failure, and it must hold even while the TUI owns the terminal. Wrap it in one helper (`run_capture(cmd, args) -> Result<Output>`) so no call site can inherit the tty by accident.

### D3: Indexing pipeline = `pm query-activities` + `pm path` + `aapt`
1. Run `pm query-activities --user 0 --brief -a android.intent.action.MAIN -c android.intent.category.LAUNCHER`. Parse the lines that contain `/` into `pkg/activity`, and keep the first activity per package.
2. For each package, run `pm path <pkg>`, take the first `package:` line (base.apk), and `stat` it to get the mtime.
3. Run `aapt dump badging <apk>` only when the package is new or the mtime changed, and take `application-label:'…'`. Fall back to the last segment of the package name.
4. Run steps 2–3 on a small worker pool (4 threads, which matches the spike).
- *Alternative: an `app_process` dex helper calling PackageManager.* It gives real labels in one call, but needs a Java/Kotlin build toolchain and a JVM boot (~0.3 s). Not needed now that `pm` works with non-pty fds. Keep it as a fallback if `aapt` breaks on future APK formats.
- Optimization (optional): `pm list packages -f -U` could replace the per-app `pm path` calls with one call, if the output carries paths. Verify on the device during implementation.

### D4: Launch via `termux-am start -n <component>`, parse output
Success means the output contains `Starting: Intent` and no `Error Code:` / `Exception`. A message containing `Display over other apps` maps to a fixed, friendly status message. "Activity not started, its current task has been brought to the front" counts as success.
- *Alternative: `$PREFIX/bin/am`.* Always works without the overlay permission, but is about 4x slower. Not used as an automatic fallback, because a silently slow path hides the fix. The status message tells the user how to grant the permission instead.

### D5: Storage layout (JSON, atomic writes)
- `~/.cache/termdeck/index.json`: `[{pkg, activity, label, apk_mtime}]`
- `~/.local/share/termdeck/state.json`: `{pinned: [pkg], recent: [pkg]}`
- Honor `XDG_CACHE_HOME`/`XDG_DATA_HOME` when set. Write to a temp file and then `rename`, so that killing Termux mid-write cannot corrupt state. Pins and recents store only the package name, and are joined against the index at render time. Pins for uninstalled apps are hidden but kept, so they come back if the app is reinstalled.

### D6: App architecture: single-threaded event loop + background index worker
The main thread owns the terminal and the `App` state and handles crossterm events. A worker thread runs the refresh and sends `IndexEvent::{Updated(Vec<AppEntry>), Failed(String)}` over an `mpsc` channel. The main loop polls input with a timeout (~100 ms) and drains the channel. No async runtime is needed.

### D7: UI built on ratatui + crossterm, with hit-testing from the last frame
- `EnableMouseCapture` makes Termux forward taps as `MouseEventKind::Down(Left)` and swipes as `ScrollUp/ScrollDown`.
- During render, each clickable widget records `(Rect, Action)` into a `Vec<HitTarget>`. On a mouse-down, the hit target containing the cell gets its action. This keeps layout and hit-testing consistent without duplicating geometry.
- Pinned grid: column count = `max(2, width / 20)`, button height 3, `Block` with `BorderType::Plain` (square), and a solid background with a contrasting foreground. Selection is shown by swapping or brightening colors.
- Rows (recent and search): 3 rows tall, label on the left, a filled `[★]`/`[☆]` pin toggle on the right as its own hit target. Keyboard shortcut for pinning the selected row: `Ctrl+P`. Manual refresh: `Ctrl+R`.
- Fuzzy matching: `nucleo-matcher` against `label` and `pkg`, keeping the best score.

### D8: Dev loop
`scripts/dev.sh` runs: `cargo zigbuild --release --target aarch64-unknown-linux-musl`, then `scp` to `phone:~/bin/termdeck`, then (re)starts it in a tmux session `termdeck` on the phone, so that `tmux send-keys`/`capture-pane` over SSH can drive it. Use `-p 8022 u0_a339@<phone-ip>` (currently 192.168.95.171), configurable through an env var.

## Risks / Trade-offs

- [Termux mouse forwarding may vary by version or settings] → Verify early (task 4.1) with a minimal ratatui mouse demo on the device before building the full UI.
- [A swipe may arrive as arrow keys instead of wheel events when mouse tracking is off] → Always enable mouse capture. Treat Up/Down keys as navigation anyway.
- [`aapt` may fail on split or unusual APKs] → Fall back to the package-name label (spec'd). Log the failure count in a debug status line.
- [Static musl binary ignores `LD_PRELOAD`] → All spawned tools are Termux ELF files or scripts with absolute shebangs (verified for `termux-am`). Never spawn `/usr/bin/env`-style scripts directly.
- [`pm` output format changes across Android versions] → Keep the parsers as small pure functions with unit tests on captured device output (fixtures from this Pixel 5a).
- [Termux killed in background → termdeck restarts from scratch] → Warm start from cache keeps this cheap.

## Migration Plan

None (greenfield). Install = copy the binary to `~/bin` (or `$PREFIX/bin`). Uninstall = delete the binary and the two state directories.

## Open Questions

- Talking to the am socket directly from Rust instead of through the bash `termux-am` client could cut launch latency from about 90 ms to about 10 ms. Defer until launch timing is measured inside termdeck. The protocol is internal to Termux, so it could change.
- Whether to auto-start termdeck when a Termux session opens (e.g. from `.bashrc`). User preference, decided after first use.
