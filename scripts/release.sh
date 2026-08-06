#!/usr/bin/env bash
#
# release.sh — build, notarize, and publish a cenno release to GitHub.
#
# Reads ALL secrets from the environment; nothing is typed inline or stored
# in this file. Export these before running (sourced from the KeePassXC vault
# — see README "Releasing an update"):
#
#   TAURI_SIGNING_PRIVATE_KEY_PASSWORD   updater key password
#   APPLE_API_KEY, APPLE_API_ISSUER, APPLE_API_KEY_PATH   App Store Connect key
#   APPLE_SIGNING_IDENTITY               "Developer ID Application: … (TEAMID)"
#
# The updater key path and signing-key contents are derived here. Usage:
#
#   scripts/release.sh                 # build + publish from current version
#   scripts/release.sh --dry-run       # build + make latest.json, skip push/release
#
set -euo pipefail

REPO="glebis/cenno"
KEY_PATH="${TAURI_SIGNING_PRIVATE_KEY_PATH:-$HOME/.tauri/cenno.key}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DRY_RUN=0
NOTES_FILE="${RELEASE_NOTES_FILE:-}"
while (($#)); do
  case "$1" in
    --dry-run)    DRY_RUN=1 ;;
    --notes-file) NOTES_FILE="$2"; shift ;;
    *) echo "error: unknown arg: $1" >&2; exit 1 ;;
  esac
  shift
done
[[ -n "$NOTES_FILE" && ! -f "$NOTES_FILE" ]] && { echo "error: notes file not found: $NOTES_FILE" >&2; exit 1; }

cd "$ROOT"

# --- Preflight: required env, clean tree, version agreement -----------------

missing=()
for v in TAURI_SIGNING_PRIVATE_KEY_PASSWORD APPLE_API_KEY APPLE_API_ISSUER \
         APPLE_API_KEY_PATH APPLE_SIGNING_IDENTITY; do
  [[ -z "${!v:-}" ]] && missing+=("$v")
