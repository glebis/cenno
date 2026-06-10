# cenno

*fare un cenno* — Italian: to beckon.

cenno is a macOS runtime that lets AI agents ask the user questions through minimal floating surfaces and receive the answers as tool results. A prompt arrives via MCP or CLI; a non-activating NSPanel slides up; the user types or picks; the caller gets `{answer, via, elapsed_s}` as JSON. Design direction: [Reporter-app-style](https://apps.apple.com/de/app/reporter-app/id779697486) minimalism — a lightweight interrupter, not a dense pro tool.

---

## Status

**Walking skeleton — plan 1 of 4 done.**

What works today:
- `ask_user` via MCP (Unix socket) or `cenno ask` CLI
- Non-activating NSPanel prompt display (never steals focus)
- `--mcp-stdio` bridge with tray autolaunch

What's next:
- **Plan 2:** A2UI rendering + design tokens
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

---

## CLI

```bash
# Ask a question; blocks until answered or timed out
cenno ask "Question" --body "Optional markdown body" --timeout 30

# Exit codes: 0 = answered, 2 = timed out, 1 = not running / error

# Run headless — no main window shown until a prompt arrives (tray icon: plan 4)
cenno --tray

# MCP bridge: pipe stdin/stdout to the socket, launching the app if needed
cenno --mcp-stdio
```

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

## Security notes

- The Unix socket is user-only (`0600`) in the app-data directory.
- `src-tauri/tauri.conf.json` ships with `"csp": null` (scaffold default; JSON has no comments, hence this note). Tighten the Content-Security-Policy before any release build that exposes remote HTTP content.
- Never expose the HTTP server without authentication tokens (addressed in plan 4).
