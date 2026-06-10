import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import PromptPanel, { Prompt, Via } from "./PromptPanel";
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
}

function toPrompt({ id, request }: PromptEvent): Prompt {
  return {
    id,
    title: request.title,
    body_md: request.body_md,
    input: request.input,
    choices: request.choices,
    flow: request.flow,
    progress: request.progress,
    a2ui: request.a2ui,
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

  useEffect(() => {
    let cancelled = false;
    const unlisten = listen<PromptEvent>("prompt", (event) => {
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
        setActive(
          (current) =>
            current ?? { prompt: toPrompt(newest), remainingS: newest.remaining_s },
        );
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
    const timer = setTimeout(() => {
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
    const timer = setTimeout(() => {
      setActive(null);
      setAnswered(false);
      void hideWindow();
    }, ANSWERED_LINGER_MS);
    return () => clearTimeout(timer);
  }, [answered, active]);

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
    setAnswered(true);
  }

  if (!active) return null;

  if (answered) {
    // Same surface (class + flow theme) so the color holds steady; only the
    // content swaps to the confirmation. data-tauri-drag-region keeps the
    // panel draggable during the linger.
    return (
      <div
        className="prompt-panel prompt-panel--answered"
        data-flow={active.prompt.flow ?? "question"}
        data-tauri-drag-region
      >
        <p className="answered-note">noted.</p>
      </div>
    );
  }

  // key={prompt.id}: a new prompt replacing the current one must remount the
  // panel so it doesn't inherit the half-typed text of the old prompt.
  return (
    <PromptPanel key={active.prompt.id} prompt={active.prompt} onAnswer={handleAnswer} />
  );
}

export default App;
