import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import PromptPanel, { Prompt } from "./PromptPanel";
import "./App.css";

interface PromptEvent {
  id: string;
  request: {
    title: string;
    body_md: string;
    input: { kind: string };
  };
}

function App() {
  const [prompt, setPrompt] = useState<Prompt | null>(null);

  useEffect(() => {
    const unlisten = listen<PromptEvent>("prompt", (event) => {
      const { id, request } = event.payload;
      setPrompt({ id, title: request.title, body_md: request.body_md, input: request.input });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  async function handleAnswer(id: string, answer: string, via: "text") {
    await invoke("answer_prompt", { id, answer, via });
    setPrompt(null);
    await getCurrentWindow().hide();
  }

  return prompt ? <PromptPanel prompt={prompt} onAnswer={handleAnswer} /> : null;
}

export default App;
