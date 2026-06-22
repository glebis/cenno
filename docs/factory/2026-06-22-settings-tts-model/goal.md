# Goal Contract — Settings surface + TTS model management (v1)

> Source of truth. Agents may propose **Goal Amendments**; they may not silently rewrite this.
> Builds on Supertonic backend (docs/factory/2026-06-22-supertonic-tts/). Folds in cenno-qbn (download) + cenno-kis (voice picker). Tracker: new epic.

## Current state (≤3)
- `~/.cenno/config.json` is **hand-edited**; cenno only *reads* it (`Config::load`) — there is **no write path** and no in-app settings UI.
- Exactly **one window** exists (`main`, swizzled into a non-activating NSPanel for prompts); the tray menu has pause/updates/quit but **no Settings**.
- Supertonic TTS works, but engine/voice/model-location are config-only and the ~380 MB model must be **staged by hand** (no download, no custom path).

## Desired future state (≤3)
- A **"Settings…" tray item** opens an in-app settings surface that **reads and writes** `~/.cenno/config.json`.
- First section — **Voice-out**: engine (system / Supertonic), voice picker, and model management: a **Download** button (with progress) + a **custom model path** field.
- Users never hand-edit JSON for these; AVSpeech fallback still covers "model not present / invalid path."

## Current constraint (Theory of Constraints)
The feature is unusable by anyone but its author — the model must be hand-staged and config hand-edited. A settings + download surface is the lever that makes voice-out (and future settings) actually adoptable.

## Target user / job (JTBD)
Solo dev (and eventually other cenno users) wants to enable and configure voice-out — pick a voice, get the model — **without touching JSON or the terminal**.

## Non-negotiable constraints (≤5)
1. **Config writes must not clobber.** Round-trip every known field (widgets, panel, routing, defaults, tts) — saving the TTS section must never drop the user's other settings.
2. **Stay R1 at speak time.** The model download is the *only* network, and it is explicit + user-initiated from a visible source (Hugging Face). No prompt content ever leaves the device.
3. **Custom path precedence:** if `tts.model_path` is set and valid → use it; else the default cache; else AVSpeech fallback. A bad path never breaks speech.
4. **Reuse the existing config schema + tts fields** — settings is a UI over them, not a second config system. No adapter layer.
5. **Safe download:** a partial/failed/cancelled download must not leave a broken model dir that defeats the fallback (download to temp, atomic move on success + integrity check).

## Desired outcomes (solution-independent, measurable; ≤5)
1. Tray "Settings…" opens a settings surface reflecting current TTS config (evidence: opens, shows engine/voice/model status).
2. Changing engine/voice persists to `~/.cenno/config.json` and the next prompt uses it (evidence: file updated + audible change).
3. "Download model" fetches supertonic-3 to the cache with visible progress; from an empty cache, Supertonic then speaks (evidence: empty→download→speaks).
4. A valid custom `model_path` is used instead of the cache (evidence: set path → speaks from there).
5. Config write preserves all other fields (evidence: round-trip test — widgets/panel/routing intact).

## Smallest shippable slice   <!-- required -->
Tray **"Settings…"** opens a settings surface with one **Voice-out** section: engine toggle (system/supertonic), voice picker (Supertonic styles), and a model block — status + **Download** (with progress) + **custom path** field — persisted via a new `write_user_config` command. **No** other settings sections, no live audio preview, no per-prompt overrides yet.

## Stop condition   <!-- required -->
Stop and ask if: the settings-window architecture destabilizes the existing NSPanel prompt surface; config write can't safely round-trip (clobber risk); or the HF download can't be made robust (resumable/atomic/verifiable) within this slice.

## Success evidence (≤5)
Maps to outcomes: settings opens + reflects config · engine/voice change persists + audible · empty-cache download→speaks · custom path used · config-write round-trip test → `evidence/verify.log` + a screenshot of the settings surface.

## Visual checkpoints
**Blocking** — this is a new user-visible UI surface. Screenshot of the settings window at the primary viewport.

## Risk classification
**R2 — user-facing, low-stakes.** Writes the user's own config + downloads a model. EU AI Act: Art 5? **no** · Art 50? **no**. Main risk is config-clobbering (constraint 1) and a partial download breaking fallback (constraint 5).

## Rollback note
Settings is additive (new window + tray item + `write_user_config`). `tts.engine=system` still disables Supertonic. Revert the commit; hand-editing config still works.

## Risks (≤5)
1. **Config clobber** on write (dropping widgets/routing/unknown shapes).
2. Settings window vs the NSPanel swizzle — window-management interactions.
3. ~380 MB download UX: progress, cancel, resume, disk, integrity.
4. Custom path validation (wrong dir → must fall back, not crash).
5. Scope creep into a general settings framework before the one section ships.

## Non-goals (≤5)
- Other settings sections (panel geometry, theme, routing UI) — later.
- Live audio preview in settings, per-prompt voice overrides.
- A general settings/preferences framework or schema-driven form engine.
- Cloud TTS providers (cenno-fx5).

## Release constraints
- macOS 26+. Download is one-time, user-initiated; speak-time stays offline.

## Tracker
**bd** — new epic; absorbs cenno-qbn (download) + cenno-kis (voice picker) as children, plus: settings window + tray item, `write_user_config` command, Voice-out settings UI, custom-path support.

---
**Fail rule:** if a goal can't produce evidence, it's a wish with better formatting — it doesn't pass.

## Size + risk triage
**Size: L** (new window surface + config-write command + settings UI + robust model download + custom path). **Plan approval REQUIRED.** First plan decision: the **settings surface architecture** — a dedicated normal `WebviewWindow` opened from the tray (recommended; keeps the NSPanel prompt surface untouched) vs. a "settings mode" in the existing panel.
