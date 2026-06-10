---
name: cenno
description: This skill should be used when an agent needs a decision, preference, confirmation, rating, or free-text input from the human user — to ask through a cenno panel and get a structured answer back, instead of guessing or blocking. Use it instead of saying "I'll assume…", before a risky or irreversible action (a yes/no), for a 1–N rating or pick-one-of-N choice, or to run a check-in / questionnaire. Also use it to set cenno up — install it or add it to a project's MCP config — when the ask_user tool is missing or an ask_user call failed with "not running / not found". Triggers on "ask me", "check with me first", "let me decide", "rate this", "confirm before", mood/EMA check-ins, "set up cenno", "add cenno to this project", and any moment a human judgment call beats an assumption. macOS only.
---

# Asking the user through cenno

cenno shows the user a small floating panel and returns their answer as structured data. Prefer it over guessing whenever a human decision genuinely changes what you do next. Don't use it for things you can determine yourself, and don't ask twice what you already know.

> **Setup mode:** if cenno isn't installed or the `ask_user` tool is missing from this project, jump to [Setting up cenno](#setting-up-cenno) first, then come back.

## Two ways to call it

**MCP tool (preferred when cenno is in your MCP config):** call the `ask_user` tool.

**CLI (always available if cenno is installed):** the binary is at
`/Applications/cenno.app/Contents/MacOS/cenno` (it auto-launches the app if it isn't running):

```bash
/Applications/cenno.app/Contents/MacOS/cenno ask "Deploy to production now?" --timeout 60
```

Exit codes: `0` answered, `2` timed out, `1` not running / error. On success it prints the result JSON to stdout. To pass anything richer than a title + body (choices, a scale, a flow theme), use the MCP tool — the CLI `ask` only takes `--body` and `--timeout`.

## The `ask_user` tool

```jsonc
{
  "title": "How focused do you feel right now?",  // the question (required)
  "body_md": "Markdown is supported here.",        // optional
  "input": { "kind": "scale" },  // text | voice_text | choice | scale | confirm | none
  "choices": ["Deep work", "Meetings", "Scattered"],  // required for kind:choice
  "flow": "ema",       // color theme: mood | question | ema | reminder | ambient
  "progress": { "step": 2, "total": 5 },  // dot pagination for multi-step flows
  "timeout_s": 120     // default 120
}
```

Returns `{"answer": "...", "via": "text"|"choice", "elapsed_s": 1.4}`, or
`{"answered": false, "prompt_id": "p_3"}` on timeout.

### Choosing the input kind

| kind | use for | answer you get back |
|---|---|---|
| `choice` | pick one of N options (provide `choices`) | the chosen string |
| `scale` | a rating — **built-in scale is fixed 1–7** | the number as a string ("5") |
| `confirm` | yes/no before an action | `"yes"` or `"no"` |
| `text` | free-form input, names, sentences | the typed text |
| `voice_text` | same as text, with a mic affordance | the text |
| `none` | show information, no answer needed | (auto-dismisses) |

### Flows = color themes (pick by intent)

`mood` (warm), `question` (neutral, the default feel), `ema` (check-in/survey), `reminder` (calm), `ambient` (quiet/info). They only change the panel color; pick the one that matches the moment.

## Patterns

**Confirm before something irreversible** — the highest-value use:

```json
{ "title": "Force-push to main? This rewrites shared history.",
  "input": { "kind": "confirm" }, "flow": "reminder", "timeout_s": 90 }
```
Proceed only on `"answer": "yes"`. On timeout (`answered: false`), do NOT proceed — treat it as "no decision".

**Pick one of N:**

```json
{ "title": "Which database for this feature?",
  "input": { "kind": "choice" },
  "choices": ["Postgres", "SQLite", "in-memory"], "flow": "question" }
```

**Multi-step questionnaire** — one question per call, carry `progress`:

```json
{ "title": "Energy level?", "input": { "kind": "scale" },
  "flow": "ema", "progress": { "step": 1, "total": 3 } }
```
Fire them sequentially, each blocking on the previous answer. If any step times out, stop and don't record a partial result.

## Custom scales (1–5, custom endpoints) via `a2ui`

The built-in `scale` is hard-wired to 1–7. For any other range or custom endpoint labels, send a rich `a2ui` payload instead of `input`/`choices`. It is an array of three A2UI v0.9 messages; the catalog is always `cenno:catalog/v1`:

```json
{
  "title": "Mood today",
  "flow": "ema",
  "timeout_s": 120,
  "a2ui": [
    { "version": "v0.9", "createSurface": { "surfaceId": "main", "catalogId": "cenno:catalog/v1" } },
    { "version": "v0.9", "updateComponents": { "surfaceId": "main", "components": [
      { "id": "root",  "component": "Column", "children": ["col"] },
      { "id": "col",   "component": "Column", "children": ["title", "body", "scale"] },
      { "id": "title", "component": "Text", "text": "Mood today", "variant": "h2" },
      { "id": "body",  "component": "Text", "text": "1 — drained · 5 — great" },
      { "id": "scale", "component": "Scale", "min": 1, "max": 5,
        "minLabel": "drained", "maxLabel": "great", "value": { "path": "/scale" },
        "selectAction": { "event": { "name": "submit-scale",
          "context": { "value": { "path": "/scale" }, "via": "choice" } } } }
    ] } },
    { "version": "v0.9", "updateDataModel": { "surfaceId": "main", "path": "/", "value": {} } }
  ]
}
```

The result text is still `{"answer":"3","via":"choice",...}`. Boundary limits: ≤200 components, ≤256 KiB, catalog must be `cenno:catalog/v1` — invalid payloads return a tool error you can correct and retry.

## Etiquette (important)

- **Ask only when the answer changes your action.** Don't narrate options you'll pursue regardless.
- **One question per panel.** Keep titles short; put detail in `body_md`.
- **Set a sane `timeout_s`.** A timeout is a real outcome — handle `answered: false` (skip, default, or abort), never loop re-asking.
- **The user may be paused or in fullscreen** (cenno's quiet mode). A timed-out prompt may simply mean they didn't see it — fall back gracefully, don't spam.
- Every answer is recorded in the user's local history; you don't need to log it yourself.

## Driving cenno over a raw socket (no MCP client)

If you must call cenno from a plain script (no MCP runtime), pipe JSON-RPC through the stdio bridge — initialize, then `tools/call`:

```bash
BIN=/Applications/cenno.app/Contents/MacOS/cenno
printf '%s\n' \
 '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"x","version":"1"}}}' \
 '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
 '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"ask_user","arguments":{"title":"Ship it?","input":{"kind":"confirm"},"timeout_s":60}}}' \
 | "$BIN" --mcp-stdio
```
Read stdout line by line until the line whose `id` is `2`; its `result.content[0].text` is the answer JSON. Keep stdin open until then. Tip: if cenno is cold, the first call can lose a socket race — launch `"$BIN" --tray` and wait for `~/Library/Application Support/app.cenno/mcp.sock` to exist before the first `ask_user`.

If cenno isn't installed or reachable, fall back to whatever local prompt you have (e.g. `osascript` dialogs) — don't block on it.

---

## Setting up cenno

Do this when the `ask_user` tool is missing from a project, or an `ask_user` call fails with "not running / not found". **macOS only** — on other platforms there is no cenno; fall back to another prompt mechanism.

### 1. Confirm it's installed

The canonical binary path is `/Applications/cenno.app/Contents/MacOS/cenno`:

```bash
test -x /Applications/cenno.app/Contents/MacOS/cenno && echo installed || echo missing
```

If missing, tell the user to install it (don't build it for them unless they ask):

- **DMG:** download the latest from https://github.com/glebis/cenno/releases/latest (signed & notarized, Apple Silicon, macOS 12+), drag cenno to Applications.
- **Homebrew:** `brew install --cask glebis/tap/cenno`

If it's installed in a non-standard location, find it: `mdfind "kMDItemCFBundleIdentifier == 'app.cenno'"`.

### 2. Add it to the project's MCP config

Merge into the project's `.mcp.json` (use the absolute path from step 1):

```json
{
  "mcpServers": {
    "cenno": {
      "command": "/Applications/cenno.app/Contents/MacOS/cenno",
      "args": ["--mcp-stdio"]
    }
  }
}
```

`--mcp-stdio` auto-launches the app in the background, so nothing else needs starting. The MCP client must be restarted/reloaded to pick up the new server (e.g. restart the Claude Code session).

### 3. Verify the round-trip

```bash
/Applications/cenno.app/Contents/MacOS/cenno ask "cenno setup check — tap or ignore" --timeout 8
```

A panel appears and the answer (or `{"answered": false, ...}` after 8s) prints as JSON → working. `cenno is not running — start it…` → it couldn't launch; recheck step 1. The socket lives at `~/Library/Application Support/app.cenno/mcp.sock`.
