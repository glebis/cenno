import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import PromptPanel, { Prompt, Via } from "./PromptPanel";
import { nextNotedWord } from "./notedWords";
import { PANEL_MIN_HEIGHT } from "./panelResize";
import { getDefaults } from "./userConfig";
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

  useEffect(() => {
    let cancelled = false;
    const unlisten = listen<PromptEvent>("prompt", (event) => {
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
        const newest = pending[pending.length - 1];
        setActive((current) => {
          if (current) return current;
          // Bump only when actually installing: a live prompt already on
          // screen has armed timers holding the current generation — an
          // unconditional bump would orphan them (they'd bail and the
          // window would never hide). Impure inside an updater, but
          // idempotent in effect: with current == null no timer is armed,
          // so an extra bump under double-invocation changes nothing.
          hideGenerationRef.current += 1;
          return { prompt: toPrompt(newest), remainingS: newest.remaining_s };
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

  // Timeout auto-hide: when remaining_s elapses the Rust side has already
  // returned TimedOut to the agent — at (roughly) the same moment the panel
  // must stop showing the now-unanswerable prompt instead of lingering
  // forever (the old behavior). Suspended while the answered linger runs.
  useEffect(() => {
    if (!active || answered) return;
    const generation = hideGenerationRef.current;
    const timer = setTimeout(() => {
      // A newer prompt was installed after this timer was armed (and before
      // cleanup could clear it) — hiding now would hide THAT prompt.
      if (hideGenerationRef.current !== generation) return;
      setActive(null);
      void hideWindow();
    }, Math.min(active.remainingS * 1000, MAX_TIMEOUT_MS));
    return () => clearTimeout(timer);
  }, [active, answered]);

  // Answered linger: keep the surface up with a quiet confirmation, then
  // hide. Never unmount straight to a blank window — that was the
  // white-flash / abrupt-vanish path.
  useEffect(() => {
    if (!answered) return;
    const generation = hideGenerationRef.current;
    const timer = setTimeout(() => {
      // Same new-prompt race as the timeout timer above.
      if (hideGenerationRef.current !== generation) return;
      setActive(null);
      setAnswered(false);
      void hideWindow();
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
    try {
      await invoke<boolean>("dismiss_prompt", { id });
    } catch (e) {
      // Even if the dismiss round-trip fails, take the panel down: the user
      // asked for it gone. The prompt will time out on its own server-side.
      console.error("dismiss_prompt failed:", e);
    }
    setAnswered(false);
    setActive(null);
    void hideWindow();
  }

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
    />
  );
}

export default App;
