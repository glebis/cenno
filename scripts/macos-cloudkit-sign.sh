#!/bin/bash
# Phase 2: make the built Tauri cenno.app actually use CloudKit.
#
# Tauri's bundler signs the .app but (a) does NOT embed a macOS provisioning
# profile and (b) signs only with the keys in Entitlements.plist. CloudKit on a
# non-App-Store macOS app needs BOTH an embedded provisioning profile for the
# iCloud.app.cenno container AND the application-identifier / team-identifier
# entitlements. This script adds the profile and re-signs with the full
# entitlement set, so cenno_relay_write_prompt() can write Prompt records.
#
# Usage:
#   scripts/macos-cloudkit-sign.sh <path-to-cenno.app> [signing-identity-sha1] [profile-path]
#
# Defaults assume a DEVELOPMENT signing identity + a Mac Development provisioning
# profile (-> CloudKit Development, which is what the dev/sim companion reads).
# For a release build, supply a Developer ID identity + a Distribution profile
# whose container is provisioned for Production, and deploy the Prod schema.
set -euo pipefail

APP="${1:?usage: macos-cloudkit-sign.sh <cenno.app> [identity] [profile]}"
IDENTITY="${2:-43AEF905517ED8686A10871AF74169C65A8C5B5A}"   # Apple Development (gleb_kalinin)
PROFILE="${3:-/tmp/app.cenno.provisionprofile}"             # Mac Team Provisioning Profile: app.cenno
ENT="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/Entitlements.plist"

[ -d "$APP" ]     || { echo "no app at $APP"; exit 1; }
[ -f "$PROFILE" ] || { echo "no provisioning profile at $PROFILE (mint one for app.cenno + iCloud.app.cenno)"; exit 1; }

echo "embedding provisioning profile"
cp "$PROFILE" "$APP/Contents/embedded.provisionprofile"

# Clear the extended attributes that trip Tauri's own xattr step, then sign
# nested mach-o first, then the bundle (deep), preserving hardened runtime.
/usr/bin/xattr -cr "$APP" || true
find "$APP/Contents" \( -name "*.dylib" -o -name "*.framework" \) -print0 2>/dev/null \
  | xargs -0 -I{} codesign --force --options runtime --sign "$IDENTITY" {} 2>/dev/null || true
codesign --force --deep --options runtime \
  --entitlements "$ENT" --sign "$IDENTITY" "$APP"

echo "=== verify ==="
codesign -d --entitlements :- "$APP/Contents/MacOS/"* 2>/dev/null \
  | grep -oE "application-identifier|icloud-container-identifiers|CloudKit" | sort -u
codesign -dvv "$APP" 2>&1 | grep -E "Authority=Apple|TeamIdentifier" | head -2
echo "signed: $APP"
