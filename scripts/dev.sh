#!/usr/bin/env bash
# build -> scp to phone -> restart in tmux session `termdeck`
set -euo pipefail
cd "$(dirname "$0")/.."
PHONE="${TERMDECK_PHONE:?set TERMDECK_PHONE, e.g. '-p 8022 u0_a123@192.168.1.50'}"
TARGET=aarch64-unknown-linux-musl
BIN=target/$TARGET/release/termdeck

# shellcheck disable=SC2206
parts=($PHONE)
host="${parts[-1]}"
port=(); [[ "${parts[0]}" == "-p" ]] && port=("${parts[1]}")
ssh_opts=(); scp_opts=()
[[ ${#port[@]} -gt 0 ]] && { ssh_opts=(-p "${port[0]}"); scp_opts=(-P "${port[0]}"); }

cargo zigbuild --release --target "$TARGET"
scp -q "${scp_opts[@]}" "$BIN" "$host:bin/termdeck.new"
ssh "${ssh_opts[@]}" "$host" '
  mv ~/bin/termdeck.new ~/bin/termdeck && chmod +x ~/bin/termdeck
  tmux kill-session -t termdeck 2>/dev/null || true
  tmux new-session -d -s termdeck -x 45 -y 40 "~/bin/termdeck; echo \"[exit \$?]\"; sleep 3600"
'
echo "started. view: ssh ${ssh_opts[*]} $host tmux capture-pane -p -t termdeck"
