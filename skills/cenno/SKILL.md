---
name: cenno
description: This skill should be used when an agent needs a decision, preference, confirmation, rating, or free-text input from the human user — to ask through a cenno panel and get a structured answer back, instead of guessing or blocking. Use it instead of saying "I'll assume…", before a risky or irreversible action (a yes/no), for a 1–N rating or pick-one-of-N choice, or to run a check-in / questionnaire. Also use it to set cenno up — install it or add it to a project's MCP config — when the ask_user tool is missing or an ask_user call failed with "not running / not found". Also use it to configure the cenno app itself — interview the user (through cenno panels by default) to build `~/.cenno/config.json` and `tokens.json`: panel size/position, default theme/timeout, custom widget types, and color theme. Triggers on "ask me", "check with me first", "let me decide", "rate this", "confirm before", mood/EMA check-ins, "set up cenno", "configure cenno", "customize cenno", "change the panel size/position/theme", "make a custom cenno widget", "set up ~/.cenno", "add cenno to this project", and any moment a human judgment call beats an assumption. macOS only.
---

## Loading your config (do this first, every time)

The skill config lives at `~/.claude/skills/cenno/config.json`. **Before doing anything else**, read it:

```bash
cat ~/.claude/skills/cenno/config.json 2>/dev/null || echo "{}"
```

If the file exists, use its values as defaults for every `ask_user` call you make this session — only override them when the caller explicitly passes something different. If the file is missing or empty, use the built-in defaults shown below.

### Config schema and built-in defaults

```jsonc
{
  "default_timeout_s": 120,          // how long to wait for an answer
  "default_flow": "question",        // color theme: mood | question | ema | reminder | ambient
  "log_answers": false,              // append every answer to a log file
  "log_file": "~/Desktop/cenno-log.md",  // path used when log_answers is true
  "preferred_input": "mcp"           // mcp (preferred) | cli (always works)
}
```

Apply these as defaults silently — don't narrate them to the user unless they asked for a setup summary.

---

## Setup wizard

Run this when the user says "set up cenno", "configure cenno", "cenno setup", or similar. Also run it automatically if `config.json` is missing and the user is about to use cenno for a non-trivial purpose.

The wizard asks 4 questions. Use cenno itself if it's available (MCP tool or CLI); fall back to `AskUserQuestion` if cenno isn't installed yet.

### Step 1 — Default timeout

**Via cenno MCP:**
```json
{
  "title": "How long should cenno wait for your answer by default?",
  "input": { "kind": "choice" },
  "choices": ["30 seconds", "60 seconds", "90 seconds", "2 minutes"],
  "flow": "question",
  "timeout_s": 60
}
```

Map answers → seconds: `"30 seconds"→30`, `"60 seconds"→60`, `"90 seconds"→90`, `"2 minutes"→120`.

**Via AskUserQuestion fallback:**
```
question: "Default timeout for cenno panels?"
options: ["30s", "60s", "90s", "2 min (default)"]
```

### Step 2 — Default flow theme

**Via cenno MCP:**
```json
{
  "title": "Which panel color feels right for most questions?",
  "input": { "kind": "choice" },
  "choices": ["question — neutral cobalt", "ema — teal check-in", "mood — warm coral", "reminder — calm slate", "ambient — quiet dark"],
  "flow": "question",
  "timeout_s": 60
}
```

Extract the word before the dash as the `default_flow` value.

**Via AskUserQuestion fallback:**
```
question: "Default cenno panel theme?"
options: ["question (cobalt)", "ema (teal)", "mood (coral)", "reminder (slate)"]
```

### Step 3 — Log answers

**Via cenno MCP:**
```json
{
  "title": "Should cenno log every answer to a file?",
  "body_md": "Useful for journaling or context-building loops.",
  "input": { "kind": "confirm" },
  "flow": "question",
  "timeout_s": 60
}
```

**Via AskUserQuestion fallback:**
```
question: "Log all cenno answers to a file?"
options: ["Yes", "No (default)"]
```

### Step 4 — Log file path (only if Step 3 → yes)

**Via cenno MCP:**
```json
{
  "title": "Where should cenno write the log?",
  "body_md": "Enter a full path, e.g. ~/Brains/brain/Daily/cenno-log.md",
  "input": { "kind": "text" },
  "flow": "question",
  "timeout_s": 90
}
```

**Via AskUserQuestion fallback:** ask `"Log file path (default: ~/Desktop/cenno-log.md)?"` as a free-text question.

