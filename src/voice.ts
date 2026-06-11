/**
 * useVoiceDictation — frontend half of the voice_text push-to-talk flow.
 *
 * Bridges the `voice_start` / `voice_stop` Tauri commands and the
 * `voice-event` stream (see src-tauri/src/voice.rs) into React state the
 * TextField adapter can render. Keeps the views Tauri-free: TextFieldView
 * only sees {recording, voiceError, onMicToggle} props.
 *
 * Transcript merge: Apple Speech streams the WHOLE session transcript on
 * every partial. Text already in the field when recording starts is kept as
 * an immutable base; each partial replaces only what follows it. The field
 * stays editable the whole time — edits made *during* recording to the
 * dictated tail are overwritten by the next partial (the base is not),
 * which matches "correct after you stop talking".
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";

export type VoiceEventPayload =
  | { type: "state"; state: "recording" | "stopped" }
  | { type: "partial"; text: string }
  | { type: "error"; message: string };

export const VOICE_EVENT = "voice-event";

/** Pure: splice a streaming transcript onto the immutable pre-recording base. */
export function mergeTranscript(base: string, transcript: string): string {
  if (!base) return transcript;
  const sep = /\s$/.test(base) ? "" : " ";
  return base + sep + transcript;
}

export function useVoiceDictation(opts: {
  enabled: boolean;
  /** Current field text — captured as the base when recording starts. */
  getBase: () => string;
  /** Receives the merged field text on every partial. */
  onText: (text: string) => void;
}) {
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const baseRef = useRef("");
  const optsRef = useRef(opts);
  optsRef.current = opts;

  useEffect(() => {
    if (!opts.enabled) return;
    let unlisten: UnlistenFn | undefined;
    let disposed = false;
    listen<VoiceEventPayload>(VOICE_EVENT, (event) => {
      const p = event.payload;
      if (p.type === "partial") {
        optsRef.current.onText(mergeTranscript(baseRef.current, p.text));
      } else if (p.type === "state") {
        setRecording(p.state === "recording");
      } else {
        setError(p.message);
        setRecording(false);
      }
    }).then((u) => {
      if (disposed) u();
      else unlisten = u;
    });
    return () => {
      disposed = true;
      unlisten?.();
      // Panel unmounting mid-recording must release the mic (idempotent).
      void invoke("voice_stop").catch(() => {});
    };
  }, [opts.enabled]);

  const toggle = () => {
    setError(null);
    if (recording) {
      void invoke("voice_stop").catch(() => {});
    } else {
      baseRef.current = optsRef.current.getBase();
      // Failures also arrive as voice-event errors; the catch only silences
      // the duplicate command rejection.
      void invoke("voice_start").catch(() => {});
    }
  };

  return { recording, error, toggle };
}
