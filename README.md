<p align="center">
  <img src="docs/design/brand/banner.png" alt="cenno — agents ask. you answer." width="800">
</p>

# cenno

*fare un cenno* — Italian: to beckon.

cenno is a macOS runtime that lets AI agents ask the user questions through minimal floating surfaces and receive the answers as tool results. A prompt arrives via MCP or CLI; a non-activating NSPanel slides up; the user types or picks; the caller gets `{answer, via, elapsed_s}` as JSON. Design direction: [Reporter-app-style](https://apps.apple.com/de/app/reporter-app/id779697486) minimalism — a lightweight interrupter, not a dense pro tool.

---

## Status

**Plan 4 done — SQLite history, tray icon, pause + fullscreen quiet mode.**

What works today:
- `ask_user` via MCP (Unix socket) or `cenno ask` CLI
- Non-activating NSPanel prompt display (never steals focus)
- `--mcp-stdio` bridge with tray autolaunch
- Single rendering path: the simple `ask_user` form desugars to an A2UI envelope; native `a2ui` arrays bypass desugaring. Both paths run through the cenno catalog.
- Token-styled component catalog (`cenno:catalog/v1`) — Reporter-style full-bleed color, dot pagination, outlined scale targets, pill chips
- CSP enforced in built artifacts (see Security notes). Tauri does not inject CSP over `devUrl` — test compliance against `npx tauri build --no-bundle` output, not `tauri dev`.
- **SQLite history** — every prompt outcome recorded automatically (see History section)
- **Tray icon** — always-visible menu-bar home; pause, fullscreen-quiet mode, quit (see Tray section)

What's next:
- **Plan 3:** Voice input via whisper.cpp + BYOK
- **Plan 5:** Tray popover inbox/history UI, single-instance enforcement

---

## Build

```bash
npm install
npx tauri build --no-bundle
# binary → src-tauri/target/release/cenno
```

> **Important:** Plain `cargo build` produces a binary that loads the dev server URL (port 1430) and will show a blank or wrong page unless `npm run dev` is running in parallel. Use the tauri-built binary (`npx tauri build --no-bundle`) for real use.

---

## MCP setup

Add to your MCP config (Claude Desktop, claude-code, etc.):

```json
{
  "mcpServers": {
    "cenno": {
      "command": "/path/to/cenno",
      "args": ["--mcp-stdio"]
    }
  }
}
```

---

## Tools

| Tool | Status | Params | Returns |
|------|--------|--------|---------|
| `ask_user` | implemented | `title`, `body_md`, `input.kind`, `choices`, `urgency`, `timeout_s`, `a2ui` | `{answer, via, elapsed_s}` or `{answered: false, prompt_id}` |
| `show_surface` | spec'd (plan 5) | — | — |
| `dismiss_surface` | spec'd (plan 5) | — | — |
| `get_response` | spec'd (plan 5) | — | — |

**`a2ui` field** is LIVE with boundary validation. Accepted value: an array of v0.9 A2UI messages (`createSurface` + `updateComponents` + optional `updateDataModel`). Guards: array of objects, ≤200 components per `updateComponents`, serialised payload ≤256 KiB, catalog must be `cenno:catalog/v1`. Passing `a2ui` bypasses desugaring entirely — the native payload renders directly. See [catalog docs](docs/design/TOKENS.md).

---

## CLI

```bash
# Ask a question; blocks until answered or timed out
cenno ask "Question" --body "Optional markdown body" --timeout 30

# Exit codes: 0 = answered, 2 = timed out, 1 = not running / error

# Run headless — no main window shown until a prompt arrives
cenno --tray

# MCP bridge: pipe stdin/stdout to the socket, launching the app if needed
cenno --mcp-stdio

# Export prompt history to stdout
cenno export
cenno export --format csv
cenno export --since 2025-06-01
cenno export --since 2025-06-01T09:00:00Z | jq '.[0].answer'
```

---

## History

### Where the database lives

```
~/Library/Application Support/app.cenno/cenno.db
```

The file is created on first launch, permissions `0600` (owner read/write only).

### What is recorded

Every prompt outcome is recorded after `registry.ask()` returns — whether the user answered or the agent timed out:

| Column | Type | Notes |
|--------|------|-------|
| `id` | integer | auto-increment row id |
| `prompt_id` | text | internal id (e.g. `p_12`) |
| `title` | text | prompt title shown to the user |
| `body_md` | text | markdown body (empty string if none) |
| `input_kind` | text | `text`, `voice_text`, `choice`, … |
| `flow` | text | flow tag if set (`mood`, `ema`, …), else NULL |
| `urgency` | text | `normal`, `high`, `critical` |
| `status` | text | `answered` or `timed_out` |
| `answer` | text | user's answer; NULL when `timed_out` |
| `via` | text | `text`, `voice`, `choice`; NULL when `timed_out` |
| `elapsed_s` | real | seconds from ask to answer; NULL when `timed_out` |
| `created_at` | text | ISO 8601 UTC — when the ask arrived |
| `resolved_at` | text | ISO 8601 UTC — when the outcome was recorded |

### Exporting

```bash
cenno export                         # JSON array to stdout (default)
cenno export --format csv            # CSV with header row
cenno export --since 2025-06-01      # YYYY-MM-DD → inclusive, midnight UTC
cenno export --since 2025-06-01T09:00:00Z  # RFC3339 inclusive boundary
```

Empty database → `[]` (JSON) or header-only (CSV), exit 0.
Missing database (app never launched) → friendly error on stderr, exit 1.

### Privacy

All data is local. Nothing is sent to any server. The DB file sits in your macOS Application Support directory under your user account (`0600` permissions). Answers are stored as plaintext — if your answers contain sensitive text, protect the file accordingly (FileVault encrypts the whole volume; the DB is inside it).

---

## Tray

The tray icon is the app's always-visible home — it appears in the menu bar whether cenno was launched with `--tray` or as a normal app. The icon is a monochrome template image generated via Codex CLI; macOS recolors it automatically for light, dark, and tinted menu-bar appearances.

### Menu map

```
Pause for ▶
    15 min
    30 min
    1 hour
    2 hours
    5 hours
    8 hours
    Until tomorrow
Resume now
──────────────────
Don't show in fullscreen  ✓ (default on)
──────────────────
Quit cenno
```

**Until tomorrow** means the next local 05:00. Rationale: a late-night worker pausing cenno at 23:45 means "leave me alone for the rest of this session" — a midnight boundary would un-pause 15 minutes later. 05:00 is past any plausible working night and before any plausible morning start.

**Resume now** is always visible; it is a no-op when nothing is paused. No pause-remaining countdown is shown in the menu.

### Pause semantics

Pause and the fullscreen quiet mode suppress the **display** only — they gate which prompts appear on screen, not which prompts register. Specifically:

- Prompts that arrive while suppressed are registered in the queue with their full timeout budget; agents keep their normal `TimedOut` contract.
- When suppression lifts (via "Resume now", unchecking the fullscreen toggle, pause expiry, or a new prompt arriving after expiry), the newest still-answerable queued prompt is re-shown. Earlier queued prompts can be answered via the tray history UI (plan 5).
- Pause expiry is backed by a Tokio timer armed at pause-set time, so prompts replay automatically at the deadline even if no new prompt arrives.

### Fullscreen quiet mode

When "Don't show in fullscreen" is checked (the default), cenno detects whether an app is fullscreen **on the screen where the cenno panel lives** — a fullscreen app on another display does not suppress prompts. Detection is a CGWindowList bounds heuristic scoped to the panel's display (resolved at check time, so a dragged panel is honored; if the panel's display can't be determined, the display under the cursor — then the main display — is used). The check runs once per incoming prompt and once per replay attempt — it is never polled.

**v1 limitation — no fullscreen-exit hook.** cenno does not subscribe to a fullscreen-end event (macOS does not provide one publicly). A suppressed prompt will reappear on the next trigger:

- a new prompt arrives
- a pause expires
- "Resume now" is clicked
- the fullscreen checkbox is toggled off and back on

It does **not** pop up the instant you exit fullscreen.

**Known false positive.** A maximized window with the Dock and menu bar both set to auto-hide has geometry identical to a fullscreen Space. cenno cannot distinguish these by bounds alone and will queue the prompt silently. The error is quiet — prompts queue and replay; they are never lost.

### Ice (menu-bar manager) caveat

If you use Ice or a similar app that collapses menu-bar icons, the cenno icon may start in the hidden section after first launch. Cmd-drag it into the visible section once; your layout is then remembered.

---

## Dev

```bash
npm run dev          # Vite dev server (port 1430)
npm run tauri dev    # full Tauri dev loop (hot-reloads webview)

cargo test           # Rust unit tests (src-tauri/)
npx vitest run       # TypeScript tests
```

Spike and research docs: `docs/superpowers/research/`

---

## Specs & plans

- Specs: `docs/superpowers/specs/`
- Plans: `docs/superpowers/plans/`
- A2UI spike findings: `docs/superpowers/research/`

---

## Design system

[TOKENS.md](docs/design/TOKENS.md) is the source of truth for the token set (palette, type scale, spacing, radius). CSS custom properties are generated from `tokens/tokens.json` (DTCG format) via Style Dictionary:

```bash
npm run tokens     # regenerate src/styles/tokens.css from tokens/tokens.json
```

Flow theming uses the `data-flow` attribute on the panel root. The catalog (`cenno:catalog/v1`) maps component names to React implementations styled from the token layer.

Demo all panel states against the release binary:

```bash
./scripts/demo.sh [mood|text|choice|scale|confirm|all]
```

---

## Security notes

- The Unix socket is user-only (`0600`) in the app-data directory.
- The history database (`cenno.db`) is `0600` — owner read/write only.
- CSP is set: `default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'`. Tauri 2 automatically appends its own IPC directives (nonces/hashes for injected scripts) when a CSP string is present. If future features load remote content, extend the policy explicitly rather than relaxing the defaults.
- **CSP is enforced only in BUILT artifacts on desktop.** Tauri does not inject CSP over `devUrl` — test compliance against `npx tauri build --no-bundle` output, not `npm run tauri dev`.
- Never expose the HTTP server without authentication tokens (addressed in plan 5).
