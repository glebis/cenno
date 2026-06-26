import { act, render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import App, { ANSWERED_LINGER_MS, draftFingerprint, draftKey } from "./App";
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
  // Captured event listeners, keyed by event name — tests push `prompt` events
  // through these. The App also listens for `dismiss-panel`; keying by name
  // keeps emitPrompt from firing that (production `listen` filters by name too).
  listeners: [] as Array<{ event: string; cb: (event: { payload: unknown }) => void }>,
  // What the pending_prompts command returns (per-test).
  pending: [] as unknown[],
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, cb: (event: { payload: unknown }) => void) => {
    mocks.listeners.push({ event, cb });
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

/** A text-input prompt — renders the editable textarea (cenno-field__input). */
const textEvent = (id: string, remaining_s = 40) => ({
  id,
  request: {
    title: "What's on your mind?",
    body_md: "",
    input: { kind: "text" as const },
    flow: "question",
  },
  remaining_s,
});

/** Deliver a `prompt` event to the App's captured `prompt` listener(s). */
function emitPrompt(event: unknown) {
  act(() => {
    for (const { event: name, cb } of mocks.listeners) {
      if (name === "prompt") cb({ payload: event });
    }
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
  for (const { event: name, cb } of mocks.listeners) {
    if (name === "prompt") cb({ payload: event });
  }
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

  it("after answering, advances to the next queued prompt instead of hiding", async () => {
    await renderApp();
    emitPrompt(moodEvent(40)); // P1
    // P2 queued in the registry behind P1.
    mocks.pending = [secondEvent(40)];

    fireEvent.click(screen.getByRole("button", { name: "good" }));
    await act(async () => {}); // answer_prompt; confirmation linger begins
    expectConfirmationShown();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(ANSWERED_LINGER_MS);
    });
    // Linger over → the queued P2 takes the panel; it was never hidden.
    expect(screen.getByText("Second question?")).toBeTruthy();
    expect(mocks.hide).not.toHaveBeenCalled();
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

  it("queues a competing prompt instead of replacing the unanswered one", async () => {
    await renderApp();
    emitPrompt(moodEvent(2)); // P1 on screen, 2s budget, unanswered
    expect(screen.getByText("How are you feeling?")).toBeTruthy();

    // P2 arrives live while the user is still deciding on P1 → it must be
    // queued (parked in the registry), NOT steamroll P1 off the screen.
    emitPrompt(secondEvent(40));
    expect(screen.getByText("How are you feeling?")).toBeTruthy();
    expect(screen.queryByText("Second question?")).toBeNull();

    // P2 is pending in the registry; when P1 times out we advance to it
    // (first come, first served) rather than hiding.
    mocks.pending = [secondEvent(40)];
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2100);
    });
    expect(screen.getByText("Second question?")).toBeTruthy();
    expect(mocks.hide).not.toHaveBeenCalled();
  });

  it("starts the timeout only when shown (mark_shown), not while queued", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    await renderApp();
    vi.mocked(invoke).mockClear();

    emitPrompt(moodEvent(40)); // P1 shown → its clock should start
    await act(async () => {});
    emitPrompt(secondEvent(40)); // P2 queued (unanswered P1 on screen) → no clock
    await act(async () => {});

    const markShown = vi
      .mocked(invoke)
      .mock.calls.filter((c) => c[0] === "mark_shown");
    expect(markShown).toEqual([["mark_shown", { id: "p_0" }]]); // only the shown P1
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

describe("App draft restore", () => {
  const ta = () =>
    document.querySelector<HTMLTextAreaElement>(".cenno-field__input");

  // Stash a draft in the exact session-namespaced, fingerprinted format the
  // save side writes, so restore (which verifies both) accepts it.
  function stashDraft(id: string, v: string, fingerprintOf = textEvent(id)) {
    window.localStorage.setItem(
      draftKey(id),
      JSON.stringify({ f: draftFingerprint(fingerprintOf.request), v }),
    );
  }

  it("restores a saved draft into the field when its prompt shows", async () => {
    // A draft was saved (per-prompt-id) before the panel was torn down.
    stashDraft("p_text", "half-written answer");
    await renderApp();
    emitPrompt(textEvent("p_text"));
    await act(async () => {});

    expect(ta()).toBeTruthy();
    expect(ta()!.value).toBe("half-written answer");
  });

  it("does not restore a draft belonging to a different prompt id", async () => {
    stashDraft("p_other", "not this one");
    await renderApp();
    emitPrompt(textEvent("p_text"));
    await act(async () => {});

    expect(ta()!.value).toBe("");
  });

  it("ignores a draft whose fingerprint doesn't match (reused id)", async () => {
    // Same id p_text and same session key, but the stored draft was for a
    // DIFFERENT prompt (the Rust id counter resets each launch, so ids get
    // reused). The content fingerprint won't match → the stale text must not
    // leak into this prompt.
    window.localStorage.setItem(
      draftKey("p_text"),
      JSON.stringify({ f: "some other prompt entirely", v: "leaked secret" }),
    );
    await renderApp();
    emitPrompt(textEvent("p_text"));
    await act(async () => {});

    expect(ta()!.value).toBe("");
    // The foreign draft is dropped so it can't resurface.
    expect(window.localStorage.getItem(draftKey("p_text"))).toBeNull();
  });

  it("leaves the field empty when no draft was saved", async () => {
    await renderApp();
    emitPrompt(textEvent("p_text"));
    await act(async () => {});

    expect(ta()!.value).toBe("");
  });

  it("clears the draft when the field is emptied (type then delete all)", async () => {
    await renderApp();
    emitPrompt(textEvent("p_text"));
    await act(async () => {});
    // Type, then clear — the save side must remove the draft, not keep stale text.
    fireEvent.input(ta()!, { target: { value: "abc" } });
    expect(window.localStorage.getItem(draftKey("p_text"))).not.toBeNull();
    fireEvent.input(ta()!, { target: { value: "" } });

    expect(window.localStorage.getItem(draftKey("p_text"))).toBeNull();
  });

  it("clears the draft on answer so it is not restored next time", async () => {
    stashDraft("p_text", "to be cleared");
    await renderApp();
    emitPrompt(textEvent("p_text"));
    await act(async () => {});
    // Submit the textarea (Enter completes a text prompt).
    fireEvent.keyDown(ta()!, { key: "Enter" });
    await act(async () => {});

    expect(window.localStorage.getItem(draftKey("p_text"))).toBeNull();
  });
});
