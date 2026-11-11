# External configuration — `~/.cenno`

cenno reads two optional files from `~/.cenno/` at launch. Both are
hot-droppable and entirely optional — with neither present, the built-in
defaults apply. A malformed file is ignored (logged, never fatal). Changes
take effect on the next launch.

```
~/.cenno/
  config.json    panel geometry/position · prompt defaults · custom widgets
  tokens.json    design-token overrides (W3C DTCG)
```

Working examples: [`docs/examples/dot-cenno/`](examples/dot-cenno/).

## `config.json`

```jsonc
{
  "panel": {
    "width": 460,                 // logical points (built-in 420; clamped 240–1200)
    "min_height": 240,            // content-driven floor (built-in 240)
    "max_height": 620,            // content-driven ceiling (built-in 560)
    "position": { "anchor": "top-right", "margin": 24 }
  },
  "defaults": {
    "timeout_s": 90,              // when an agent omits timeout_s (built-in 120)
    "flow": "ema"                 // when an agent omits flow (mood|question|ema|reminder|ambient)
  },
  "agent_colors": {               // per-agent panel colors (hex only)
    "claude-code": "#A8482B",
    "codex": "#6E5A12"
  },
  "capture": {
    "enabled": true,              // global screen-context kill switch
    "passive_sampling": false,    // opt-in; on-demand reads still allowed
    "denylist_bundles": ["com.example.SecretApp"],
    "denylist_hosts": ["private.example"],
    "redaction": true
  },
  "widgets": { /* see below */ }
}
```

Bundle identifiers are exact, case-sensitive matches. A denied host also
denies its subdomains (`private.example` blocks `mail.private.example`) but not
lookalikes such as `notprivate.example`. Built-in password-manager and Keychain
bundle identifiers are always denied. Capture and high-confidence secret
redaction default on; passive sampling defaults off.

**Position** is either a screen-corner anchor or explicit coordinates:

```jsonc
"position": { "anchor": "top-right", "margin": 24 }   // top-left|top-right|bottom-left|bottom-right|center
"position": { "x": 1200, "y": 60 }                     // logical points, top-left origin
```

It sets where a fresh panel appears; cenno still remembers where you drag it.
Unknown keys are rejected (so typos surface as "ignored malformed config" rather
than silently doing nothing).

## Per-agent panel colors

`agent_colors` maps an agent name (the exact name the agent reports, e.g.
`claude-code` or `codex-mcp-client`) to the panel surface color used for its
prompts. Keys are matched case-insensitively; values must be opaque
`#RGB` or `#RRGGBB` hex. Invalid entries are ignored (and logged) instead of
poisoning the whole config.

A panel's color is resolved in this order:

1. **An explicit `agent_colors` entry** for the agent — user config wins.
2. **The agent's own `flow`** — `mood`, `question`, `ema`, `reminder`, or
   `ambient` keeps that flow's hue (`color.flow.*` in `tokens.json`).
3. **The agent's built-in color** — only when the prompt sends no `flow`.
   Known agents use a hue near their mark; unknown agents get a stable color
   hashed from the name out of a fixed palette. Every built-in color clears
   WCAG AA for the panel's white text.
4. **`question` blue** — when there is no agent name and no flow.

A configured `defaults.flow` counts as an explicit user choice, so it keeps
its flow color for every agent (step 2) and the per-agent colors in step 3 are
not used. Leave `defaults.flow` out of the file to let agents look different
by default.

The Settings window's **Panel colors — per agent** section lists the agents
seen so far (from the history database) with a color picker and a reset. Like
the rest of `~/.cenno`, changes apply on the next launch (the Settings window
writes the file, and `agent_colors` is read at startup).

```jsonc
"agent_colors": {
  "claude-code": "#A8482B",
  "codex": "#6E5A12"
}
```

## Custom widgets (declarative)

Define new `input.kind` values as **templates that compose built-in controls** —
no code, validated like any agent payload. An agent then invokes one by name:
`ask_user({ "title": "...", "input": { "kind": "rating5" } })`.

```jsonc
"widgets": {
  "rating5": {
    "childIds": ["scale"],
    "components": [
      {
        "id": "scale", "component": "Scale",
        "min": 1, "max": 5, "minLabel": "poor", "maxLabel": "great",
        "value": { "path": "/scale" },
        "selectAction": {
          "event": { "name": "submit-scale", "context": { "value": { "path": "/scale" }, "via": "choice" } }
        }
      }
    ],
    "dataModel": {}
  }
}
```

A template is the `input` half of a desugared prompt: `childIds` (which
components mount under the question), `components` (the catalog components — see
[design/CONTROLS.md](design/CONTROLS.md) for the inventory), and an optional
`dataModel`. The action contract is the same as built-in controls: a completing
action name starts with `submit` and its context carries `{ value, via }`.

## `tokens.json` — design tokens

Drop a [W3C DTCG](https://www.w3.org/community/design-tokens/) token document to
override the theme. Only the leaves you include change; everything else keeps
its built-in value. Each `$value` leaf becomes a `--cenno-<path>` CSS variable
(e.g. `color.flow.mood` → `--cenno-color-flow-mood`) injected over the built-in
theme.

```jsonc
{
  "color": {
    "$type": "color",
    "flow": {
      "mood":     { "$value": "#E85D9C" },
      "question": { "$value": "#6C4DF6" }
    }
  }
}
```

The full token surface (names, scale, components) is documented in
[design/TOKENS.md](design/TOKENS.md); the canonical source is
[`tokens/tokens.json`](../tokens/tokens.json).
