import { act, render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import App, { ANSWERED_LINGER_MS } from "./App";

// Shared, hoisted so the vi.mock factories (hoisted above imports) see them.
const mocks = vi.hoisted(() => ({
  hide: vi.fn(() => Promise.resolve()),
  // Captured `prompt` event listeners — tests push events through these.
  listeners: [] as Array<(event: { payload: unknown }) => void>,
  // What the pending_prompts command returns (per-test).
  pending: [] as unknown[],
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((_event: string, cb: (event: { payload: unknown }) => void) => {
    mocks.listeners.push(cb);
    return Promise.resolve(() => {});
  }),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string) =>
    cmd === "pending_prompts" ? Promise.resolve(mocks.pending) : Promise.resolve(true),
  ),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({ hide: mocks.hide })),
}));

const moodEvent = (remaining_s: number) => ({
  id: "p_0",
  request: {
    title: "How are you feeling?",
    body_md: "",
    input: { kind: "choice" },
    choices: ["great", "good", "okay", "low", "rough"],
    flow: "mood",
  },
  remaining_s,
});

/** Deliver a `prompt` event to the App's captured listener(s). */
function emitPrompt(event: unknown) {
  act(() => {
    for (const cb of mocks.listeners) cb({ payload: event });
  });
}

/**
 * Mount App and flush the listen→pending_prompts microtask chain. Fake
 * timers are already active, so RTL's findBy/waitFor (setTimeout-based)
 * would stall — but this startup path is promise-only, so awaiting inside
 * act() settles it.
 */
async function renderApp() {
  const view = render(<App />);
  await act(async () => {});
  return view;
}

beforeEach(() => {
  vi.useFakeTimers();
  mocks.hide.mockClear();
  mocks.listeners.length = 0;
  mocks.pending = [];
});

afterEach(() => {
  vi.useRealTimers();
});

describe("App cold-start replay", () => {
  it("renders a pending prompt pulled on mount when no event ever fires", async () => {
    mocks.pending = [moodEvent(40)];
    await renderApp();
    expect(screen.getByText("How are you feeling?")).toBeTruthy();
    expect(screen.getByRole("button", { name: "great" })).toBeTruthy();
  });
});

describe("App answered state machine", () => {
  it("shows the confirmation after an answer, then hides and clears", async () => {
    await renderApp();
    emitPrompt(moodEvent(40));
    expect(screen.getByText("How are you feeling?")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "good" }));
    await act(async () => {}); // flush answer_prompt invoke

    // Confirmation lingers — panel did NOT vanish, window not hidden yet.
    expect(screen.getByText("noted.")).toBeTruthy();
    expect(mocks.hide).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(ANSWERED_LINGER_MS);
    });
    expect(mocks.hide).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("noted.")).toBeNull();
    expect(screen.queryByText("How are you feeling?")).toBeNull();
  });

  it("a new prompt during the linger cancels the hide and shows the prompt", async () => {
    await renderApp();
    emitPrompt(moodEvent(40));
    fireEvent.click(screen.getByRole("button", { name: "good" }));
    await act(async () => {});
    expect(screen.getByText("noted.")).toBeTruthy();

    emitPrompt({
      ...moodEvent(40),
      id: "p_1",
      request: { ...moodEvent(40).request, title: "Second question?" },
    });
    expect(screen.getByText("Second question?")).toBeTruthy();

    // The cancelled linger timer must not hide the new prompt.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(ANSWERED_LINGER_MS * 2);
    });
    expect(mocks.hide).not.toHaveBeenCalled();
    expect(screen.getByText("Second question?")).toBeTruthy();
  });
});

describe("App timeout auto-hide", () => {
  it("hides and clears when remaining_s elapses unanswered", async () => {
    await renderApp();
    emitPrompt(moodEvent(2));
    expect(screen.getByText("How are you feeling?")).toBeTruthy();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1900);
    });
    expect(mocks.hide).not.toHaveBeenCalled(); // not before the deadline

    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });
    expect(mocks.hide).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("How are you feeling?")).toBeNull();
  });

  it("runs the auto-hide on a replayed prompt's REMAINING budget", async () => {
    mocks.pending = [moodEvent(3)]; // original timeout_s was larger; 3s left
    await renderApp();
    expect(screen.getByText("How are you feeling?")).toBeTruthy();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(3100);
    });
    expect(mocks.hide).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("How are you feeling?")).toBeNull();
  });

  it("an answer cancels the timeout auto-hide", async () => {
    await renderApp();
    emitPrompt(moodEvent(2));
    fireEvent.click(screen.getByRole("button", { name: "good" }));
    await act(async () => {});
    expect(screen.getByText("noted.")).toBeTruthy();

    // The old 2s timeout firing mid-linger must not double-hide; only the
    // linger hide (at 900ms) runs.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2500);
    });
    expect(mocks.hide).toHaveBeenCalledTimes(1);
  });
});
