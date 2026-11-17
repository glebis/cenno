/**
 * Playback engine for agent-supplied audio (a generated clip, a jingle).
 *
 * Deliberately WebAudio (`AudioContext` + `decodeAudioData`) rather than an
 * `<audio>` element:
 *
 *  - one decoded buffer gives sample-accurate `currentTime` for the scrubber,
 *    which `<audio>` approximates badly for short clips;
 *  - no `src` attribute means no CSP surface (`media-src`) and no second read
 *    of the payload — the bytes arrive as a `data:` URI and are decoded once;
 *  - start/stop is explicit, so "pause" is really a stop plus a remembered
 *    offset, and the countdown-pause hook has an unambiguous playing state.
 *
 * The Rust side already validated the payload (extension/magic bytes/size), so
 * this code does not re-validate; it only handles the two failures that can
 * still happen locally: no WebAudio at all (jsdom, an ancient webview) and a
 * buffer the decoder refuses.
 */
import { useCallback, useEffect, useRef, useState } from "react";

export type AudioStatus = "loading" | "ready" | "playing" | "paused" | "error";

export interface AudioPlayer {
  status: AudioStatus;
  /** Playback position in seconds. */
  current: number;
  /** Clip length in seconds; 0 until the buffer is decoded. */
  duration: number;
  error: string | null;
  /** Play, or pause if already playing. After the clip ends, from the top. */
  toggle: () => void;
  /** Jump to `seconds` (clamped); keeps playing if it was playing. */
  seek: (seconds: number) => void;
}

export interface UseAudioPlayerOptions {
  /** Validated `data:audio/…;base64,…` URI, or null for no player. */
  dataUri?: string | null;
  /**
   * Start playing as soon as the buffer is ready. False when the prompt is
   * muted — mute silences *auto* playback only; the user can still press play.
   */
  autoplay?: boolean;
  /**
   * Fired on every real playing-state transition, so the caller can tell the
   * backend to suspend/resume the prompt countdown. Never fires for a
   * load/decode failure.
   */
  onPlayingChange?: (playing: boolean) => void;
}

/** How often the position readout updates while playing. */
const TICK_MS = 120;

/**
 * Decode a base64 `data:` URI into raw bytes without `fetch`: the payload is
 * already in memory and a `data:` fetch is both slower and, in some webviews,
 * subject to the same fetch rules as a network URL.
 */