If the user leaves it blank or times out, use `~/Desktop/cenno-log.md`.

### Saving the config

After all answers are collected, write `~/.claude/skills/cenno/config.json`:

```bash
cat > ~/.claude/skills/cenno/config.json << 'EOF'
{
  "default_timeout_s": <answered_value>,
  "default_flow": "<answered_value>",
  "log_answers": <true|false>,
  "log_file": "<answered_value>"
}
EOF
```

Then confirm to the user: *"cenno configured — defaults saved to ~/.claude/skills/cenno/config.json."* Show the saved values in one line.

---

## Configuring the cenno APP (`~/.cenno`)

> This is different from the skill config above. The **skill** config
> (`~/.claude/skills/cenno/config.json`) tunes how *you* call cenno. The **app**
> config (`~/.cenno/`) tunes the cenno *app itself* — panel size, where it
> appears, default theme/timeout, custom widget types, and the color theme. Full
> reference: the app's `docs/CONFIG.md`.

Run this when the user says "configure the cenno app", "customize cenno", "change
the panel size / position / theme", "make my own cenno widget", or "set up
`~/.cenno`". **Interview the user — don't guess.** By default conduct the
interview **through cenno itself** (`ask_sequence`, so the user picks answers in
the very panels they're customizing); fall back to `AskUserQuestion` only if
cenno isn't running.

### The interview (via cenno — default)

Read any existing config first so you preset, not clobber:

```bash
cat ~/.cenno/config.json 2>/dev/null || echo "{}"
```

Then ask the geometry/defaults in one panel sequence:

```json
{
  "flow": "question",
  "questions": [
    { "title": "How wide should the panel be?", "input": { "kind": "choice" },
      "choices": ["Compact (380)", "Default (420)", "Roomy (460)", "Wide (520)"], "progress": { "step": 1, "total": 4 } },
    { "title": "Where should it appear?", "input": { "kind": "choice" },
      "choices": ["Top-right", "Top-left", "Bottom-right", "Bottom-left", "Center", "Remember where I drag it"], "progress": { "step": 2, "total": 4 } },
    { "title": "Default color theme when an agent doesn't pick one?", "input": { "kind": "choice" },
      "choices": ["mood", "question", "ema", "reminder", "ambient"], "progress": { "step": 3, "total": 4 } },
    { "title": "Default wait time before a prompt times out?", "input": { "kind": "choice" },
      "choices": ["30 seconds", "60 seconds", "90 seconds", "2 minutes"], "progress": { "step": 4, "total": 4 } }
  ]
}
```

Then ask the two **optional** add-ons (only if the user seems interested, or they
asked for a custom widget / new colors):

```json
{
  "flow": "ema",
  "questions": [
    { "title": "Add a custom widget you can ask with by name?", "input": { "kind": "choice" },
      "choices": ["No thanks", "rating5 (1–5 scale)", "nps (0–10 scale)"], "progress": { "step": 1, "total": 2 } },
    { "title": "Recolor a flow theme?", "input": { "kind": "choice" },
      "choices": ["Leave the built-in colors", "Let me name a flow + color"] }
  ]
}
```

If they pick "Let me name a flow + color", follow up with two `text` prompts
(which flow; which color as a hex like `#6C4DF6`) — or just `AskUserQuestion` for
a freeform hex, since typing hex in a panel is fiddly.

### Mapping answers → files

Map the choices and **write `~/.cenno/config.json`** (omit keys the user left at
default; `position` becomes an `anchor` object, or drop it entirely for "remember
where I drag it"):

```bash
mkdir -p ~/.cenno
cat > ~/.cenno/config.json << 'EOF'
{
  "panel": {
    "width": <380|420|460|520>,
    "position": { "anchor": "<top-right|top-left|bottom-right|bottom-left|center>", "margin": 16 }
  },
  "defaults": {
    "timeout_s": <30|60|90|120>,
    "flow": "<mood|question|ema|reminder|ambient>"
  },
  "widgets": {
    // include ONLY if they chose a custom widget:
    "rating5": {
      "childIds": ["scale"],
      "components": [
        { "id": "scale", "component": "Scale", "min": 1, "max": 5,
          "minLabel": "poor", "maxLabel": "great", "value": { "path": "/scale" },
          "selectAction": { "event": { "name": "submit-scale", "context": { "value": { "path": "/scale" }, "via": "choice" } } } }
      ]
    }
  }
}
EOF
```

(For `nps`, use the same template with `"max": 10`, `"minLabel": "not likely"`,
`"maxLabel": "very likely"`. To add more controls per widget, see the app's
`docs/design/CONTROLS.md`.)

If they chose to recolor a flow, **also write `~/.cenno/tokens.json`** (W3C DTCG;
include only the colors changed):

```bash
cat > ~/.cenno/tokens.json << 'EOF'
{ "color": { "$type": "color", "flow": { "<flow>": { "$value": "<#hex>" } } } }
EOF
```

### After saving

cenno reads `~/.cenno` **at launch**, so tell the user to **quit and reopen cenno**
(or "Quit cenno" from the tray, then relaunch) for changes to take effect.
Confirm in one line what changed, e.g. *"Saved ~/.cenno: 460px panel, top-right,
ema default, 90s timeout, + a rating5 widget. Relaunch cenno to apply."*

### `AskUserQuestion` fallback (cenno not running)

If `ask_user`/`ask_sequence` aren't available, run the same interview with
`AskUserQuestion` (same questions/choices), then write the same files.

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

## Widget advisor — which widget for which question

Match the *shape of the decision* to the widget. Work down this table and take the first row that fits; prefer a built-in `input.kind` over an `a2ui` payload whenever both could work — built-ins are cheaper to send and harder to get wrong.

| the question is… | use | notes |
|---|---|---|
| yes/no before an action | `confirm` | timeout ≠ yes — never proceed on `answered: false` |
| pick one of 2–5 named options | `choice` | keep labels short; ≤5 or the panel crowds |
| pick one of 6+ / an open set | `text` (or `choice` with the top 4 + let text catch the rest) | don't scroll the user through a long list |
| a rating or intensity, 1–7 fits | `scale` | built-in is hard-wired 1–7, discrete |
| a rating with another range or labeled ends (1–5, NPS 0–10) | `a2ui` **Scale** | discrete numeral row, `min`/`max`/`minLabel`/`maxLabel` |
| a continuous quantity (percent, budget, "how much") | `a2ui` **Slider** | `min`/`max`/`step`; commit fires `selectAction` — or omit it and pair with a **Button** for an explicit confirm |
| open-ended, a sentence or more | `voice_text` | same as `text` plus a mic — default to it over `text` when the answer is likely > a few words |
| a date, time, or deadline | `a2ui` **DateTimeInput** | native picker; `enableDate`/`enableTime` |
| judge or pick among visual artifacts | `a2ui` **Image** (+ ChoicePicker or Buttons) | see image conventions below |
| several related questions | `ask_sequence` | one panel, instant advance; hand-roll a loop only when a later question depends on an earlier answer |
| FYI only, no answer needed | `none` | auto-dismisses; don't dress information up as a question |

Two rules that trump the table: **one decision per panel** (if a prompt needs two widgets to answer two things, it's two questions — use `ask_sequence`), and **the widget must constrain the answer to what you can act on** (if you'd have to parse or re-ask, you picked too loose a widget).

### Showing images — conventions

The release build's CSP allows only `img-src 'self' data:` — **remote URLs and local file paths will not render.** Embed images as base64 `data:` URIs, and remember the whole `a2ui` payload is capped at 256 KiB, so downscale/compress first (~600px JPEG ≈ 60 KB is plenty at panel size):

```bash
sips -Z 640 -s format jpeg -s formatOptions 60 input.png --out /tmp/small.jpg
B64=$(base64 -i /tmp/small.jpg)   # → "data:image/jpeg;base64,$B64"
```

Scaling is fixed by convention — images are display-only, no zoom/pan:

- Always set `"fit": "contain"` (the API default is `fill`, which distorts).
- Default `"variant": "mediumFeature"` (120px cap) when the image accompanies the question; `"largeFeature"` (160px, full width) when the image **is** the question — e.g. "keep this one?".
- `icon` (24px) / `avatar` (44px, round) for decoration only, never for content the user must judge.

A "keep this thumbnail?" prompt — Image + choice buttons:

```json
{
  "title": "Keep this thumbnail?",
  "timeout_s": 90,
  "a2ui": [
    { "version": "v0.9", "createSurface": { "surfaceId": "main", "catalogId": "cenno:catalog/v1" } },
    { "version": "v0.9", "updateComponents": { "surfaceId": "main", "components": [
      { "id": "root",  "component": "Column", "children": ["col"] },
      { "id": "col",   "component": "Column", "children": ["title", "img", "picker"] },
      { "id": "title", "component": "Text", "text": "Keep this thumbnail?", "variant": "h2" },
      { "id": "img",   "component": "Image", "url": "data:image/jpeg;base64,…",
        "description": "generated thumbnail candidate", "fit": "contain", "variant": "largeFeature" },
      { "id": "picker", "component": "ChoicePicker", "options": [
          { "label": "Keep", "value": "keep" },
          { "label": "Regenerate", "value": "regenerate" },
          { "label": "Skip", "value": "skip" } ],
        "value": { "path": "/pick" },
        "selectAction": { "event": { "name": "submit-pick",
          "context": { "value": { "path": "/pick" }, "via": "choice" } } } }
    ] } },
    { "version": "v0.9", "updateDataModel": { "surfaceId": "main", "path": "/", "value": {} } }
  ]
}
```

To choose among N candidates, prefer one panel per candidate via `ask_sequence` (keep/reject each) over cramming a grid into one panel — the panel is small and the per-image verdict is cleaner data.

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

**Multi-step questionnaire** — prefer the [`ask_sequence`](#the-ask_sequence-tool--several-questions-in-one-panel) tool, which keeps the panel up and advances instantly between questions. Only hand-roll a loop of single `ask_user` calls (carrying `progress` yourself) when the next question genuinely depends on the previous answer:

```json
{ "title": "Energy level?", "input": { "kind": "scale" },
  "flow": "ema", "progress": { "step": 1, "total": 3 } }
```
Fire them sequentially, each blocking on the previous answer. If any step times out, stop and don't record a partial result.

## The `ask_sequence` tool — several questions in one panel

When you have a short questionnaire — a few related questions where the panel should stay up and advance instantly between them — call `ask_sequence` instead of firing `ask_user` N times. The questions run back-to-back in a single panel: answering one swaps the content to the next with no hide/reshow gap, and the panel hides only after the last.

```jsonc
{
  "questions": [ /* an array of ask_user args, in order */ ],
  "flow": "ema"   // optional default flow applied to any question that lacks its own
}
```

- **Progress dots auto-fill.** You don't need to set `progress` on each question — `ask_sequence` fills `{step: i+1, total: N}` for any question that omits it, so the dots advance 1/3 → 2/3 → 3/3 on their own.
- **Per-question timeout ends the run early.** If a question times out, the run stops there. The returned `answers` array is exactly as long as the user got: the last entry is then the `{answered: false, prompt_id}` timeout shape, and no later questions are shown.
- **Answers come back as an ordered array** aligned to `questions`. Returns `{"answers": [ {answer, via, elapsed_s}, ... ]}`. Each question is also recorded as its own history row, same as `ask_user`.

A concrete 3-question check-in — a mood scale (custom 1–5 via `a2ui`), a choice, then a short text:

```json
{
  "flow": "ema",
  "questions": [
    {
      "title": "How's your mood right now?",
      "timeout_s": 60,
      "a2ui": [
        { "version": "v0.9", "createSurface": { "surfaceId": "main", "catalogId": "cenno:catalog/v1" } },
        { "version": "v0.9", "updateComponents": { "surfaceId": "main", "components": [
          { "id": "root",  "component": "Column", "children": ["col"] },
          { "id": "col",   "component": "Column", "children": ["title", "scale"] },
          { "id": "title", "component": "Text", "text": "How's your mood right now?", "variant": "h2" },
          { "id": "scale", "component": "Scale", "min": 1, "max": 5,
            "minLabel": "low", "maxLabel": "great", "value": { "path": "/scale" },
            "selectAction": { "event": { "name": "submit-scale",
              "context": { "value": { "path": "/scale" }, "via": "choice" } } } }
        ] } },
        { "version": "v0.9", "updateDataModel": { "surfaceId": "main", "path": "/", "value": {} } }
      ]
    },
    {
      "title": "What's pulling at your attention?",
      "input": { "kind": "choice" },
      "choices": ["Work", "People", "Body", "Nothing in particular"],
      "timeout_s": 60
    },
    {
      "title": "Anything you want to note?",
      "input": { "kind": "text" },
      "timeout_s": 60
    }
  ]
}
```

You'll get back `{"answers": ["4"-shaped, "Work"-shaped, "…text…"-shaped]}` — three ordered entries (fewer if a step timed out). Prefer `ask_sequence` over a hand-rolled loop of `ask_user` calls whenever the questions belong together: it keeps the panel up and the advance is instant.

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
