#!/usr/bin/env bash
# E2E smoke: requires the cenno app running (or reachable via --mcp-stdio autolaunch).
# Verifies the socket round-trip; accepts Answered OR TimedOut as "wired".
# Exit codes from `cenno ask`: 0 = answered, 2 = timed out, 1 = not running / error.
set -euo pipefail

BIN="${CENNO_BIN:-src-tauri/target/release/cenno}"
[ -x "$BIN" ] || {
  echo "build first: npx tauri build --no-bundle  (cargo-built binaries load devUrl)"
  exit 1
}

# cenno ask exits 2 on timeout — capture that without triggering set -e
OUT=$("$BIN" ask "Smoke test — answer or let it time out" \
        --timeout "${CENNO_SMOKE_TIMEOUT:-10}") && RC=0 || RC=$?

# RC 0 = answered, RC 2 = timed out; anything else is a hard error
if [ "$RC" -eq 1 ]; then
  echo "SMOKE FAIL (not running or internal error): $OUT"
  exit 1
fi

# Both Answered and TimedOut are valid wire-up proofs
if echo "$OUT" | jq -e '(.answer != null) or (.answered == false)' >/dev/null; then
  echo "SMOKE OK (rc=$RC): $OUT"
else
  echo "SMOKE FAIL (unexpected JSON): $OUT"
  exit 1
fi
