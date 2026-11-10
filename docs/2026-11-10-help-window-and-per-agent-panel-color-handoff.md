# Help window and per-agent panel color — handoff

Date: 2026-11-10 · Base: `main` (0.5.0)

## Problem

1. cenno has no Help/About window. Users can't see the version, who made it, or the
   third-party credits.
2. Panels often show up coral/pink. That color is the `mood` flow theme
   (`color.flow.mood` = `#FF6250` in `tokens/tokens.json`). The agent picks the flow for each
   prompt (`flow` in `src-tauri/src/protocol.rs`, rendered as `data-flow` in
   `src/PromptPanel.tsx:255`, default `question`). The user wants the panel color to be
   **configurable** and, **by default, based on which agent sent the prompt**. The server already
   sets `prompt.agent` (`protocol.rs:171-174`), and agents can't fake it.

## Scope

### A. Help window
- Add a "Help & About…" tray menu item (`src-tauri/src/tray.rs`) that opens a small window.
  Follow the pattern of `open_settings_window` (`src-tauri/src/lib.rs:621`, `src/SettingsWindow.tsx`).
- Contents:
  - App name, mark (`docs/design/BRAND.md`) and **version**. Read the version at runtime
    (`@tauri-apps/api/app` `getVersion()`); don't hardcode it.
  - Author links: GitHub `https://github.com/glebis` and LinkedIn
    `https://www.linkedin.com/in/glebkalinin/`.
  - A link to the project repo/README and the docs (`docs/CONFIG.md`).
  - **Acknowledgements:** build this from `NOTICE`: the vendored agent marks (simple-icons
    CC0-1.0, @lobehub/icons-static-svg), the Apache-2.0 license, the AI-assistance statement,
    and the major runtime dependencies (Tauri, React, Apple SpeechTranscriber). Keep NOTICE as
    the source of truth. Either generate the list from it or add a test that fails when they drift.
- External links open in the default browser through the opener plugin (no in-webview navigation).
  These are user-clicked links, so the no-network rule in AGENTS.md is not violated. Check
  that CSP and capabilities allow it in a bundled build.
- Styling comes from tokens only (`tokens/tokens.json` → `npm run tokens`). Never edit
  `src/styles/` by hand. It must look finished in both light and dark mode.

### B. Per-agent panel color
- New resolution order for a panel's theme color:
  1. the user's explicit per-agent override in `~/.cenno/config.json`
     (e.g. `"agent_colors": {"claude": "#D97757", "codex": "#1E4FD8"}`). Explicit user config
     beats everything,
  2. **the agent's own choice**: if the prompt sets `flow`, keep today's flow theme color
     (mood/question/ema/…). Agent-chosen color stays the default,
  3. only when the agent sent **no** `flow`: a per-agent color, so different agents look
     different. Known agents get a built-in hue near their mark in `src/agentLogos.tsx`;
     unknown agents get a stable color hashed from the agent name, from a fixed palette.
     Every color must be dark enough for white text: check WCAG AA against
     `color.text.default`,
  4. fallback when there is no agent name either: the current `question` blue.
- `flow` keeps controlling layout/semantics (EMA progress dots etc.) as today. Note: today a
  missing `flow` renders as `question` (`PromptPanel.tsx:255`, `data-flow={prompt.flow ?? "question"}`).
  Separate "no flow given" from "flow=question" so step 3 can apply.
- Add the setting to the Settings window: a list of the agents seen so far (from the
  history DB), each with a color picker (used when the agent sends no flow, or as an
  override), plus "reset to default".
- Validate config colors in `config.rs` (hex only; ignore and log anything invalid). Document
  the new keys in `docs/CONFIG.md` and in `skills/cenno/references/configure.md`.

## Out of scope
- Changing the A2UI catalog or the `ask_user` tool schema.
- Release/version bump, updater, Homebrew cask.
- Companion iOS/watch apps.

## Done when
- The tray has "Help & About…". The window shows the runtime version, GitHub + LinkedIn
  links, and acknowledgements that match NOTICE.
- A prompt with `flow` keeps its flow color. Prompts *without* `flow` from two different agents
  show different colors with no config, and the same agent always shows the same color. A config override wins. Invalid config falls back cleanly.
- All text on every default agent color passes WCAG AA (test it).
- New logic is unit-tested (Rust config parsing + TS color resolution). Existing tests still pass.
- Screenshots of the help window and of panels from two agents are in your report
  (`./scripts/demo.sh` against a running app, or `scripts/visual-qa.sh`).

## Checks
```
cd src-tauri && cargo test
npx vitest run
npm run typecheck:tests
npm run tokens
```
