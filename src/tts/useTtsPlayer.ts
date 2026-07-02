import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { orchestratePrompt, type PlayerConfig, type SpeakablePrompt } from "./orchestrate";
import { readFreshTts } from "../userConfig";

export type { PlayerConfig, SpeakablePrompt } from "./orchestrate";

export interface TtsPlayer {
  /** True while this prompt is being (or was just) read aloud. */
  speaking: boolean;
  /** Stop the current utterance without dismissing the prompt. */
  stop: () => void;
}

/**
 * Speaks a prompt aloud when it appears, if voice-out is enabled and the
 * prompt's urgency clears the configured threshold. Fires once per prompt
 * identity (keyed on `id`), never on incidental re-renders, so a prompt is
 * never read twice. Unmounting or swapping prompts stops any in-flight speech.
 *
 * Also owns the panel-show handshake: Rust keeps the panel hidden until the
 * webview invokes `panel_ready` (with a Rust-side fallback deadline), and this
 * hook fires that signal — immediately for silent prompts, at playback start
 * for spoken ones — so the panel never sits mute while audio is synthesized.
 * The sequencing itself lives in orchestrate.ts.
 *
 * Outside Tauri (tests/browser) the invokes simply reject and are swallowed.
 */
export function useTtsPlayer(prompt: SpeakablePrompt | null, cfg: PlayerConfig): TtsPlayer {
  const [speaking, setSpeaking] = useState(false);
  const id = prompt?.id;

  useEffect(() => {
    if (!prompt) {
      setSpeaking(false);
      return;
    }
    let cancelled = false;
    void orchestratePrompt(prompt, cfg, {
      readFresh: readFreshTts,
      speak: (text, voice) => invoke("tts_speak", { text, voice }),
      panelReady: () => {
        void invoke("panel_ready").catch(() => {});
      },
      setSpeaking: (v) => {
        if (!cancelled) setSpeaking(v);
      },
      cancelled: () => cancelled,
    });
    return () => {
      cancelled = true;
      setSpeaking(false);
      // Stops started playback AND cancels an utterance still synthesizing.
      void invoke("tts_stop").catch(() => {});
    };
    // Re-run only on a new prompt identity; config is read fresh at fire time.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  const stop = useCallback(() => {
    setSpeaking(false);
    void invoke("tts_stop").catch(() => {});
  }, []);

  return { speaking, stop };
}
