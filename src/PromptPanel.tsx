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
      <div className="prompt-body">
        <ReactMarkdown>{prompt.body_md}</ReactMarkdown>
      </div>
      {/* Empty submit is allowed on purpose: an empty answer is a deliberate ack/skip. */}
      <div className="prompt-row">
        <input
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) =>
            e.key === "Enter" &&
            !e.nativeEvent.isComposing && // IME: Enter confirms composition, not the answer
            onAnswer(prompt.id, text, "text")
          }
          autoFocus
        />
        <button onClick={() => onAnswer(prompt.id, text, "text")}>Send</button>
      </div>
    </div>
  );
}
