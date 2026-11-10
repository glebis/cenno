# Configuring cenno

There are two configs. Keep them apart:

| file | tunes | owns |
|---|---|---|
| `~/.claude/skills/cenno/config.json` | how the **agent** uses cenno | answer logging, CLI vs MCP |
| `~/.cenno/config.json` (+ `tokens.json`) | the **app** itself | panel size and position, **default timeout and theme**, custom widgets, colors |

Default timeout and theme live only in the app config; the skill config does not duplicate them. The app's own Settings window edits the same defaults, so a user who only wants those two changed can do it there. Full app reference: https://github.com/glebis/cenno/blob/main/docs/CONFIG.md

Run every interview **through cenno** (`ask_sequence`) when it's available, so the user answers in the panels being configured. If cenno isn't running, ask the same questions with `AskUserQuestion`, then write the same files. **Interview — don't guess.** Read the existing file first and preset from it rather than clobbering it.

## Skill config — "set up cenno", "log my cenno answers"

```bash
cat ~/.claude/skills/cenno/config.json 2>/dev/null || echo "{}"
```

```json
{
  "flow": "question",
  "questions": [
    { "title": "Log every cenno answer to a file?",
      "body_md": "One line per question — useful for journaling or context-building loops.",
      "input": { "kind": "confirm" } },
    { "title": "Where should the log go?",
      "body_md": "A full path, e.g. `~/Brains/brain/Daily/cenno-log.md`. Leave empty for `~/Desktop/cenno-log.md`.",
      "input": { "kind": "text" } }
  ]
}
```

Skip the path answer if the first is "no". Then write the file, keeping any key the interview didn't cover (`preferred_input` defaults to `"mcp"`; set `"cli"` only when the user wants the CLI even with MCP available):

```bash
cat > ~/.claude/skills/cenno/config.json << 'EOF'
{
  "log_answers": <true|false>,
  "log_file": "<path>",
  "preferred_input": "mcp"
}
EOF
```

Drop `default_timeout_s` / `default_flow` if an older file has them — they are superseded by the app config, and leaving them in suggests they still matter. Confirm in one line: *"cenno skill configured — logging to ~/Brains/brain/Daily/cenno-log.md."*

## App config — "configure the cenno app", panel size/position/theme, custom widgets, colors

```bash
cat ~/.cenno/config.json 2>/dev/null || echo "{}"
```

Geometry and defaults in one sequence:

```json
{
  "flow": "question",
  "questions": [
    { "title": "How wide should the panel be?", "input": { "kind": "choice" },
      "choices": ["Compact (380)", "Default (420)", "Roomy (460)", "Wide (520)"] },
    { "title": "Where should it appear?", "input": { "kind": "choice" },
      "choices": ["Top-right", "Top-left", "Bottom-right", "Bottom-left", "Center", "Remember where I drag it"] },
    { "title": "Default color theme when an agent doesn't pick one?", "input": { "kind": "choice" },
      "choices": ["mood", "question", "ema", "reminder", "ambient"] },
    { "title": "Default wait time before a prompt times out?", "input": { "kind": "choice" },
      "choices": ["30 seconds", "60 seconds", "90 seconds", "2 minutes"] }
  ]
}
```

Then the two **optional** add-ons — only if the user seems interested, or asked for a custom widget or new colors:

```json
{
  "flow": "ema",
  "questions": [
    { "title": "Add a custom widget you can ask with by name?", "input": { "kind": "choice" },
      "choices": ["No thanks", "rating5 (1–5 scale)", "nps (0–10 scale)"] },
    { "title": "Recolor a flow theme?", "input": { "kind": "choice" },
      "choices": ["Leave the built-in colors", "Let me name a flow + color"] }
  ]
}
```

For "Let me name a flow + color", follow up for the flow name and a hex color like `#6C4DF6` — `AskUserQuestion` is easier for hex than a panel text field.

### Mapping answers → files

Write `~/.cenno/config.json`, merging into what's there. Omit keys left at default; `position` becomes an `anchor` object, or is dropped for "remember where I drag it". Timeouts map `30 seconds→30`, `60 seconds→60`, `90 seconds→90`, `2 minutes→120`.

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
  "agent_colors": {
    // ONLY if they chose per-agent colors; exact agent names, hex only.
    // e.g. "claude-code": "#A8482B", "codex": "#0B6B57"
  },
  "widgets": {
    // ONLY if they chose a custom widget:
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

`agent_colors` sets a panel color per agent (exact reported name, hex only). It is an explicit override: it wins even when the agent picks a flow. With no `defaults.flow`, an agent that sends no flow gets its own built-in color (near its mark for known agents, a stable hash for unknown ones). If `defaults.flow` IS set, that flow color applies to every agent instead. The Settings window lists agents seen so far with color pickers.

For `nps`, use the same template with `"max": 10`, `"minLabel": "not likely"`, `"maxLabel": "very likely"`. For more controls per widget, see https://github.com/glebis/cenno/blob/main/docs/design/CONTROLS.md

If they recolored a flow, also write `~/.cenno/tokens.json` (W3C DTCG; only the changed colors):

```bash
cat > ~/.cenno/tokens.json << 'EOF'
{ "color": { "$type": "color", "flow": { "<flow>": { "$value": "<#hex>" } } } }
EOF
```

### After saving

cenno reads `~/.cenno` **at launch** — tell the user to quit cenno from the tray and reopen it. Confirm in one line, e.g. *"Saved ~/.cenno: 460px panel, top-right, ema default, 90s timeout, + a rating5 widget. Relaunch cenno to apply."*
