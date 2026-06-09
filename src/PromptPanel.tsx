import { useState } from "react";
import ReactMarkdown from "react-markdown";

export interface Prompt {
  id: string;
  title: string;
  body_md: string;
  input: { kind: string };
}

export default function PromptPanel({
  prompt,
  onAnswer,
}: {
  prompt: Prompt;
  onAnswer: (id: string, answer: string, via: "text") => void;
}) {
  const [text, setText] = useState("");
  return (
    <div className="prompt-panel">
      <h1>{prompt.title}</h1>
      <ReactMarkdown>{prompt.body_md}</ReactMarkdown>
      <div className="prompt-row">
        <input
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && onAnswer(prompt.id, text, "text")}
          autoFocus
        />
        <button onClick={() => onAnswer(prompt.id, text, "text")}>Send</button>
      </div>
    </div>
  );
}
