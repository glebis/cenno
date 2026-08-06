#!/usr/bin/env bash
#
# release-interactive.sh — wrapper around release.sh for a terminal session.
#
# The updater-key passphrase is the one secret that cannot be read from the
# environment or from SOPS, so it is prompted for here (hidden, never echoed,
# never written to disk). Everything else is derived from the known App Store
# Connect key + Developer ID identity. Run this in a herdr pane and type the
# passphrase when asked.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
LOG="${RELEASE_LOG:-$ROOT/.release-interactive.log}"
: > "$LOG"

# The pane closes the moment this script exits, taking any error message with
# it. Everything is teed to $LOG so the outcome survives, and the script waits
# for a keypress at the end so a human watching the pane can read it too.
finish() {
  local rc=$1
  echo
  echo "--- exit $rc — full log: $LOG"
  read -r -n1 -p "press any key to close…" _ || true
  exit "$rc"
}

export APPLE_API_KEY=KY2K3HP233
export APPLE_API_ISSUER=c59b00aa-278a-4d21-9196-3acb0461e584
export APPLE_API_KEY_PATH="$HOME/.appstoreconnect/AuthKey_KY2K3HP233.p8"
export APPLE_SIGNING_IDENTITY="Developer ID Application: Gleb Kalinin (79SP7TM5PH)"

KEY="$HOME/.tauri/cenno.key"
# The on-PATH tauri, never `npx tauri` — npx hangs downloading the CLI.
TAURI="$(command -v tauri || echo "$ROOT/node_modules/.bin/tauri")"
[[ -x "$TAURI" ]] || { echo "error: tauri CLI not found" | tee -a "$LOG"; finish 1; }

echo "cenno release — updater key passphrase ($KEY, in KeePassXC)"
read -r -s -p "passphrase: " TAURI_SIGNING_PRIVATE_KEY_PASSWORD
echo
[[ -n "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD" ]] || { echo "error: empty passphrase" | tee -a "$LOG"; finish 1; }
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD

# Fail fast on a wrong passphrase instead of re-notarizing first: sign a
# throwaway file and check the key decodes. The probe's own output is kept so a
# tooling failure is not misreported as a bad passphrase.
TMP="$(mktemp)"
PROBE="$("$TAURI" signer sign --private-key-path "$KEY" \
          --password "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD" "$TMP" 2>&1)"
PROBE_RC=$?
rm -f "$TMP" "$TMP.sig"
if ((PROBE_RC)); then
  { echo "error: updater key probe failed (rc $PROBE_RC) — nothing published."
    echo "$PROBE"; } | tee -a "$LOG"
  finish 1
fi
echo "==> passphrase ok" | tee -a "$LOG"

scripts/release.sh "$@" 2>&1 | tee -a "$LOG"
finish "${PIPESTATUS[0]}"
