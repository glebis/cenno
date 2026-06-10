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
  };
}

function App() {
  const [prompt, setPrompt] = useState<Prompt | null>(null);

  useEffect(() => {
    const unlisten = listen<PromptEvent>("prompt", (event) => {
      const { id, request } = event.payload;
      setPrompt({
        id,
        title: request.title,
        body_md: request.body_md,
        input: request.input,
        choices: request.choices,
        flow: request.flow,
        progress: request.progress,
      });
    });
    return () => {
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
