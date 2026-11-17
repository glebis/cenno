import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { afterEach, beforeEach, describe, it, expect, vi } from "vitest";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { AudioPlayer, formatBytes, formatTime } from "./AudioPlayer";
import { dataUriToBytes } from "./useAudioPlayer";

vi.mock("@tauri-apps/plugin-opener", () => ({
  revealItemInDir: vi.fn().mockResolvedValue(undefined),
}));

// A minimal WebAudio stand-in: jsdom has none, and the real one is exactly the
// surface the hook touches (decode → buffer, buffer source → start/stop).
interface FakeSource {
  buffer: unknown;
  onended: (() => void) | null;
  connect: (n: unknown) => void;
  disconnect: () => void;
  start: (when: number, offset: number) => void;
  stop: () => void;
}

class FakeAudioContext {
  static last: FakeAudioContext | null = null;
  currentTime = 0;
  destination = {};
  started: FakeSource[] = [];
  closed = false;
  constructor() {
    FakeAudioContext.last = this;
  }
  createBufferSource(): FakeSource {
    const source: FakeSource = {
      buffer: null,
      onended: null,
      connect: () => {},
      disconnect: () => {},
      start: () => {
        this.started.push(source);
      },
      stop: () => {},
    };
    return source;
  }
  decodeAudioData(): Promise<AudioBuffer> {
    // 3.5s of "audio"; the length is all the UI reads from the buffer.
    return Promise.resolve({ duration: 3.5 } as unknown as AudioBuffer);
  }
  resume(): Promise<void> {
    return Promise.resolve();
  }
  close(): Promise<void> {
    this.closed = true;
    return Promise.resolve();
  }
}

function withAudioContext() {
  (globalThis as unknown as { AudioContext?: unknown }).AudioContext =
    FakeAudioContext;
}

function withoutAudioContext() {
  delete (globalThis as unknown as { AudioContext?: unknown }).AudioContext;
  delete (globalThis as unknown as { webkitAudioContext?: unknown })
    .webkitAudioContext;
}

/** `data:` URI for `bytes`, the shape the Rust side produces. */
function dataUri(bytes: number[], mime = "audio/wav"): string {
  return `data:${mime};base64,${btoa(String.fromCharCode(...bytes))}`;
}

const WAV = dataUri([0x52, 0x49, 0x46, 0x46]);

describe("formatTime", () => {
  it("formats m:ss", () => {
    expect(formatTime(0)).toBe("0:00");
    expect(formatTime(9.9)).toBe("0:09");
    expect(formatTime(83)).toBe("1:23");
    expect(formatTime(600)).toBe("10:00");
  });

  it("shows placeholders for unknown lengths", () => {
    expect(formatTime(-1)).toBe("-:--");
    expect(formatTime(Number.NaN)).toBe("-:--");
    expect(formatTime(Number.POSITIVE_INFINITY)).toBe("-:--");
  });
});

describe("formatBytes", () => {
  it("uses KB below a megabyte and MB above, matching the Rust side", () => {
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MB");
    expect(formatBytes(8 * 1024 * 1024)).toBe("8.0 MB");
  });
});

describe("dataUriToBytes", () => {
  it("decodes base64 payloads, ignoring whitespace", () => {
    const uri = dataUri([1, 2, 3], "audio/mpeg");
    expect(Array.from(dataUriToBytes(uri))).toEqual([1, 2, 3]);
    expect(
      Array.from(dataUriToBytes(`data:audio/mpeg;base64,${btoa("\x01\x02\x03")}`)),
    ).toEqual([1, 2, 3]);
  });

  it("rejects a non-base64 or malformed URI", () => {
    expect(() => dataUriToBytes("data:audio/wav,raw")).toThrow(/not base64/);
    expect(() => dataUriToBytes("data:audio/wav")).toThrow(/malformed/);
  });
});

