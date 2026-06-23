#!/usr/bin/env bash
#
# gen-release-notes.sh — draft release notes for the next cenno release from the
# commit log since the previous tag. Prints Markdown to stdout.
#
# This is the DRAFT only. The release flow (see the `cenno` skill / README
# "Releasing an update") generates these notes, presents them to the human for
# confirmation/editing (via AskUserQuestion when an agent drives the release),
# writes the approved text to a file, and passes it to:
#
#     scripts/release.sh --notes-file <file>
#
# Usage:
#   scripts/gen-release-notes.sh            # range = last tag..HEAD
#   scripts/gen-release-notes.sh v0.3.0     # range = v0.3.0..HEAD
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"

PREV="${1:-$(git describe --tags --abbrev=0 2>/dev/null || true)}"
VERSION="$(node -p "require('./package.json').version")"
RANGE="${PREV:+$PREV..}HEAD"

echo "## cenno v$VERSION"
echo
# Group Conventional-Commits-style messages; everything else under "Other".
declare -a feats fixes other
while IFS= read -r line; do
  msg="${line#* }"                       # strip leading short-hash
  case "$msg" in
    feat*|feature*) feats+=("- ${msg#*: }") ;;
    fix*)           fixes+=("- ${msg#*: }") ;;
    chore*|docs*|refactor*|test*|ci*|build*|style*) ;;  # omit housekeeping
    *)              other+=("- $msg") ;;
  esac
done < <(git log --no-merges --pretty='%h %s' "$RANGE")

if ((${#feats[@]})); then printf '### Added\n\n'; printf '%s\n' "${feats[@]}"; echo; fi
if ((${#fixes[@]})); then printf '### Fixed\n\n'; printf '%s\n' "${fixes[@]}"; echo; fi
if ((${#other[@]})); then printf '### Other\n\n'; printf '%s\n' "${other[@]}"; echo; fi

if ((${#feats[@]} + ${#fixes[@]} + ${#other[@]} == 0)); then
  echo "_No user-facing changes since ${PREV:-the start}._"
fi
