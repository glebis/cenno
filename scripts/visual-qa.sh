#!/usr/bin/env bash
# Visual QA capture — fire one prompt of each kind at cenno and screenshot
# the panel window into docs/design/qa/qa-<state>.png for side-by-side
# comparison with docs/design/frames/final/.
#
# Usage: scripts/visual-qa.sh [state ...]    (default: all five)
# States: mood text choice scale confirm
#
# Mechanics (same stdio-bridge pattern as scripts/demo.sh):
# - launches ONE `cenno --tray` for the whole run (relaunching between
#   states races the bridge's own autolaunch against a stale socket file
#   and you end up screenshotting a blank second instance)
# - per state: fires ask_user via `cenno --mcp-stdio` in the background
#   (timeout_s 12), waits ~3s for the panel to paint, finds the panel
#   window via CGWindowList and captures it with `screencapture -l`
# - between states: waits for the prompt to time out and the window to
#   hide, so the next prompt starts from a clean panel
# - targeted `pkill -x cenno` at the end
# Requires Screen Recording permission for the terminal running this.
set -euo pipefail
cd "$(dirname "$0")/.."

BIN="${CENNO_BIN:-src-tauri/target/release/cenno}"
OUT="${CENNO_QA_DIR:-docs/design/qa}"
SOCK="$HOME/Library/Application Support/com.glebkalinin.cenno/mcp.sock"
RENDER_WAIT="${CENNO_QA_RENDER_WAIT:-3}"
TIMEOUT_S=12

[ -x "$BIN" ] || { echo "build first: npm run build && npx tauri build --no-bundle"; exit 1; }
mkdir -p "$OUT"

launch() {
  if ! pgrep -xq cenno; then
    # a stale socket file from a previous run makes both our wait loop and
    # the bridge's "is it running?" check lie — clear it (trash, never rm)
    [ -S "$SOCK" ] && trash "$SOCK" 2>/dev/null || true
    "$BIN" --tray >>/tmp/cenno_qa_app.log 2>&1 &
  fi
  for _ in $(seq 1 50); do
    if [ -S "$SOCK" ]; then
      # socket is up before the webview finishes loading dist/ — give the
      # hidden window time to finish its first load, or the first capture
      # is a white flash
      sleep "${CENNO_QA_WARMUP:-5}"
      return 0
    fi
    sleep 0.2
  done
  echo "cenno --tray did not come up (no socket at $SOCK)"; exit 1
}

ask_bg() { # $1 = ask_user arguments JSON; fired in the background
  (printf '%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"visual-qa","version":"0"}}}' \
    '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
    "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"ask_user\",\"arguments\":$1}}"; \
   sleep $((TIMEOUT_S + 3))) | "$BIN" --mcp-stdio >/dev/null 2>&1 &
}

window_id() { # prints the on-screen cenno window id, if any
  python3 - <<'PY'
import Quartz
wins = Quartz.CGWindowListCopyWindowInfo(
    Quartz.kCGWindowListOptionOnScreenOnly, Quartz.kCGNullWindowID)
for w in wins:
    if w.get("kCGWindowOwnerName") == "cenno" and w.get("kCGWindowAlpha", 0) > 0:
        print(w["kCGWindowNumber"])
        break
PY
}

wait_hidden() { # wait for the prompt to expire and the panel to hide
  for _ in $(seq 1 $((TIMEOUT_S * 2 + 10))); do
    [ -z "$(window_id)" ] && return 0
    sleep 1
  done
  echo "warning: panel still visible after timeout" >&2
}

args_for() {
  case "$1" in
    mood)    echo '{"title":"How are you feeling?","input":{"kind":"choice"},"choices":["great","good","okay","low","rough"],"flow":"mood","timeout_s":'$TIMEOUT_S'}' ;;
    text)    echo '{"title":"Quick note","body_md":"What are you working on **right now**? See [the plan](https://example.com).","timeout_s":'$TIMEOUT_S'}' ;;
    choice)  echo '{"title":"Where did this hour go?","input":{"kind":"choice"},"choices":["Deep work","Meetings","Email","Wandering"],"timeout_s":'$TIMEOUT_S'}' ;;
    scale)   echo '{"title":"How focused were you this hour?","input":{"kind":"scale"},"flow":"ema","progress":{"step":1,"total":3},"timeout_s":'$TIMEOUT_S'}' ;;
    confirm) echo '{"title":"Stand up and stretch.","input":{"kind":"confirm"},"flow":"reminder","timeout_s":'$TIMEOUT_S'}' ;;
    *) echo "unknown state: $1" >&2; return 1 ;;
  esac
}

capture() { # $1 = state
  local args wid
  args=$(args_for "$1") || exit 1
  ask_bg "$args"
  sleep "$RENDER_WAIT"
  wid=$(window_id)
  if [ -z "$wid" ]; then
    echo "FAIL $1: no visible cenno window"; return 1
  fi
  screencapture -o -x -l "$wid" "$OUT/qa-$1.png"
  echo "captured $OUT/qa-$1.png (window $wid)"
  wait_hidden
}

STATES=(mood text choice scale confirm)
[ $# -gt 0 ] && STATES=("$@")

launch
for s in "${STATES[@]}"; do capture "$s"; done
pkill -x cenno 2>/dev/null || true
echo "done — captures in $OUT/"
