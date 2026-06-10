/**
 * PromptPanel — single rendering engine: every prompt flows through the A2UI
 * renderer with the cenno catalog. Simple prompts are desugared to the v0.9
 * envelope; prompts carrying a native `a2ui` payload (vetted by the Rust
 * boundary guard) feed it to the processor as-is, skipping desugar.
 * External contract unchanged: {prompt, onAnswer}.
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
  /**
   * Native A2UI v0.9 message array (the desugar envelope shape). When
   * present it REPLACES desugaring: the payload is fed to the processor
   * as-is. Shape is vetted by the Rust boundary guard
   * (src-tauri/src/a2ui_guard.rs) before it ever reaches the webview, so
   * this side only feeds it through.
   */
  a2ui?: unknown;
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
        // Only "submit*" actions complete the prompt (desugar contract; rich
        // a2ui payloads use the same action contract).
        if (!action.name.startsWith("submit")) return;
        // Harden against payload-authored contexts: via defaults to "text"
        // unless it is literally "choice"; value is coerced via String()
        // with null/undefined → "" (ack).
        const ctx = action.context as
          | { value?: unknown; via?: unknown }
          | null
          | undefined;
        const via: Via = ctx?.via === "choice" ? "choice" : "text";
        // /choice binding arrives as a 1-element array; unwrap it.
        const raw = Array.isArray(ctx?.value) ? ctx.value[0] : ctx?.value;
        // /scale arrives as a number; stringify. null/undefined → "" (ack).
        const answer = raw == null ? "" : String(raw);
        onAnswerRef.current(promptIdRef.current, answer, via);
      });
      // Native a2ui payload (vetted by the Rust guard) replaces desugaring.
      const messages = prompt.a2ui ?? desugar(prompt);
      processor.processMessages(
        messages as Parameters<typeof processor.processMessages>[0],
      );
      // Desugar always targets SURFACE_ID; native payloads may pick their
      // own surfaceId, so fall back to the first created surface.
      const created =
        processor.model.surfacesMap.get(SURFACE_ID) ??
        processor.model.surfacesMap.values().next().value;
      if (!created) throw new Error("prompt produced no surface");
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