export function dataUriToBytes(uri: string): Uint8Array {
  const comma = uri.indexOf(",");
  if (comma < 0) throw new Error("malformed data URI");
  if (!/;base64/i.test(uri.slice(0, comma))) {
    throw new Error("audio data URI is not base64");
  }
  const binary = atob(uri.slice(comma + 1).replace(/\s+/g, ""));
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

type AudioContextCtor = new () => AudioContext;

function audioContextCtor(): AudioContextCtor | null {
  const g = globalThis as unknown as {
    AudioContext?: AudioContextCtor;
    webkitAudioContext?: AudioContextCtor;
  };
  return g.AudioContext ?? g.webkitAudioContext ?? null;
}

export function useAudioPlayer({
  dataUri,
  autoplay = false,
  onPlayingChange,
}: UseAudioPlayerOptions): AudioPlayer {
  const [status, setStatus] = useState<AudioStatus>("loading");
  const [current, setCurrent] = useState(0);
  const [duration, setDuration] = useState(0);
  const [error, setError] = useState<string | null>(null);

  const ctxRef = useRef<AudioContext | null>(null);
  const bufferRef = useRef<AudioBuffer | null>(null);
  const sourceRef = useRef<AudioBufferSourceNode | null>(null);
  /** Where the current playback run started within the clip, in seconds. */
  const offsetRef = useRef(0);
  /** `ctx.currentTime` when the current run started. */
  const startedAtRef = useRef(0);
  /** Distinguishes a natural end from a pause/dispose stop. */
  const naturalEndRef = useRef(false);
  const playingRef = useRef(false);
  const playingCbRef = useRef(onPlayingChange);
  playingCbRef.current = onPlayingChange;

  const setPlaying = useCallback((playing: boolean) => {
    if (playingRef.current === playing) return;
    playingRef.current = playing;
    playingCbRef.current?.(playing);
  }, []);

  /** Stop the running source node without reporting a natural end. */
  const stopSource = useCallback(() => {
    const source = sourceRef.current;
    sourceRef.current = null;
    if (!source) return;
    naturalEndRef.current = false;
    source.onended = null;
    try {
      source.stop();
    } catch {
      /* already stopped */
    }
    try {
      source.disconnect();
    } catch {
      /* already disconnected */
    }
  }, []);

  const positionNow = useCallback(() => {
    const ctx = ctxRef.current;
    const buffer = bufferRef.current;
    if (!ctx || !buffer) return offsetRef.current;
    if (!playingRef.current) return offsetRef.current;
    return Math.min(
      offsetRef.current + (ctx.currentTime - startedAtRef.current),
      buffer.duration,
    );
  }, []);

  const pause = useCallback(() => {
    if (!playingRef.current) return;
    offsetRef.current = positionNow();
    stopSource();
    setPlaying(false);
    setCurrent(offsetRef.current);
    setStatus("paused");
  }, [positionNow, setPlaying, stopSource]);

  const playFrom = useCallback(
    (seconds: number) => {
      const ctx = ctxRef.current;
      const buffer = bufferRef.current;
      if (!ctx || !buffer) return;
      stopSource();
      const offset = Math.max(0, Math.min(seconds, buffer.duration));
      const source = ctx.createBufferSource();
      source.buffer = buffer;
      source.connect(ctx.destination);
      source.onended = () => {
        if (!naturalEndRef.current) return;
        naturalEndRef.current = false;
        sourceRef.current = null;
        // A finished clip rewinds: the row goes back to 0:00 rather than
        // sitting at the end with a play button, which reads as stuck.
        offsetRef.current = 0;
        setCurrent(0);
        setStatus("ready");
        setPlaying(false);
      };
      naturalEndRef.current = true;
      try {
        source.start(0, offset);
      } catch (e) {
        naturalEndRef.current = false;
        setError(e instanceof Error ? e.message : String(e));
        setStatus("error");
        setPlaying(false);
        return;
      }
      sourceRef.current = source;
      offsetRef.current = offset;
      startedAtRef.current = ctx.currentTime;
      setCurrent(offset);
      setStatus("playing");
      setPlaying(true);
      // WKWebView may hand back a suspended context until a gesture happens.
      // Resuming here costs nothing when already running; if it stays
      // suspended the readout simply does not advance and the ▶ control is
      // still there for the user to press (a real gesture).
      void ctx.resume?.().catch(() => {});
    },
    [setPlaying, stopSource],
  );

  const toggle = useCallback(() => {
    const buffer = bufferRef.current;
    if (!buffer) return;
    if (playingRef.current) {
      pause();
      return;
    }
    playFrom(offsetRef.current);
  }, [pause, playFrom, status]);

  const seek = useCallback(
    (seconds: number) => {
      const buffer = bufferRef.current;
      if (!buffer) return;
      const target = Math.max(0, Math.min(seconds, buffer.duration));
      if (playingRef.current) {
        playFrom(target);
      } else {
        offsetRef.current = target;
        setCurrent(target);
      }
    },
    [playFrom],
  );

  // Decode once per payload. A new prompt means a new data URI (the panel is
  // keyed on prompt.id), so there is no cross-prompt buffer to invalidate.
  useEffect(() => {
    if (!dataUri) {
      setStatus("error");
      setError("no audio");
      return;
    }
    const Ctor = audioContextCtor();
    if (!Ctor) {
      setStatus("error");
      setError("audio playback is unavailable in this environment");
      return;
    }
    let cancelled = false;
    const ctx = new Ctor();
    ctxRef.current = ctx;
    setStatus("loading");

    const decoded = (async () => {
      const bytes = dataUriToBytes(dataUri);
      // decodeAudioData detaches the buffer, so hand it a copy of the exact
      // slice — a Uint8Array view over a larger ArrayBuffer would decode the
      // wrong bytes.
      const copy = bytes.slice().buffer;
      return ctx.decodeAudioData(copy);
    })();

    decoded
      .then((buffer) => {
        if (cancelled) return;
        bufferRef.current = buffer;
        offsetRef.current = 0;
        setDuration(buffer.duration);
        setCurrent(0);
        setStatus("ready");
        if (autoplay) playFrom(0);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setError(
          e instanceof Error ? e.message : "couldn't decode this audio file",
        );
        setStatus("error");
      });

    return () => {
      cancelled = true;
      // Unmounting (answered, dismissed, or a new prompt) must stop the sound
      // and clear the countdown pause with it — never leave the backend
      // believing audio is still playing for a prompt that is gone.
      stopSource();
      setPlaying(false);
      bufferRef.current = null;
      ctxRef.current = null;
      void ctx.close?.().catch(() => {});
    };
    // Keyed on the payload: one decode per clip, never per render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dataUri]);

  // Position readout. Only while playing, so a paused panel does no work.
  useEffect(() => {
    if (status !== "playing") return;
    const timer = setInterval(() => setCurrent(positionNow()), TICK_MS);
    return () => clearInterval(timer);
  }, [status, positionNow]);

  return { status, current, duration, error, toggle, seek };
}
