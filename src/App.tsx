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

function App() {
  const [prompt, setPrompt] = useState<Prompt | null>(null);

  useEffect(() => {
    let cancelled = false;
    const unlisten = listen<PromptEvent>("prompt", (event) => {
      setPrompt(toPrompt(event.payload));
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
        setPrompt((current) => current ?? toPrompt(newest));
      } catch (e) {
        console.error("pending_prompts failed:", e);
      }
    });
    return () => {
      cancelled = true;
      unlisten.then((fn) => fn());
    };
  }, []);

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
      // answer. Skeleton behavior: log it, still clear and hide.
      console.warn(`prompt ${id} already expired; answer was not delivered`);
    }
    setPrompt(null);
    // hide() = orderOut: — the correct counterpart to the Rust side's
    // order_front_regardless() (panel never became key/active app-wide).
    await getCurrentWindow().hide();
  }

  // key={prompt.id}: a new prompt replacing the current one must remount the
  // panel so it doesn't inherit the half-typed text of the old prompt.
  return prompt ? <PromptPanel key={prompt.id} prompt={prompt} onAnswer={handleAnswer} /> : null;
}

export default App;
