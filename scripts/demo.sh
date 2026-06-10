#!/usr/bin/env bash
# Fire a demo prompt of each kind at the running (or auto-launched) cenno.
# Usage: scripts/demo.sh [text|choice|scale|confirm|mood|all]
set -euo pipefail
cd "$(dirname "$0")/.."
BIN="${CENNO_BIN:-src-tauri/target/release/cenno}"
[ -x "$BIN" ] || { echo "build first: npx tauri build --no-bundle"; exit 1; }

ask() { # $1 = ask_user arguments JSON
  (printf '%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"demo","version":"0"}}}' \
    '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
    "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"ask_user\",\"arguments\":$1}}"; \
   sleep "${CENNO_DEMO_WAIT:-45}") | "$BIN" --mcp-stdio | tail -1
}

case "${1:-all}" in
  text)    ask '{"title":"Quick note","body_md":"What are you working on **right now**?","timeout_s":40}' ;;
  choice)  ask '{"title":"Where did this hour go?","input":{"kind":"choice"},"choices":["Deep work","Meetings","Email","Wandering"],"timeout_s":40}' ;;
  scale)   ask '{"title":"How focused were you this hour?","input":{"kind":"scale"},"flow":"ema","progress":{"step":1,"total":3},"timeout_s":40}' ;;
  confirm) ask '{"title":"Stand up and stretch.","input":{"kind":"confirm"},"flow":"reminder","timeout_s":40}' ;;
  mood)    ask '{"title":"How are you feeling?","input":{"kind":"choice"},"choices":["great","good","okay","low","rough"],"flow":"mood","timeout_s":40}' ;;
  all)     for k in mood text choice scale confirm; do "$0" "$k"; done ;;
  *) echo "usage: $0 [text|choice|scale|confirm|mood|all]"; exit 1 ;;
esac
