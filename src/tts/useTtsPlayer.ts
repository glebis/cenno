import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { normalizeForSpeech } from "./normalize";
import { shouldSpeak, type TtsConfig } from "./gating";

/** The bits of a prompt the player needs. */
export interface SpeakablePrompt {
  id: string;
  title: string;
  body_md: string;
  urgency?: string;
}

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
export function useTtsPlayer(prompt: SpeakablePrompt | null, cfg: TtsConfig): TtsPlayer {
  const [speaking, setSpeaking] = useState(false);
  const id = prompt?.id;

  useEffect(() => {
    if (!prompt || !shouldSpeak(prompt.urgency, cfg)) {
      setSpeaking(false);
      return;
    }
    const text = [normalizeForSpeech(prompt.title), normalizeForSpeech(prompt.body_md)]
      .filter(Boolean)
      .join(". ");
    if (!text) {
      setSpeaking(false);
      return;
    }
    setSpeaking(true);
    void invoke("tts_speak", { text }).catch(() => {});
    return () => {
      setSpeaking(false);
      void invoke("tts_stop").catch(() => {});
    };
    // Re-run only on a new prompt identity; cfg is read at fire time.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  const stop = useCallback(() => {
    setSpeaking(false);
    void invoke("tts_stop").catch(() => {});
  }, []);

  return { speaking, stop };
}
