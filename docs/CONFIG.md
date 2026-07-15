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
