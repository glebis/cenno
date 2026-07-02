/**
 * orchestrate.ts — the speak-then-show sequence for one prompt.
 *
 * The panel window stays hidden until the webview reports readiness (the
 * `panel_ready` command); Rust only force-shows after a fallback deadline.
 * This module decides when "ready" is: immediately for a silent prompt, at
 * playback start for a spoken one — so the panel and the voice arrive
 * together instead of the panel sitting mute while Supertonic synthesizes.
 *
 * Pure orchestration over an injected `SpeakIo`, so the sequencing (and its
 * cancellation semantics) is testable without Tauri or a DOM.
 */
import { shouldSpeak, type TtsConfig } from "./gating";
import { speechTextFor } from "./speechText";

/** The bits of a prompt the player needs. */
export interface SpeakablePrompt {
  id: string;
  title: string;
  body_md: string;
  /** Optional agent-authored spoken summary; spoken instead of the body. */
  say?: string;
  urgency?: string;
}

/** Gating config plus the optional on-device voice identifier to speak with. */
export type PlayerConfig = TtsConfig & { voice?: string };

/** Side effects the orchestration drives, injected for testability. */
export interface SpeakIo {
  /** Read gating config fresh from disk; rejection falls back to the snapshot. */
  readFresh(): Promise<PlayerConfig>;
  /** Start speech; resolves once playback has STARTED (not finished). */
  speak(text: string, voice: string | null): Promise<void>;
  /** Tell Rust the panel may be shown now. */
  panelReady(): void;
  setSpeaking(speaking: boolean): void;
  /** True once this prompt was superseded/unmounted — stand down silently. */
  cancelled(): boolean;
}

export async function orchestratePrompt(
  prompt: SpeakablePrompt,
  fallbackCfg: PlayerConfig,
  io: SpeakIo,
): Promise<void> {
  // Gate against config read fresh from disk, not the startup snapshot —
  // otherwise enabling/retuning voice-out in settings is ignored until the
  // app restarts. The snapshot is the fallback (tests/browser, read failure).
  const fresh = await io.readFresh().catch(() => fallbackCfg);
  if (io.cancelled()) return;

  const text = speechTextFor(prompt);
  if (!shouldSpeak(prompt.urgency, fresh) || !text) {
    io.setSpeaking(false);
    // Nothing to wait for — the panel may show right away.
    io.panelReady();
    return;
  }

  io.setSpeaking(true);
  // speak() resolves at playback START. A failed engine must never hold the
  // panel hostage, so errors fall through to panelReady all the same.
  await io.speak(text, fresh.voice ?? null).catch(() => {});
  // Superseded while audio was being prepared (user answered, prompt swapped,
  // panel dismissed): the cleanup path owns teardown — do not re-show.
  if (io.cancelled()) return;
  io.panelReady();
}
