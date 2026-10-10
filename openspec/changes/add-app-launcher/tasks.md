## 1. Toolchain & dev loop

- [x] 1.1 `git init`, then `cargo init --name termdeck` with `.gitignore` for `target/`. Verify that `cargo build` succeeds on the laptop.
- [x] 1.2 Install the cross toolchain (`rustup target add aarch64-unknown-linux-musl`, `zig`, `cargo install cargo-zigbuild`). Verify that `cargo zigbuild --release --target aarch64-unknown-linux-musl` produces a static aarch64 ELF (`file` reports "statically linked").
- [x] 1.3 Write `scripts/dev.sh` (build → scp to `~/bin/termdeck` → restart in phone tmux session `termdeck`; phone address from `TERMDECK_PHONE`, default `-p 8022 u0_a339@192.168.95.171`). Verify that a hello-world binary prints on the phone when run through `ssh ... tmux capture-pane -p -t termdeck`.

## 2. Process helper & parsers

- [x] 2.1 Implement `run_capture` (stdin null, stdout/stderr piped). Verify on the phone that running `pm list packages` through it from inside a pty session returns packages, not `Failed transaction`.
- [ ] 2.2 Capture fixtures from the Pixel 5a (`pm query-activities ...`, `pm path`, `aapt dump badging` for Chrome, `termux-am` outputs for: success, task brought to front, missing overlay permission). Verify that the files exist under `tests/fixtures/`.
- [x] 2.3 Implement pure parsers: query-activities → `(pkg, activity)` (first per package), `pm path` → base.apk, badging → label, package-name label fallback. Verify that unit tests pass against the fixtures.
- [x] 2.4 Implement the launch-output classifier (Success / BroughtToFront / NeedsOverlayPermission / Failed(msg)). Verify with unit tests on the three termux-am fixtures.

## 3. Index & storage

- [x] 3.1 Implement the XDG-aware paths and atomic JSON write (temp + rename) for `index.json` and `state.json`. Verify with a unit test that round-trips both files in a temp dir.
- [x] 3.2 Implement the index build with a 4-thread worker pool and incremental reuse by APK mtime. Verify on the phone: a cold build lists ~96 apps with labels; a second run reads 0 labels from aapt (log the count).
- [x] 3.3 Implement the background refresh worker sending `Updated`/`Failed` over an mpsc channel, which keeps the old index on failure. Verify with a unit test that uses a fake discovery function returning an error.

## 4. UI

- [x] 4.1 Spike: a minimal ratatui app with `EnableMouseCapture` that prints received mouse events. Verify on the phone by tapping and swiping manually, and confirm that Termux delivers `Down(Left)` with cell coordinates and `ScrollUp/Down`. Record the result in design.md if it differs.
- [x] 4.2 Implement the `App` state + event loop (input poll ~100 ms, drain index channel, Ctrl+C quit, Esc semantics). Verify that the binary starts, renders, and quits cleanly on the phone with the terminal restored.
- [x] 4.3 Render the home screen: the pinned grid (square, filled, 3 rows tall, columns = `max(2, width/20)`, truncated labels), the empty-pins hint, the recent list (≤8, pinned excluded, uninstalled hidden), and the search prompt. Verify by `capture-pane` at ~45 columns.
- [x] 4.4 Implement search mode: type to enter, fuzzy match on label+pkg with `nucleo-matcher`, Up/Down selection, Enter launches the selection, Esc/empty query returns home. Verify that typing `yt` via `tmux send-keys` lists the YT/YouTube Morphe apps first.
- [x] 4.5 Implement hit targets recorded during render; tap launches, the pin toggle pins or unpins without launching, swipe scrolls lists; Ctrl+P toggles the pin on the selection; Ctrl+R refreshes. Verify by tapping on the device (pin an app from search, see it in the grid).

## 5. Launch & persistence wiring

- [ ] 5.1 Wire launch: call `termux-am start -n`, classify the output, show a status message; on success update recent and return to home with the query cleared; on failure leave recent unchanged; if the activity is missing, start a refresh. Verify by launching Chrome from termdeck, and seeing the overlay message after revoking the permission.
- [x] 5.2 Persist pins and recent on every change; restore on start. Verify by pinning Chrome, quitting, restarting, and seeing that Chrome is still pinned.
- [x] 5.3 Measure launch latency inside termdeck (time from Enter to the classifier result, shown in a debug status). Verify that it is under 150 ms for a warm app on the Pixel 5a.

## 6. Finish

- [ ] 6.1 Write a README with install instructions (binary to `~/bin`, `pkg install aapt termux-am-socket`, grant the overlay permission) and the dev loop. Verify by following it on a clean Termux home dir.
- [x] 6.2 Run `cargo clippy -- -D warnings` and `cargo test`. Verify that both pass.
