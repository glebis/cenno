import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { shouldSpeak, type TtsConfig } from "./gating";
import { speechTextFor } from "./speechText";
import { readFreshTts } from "../userConfig";

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
    // Resolve the gate against config read FRESH from disk, not the startup
    // snapshot in `cfg` — otherwise enabling/retuning voice-out in settings is
    // ignored until the app is restarted. `cfg` is the fallback when the fresh
    // read is unavailable (tests/browser). Read fresh, then gate, then speak.
    void readFreshTts()
      .catch(() => cfg)
      .then((fresh) => {
        if (cancelled) return;
        if (!shouldSpeak(prompt.urgency, fresh)) {
          setSpeaking(false);
          return;
        }
        const text = speechTextFor(prompt);
        if (!text) {
          setSpeaking(false);
          return;
        }
        setSpeaking(true);
        void invoke("tts_speak", { text, voice: fresh.voice ?? null }).catch(() => {});
      });
    return () => {
      cancelled = true;
      setSpeaking(false);
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
