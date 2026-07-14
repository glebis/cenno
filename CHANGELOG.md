# Changelog

All notable changes to cenno are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/).

## [0.4.0] — 2026-07-14

> Includes everything staged under the never-published 0.3.2 bump.

### Added

- **Voice-mute** — prompts can open silently (`--muted`), with an in-panel
  mute/unmute toggle; the final mute state is reported in the answer.

### Changed

- **Voice-out timing** — the panel now appears when speech actually starts,
  and stopping a voice fades it out (~200 ms) instead of cutting it off;
  stop also cancels any in-flight synthesis.
- **cenno is now pronounced the Italian way** ("CHEN-no") when read aloud.
- The **About** tab shows the live app version instead of a hard-coded one.

### Fixed

- Tray: **"Show pending prompt"** is now truthful and functional.

- **The panel no longer times out while you're typing — and never loses your
  text.** Editing a field now holds the prompt open (the deadline is pushed far
  out on every keystroke), and after you stop you get at least a 45-second
  think-window before it can expire. The keep-alive reaches the agent side too,
  so a late answer still delivers, and in-progress text is saved as a draft as a
  safety net. Previously a prompt could close mid-edit and discard what you'd
  typed.
- **Voice-out respects live config changes.** The prompt
  panel decided whether to speak using the config loaded at app launch, so
  enabling or retuning voice-out in Settings did nothing until a restart —
  while "Test voice" (which reads fresh and skips the gate) worked, masking the
  bug. The panel now re-reads the voice config fresh for every prompt, so
  Settings changes take effect on the next prompt with no restart.

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
