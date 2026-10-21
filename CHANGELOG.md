# Changelog

All notable changes to cenno are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/).

## [0.4.1] — 2026-08-08

### Added

- **Panels can render a table.** An `a2ui` `Table` shows up to 4 columns and 12
  rows of plain data — plans, metrics, options — with an optional caption and
  per-column end alignment for numbers. Rows wash faintly on hover and hovering
  a column header washes the whole column, so the eye can follow one record
  across four columns without losing the line.
- **Panels can render radios and checkboxes.** `ChoicePicker` now honours
  A2UI's `displayStyle`: `"checkbox"` renders a real radio/checkbox list with
  arrow-key navigation and a single tab stop; the default stays chips, so every
  existing payload looks exactly as before.
- **Rate several things in one panel.** A new `RatingMatrix` lays out one row
  per item — either a row of numeral targets (`mode: "matrix"`) or a slider per
  item (`mode: "sliders"`) — and answers with a JSON object keyed by row value,
  e.g. `{"pace":4,"usefulness":3}`. Nothing submits until you press **Done**,
  so a three-row evaluation can actually be filled in; clicking a column numeral
  rates every row at once, and a partial answer is allowed.
- **Choose from pictures.** A new `ImageChoice` renders up to 10 image tiles in
  an adaptive grid (1–5 columns depending on the count, never leaving a lone
  tile on the last row), round selection marks for one answer and square for
  several, with an optional `fit: "contain"` for images that must not be cropped.
- **Images now say when they fail.** A failed image names the file it could not
  read and keeps its slot in the layout — a missing preview is not a missing
  option, so the tile stays selectable. Local files load through a bounded,
  extension-checked, magic-byte-checked Rust command.

### Changed

- **Images are sandboxed to `data:` URIs and local file paths.** Remote URLs are
  rejected at the boundary with an explanation, because cenno never fetches.

### Fixed

- A matrix answer keeps **every** rated row, not just the last one you touched,
  and a pre-seeded value can no longer override the rating you just gave.
- A slider range such as `min: 0, max: 1e9` is now rejected instead of hanging
  the panel while it materialises a billion numerals.

## [0.4.0] — 2026-08-06

> Includes everything staged under the never-published 0.3.2 bump.

### Added

- **Every panel now takes a free-form answer.** Choice, scale, confirm and
  custom-widget panels — and even pure notifications — render a voice-capable
  text field below their own control, so you can always ignore the agent's
  options and answer in your own words. On Mac, iPhone and Watch. The mic is
  offered on every text field, not only when the agent asked for dictation.
- **Screen context (L1a)** — cenno can read the focused window's accessibility
  snapshot and expose it to agents through a guarded MCP tool.
- **Voice-mute** — prompts can open silently (`--muted`), with an in-panel
  mute/unmute toggle; the final mute state is reported in the answer.

### Changed

- **Voice-out timing** — the panel now appears when speech actually starts,
  and stopping a voice fades it out (~200 ms) instead of cutting it off;
  stop also cancels any in-flight synthesis.
- **cenno is now pronounced the Italian way** ("CHEN-no") when read aloud.
- The **About** tab shows the live app version instead of a hard-coded one.

### Fixed

- **Sloppy agent formatting is repaired on the way in.** A body sent with
  literal `\n` / `\t` escapes rendered those two characters verbatim in the
  panel; they now become real whitespace. Windows paths and regexes
  (`C:\path`, `\d+`) are left alone.
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
