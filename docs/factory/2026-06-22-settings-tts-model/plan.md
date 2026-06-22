# Plan — Settings + TTS model (L, plan-approval gate) — rev2 (post-Codex audit)

Goal approved. Surface decided: **dedicated normal `WebviewWindow`** from a tray "Settings…" item (NSPanel prompt surface untouched). Epic cenno-cmr.
Codex audit (read-only, fresh context) folded in below — see `evidence/audit-plan-codex.md`.

## Architecture
- **One Vite bundle, route by window label.** `main.tsx` reads the current window label **synchronously** (Tauri `getCurrentWindow().label`) *before* the `loadUserConfig().finally(mount)` render: `settings` → `<SettingsView/>`, else prompt `<App/>`. The settings window loads the **same index URL** (no `/settings` route). Client-only render → no SSR/hydration concern.
- Settings window: standard decorations, resizable, ~480×560, label `settings`. **Only `"main"` ever goes through `to_panel()`** (the swizzle at lib.rs:370) — assert that. Opening from a tray app needs explicit `show()` + `set_focus()` + app activation on the main thread; reuse+focus if already open. Verify from `--tray`.

## Build sequence (TDD where logic is pure) — reordered per audit
0. **Schema first (unblocks everything).** Add `model_path: Option<String>` to Rust `TtsConfig` (config.rs) **and** `RawTtsConfig`/`ResolvedTtsConfig` (userConfig.ts). Add a **read-back deserialize test**: a config file containing `tts.model_path` + `engine` + `voice` parses cleanly under `deny_unknown_fields`. *Without this, writing the field bricks config load.*
1. **`write_user_config` (cenno-jpf) — atomic + clobber-safe.** Load existing `config.json` as `serde_json::Value`, deep-merge the incoming `tts` patch, write atomically: **same-dir** temp file → `flush` + `fsync` → rename → `fsync` parent dir, mode `0600`, guarded by a **process-wide write mutex**. Tests: (a) round-trip preserves widgets/panel/routing/defaults; (b) the merged file **re-deserializes** as `Config` (catches deny_unknown_fields regressions). Note: because `deny_unknown_fields` already forbids unknown top-level keys at read time, the config only ever holds modeled keys — Value-merge preserves exactly those; we are NOT promising to preserve arbitrary unknowns.
2. **Config reload after write (fixes "writes don't take effect").** `get_user_config` reloads from disk (or `write_user_config` updates the managed state) so same-process prompt gating sees new `enabled`/`min_urgency`. Frontend refreshes its cache (`loadUserConfig()` re-run) after a successful save.
3. **Custom path precedence + engine invalidation (cenno-dke).** `supertonic::model_dir()` → `config.tts.model_path` if set+valid, else default cache. **Cache the loaded `ENGINE` keyed by resolved dir** (or clear it) so a path change / finished download reloads sessions instead of serving the stale one. Unit-test resolution + validation (bad path → None → AVSpeech fallback, never crash).
4. **Settings window + tray item (cenno-x3j).** tray.rs "Settings…" → `open_settings_window` (show+focus+activate, reuse if open); `main.tsx` label routing.
5. **Backend status commands (before UI).** `tts_model_status` → {present, dir, missing_files[]} validating **all** required files (see manifest). `download_supertonic_model` + a `tts-download-progress` event carrying `{download_id, done, total, status}` and a cancel token.
6. **Voice-out UI (cenno-smk).** `<SettingsView/>`: engine toggle, voice picker (M1–M5/F1–F5), model status, **Download** (progress), **custom path** field; reads `get_user_config`, writes `write_user_config`, calls status/download.
7. **Model download (cenno-qbn) — robust.** **Pin a HF revision** (commit sha, not `main`) + ship a **manifest**: every required path (`onnx/{text_encoder,duration_predictor,vector_estimator,vocoder}.onnx`, `onnx/tts.json`, `onnx/unicode_indexer.json`, selected voice styles) with **sha256 + size**. Download to a **temp dir on the same volume**, verify every file, then publish via **versioned dir + atomic pointer** (or backup→swap with rollback). Cancel leaves only temp artifacts (cleaned via `trash`).

## assets_present() fix
Current check verifies only 2 files (supertonic/mod.rs:49); replace with the manifest validation so a partial/corrupt model reports absent → AVSpeech fallback, not a load crash.

## Verify
read-back deserialize test · config round-trip preserve test · path-resolution + engine-invalidation test · settings opens+reflects+persists (manual+screenshot) · engine/voice change takes effect **without restart** · empty-cache download→speaks · custom path used (after cache invalidation) → `evidence/verify.log`.

## Stop / fallback
If robust download (pinned+manifest+atomic) can't land in this slice → ship settings + custom-path + status, **defer in-app download** (user clones manually), and STOP to confirm. Never ship a download that can leave a broken dir defeating the fallback.

## Out of scope (slice)
Other settings sections, live preview, per-prompt overrides, settings framework.