done
if ((${#missing[@]})); then
  echo "error: missing env vars: ${missing[*]}" >&2
  echo "see README 'Releasing an update' for what to export." >&2
  exit 1
fi
[[ -f "$KEY_PATH" ]] || { echo "error: signing key not found at $KEY_PATH" >&2; exit 1; }

VERSION="$(node -p "require('./package.json').version")"
CONF_VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
if [[ "$VERSION" != "$CONF_VERSION" ]]; then
  echo "error: version mismatch — package.json $VERSION vs tauri.conf.json $CONF_VERSION" >&2
  exit 1
fi
TAG="v$VERSION"
echo "==> releasing $TAG"

if git rev-parse "$TAG" >/dev/null 2>&1 || \
   gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1; then
  echo "error: $TAG already exists (tag or release). Bump the version first." >&2
  exit 1
fi

# Guard: the UI must not carry a HARD-CODED version literal (the About footer
# reads it at runtime via getVersion()). A literal like "cenno v0.2.0" drifts
# silently from the shipped build — exactly what stranded the About page at
# 0.2.0. Fail the release if one reappears.
if STALE="$(grep -rnE 'cenno v[0-9]+\.[0-9]+\.[0-9]+' src/ 2>/dev/null)"; then
  echo "error: hard-coded version literal in UI — read it at runtime instead:" >&2
  echo "$STALE" >&2
  exit 1
fi

# --- Build (PATH=/usr/bin first: shadow conda/Python xattr that breaks bundling) ---

echo "==> building (signed, notarized, updater artifacts)"
PATH="/usr/bin:$PATH" \
TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY_PATH")" \
  npx tauri build

BUNDLE="src-tauri/target/release/bundle"
DMG="$(ls "$BUNDLE"/dmg/cenno_"$VERSION"_*.dmg)"
TARGZ="$BUNDLE/macos/cenno.app.tar.gz"
SIG="$TARGZ.sig"
for f in "$DMG" "$TARGZ" "$SIG"; do
  [[ -f "$f" ]] || { echo "error: expected artifact missing: $f" >&2; exit 1; }
done

# --- Launch gate: catch AMFI/provisioning bricks that codesign + notarization
#     do NOT detect (they passed for 0.3.0, which still couldn't launch). ------

APP="$BUNDLE/macos/cenno.app"

# (1) Static guard: a Developer-ID bundle carrying RESTRICTED entitlements
#     (iCloud/CloudKit, app/team identifiers) MUST embed a provisioning profile,
#     or macOS AMFI SIGKILLs it at spawn. This is exactly what bricked 0.3.0.
ENTS="$(codesign -d --entitlements - --xml "$APP" 2>/dev/null || true)"
if grep -qE 'com\.apple\.developer\.icloud|com\.apple\.developer\.team-identifier|com\.apple\.application-identifier' <<<"$ENTS"; then
  if [[ ! -f "$APP/Contents/embedded.provisionprofile" ]]; then
    echo "error: bundle carries RESTRICTED entitlements but has no" >&2
    echo "       Contents/embedded.provisionprofile — AMFI will SIGKILL it at" >&2
    echo "       launch (the 0.3.0 brick). Either remove the iCloud/CloudKit" >&2
    echo "       entitlements from src-tauri/Entitlements.plist for Developer-ID" >&2
    echo "       builds, or embed a Developer ID provisioning profile that" >&2
    echo "       authorizes the iCloud.app.cenno container." >&2
    exit 1
  fi
fi

# (2) Live spawn test: actually launch the built binary and require it to stay
#     alive. A restricted-entitlement / dyld / signature brick dies immediately
#     (SIGKILL -> exit 137); a healthy tray app keeps running until we stop it.
#     Run it under a throwaway HOME: on a dev machine the installed cenno is
#     almost always running and owns ~/Library/.../app.cenno/mcp.sock, so a
#     healthy build exits 0 ("another MCP server is already listening") and the
#     gate reads that as a launch failure. An isolated HOME removes the
#     contention while still exercising signature, dyld and AMFI. It must be
#     SHORT — a long socket path trips SUN_LEN and reintroduces the same exit-0.
echo "==> launch smoke-test (AMFI spawn check)"
SMOKE_HOME="$(mktemp -d /tmp/cenno-smoke.XXXX)"
HOME="$SMOKE_HOME" "$APP/Contents/MacOS/cenno" >/dev/null 2>&1 &
SMOKE_PID=$!
( sleep 4; kill -0 "$SMOKE_PID" 2>/dev/null && kill "$SMOKE_PID" 2>/dev/null ) &
WATCH_PID=$!
if wait "$SMOKE_PID" 2>/dev/null; then SMOKE_RC=0; else SMOKE_RC=$?; fi
kill "$WATCH_PID" 2>/dev/null || true
# rc 143 (SIGTERM from our watchdog) = it was alive at 4s => launched fine.
rm -rf "$SMOKE_HOME"
if [[ "$SMOKE_RC" == "143" ]]; then
  echo "    ok: app launched and stayed alive"
else
  echo "error: built app failed to launch (exit $SMOKE_RC) — NOT publishing." >&2
  [[ "$SMOKE_RC" == "137" ]] && \
    echo "       exit 137 = SIGKILL by AMFI, almost always a restricted entitlement" >&2 && \
    echo "       (iCloud/CloudKit) without an embedded provisioning profile." >&2
  exit 1
fi

# --- latest.json (the updater manifest) -------------------------------------

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
cp "$DMG" "$STAGE/"
cp "$TARGZ" "$STAGE/cenno.app.tar.gz"
cp "$SIG" "$STAGE/cenno.app.tar.gz.sig"

PUB_DATE="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
# Release notes, in priority order:
#   1. --notes-file / RELEASE_NOTES_FILE — the human-confirmed notes (the agent
#      drafts these from the commit log via scripts/gen-release-notes.sh and has
#      the user confirm/edit them with AskUserQuestion before release).
#   2. the CHANGELOG section for this version.
#   3. an auto-draft from the commit log (last resort, so we never ship blank).
if [[ -n "$NOTES_FILE" ]]; then
  BODY="$(cat "$NOTES_FILE")"
else
  BODY="$(awk "/^## \[$VERSION\]/{f=1;next} /^## \[/{f=0} f" CHANGELOG.md)"
  BODY="$(printf '%s' "$BODY" | sed -e 's/^[[:space:]]*//' -e '/./,$!d')"
  [[ -z "${BODY//[[:space:]]/}" ]] && BODY="$(scripts/gen-release-notes.sh)"
fi
# latest.json carries a flattened one-line summary (shown by the in-app updater).
NOTES="$(printf '%s' "$BODY" | tr '\n' ' ' | sed 's/  */ /g;s/^ *//;s/ *$//')"
node -e "
  const fs=require('fs');
  fs.writeFileSync('$STAGE/latest.json', JSON.stringify({
    version: '$VERSION',
    notes: process.env.N || 'See CHANGELOG.md.',
    pub_date: '$PUB_DATE',
    platforms: { 'darwin-aarch64': {
      signature: fs.readFileSync('$SIG','utf8').trim(),
      url: 'https://github.com/$REPO/releases/download/$TAG/cenno.app.tar.gz'
    }}
  }, null, 2));
" N="$NOTES"
echo "==> latest.json:"; cat "$STAGE/latest.json"

if ((DRY_RUN)); then
  echo "==> --dry-run: artifacts staged in $STAGE (not pushed). Copying out…"
  OUT="$ROOT/dist-release-$VERSION"; mkdir -p "$OUT"; cp "$STAGE"/* "$OUT/"
  echo "    $OUT"
  trap - EXIT
  exit 0
fi

# --- Publish ----------------------------------------------------------------

echo "==> pushing main"
git push origin main

echo "==> creating GitHub release $TAG"
gh release create "$TAG" --repo "$REPO" --title "cenno $TAG" \
  --notes "${BODY:-Release $TAG. See CHANGELOG.md.}" \
  "$STAGE/$(basename "$DMG")" \
  "$STAGE/cenno.app.tar.gz" \
  "$STAGE/cenno.app.tar.gz.sig" \
  "$STAGE/latest.json"

echo "==> verifying live endpoint"
sleep 3
curl -sL "https://github.com/$REPO/releases/latest/download/latest.json" \
  | node -e "const d=JSON.parse(require('fs').readFileSync(0));console.log('live version:',d.version)"

# --- Bump the Homebrew cask -------------------------------------------------
# glebis/homebrew-tap Casks/cenno.rb pins both the version and the DMG sha256,
# so `brew install --cask cenno` keeps serving the OLD release until it is
# bumped. Doing it by hand was forgotten for 0.3.1 AND 0.4.0 — Homebrew users
# sat two releases behind — so it runs here, right after the release exists.
# Skippable with SKIP_CASK_BUMP=1. Failures warn rather than abort: the GitHub
# release is already live at this point and re-running would hit the "tag
# already exists" guard.
if [[ "${SKIP_CASK_BUMP:-0}" == "1" ]]; then
  echo "==> SKIP_CASK_BUMP=1 — not bumping the Homebrew cask"
else
  echo "==> bumping Homebrew cask to $VERSION"
  CASK_SHA="$(shasum -a 256 "$DMG" | awk '{print $1}')"
  TAP_DIR="$(mktemp -d)"
  if gh repo clone glebis/homebrew-tap "$TAP_DIR/tap" -- -q 2>/dev/null &&
     [[ -f "$TAP_DIR/tap/Casks/cenno.rb" ]]; then
    # Anchored to the cask's own fields so nothing else in the file can match.
    /usr/bin/sed -i '' \
      -e "s|^  version \".*\"|  version \"$VERSION\"|" \
      -e "s|^  sha256 \".*\"|  sha256 \"$CASK_SHA\"|" \
      "$TAP_DIR/tap/Casks/cenno.rb"
    if git -C "$TAP_DIR/tap" diff --quiet; then
      echo "    cask already at $VERSION — nothing to do"
    elif git -C "$TAP_DIR/tap" commit -qam "cenno $VERSION" &&
         git -C "$TAP_DIR/tap" push -q; then
      echo "    cask bumped to $VERSION ($CASK_SHA)"
    else
      echo "    warning: cask commit/push failed — bump Casks/cenno.rb by hand" >&2
    fi
  else
    echo "    warning: could not clone glebis/homebrew-tap — bump Casks/cenno.rb" >&2
    echo "    by hand: version $VERSION, sha256 $CASK_SHA" >&2
  fi
  rm -rf "$TAP_DIR"
fi

# --- Deploy the marketing site ----------------------------------------------
# site/ (Astro, Vercel project 'cenno') resolves the download link + version
# from the releases/latest API at BUILD time, so it must be redeployed AFTER the
# GitHub release exists. A git push does NOT auto-deploy this project. Skippable
# with SKIP_SITE_DEPLOY=1 (e.g. if the Vercel CLI isn't authed on this machine).
if [[ "${SKIP_SITE_DEPLOY:-0}" == "1" ]]; then
  echo "==> SKIP_SITE_DEPLOY=1 — not deploying site (run 'cd site && vercel --prod' yourself)"
elif command -v vercel >/dev/null 2>&1; then
  echo "==> deploying site to production (refreshes download link to $TAG)"
  ( cd site && vercel --prod --yes >/dev/null ) \
    && echo "    site deployed: https://cenno.salient.community/" \
    || echo "    warning: site deploy failed — run 'cd site && vercel --prod' manually" >&2
else
  echo "==> vercel CLI not found — deploy the site manually so its download link" >&2
  echo "    updates to $TAG: cd site && vercel --prod" >&2
fi

echo "==> done: https://github.com/$REPO/releases/tag/$TAG"
