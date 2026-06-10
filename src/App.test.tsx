import { act, render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import App, { ANSWERED_LINGER_MS } from "./App";
import { NOTED_WORDS } from "./notedWords";

/** The confirmation card, if shown. Its text rotates through NOTED_WORDS,
 *  so tests assert membership rather than the literal "noted.". */
function confirmationEl() {
  return document.querySelector(".answered-note");
}

function expectConfirmationShown() {
  const el = confirmationEl();
  expect(el).toBeTruthy();
  expect(NOTED_WORDS).toContain(el!.textContent);
}

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

const secondEvent = (remaining_s: number) => ({
  ...moodEvent(remaining_s),
  id: "p_1",
  request: { ...moodEvent(remaining_s).request, title: "Second question?" },
});

/**
 * A sequence step event: same shape as a plain prompt plus the top-level
 * `seq` marker the Rust ask_sequence run attaches (sibling to id/request/
 * remaining_s, see PromptEvent in src-tauri/src/lib.rs).
 */
const seqEvent = (
  index: number,
  total: number,
  last: boolean,
  title: string,
  remaining_s = 40,
) => ({
  id: `s_${index}`,
  request: { ...moodEvent(remaining_s).request, title },
  remaining_s,
  seq: { index, total, last },
});

/** Deliver a `prompt` event to the App's captured listener(s). */
function emitPrompt(event: unknown) {
  act(() => {
    for (const cb of mocks.listeners) cb({ payload: event });
  });
}

/**
 * Deliver a `prompt` event WITHOUT act(): the listener runs (and bumps the
 * hide-generation ref synchronously) but React does not commit, so effect
 * cleanups don't run — reproducing the live race where a hide timer fires
 * in the gap between a new prompt's arrival and the commit that would have
 * cleared the timer.
 */
function emitPromptWithoutCommit(event: unknown) {
  for (const cb of mocks.listeners) cb({ payload: event });
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
  // Deterministic confirmation rotation per test (nextNotedWord persists
  // its cursor in localStorage).
  window.localStorage.clear();
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
    expectConfirmationShown();
    expect(mocks.hide).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(ANSWERED_LINGER_MS);
    });
    expect(mocks.hide).toHaveBeenCalledTimes(1);
    expect(confirmationEl()).toBeNull();
    expect(screen.queryByText("How are you feeling?")).toBeNull();
  });

  it("asks Rust to shrink the panel to min height for the confirmation card", async () => {
    const { invoke } = await import("@tauri-apps/api/core"); // the mock above
    vi.mocked(invoke).mockClear();
    await renderApp();
    emitPrompt(moodEvent(40));
    fireEvent.click(screen.getByRole("button", { name: "good" }));
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith("resize_panel", { height: 240 });
  });

  it("a new prompt during the linger cancels the hide and shows the prompt", async () => {
    await renderApp();
    emitPrompt(moodEvent(40));
    fireEvent.click(screen.getByRole("button", { name: "good" }));
    await act(async () => {});
    expectConfirmationShown();

    emitPrompt(secondEvent(40));
    expect(screen.getByText("Second question?")).toBeTruthy();

    // The cancelled linger timer must not hide the new prompt.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(ANSWERED_LINGER_MS * 2);
    });
    expect(mocks.hide).not.toHaveBeenCalled();
    expect(screen.getByText("Second question?")).toBeTruthy();
  });
});

describe("App ask_sequence instant advance", () => {
  it("a non-last seq step answered does NOT hide or linger; the next step swaps in", async () => {
    await renderApp();
    emitPrompt(seqEvent(0, 3, false, "First of three?"));
    expect(screen.getByText("First of three?")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "good" }));
    await act(async () => {}); // flush answer_prompt invoke

    // No confirmation card, no linger: the panel stays on the (now-answered)
    // question until the next step's event lands.
    expect(confirmationEl()).toBeNull();

    // Even after the linger budget passes, nothing hides — we're mid-sequence.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(ANSWERED_LINGER_MS * 2);
    });
    expect(mocks.hide).not.toHaveBeenCalled();

    // The next step arrives ~instantly from the Rust loop and replaces content.
    emitPrompt(seqEvent(1, 3, false, "Second of three?"));
    expect(screen.getByText("Second of three?")).toBeTruthy();
    expect(mocks.hide).not.toHaveBeenCalled();
  });

  it("the last seq step answered hides after the linger", async () => {
    await renderApp();
    emitPrompt(seqEvent(2, 3, true, "Last of three?"));
    expect(screen.getByText("Last of three?")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "good" }));
    await act(async () => {});
    expectConfirmationShown();
    expect(mocks.hide).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(ANSWERED_LINGER_MS);
    });
    expect(mocks.hide).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Last of three?")).toBeNull();
  });

  it("a mid-sequence step left unanswered still times out and hides", async () => {
    await renderApp();
    emitPrompt(seqEvent(0, 3, false, "First of three?", 2));
    expect(screen.getByText("First of three?")).toBeTruthy();

    // No answer arrives: the timeout auto-hide must still fire (no next event
    // comes when ask_sequence times a question out).
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2100);
    });
    expect(mocks.hide).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("First of three?")).toBeNull();
  });
});

describe("App hide-timer generation guard (new-prompt races)", () => {
  // Rust orders the window front for a new prompt BEFORE the JS event
  // lands; if an old hide timer fires in between, its hide() would land
  // after the show and leave live content in an invisible window. The
  // generation ref must make stale timers no-ops even when they fire
  // before React commits the new prompt (i.e. before effect cleanup runs).

  it("P1's linger timer firing before P2's commit does not hide", async () => {
    await renderApp();
    emitPrompt(moodEvent(40));
    fireEvent.click(screen.getByRole("button", { name: "good" }));
    await act(async () => {}); // linger armed for P1
    expectConfirmationShown();

    // P2 arrives but React has not committed; P1's linger timer is still
    // armed and fires first.
    emitPromptWithoutCommit(secondEvent(40));
    vi.advanceTimersByTime(ANSWERED_LINGER_MS);
    expect(mocks.hide).not.toHaveBeenCalled();

    await act(async () => {}); // commit P2
    expect(screen.getByText("Second question?")).toBeTruthy();
    expect(mocks.hide).not.toHaveBeenCalled();
  });

  it("P1's timeout timer firing before P2's commit does not hide", async () => {
    await renderApp();
    emitPrompt(moodEvent(2)); // P1's auto-hide armed for 2s

    emitPromptWithoutCommit(secondEvent(40));
    vi.advanceTimersByTime(2000); // P1's timer fires pre-commit
    expect(mocks.hide).not.toHaveBeenCalled();

    await act(async () => {});
    expect(screen.getByText("Second question?")).toBeTruthy();
    expect(mocks.hide).not.toHaveBeenCalled();
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
    expectConfirmationShown();

    // The old 2s timeout firing mid-linger must not double-hide; only the
    // linger hide (at 900ms) runs.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2500);
    });
    expect(mocks.hide).toHaveBeenCalledTimes(1);
  });
});
