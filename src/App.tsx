import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import PromptPanel, { Prompt, Via } from "./PromptPanel";
import { nextNotedWord } from "./notedWords";
import { PANEL_MIN_HEIGHT } from "./panelResize";
import { getDefaults, getTts } from "./userConfig";
import { useTtsPlayer } from "./tts/useTtsPlayer";
import "./App.css";

// The Rust side emits the whole AskRequest as `request` (see PromptEvent in
// src-tauri/src/lib.rs); we pick out the fields the panel renders.
interface PromptEvent {
  id: string;
  request: {
    title: string;
    body_md: string;
    input: { kind: string };
    choices?: string[];
    flow?: Prompt["flow"];
    progress?: { step: number; total: number };
    // Queue priority (low|normal|high). Reused by sound-out to gate voice-out.
    urgency?: string;
    // Optional short spoken summary for sound-out (spoken instead of the body).
    say?: string;
    // Native A2UI payload (already vetted by src-tauri/src/a2ui_guard.rs).
    a2ui?: unknown;
  };
  // Seconds until the Rust side times this prompt out. Full timeout_s on a
  // live event; partially elapsed on a prompt replayed via pending_prompts.
  remaining_s: number;
  // Set only for prompts emitted by an `ask_sequence` run (sibling to id/
  // request/remaining_s on the Rust PromptEvent). Tells the panel to swap
  // content instead of hiding between steps; absent for plain ask_user.
  seq?: { index: number; total: number; last: boolean };
}

const FLOWS = ["mood", "question", "ema", "reminder", "ambient"] as const;

function toPrompt({ id, request, seq }: PromptEvent): Prompt {
  // Fall back to the configured default flow (~/.cenno) when the agent omits
  // one; ignore an invalid configured value.
  const fallbackFlow = getDefaults()?.flow;
  const flow =
    request.flow ??
    (FLOWS.includes(fallbackFlow as (typeof FLOWS)[number])
      ? (fallbackFlow as Prompt["flow"])
      : undefined);
  return {
    id,
    title: request.title,
    body_md: request.body_md,
    input: request.input,
    choices: request.choices,
    flow,
    progress: request.progress,
    urgency: request.urgency,
    say: request.say,
    a2ui: request.a2ui,
    seq,
  };
}

/** The prompt on screen plus its auto-hide budget (see PromptEvent). */
interface ActivePrompt {
  prompt: Prompt;
  remainingS: number;
}

/** How long the "noted." confirmation lingers before the panel hides. */
export const ANSWERED_LINGER_MS = 900;

// setTimeout clamps to a 32-bit signed ms count; beyond that it fires
// immediately, which would instantly hide a long-timeout prompt.
const MAX_TIMEOUT_MS = 2 ** 31 - 1;

// Keep-alive floors (seconds). While the user is editing a text field the panel
// must NOT time out from under them — each keystroke/focus floors the deadline
// well into the future; when they stop (blur) it relaxes to a think-window. The
// effective deadline is always max(agent budget, this floor), so keep-alive only
// ever extends — it can never cut an agent's own longer timeout short.
const KEEPALIVE_EDIT_S = 600; // actively editing → effectively never expires
const KEEPALIVE_IDLE_S = 45; // stopped editing → at least this long to think

// Write a value into a (possibly React-controlled) text field so React's
// onChange actually fires. A plain `el.value = v` is swallowed by React's
// value tracker; going through the prototype setter defeats that shim, and the
// dispatched `input` event then drives the component's state + the keep-alive
// re-save. Mirrors the DOM-level *save* in the keep-alive effect.
function setFieldValue(el: HTMLInputElement | HTMLTextAreaElement, v: string) {
  const proto =
    el instanceof HTMLTextAreaElement
      ? HTMLTextAreaElement.prototype
      : HTMLInputElement.prototype;
  const setter = Object.getOwnPropertyDescriptor(proto, "value")?.set;
  if (setter) setter.call(el, v);
  else el.value = v;
  el.dispatchEvent(new Event("input", { bubbles: true }));
}

