/**
 * PromptPanel — single rendering path: every prompt (simple or rich) is
 * desugared to the A2UI v0.9 envelope and rendered through the A2UI renderer
 * with the cenno catalog. External contract unchanged: {prompt, onAnswer}.
 *
 * Surface lifecycle: the processor (and its surface) is built per prompt.id
 * and the renderer subtree is keyed on it, so a second prompt fully rebuilds
 * the surface instead of patching stale components/data.
 *
 * Action contract (see src/a2ui/desugar.ts header): action names starting
 * with "submit" complete the prompt; context carries {value, via}. The
 * /choice binding resolves to a 1-element array (unwrapped here), /scale to a
 * number (stringified here). Empty answers stay allowed (ack/skip).
 */
import { useMemo, useRef } from "react";
import { MessageProcessor } from "@a2ui/web_core/v0_9";
import { injectBasicCatalogStyles } from "@a2ui/web_core/v0_9/basic_catalog";
import { A2uiSurface } from "@a2ui/react/v0_9";
import { cennoCatalog } from "./a2ui/catalog";
import { desugar, SURFACE_ID } from "./a2ui/desugar";

// Once at module level. Guarded: jsdom (vitest) has no adoptedStyleSheets,
// and the helper assumes it exists.
if (typeof document !== "undefined" && document.adoptedStyleSheets) {
  injectBasicCatalogStyles();
}

export interface Prompt {
  id: string;
  title: string;
  body_md: string;
  input: { kind: string };
  choices?: string[];
  flow?: "mood" | "question" | "ema" | "reminder" | "ambient";
  progress?: { step: number; total: number };
}

export type Via = "text" | "choice";

export default function PromptPanel({
  prompt,
  onAnswer,
}: {
  prompt: Prompt;
  onAnswer: (id: string, answer: string, via: Via) => void;
}) {
  // Refs keep the action handler (created once per prompt.id) pointed at the
  // latest props without rebuilding the processor on every render.
  const onAnswerRef = useRef(onAnswer);
  onAnswerRef.current = onAnswer;
  const promptIdRef = useRef(prompt.id);
  promptIdRef.current = prompt.id;

  const surface = useMemo(
    () => {
      const processor = new MessageProcessor([cennoCatalog], (action) => {
        // Only "submit*" actions complete the prompt (desugar contract).
        if (!action.name.startsWith("submit")) return;
        const { value, via } = action.context as { value: unknown; via: Via };
        // /choice binding arrives as a 1-element array; unwrap it.
        const raw = Array.isArray(value) ? value[0] : value;
        // /scale arrives as a number; stringify. null/undefined → "" (ack).
        const answer = raw == null ? "" : String(raw);
        onAnswerRef.current(promptIdRef.current, answer, via);
      });
      processor.processMessages(
        desugar(prompt) as Parameters<typeof processor.processMessages>[0],
      );
      const created = processor.model.surfacesMap.get(SURFACE_ID);
      if (!created) throw new Error("desugar produced no surface");
      return created;
    },
    // Keyed by prompt.id: a replacing prompt rebuilds processor + surface.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [prompt.id],
  );

  // The panel root owns the surface color (catalog components stay
  // transparent); data-flow switches the semantic theme.
  return (
    <div className="prompt-panel" data-flow={prompt.flow ?? "question"}>
      <A2uiSurface key={prompt.id} surface={surface} />
    </div>
  );
}
