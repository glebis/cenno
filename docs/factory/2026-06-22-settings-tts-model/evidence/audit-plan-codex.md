# Plan audit — Codex (read-only, fresh context) — 2026-06-22

Findings folded into plan.md rev2.

## BLOCKER
1. `tts.model_path` not in TtsConfig + `deny_unknown_fields` (config.rs:81,83) → writing it bricks config read (Config::load falls back to defaults on parse error, config.rs:127). Fix: add field to Rust + TS first; read-back test. → plan step 0.
2. Writes don't update the running app: get_user_config returns once-loaded managed state (lib.rs:93,473); frontend caches once (userConfig.ts:123). Saving enabled/min_urgency hits disk but not in-process gating. Fix: reload after write. → plan step 2.

## MAJOR
- Value-merge vs deny_unknown_fields: don't promise "preserve arbitrary unknowns" — they'd fail read-back. (In practice only modeled keys ever exist.) Add read-back test. → step 1.
- Atomic write underspecified: same-dir temp, flush+fsync, parent-dir sync, 0600, process mutex. → step 1.
- Settings window focus on a tray app needs explicit show+set_focus+activation (main thread); ensure only "main" → to_panel() (lib.rs:370). → architecture.
- Window-label routing: get label synchronously before mount; settings loads same index URL, not /settings. → architecture.
- Download integrity undefined; assets_present() only checks 2 files (supertonic/mod.rs:49) but loader needs all ONNX + tts.json + unicode_indexer.json + voice styles. Fix: pin HF revision + manifest (sha256+size). → step 7 + assets_present fix.
- Atomic dir move over non-empty dir isn't clean: versioned dir + atomic pointer, or backup→swap+rollback. → step 7.
- ENGINE static cache (supertonic/mod.rs:35) won't reload on path change/new download. Fix: key cache by resolved dir / invalidate. → step 3.

## MINOR
- Progress/cancel needs a download_id + explicit cancel token; scope events. → step 5.

## Ordering flaws
- Schema (model_path) + read-back before write_user_config. → fixed (step 0 first).
- Backend status/download commands before UI. → fixed (step 5 before 6).
- Custom-path cache invalidation before "custom path used" verification. → fixed (step 3).
