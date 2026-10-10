# termdeck

Touch-friendly app launcher TUI for Termux: pinned grid, recent list, fuzzy search, launch in ~50 ms via `termux-am`.

## Install (on the phone)

```sh
pkg install aapt termux-am-socket tmux   # tmux only for the dev loop
mkdir -p ~/bin && cp termdeck ~/bin/     # binary built below
```

Grant Termux **Settings > Apps > Termux > Display over other apps**, otherwise launches fail with a hint in the status line.

## Build (on the laptop)

```sh
rustup target add aarch64-unknown-linux-musl
cargo install cargo-zigbuild   # plus `zig` on PATH
cargo zigbuild --release --target aarch64-unknown-linux-musl
# -> target/aarch64-unknown-linux-musl/release/termdeck (static)
```

## Dev loop

`scripts/dev.sh` builds, copies the binary to the phone and restarts it in tmux session `termdeck`.
Phone address comes from `TERMDECK_PHONE` (default `-p 8022 u0_a339@192.168.95.171`).

```sh
scripts/dev.sh
ssh -p 8022 u0_a339@192.168.95.171 tmux capture-pane -p -t termdeck
ssh -p 8022 u0_a339@192.168.95.171 tmux send-keys -t termdeck -l yt
```

## Keys

| Key | Action |
| --- | --- |
| type | search |
| Up/Down | move selection |
| Enter | launch selection |
| Ctrl+P | pin/unpin selection |
| Ctrl+R | refresh app index |
| Esc | clear query / quit |
| Ctrl+C | quit |

Tap a row/button to launch, tap `[☆]` to pin, swipe to scroll.

State: `~/.cache/termdeck/index.json`, `~/.local/share/termdeck/state.json` (honours `XDG_*`).
