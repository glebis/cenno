# cenno

*fare un cenno* — Italian: to beckon.

cenno is a macOS runtime that lets AI agents ask the user questions through minimal floating surfaces and receive the answers as tool results. A prompt arrives via MCP or CLI; a non-activating NSPanel slides up; the user types or picks; the caller gets `{answer, via, elapsed_s}` as JSON. Design direction: [Reporter-app-style](https://apps.apple.com/de/app/reporter-app/id779697486) minimalism — a lightweight interrupter, not a dense pro tool.

---

## Status

**Plan 2 done — one rendering path, token-styled catalog, CSP enabled.**

What works today:
- `ask_user` via MCP (Unix socket) or `cenno ask` CLI
- Non-activating NSPanel prompt display (never steals focus)
- `--mcp-stdio` bridge with tray autolaunch
- Single rendering path: the simple `ask_user` form desugars to an A2UI envelope; native `a2ui` arrays bypass desugaring. Both paths run through the cenno catalog.
- Token-styled component catalog (`cenno:catalog/v1`) — Reporter-style full-bleed color, dot pagination, outlined scale targets, pill chips
- CSP enforced in built artifacts (see Security notes). Tauri does not inject CSP over `devUrl` — test compliance against `npx tauri build --no-bundle` output, not `tauri dev`.

What's next:
- **Plan 3:** Voice input via whisper.cpp + BYOK
- **Plan 4:** Fullscreen/tray surfaces, urgency policy, history

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
| `show_surface` | spec'd (plan 4) | — | — |
| `dismiss_surface` | spec'd (plan 4) | — | — |
| `get_response` | spec'd (plan 4) | — | — |

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
```

---

## Tray

Full tray docs land with the close-out; one behavior worth knowing now: pause ("Pause for …") and "Don't show in fullscreen" suppress the *display* only — prompts still register and agents keep their normal timeout contract. Suppressed prompts reappear when suppression lifts: on "Resume now", on unchecking the fullscreen toggle, when a pause expires, or when the next prompt arrives. Exiting fullscreen has **no event hook in v1** — cenno doesn't notice the instant a fullscreen app goes away, so a queued prompt waits for one of those triggers instead of popping up immediately.

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
- CSP is set: `default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'`. Tauri 2 automatically appends its own IPC directives (nonces/hashes for injected scripts) when a CSP string is present. If future features load remote content, extend the policy explicitly rather than relaxing the defaults.
- **CSP is enforced only in BUILT artifacts on desktop.** Tauri does not inject CSP over `devUrl` — test compliance against `npx tauri build --no-bundle` output, not `npm run tauri dev`.
- Never expose the HTTP server without authentication tokens (addressed in plan 4).