// Per-app-launch nonce. Rust prompt ids (`p_N`) restart from zero each launch,
// so a draft persisted under a bare id could be restored into an unrelated
// prompt that reuses that id on a LATER launch — leaking one agent's half-typed
// answer into another's field. Namespacing draft keys by a fresh per-launch
// nonce confines restore to the same app session (where ids are unique) and
// makes prior-session drafts unmatchable. The content fingerprint below is then
// defense-in-depth on top of that.
const DRAFT_PREFIX = "cenno-draft-";
const DRAFT_SESSION =
  globalThis.crypto?.randomUUID?.() ?? `s${Date.now()}-${Math.random()}`;
export function draftKey(id: string): string {
  return `${DRAFT_PREFIX}${DRAFT_SESSION}-${id}`;
}

// Drop drafts left by previous app launches (any other session nonce) so a
// reused prompt id can never restore stale text and old drafts don't pile up.
// Runs once at module load — i.e. once per webview launch.
function sweepForeignDrafts() {
  try {
    const mine = `${DRAFT_PREFIX}${DRAFT_SESSION}-`;
    const stale: string[] = [];
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i);
      if (k && k.startsWith(DRAFT_PREFIX) && !k.startsWith(mine)) stale.push(k);
    }
    for (const k of stale) localStorage.removeItem(k);
  } catch {
    /* storage unavailable — nothing to sweep */
  }
}
sweepForeignDrafts();

// A draft is also fingerprinted by its prompt's content (JSON-encoded so the
// title/body separator can't collide) — belt-and-suspenders against any id
// reuse: restore refuses a draft whose fingerprint doesn't match this prompt.
export function draftFingerprint(p: { title: string; body_md: string }): string {
  return JSON.stringify([p.title, p.body_md]);
}

// Save half-typed text for `prompt` (fingerprinted). Empty text clears the
// draft so "type then delete all" doesn't leave a stale value to restore.
function saveDraft(prompt: Prompt, v: string) {
  try {
    if (v) {
      localStorage.setItem(
        draftKey(prompt.id),
        JSON.stringify({ f: draftFingerprint(prompt), v }),
      );
    } else {
      localStorage.removeItem(draftKey(prompt.id));
    }
  } catch {
    /* storage unavailable — keep-alive still protects the live field */
  }
}

// Restore a stashed draft into the first text field on screen. Restores only
// when the stored fingerprint matches THIS prompt (else the draft is foreign —
// a reused id — and is dropped) and only into an empty field (never clobber a
// prompt the user is already typing into). Returns whether it restored.
function restoreDraft(prompt: Prompt): boolean {
  let raw: string | null = null;
  try {
    raw = localStorage.getItem(draftKey(prompt.id));
  } catch {
    return false;
  }
  if (!raw) return false;
  let saved: string | null = null;
  try {
    const parsed = JSON.parse(raw) as { f?: unknown; v?: unknown };
    if (parsed?.f === draftFingerprint(prompt) && typeof parsed.v === "string") {
      saved = parsed.v;
    }
  } catch {
    /* legacy/garbled value — treat as no match */
  }
  if (saved == null) {
    // Foreign or unparseable draft for this id — drop it so it can't resurface.
    try {
      localStorage.removeItem(draftKey(prompt.id));
    } catch {
      /* storage unavailable */
    }
    return false;
  }
  const root = document.getElementById("root") ?? document.body;
  const field = root.querySelector<HTMLInputElement | HTMLTextAreaElement>(
    'textarea, input[type="text"], input:not([type])',
  );
  if (field && field.value === "") {
    setFieldValue(field, saved);
    return true;
  }
  return false;
}

async function hideWindow() {
  try {
    // hide() = orderOut: — the correct counterpart to the Rust side's
    // order_front_regardless() (panel never became key/active app-wide).
    await getCurrentWindow().hide();
  } catch (e) {
    console.error("window hide failed:", e);
  }
}

