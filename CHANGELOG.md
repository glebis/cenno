# Changelog

All notable changes to cenno are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/).

## [0.3.0] — 2026-06-22

### Added

- **Sound-out (voice-out):** cenno can read a prompt aloud when it appears.
  Opt-in, urgency-gated, with an optional short `say` summary. Two engines —
  the fast macOS system voice and **Supertonic**, a fully on-device neural
  voice (10 styles, F1–F5 / M1–M5) that downloads and runs locally.
- **In-app settings window** (tray → "cenno settings…"): a tabbed
  Settings / Integration / About window styled after the website. Configure
  voice (engine, voice, urgency, **audio output device**, model download /
  delete), behavior (launch at login, hide from Dock), and defaults — written
  to `~/.cenno` and applied without a restart.
- **`dismiss_pending` MCP tool:** unparks pending prompts and hides the panel
  immediately, for agent-driven voice loops that capture the answer via an
  external speech-to-text.
- **Cross-device prompt routing** (second screens) and an iOS / watchOS
  companion app that surfaces prompts on the phone and Watch.

### Fixed

- **CloudKit relay no longer crashes the app** on builds without the iCloud
  entitlement: the relay is gated on the entitlement being present instead of
  trapping inside CloudKit (uncatchable from Swift).

## [0.2.0] — 2026-06-11

### Added

- `Slider` catalog control (standard `SliderApi` + `minLabel`/`maxLabel`/
  `selectAction`): continuous "how much" answers, committing on thumb
  release or Enter.
- `DateTimeInput` catalog control (standard API + `submitAction`): native
  date/time pickers for "when?" answers.
- `Image` catalog control (standard `ImageApi`): display-only visual context
  in prompts.
- In-app updates from GitHub releases via the Tauri updater, with
  **Check for updates…** in the tray menu (signature-verified; installs and
  restarts only after confirmation).
- `docs/design/CONTROLS.md`: the catalog inventory and the recipe for
  adding controls.
- Repository meta: LICENSE (Apache-2.0), NOTICE, AUTHORSHIP, SECURITY,
  CONTRIBUTING, CODE_OF_CONDUCT, CHANGELOG.

## [0.1.0] — 2026-06-10

Initial release.

- `ask_user` MCP tool: text / voice-text stub / choice / scale / confirm
  inputs, flow color themes, dot pagination, timeouts.
- Focus-preserving non-activating panel (NSPanel), suppression policy
  (pause presets, fullscreen quiet mode), tray menu, launch at login.
- Raw `a2ui` payload passthrough with boundary validation.
- Local SQLite history with JSON/CSV export (`cenno export`).
- CLI (`cenno ask`) and MCP stdio bridge (`cenno --mcp-stdio`).
