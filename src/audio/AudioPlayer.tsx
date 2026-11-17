/**
 * AudioPlayer — one row above the prompt body: play/pause, a scrubber, and a
 * `0:23 / 1:47` readout.
 *
 * Three visually distinct states, all occupying the same slot so the panel
 * never jumps when audio is missing:
 *
 *  - player: a decoded clip, ready or playing;
 *  - notice: the agent supplied audio that was too large, so the panel says so
 *    and offers to reveal the file (only when the source was a path);
 *  - notice: the clip could not be decoded (the prompt stays answerable).
 *
 * Autoplay is a default, not a rule: `muted` (from the request or the panel's
 * own toggle) suppresses it, and pressing ▶ always works.
 */
import { useMemo } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { useAudioPlayer } from "./useAudioPlayer";

export interface AudioOversizeNote {
  /** Size the source claimed, in bytes. */
  bytes: number;
  /** The cap that refused it, in bytes. */
  max: number;
  /** Present when the source was a file path — enables "Open in Finder". */
  source_path?: string | null;
}

export interface AudioPlayerProps {
  /** Validated `data:audio/…;base64,…` URI (absent when audio is unusable). */
  dataUri?: string | null;
  /** Oversize note from the backend; takes precedence over playback. */
  oversize?: AudioOversizeNote | null;
  /** Start on open (unless muted). */
  autoplay?: boolean;
  /** Reports real playing-state transitions for the countdown pause. */
  onPlayingChange?: (playing: boolean) => void;
}

/** `m:ss`; `-:--` while the length is unknown. */
export function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "-:--";
  const total = Math.floor(seconds);
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

/** Same shape as the Rust side's human_size: KB below a megabyte. */
export function formatBytes(bytes: number): string {
  const kb = 1024;
  const mb = kb * 1024;
  return bytes < mb
    ? `${(bytes / kb).toFixed(1)} KB`
    : `${(bytes / mb).toFixed(1)} MB`;
}

function Notice({
  children,
  action,
}: {
  children: React.ReactNode;
  action?: { label: string; onClick: () => void };
}) {
  return (
    <div className="cenno-audio cenno-audio--notice" role="note">
      <span className="cenno-audio__notice-text">{children}</span>
      {action && (
        <button
          type="button"
          className="cenno-audio__reveal"
          onClick={action.onClick}
        >
          {action.label}
        </button>
      )}
    </div>
  );
}

export function AudioPlayer({
  dataUri,
  oversize,
  autoplay = false,
  onPlayingChange,
}: AudioPlayerProps) {
  // Always call the hook (hook order), but hand it nothing when the payload
  // is absent or refused — it then reports an inert error state we override
  // with the notice below.
  const player = useAudioPlayer({
    dataUri: oversize ? null : dataUri ?? null,
    autoplay,
    onPlayingChange,
  });

  const revealPath = useMemo(
    () => (oversize?.source_path ? oversize.source_path : null),
    [oversize],
  );

  if (oversize) {
    const size = formatBytes(oversize.bytes);
    const max = formatBytes(oversize.max);
    return (
      <Notice
        action={
          revealPath
            ? {
                label: "Open in Finder",
                onClick: () => {
                  void revealItemInDir(revealPath).catch((e) => {
                    console.error("reveal audio failed:", e);
                  });
                },
              }
            : undefined
        }
      >
        Audio too large ({size} &gt; {max})
      </Notice>
    );
  }

  if (!dataUri) return null;

  if (player.status === "error") {
    return <Notice>Couldn't decode this audio file</Notice>;
  }

  const playing = player.status === "playing";
  const ready = player.status === "ready" || playing || player.status === "paused";
  // One glyph for "not playing": a plain play triangle, whether the clip is at
  // the start, paused, or just finished and rewound.
  const label = playing ? "Pause audio" : "Play audio";
  const total = player.duration > 0 ? player.duration : 1;

  return (
    <div className="cenno-audio" data-state={player.status}>
      <button
        type="button"
        className="cenno-audio__toggle"
        aria-label={label}
        title={playing ? "Pause" : "Play"}
        disabled={!ready}
        onClick={() => player.toggle()}
      >
        {playing ? (
          <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
            <rect x="2" y="2" width="3" height="8" rx="0.6" fill="currentColor" />
            <rect x="7" y="2" width="3" height="8" rx="0.6" fill="currentColor" />
          </svg>
        ) : (
          <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
            <path d="M3 1.8l7 4.2-7 4.2z" fill="currentColor" />
          </svg>
        )}
      </button>
      <input
        type="range"
        className="cenno-audio__scrub"
        aria-label="Seek"
        min={0}
        max={total}
        step={0.01}
        value={Math.min(player.current, total)}
        disabled={!ready}
        onChange={(e) => player.seek(Number(e.currentTarget.value))}
        style={{
          ["--cenno-audio-progress" as string]: `${(Math.min(player.current, total) / total) * 100}%`,
        }}
      />
      <span className="cenno-audio__time">
        {formatTime(player.current)} /{" "}
        {player.duration > 0 ? formatTime(player.duration) : "-:--"}
      </span>
    </div>
  );
}

export default AudioPlayer;
