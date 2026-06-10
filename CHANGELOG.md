# Changelog

All notable changes to cenno are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/).

## [Unreleased]

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

## [0.1.0] — 2026-06-09

Initial release.

- `ask_user` MCP tool: text / voice-text stub / choice / scale / confirm
  inputs, flow color themes, dot pagination, timeouts.
- Focus-preserving non-activating panel (NSPanel), suppression policy
  (pause presets, fullscreen quiet mode), tray menu, launch at login.
- Raw `a2ui` payload passthrough with boundary validation.
- Local SQLite history with JSON/CSV export (`cenno export`).
- CLI (`cenno ask`) and MCP stdio bridge (`cenno --mcp-stdio`).