describe("AudioPlayer", () => {
  beforeEach(() => {
    vi.mocked(revealItemInDir).mockClear();
    withoutAudioContext();
  });

  afterEach(() => {
    withoutAudioContext();
    FakeAudioContext.last = null;
  });

  it("renders nothing when the prompt carries no audio", () => {
    const { container } = render(<AudioPlayer />);
    expect(container.firstChild).toBeNull();
  });

  it("shows the player and the clip length once decoded", async () => {
    withAudioContext();
    render(<AudioPlayer dataUri={WAV} />);
    await waitFor(() =>
      expect(screen.getByText("0:00 / 0:03")).toBeTruthy(),
    );
    expect(screen.getByLabelText("Play audio")).toBeTruthy();
    expect(screen.getByLabelText("Seek")).toBeTruthy();
  });

  it("autoplays and reports playing, then reports the pause on unmount", async () => {
    withAudioContext();
    const onPlayingChange = vi.fn();
    const { unmount } = render(
      <AudioPlayer dataUri={WAV} autoplay onPlayingChange={onPlayingChange} />,
    );
    await waitFor(() => expect(onPlayingChange).toHaveBeenCalledWith(true));
    expect(screen.getByLabelText("Pause audio")).toBeTruthy();
    unmount();
    // Unmount must clear the backend's pause — never leave it believing audio
    // is still playing for a prompt that no longer exists.
    expect(onPlayingChange).toHaveBeenLastCalledWith(false);
  });

  it("does not autoplay when muted, but still offers play", async () => {
    withAudioContext();
    const onPlayingChange = vi.fn();
    render(
      <AudioPlayer dataUri={WAV} autoplay={false} onPlayingChange={onPlayingChange} />,
    );
    await waitFor(() => expect(screen.getByLabelText("Play audio")).toBeTruthy());
    expect(onPlayingChange).not.toHaveBeenCalled();
    act(() => {
      fireEvent.click(screen.getByLabelText("Play audio"));
    });
    expect(onPlayingChange).toHaveBeenCalledWith(true);
  });

  it("stops playback when the user pauses", async () => {
    withAudioContext();
    const onPlayingChange = vi.fn();
    render(
      <AudioPlayer dataUri={WAV} autoplay onPlayingChange={onPlayingChange} />,
    );
    await waitFor(() => expect(screen.getByLabelText("Pause audio")).toBeTruthy());
    act(() => {
      fireEvent.click(screen.getByLabelText("Pause audio"));
    });
    expect(onPlayingChange).toHaveBeenLastCalledWith(false);
    expect(screen.getByLabelText("Play audio")).toBeTruthy();
  });

  it("keeps one play glyph from start through pause to ended", async () => {
    withAudioContext();
    const { unmount } = render(<AudioPlayer dataUri={WAV} />);
    await waitFor(() => expect(screen.getByLabelText("Play audio")).toBeTruthy());
    // The icon is the same play triangle in every non-playing state; only the
    // accessible name distinguishes "start" from "replay".
    const start = screen.getByLabelText("Play audio").innerHTML;
    act(() => {
      fireEvent.click(screen.getByLabelText("Play audio"));
    });
    act(() => {
      fireEvent.click(screen.getByLabelText("Pause audio"));
    });
    expect(screen.getByLabelText("Play audio").innerHTML).toBe(start);
    unmount();
  });

  it("rewinds to the start when the clip ends, instead of sitting at the end", async () => {
    withAudioContext();
    const onPlayingChange = vi.fn();
    render(<AudioPlayer dataUri={WAV} autoplay onPlayingChange={onPlayingChange} />);
    await waitFor(() => expect(screen.getByLabelText("Pause audio")).toBeTruthy());
    // Simulate the decoder reporting the end of the buffer.
    act(() => {
      FakeAudioContext.last!.started.at(-1)!.onended?.();
    });
    // Back to the top and offering play again — never parked at 0:03 with a
    // play button, which reads as a stuck control.
    expect(screen.getByText("0:00 / 0:03")).toBeTruthy();
    expect(screen.getByLabelText("Play audio")).toBeTruthy();
    expect(onPlayingChange).toHaveBeenLastCalledWith(false);
  });

  it("explains an oversize clip instead of playing it, and reveals the file", async () => {
    render(
      <AudioPlayer
        oversize={{ bytes: 8 * 1024 * 1024, max: 5 * 1024 * 1024, source_path: "/tmp/big.mp3" }}
      />,
    );
    expect(screen.getByRole("note").textContent).toContain("Audio too large");
    expect(screen.getByRole("note").textContent).toContain("8.0 MB");
    fireEvent.click(screen.getByText("Open in Finder"));
    await waitFor(() =>
      expect(revealItemInDir).toHaveBeenCalledWith("/tmp/big.mp3"),
    );
  });

  it("offers no reveal for an oversize data URI (nothing on disk)", () => {
    render(
      <AudioPlayer
        oversize={{ bytes: 8 * 1024 * 1024, max: 5 * 1024 * 1024, source_path: null }}
      />,
    );
    expect(screen.queryByText("Open in Finder")).toBeNull();
  });

  it("survives a decode failure with a notice and no player", async () => {
    // No AudioContext at all is the jsdom case and the ancient-webview case.
    render(<AudioPlayer dataUri={WAV} autoplay />);
    await waitFor(() =>
      expect(screen.getByRole("note").textContent).toContain(
        "Couldn't decode this audio file",
      ),
    );
    expect(screen.queryByLabelText("Play audio")).toBeNull();
  });
});