function App() {
  const [active, setActive] = useState<ActivePrompt | null>(null);
  // Post-answer linger: true between a delivered answer and the hide.
  const [answered, setAnswered] = useState(false);
  // The confirmation word for THIS answer — drawn from the rotating cycle
  // once per answer (state, not a render-time call: re-renders during the
  // linger must not advance the cycle).
  const [confirmation, setConfirmation] = useState("");
  // Bumped synchronously whenever a prompt is installed (live event or
  // cold-start pull). Hide timers capture the value at arm time and bail if
  // it moved: effect cleanup only clears a timer at the next React commit,
  // so a timer can fire in the gap between a new prompt's arrival (Rust has
  // already ordered the window front by then) and that commit — and its
  // hideWindow() would land AFTER the show, leaving live content in an
  // invisible window.
  const hideGenerationRef = useRef(0);
  // Wall-clock (ms) when the current prompt reached the screen, and an
  // interaction floor the user's typing pushes forward. The auto-hide timer
  // expires at max(shownAt + budget, interactionFloor). A nonce re-arms that
  // timer the instant a keep-alive moves the floor.
  const shownAtRef = useRef(0);
  const interactionFloorRef = useRef(0);
  const [keepaliveTick, setKeepaliveTick] = useState(0);
  // Render-synced mirrors so the `prompt` listener (created once) can read the
  // latest state without re-subscribing.
  const activeRef = useRef<ActivePrompt | null>(null);
  activeRef.current = active;
  const answeredRef = useRef(false);
  answeredRef.current = answered;
  // True only in the gap between answering a non-last ask_sequence step and the
  // next step's `prompt` event — that event is allowed to swap in place. Any
  // OTHER incoming prompt while one is on screen is queued, not shown.
  const awaitingSwapRef = useRef(false);

  // After the on-screen prompt resolves, show the next still-pending one
  // (oldest first — first come, first served across agents) instead of hiding.
  // The registry holds every parked prompt; only the *display* is one-at-a-time.
  async function advanceOrHide() {
    // Exclude the prompt that just resolved: the registry removes it on
    // answer/dismiss/timeout, but the frontend timer can race that removal, and
    // we must never re-show the prompt we're leaving.
    const resolvedId = activeRef.current?.prompt.id;
    let next: PromptEvent | null = null;
    try {
      const pending = await invoke<PromptEvent[]>("pending_prompts");
      next = pending.find((p) => p.id !== resolvedId) ?? null;
    } catch (e) {
      console.error("pending_prompts failed:", e);
    }
    hideGenerationRef.current += 1;
    awaitingSwapRef.current = false;
    setAnswered(false);
    if (next) {
      setActive({ prompt: toPrompt(next), remainingS: next.remaining_s });
    } else {
      setActive(null);
      void hideWindow();
    }
  }

  useEffect(() => {
    let cancelled = false;
    const unlisten = listen<PromptEvent>("prompt", (event) => {
      // Queue, don't steamroll: if a different prompt is already on screen and
      // the user hasn't answered it yet, leave it up. The incoming prompt is
      // already parked in the registry and will be shown when the current one
      // resolves (advanceOrHide). The one exception is a sequence's next step,
      // which is meant to swap in place.
      if (
        activeRef.current &&
        !answeredRef.current &&
        !awaitingSwapRef.current
      ) {
        return;
      }
      awaitingSwapRef.current = false;
      hideGenerationRef.current += 1;
      setActive({ prompt: toPrompt(event.payload), remainingS: event.payload.remaining_s });
      // A new prompt cancels any in-flight answered linger.
      setAnswered(false);
    });
    // Cold-start race: on a bridge autolaunch the agent's ask (and its
    // `prompt` event) can land before this listener existed — the event is
    // lost and the panel window sits blank. Once the listener is registered,
    // pull anything still answerable and show the newest. The functional
    // update keeps this idempotent: a live event that already set state wins
    // over the (same-or-older) pulled prompt.
    unlisten.then(async () => {
      try {
        const pending = await invoke<PromptEvent[]>("pending_prompts");
        if (cancelled || pending.length === 0) return;
        const oldest = pending[0]; // first come, first served
        setActive((current) => {
          if (current) return current;
          // Bump only when actually installing: a live prompt already on
          // screen has armed timers holding the current generation — an
          // unconditional bump would orphan them (they'd bail and the
          // window would never hide). Impure inside an updater, but
          // idempotent in effect: with current == null no timer is armed,
          // so an extra bump under double-invocation changes nothing.
          hideGenerationRef.current += 1;
          return { prompt: toPrompt(oldest), remainingS: oldest.remaining_s };
        });
      } catch (e) {
        console.error("pending_prompts failed:", e);
      }
    });
    return () => {
      cancelled = true;
      unlisten.then((fn) => fn());
    };
  }, []);

  // External dismiss: the `dismiss_pending` MCP tool already unparked the
  // prompt(s) server-side and emits this event so the panel comes down now,
  // instead of lingering until its timeout. Used by agent-driven voice loops
  // that speak the question via cenno but capture the answer elsewhere (an
  // external STT) — the panel hides the moment that answer lands.
  useEffect(() => {
    const unlisten = listen("dismiss-panel", () => {
      hideGenerationRef.current += 1;
      awaitingSwapRef.current = false;
      setAnswered(false);
      setActive(null);
      void hideWindow();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Whenever a prompt reaches the screen — fresh, replayed, or pulled off the
  // queue — tell Rust it's shown so its timeout starts now, not when it was
  // received. A prompt waiting its turn in the queue thus can't expire before
  // the user ever sees it. Idempotent server-side, so re-renders are harmless.
  const shownIdRef = useRef<string | null>(null);
  useEffect(() => {
    if (!active) {
      shownIdRef.current = null;
      return;
    }
    if (shownIdRef.current === active.prompt.id) return;
    shownIdRef.current = active.prompt.id;
    // New prompt on screen: anchor its budget clock and clear any stale
    // interaction floor carried over from the previous prompt.
    shownAtRef.current = Date.now();
    interactionFloorRef.current = 0;
    // Read-back half of the draft safety net (the save side lives in the
    // keep-alive effect): if this prompt was torn down mid-typing, its text
    // was stashed under cenno-draft-<id>. Restore it now — once, before paint
    // — so a reopened panel comes back with the user's words instead of blank.
    if (restoreDraft(active.prompt)) {
      // The keep-alive input-listener isn't mounted yet this render (its effect
      // runs after this one), so the restore's dispatched `input` won't floor
      // the deadline. Floor it here directly: a reopened, half-answered prompt
      // must not expire out from under the restored text before the user reacts.
      interactionFloorRef.current = Date.now() + KEEPALIVE_EDIT_S * 1000;
      setKeepaliveTick((t) => t + 1);
      void invoke("keepalive", {
        id: active.prompt.id,
        secs: KEEPALIVE_EDIT_S,
      }).catch(() => {});
    }
    invoke("mark_shown", { id: active.prompt.id }).catch((e) =>
      console.error("mark_shown failed:", e),
    );
  }, [active]);

  // Timeout auto-hide: when remaining_s elapses the Rust side has already
  // returned TimedOut to the agent — at (roughly) the same moment the panel
  // must stop showing the now-unanswerable prompt instead of lingering
  // forever (the old behavior). Suspended while the answered linger runs.
  useEffect(() => {
    if (!active || answered) return;
    const generation = hideGenerationRef.current;
    // The agent's own budget, anchored to when this prompt reached the screen.
    const budgetDeadline = shownAtRef.current + active.remainingS * 1000;
    let timer: ReturnType<typeof setTimeout> | undefined;
    // Self-rescheduling: each tick re-reads the interaction floor, so a
    // keystroke that pushed the floor out (and bumped keepaliveTick to re-run
    // this effect) keeps the panel up instead of hiding mid-typing.
    const tick = () => {
      // A newer prompt was installed after this timer was armed — hiding now
      // would hide THAT prompt.
      if (hideGenerationRef.current !== generation) return;
      const deadline = Math.max(budgetDeadline, interactionFloorRef.current);
      const left = deadline - Date.now();
      if (left <= 0) {
        // Timed out (and the user isn't mid-edit): show the next queued prompt
        // or hide. The Rust side honors the same keep-alive floor, so a
        // delivered answer still resolves up to this moment.
        void advanceOrHide();
        return;
      }
      timer = setTimeout(tick, Math.min(left, MAX_TIMEOUT_MS));
    };
    tick();
    return () => {
      if (timer) clearTimeout(timer);
    };
  }, [active, answered, keepaliveTick]);

  // Keep-alive: never let the panel time out while the user is editing a text
  // field, and give them a think-window after they stop. Any input/focus floors
  // the deadline far out; blur relaxes it to ~45s. Each move also tells Rust
  // (so the parked ask() doesn't expire) and re-arms the hide timer above.
  useEffect(() => {
    if (!active) return;
    const id = active.prompt.id;
    const root = document.getElementById("root") ?? document.body;
    const isTextEntry = (el: EventTarget | null) =>
      el instanceof HTMLElement &&
      (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);
    const floor = (secs: number) => {
      interactionFloorRef.current = Date.now() + secs * 1000;
      setKeepaliveTick((t) => t + 1);
      void invoke("keepalive", { id, secs }).catch(() => {});
    };
    const onEdit = (e: Event) => {
      if (!isTextEntry(e.target)) return;
      floor(KEEPALIVE_EDIT_S);
      // Safety net: persist the in-progress text per prompt so it survives even
      // an unexpected teardown. Cleared once the prompt is answered/dismissed,
      // or when the field is emptied (saveDraft removes an empty draft).
      const el = e.target as HTMLInputElement & HTMLElement;
      const v = typeof el.value === "string" ? el.value : el.textContent ?? "";
      saveDraft(active.prompt, v);
    };
    const onFocusOut = (e: FocusEvent) => {
      if (isTextEntry(e.target)) floor(KEEPALIVE_IDLE_S);
    };
    root.addEventListener("input", onEdit, true);
    root.addEventListener("focusin", onEdit, true);
    root.addEventListener("focusout", onFocusOut, true);
    return () => {
      root.removeEventListener("input", onEdit, true);
      root.removeEventListener("focusin", onEdit, true);
      root.removeEventListener("focusout", onFocusOut, true);
    };
  }, [active]);

  // Answered linger: keep the surface up with a quiet confirmation, then
  // hide. Never unmount straight to a blank window — that was the
  // white-flash / abrupt-vanish path.
  useEffect(() => {
    if (!answered) return;
    const generation = hideGenerationRef.current;
    const timer = setTimeout(() => {
      // Same new-prompt race as the timeout timer above.
      if (hideGenerationRef.current !== generation) return;
      // Linger done: advance to the next queued prompt, or hide if none.
      void advanceOrHide();
    }, ANSWERED_LINGER_MS);
    return () => clearTimeout(timer);
  }, [answered]);

  async function handleAnswer(id: string, answer: string, via: Via) {
    let resolved: boolean;
    try {
      resolved = await invoke<boolean>("answer_prompt", { id, answer, via });
    } catch (e) {
      // Keep the panel up so the user can retry instead of silently losing it.
      console.error("answer_prompt failed:", e);
      return;
    }
    if (!resolved) {
      // Prompt already timed out (or unknown id) — the agent never saw this
      // answer. Skeleton behavior: log it, still confirm-and-hide.
      console.warn(`prompt ${id} already expired; answer was not delivered`);
    }
    // Answered → the saved draft is no longer needed.
    try {
      localStorage.removeItem(draftKey(id));
    } catch {
      /* storage unavailable */
    }
    // Mid-sequence step (ask_sequence, not the last): do NOT hide and do NOT
    // run the "noted." linger. The Rust loop fires the next registry.ask the
    // instant this answer resolves, so the next `prompt` event is already on
    // its way and will overwrite `active` (the listener calls setActive) —
    // keeping the current panel mounted until then avoids a hide/reshow flash.
    // Bump the hide generation so this step's armed timeout-hide timer bails
    // instead of taking the panel down before the next step lands.
    const seq = active?.prompt.seq;
    if (seq && !seq.last) {
      hideGenerationRef.current += 1;
      // The next sequence step's `prompt` event is already on its way; let it
      // swap in place (rather than being queued behind this resolved step).
      awaitingSwapRef.current = true;
      // Clear any stale answered/confirmation state so the next step renders
      // clean (defensive — it should already be false at this point).
      setAnswered(false);
      return;
    }
    setConfirmation(nextNotedWord());
    setAnswered(true);
    // The confirmation card needs no more than the minimum panel: shrink a
    // tall prompt's window back down for the linger. Best-effort — the card
    // centers correctly at any height.
    invoke("resize_panel", { height: PANEL_MIN_HEIGHT }).catch((e) => {
      console.error("resize_panel (answered) failed:", e);
    });
  }

  // User clicked the panel's ✕: end the parked ask() as a no-answer
  // (dismiss_prompt → registry.dismiss → ask() returns TimedOut, the same
  // wire shape the agent already handles on timeout) and take the panel down
  // SILENTLY — dismiss isn't an answer, so no "noted." linger. Bump the hide
  // generation like the answer path so the timeout/linger timers bail instead
  // of fighting this teardown.
  async function handleDismiss(id: string) {
    hideGenerationRef.current += 1;
    // The user took the panel down themselves — drop the saved draft.
    try {
      localStorage.removeItem(draftKey(id));
    } catch {
      /* storage unavailable */
    }
    try {
      await invoke<boolean>("dismiss_prompt", { id });
    } catch (e) {
      // Even if the dismiss round-trip fails, take the panel down: the user
      // asked for it gone. The prompt will time out on its own server-side.
      console.error("dismiss_prompt failed:", e);
    }
    // Dismissing the current prompt advances to the next queued one (or hides).
    void advanceOrHide();
  }

  // sound-out: speak the prompt aloud when it appears, gated by urgency +
  // ~/.cenno config. Called before the early returns so hook order stays
  // stable; with no active prompt it gets null and stays silent.
  const tts = useTtsPlayer(
    active
      ? {
          id: active.prompt.id,
          title: active.prompt.title,
          body_md: active.prompt.body_md,
          say: active.prompt.say,
          urgency: active.prompt.urgency,
        }
      : null,
    getTts(),
  );

  if (!active) return null;

  if (answered) {
    // Same surface (class + flow theme) so the color holds steady; only the
    // content swaps to the confirmation (one word from the rotating cycle,
    // centered both ways by .prompt-panel--answered). data-tauri-drag-region
    // keeps the panel draggable during the linger.
    return (
      <div
        className="prompt-panel prompt-panel--answered"
        data-flow={active.prompt.flow ?? "question"}
        data-tauri-drag-region
      >
        <p className="answered-note">{confirmation}</p>
      </div>
    );
  }

  // key={prompt.id}: a new prompt replacing the current one must remount the
  // panel so it doesn't inherit the half-typed text of the old prompt.
  return (
    <PromptPanel
      key={active.prompt.id}
      prompt={active.prompt}
      onAnswer={handleAnswer}
      onDismiss={handleDismiss}
      onStopReading={tts.speaking ? tts.stop : undefined}
    />
  );
}

export default App;
