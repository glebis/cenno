# Changelog

All notable changes to cenno are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/).

## [0.3.2] — 2026-06-23

### Fixed

- **Voice-out (sound-out) stayed silent after you turned it on.** The prompt
  panel decided whether to speak using the config loaded at app launch, so
  enabling or retuning voice-out in Settings did nothing until a restart —
  while "Test voice" (which reads fresh and skips the gate) worked, masking the
  bug. The panel now re-reads the voice config fresh for every prompt, so
  Settings changes take effect on the next prompt with no restart.

### Changed

- **cenno is now pronounced the Italian way** ("CHEN-no") when read aloud.
- The **About** tab shows the live app version instead of a hard-coded one.

## [0.3.1] — 2026-06-23

### Fixed

- **0.3.0 would not launch ("The application "cenno" can't be opened").** The
  0.3.0 build added restricted iCloud/CloudKit entitlements for the relay, but a
  Developer-ID (non-App-Store) bundle ships no `embedded.provisionprofile` to
  authorize them, so macOS AMFI killed the app at spawn (exit 137). The iCloud
  entitlements are removed; the relay is already gated to run without them, so no
  functionality is lost on the Mac build. The release pipeline now launch-tests
  the built app and rejects restricted entitlements lacking a provisioning
  profile, so this class of brick cannot ship again.

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
