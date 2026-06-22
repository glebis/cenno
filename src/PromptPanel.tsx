/**
 * PromptPanel — single rendering engine: every prompt flows through the A2UI
 * renderer with the cenno catalog. Simple prompts are desugared to the v0.9
 * envelope; prompts carrying a native `a2ui` payload (vetted by the Rust
 * boundary guard) feed it to the processor as-is, skipping desugar.
 * External contract unchanged: {prompt, onAnswer}.
 *
 * Rich-path safety net: the Rust guard validates envelope SHAPE only, so a
 * guard-passing payload can still fail to build (unknown component type,
 * no surface, no "root" component — the React renderer mounts component id
 * "root") or throw mid-render. Either way the panel falls back to rendering
 * desugar(prompt) — title/body always exist, the user can still answer, and
 * the agent gets a real answer instead of a parked prompt burning timeout_s.
 * Build failures are caught in the memo; render failures by an error
 * boundary around <A2uiSurface>.
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
import { Component, useLayoutEffect, useMemo, useRef, type ReactNode } from "react";
import { MessageProcessor } from "@a2ui/web_core/v0_9";
import { injectBasicCatalogStyles } from "@a2ui/web_core/v0_9/basic_catalog";
import { A2uiSurface } from "@a2ui/react/v0_9";
import { cennoCatalog } from "./a2ui/catalog";
import { desugar, SURFACE_ID } from "./a2ui/desugar";
import { getWidgets } from "./userConfig";
import { observePanelContent } from "./panelResize";

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
  /** Queue priority (low|normal|high); sound-out gates voice-out on it. */
  urgency?: string;
  /**
   * Native A2UI v0.9 message array (the desugar envelope shape). When
   * present it REPLACES desugaring: the payload is fed to the processor
   * as-is. Shape is vetted by the Rust boundary guard
   * (src-tauri/src/a2ui_guard.rs) before it ever reaches the webview; if it
   * still fails to build/render, the panel falls back to desugar(prompt).
   */
  a2ui?: unknown;
  /**
   * Set only for prompts emitted by an `ask_sequence` run. When present with
   * `last === false`, answering this prompt keeps the panel up (no hide, no
   * "noted." linger) so the next step's event can swap in without a flash;
   * `last === true` (or absent) hides as usual. See handleAnswer in App.tsx.
   */
  seq?: { index: number; total: number; last: boolean };
}

export type Via = "text" | "choice" | "voice_text";

/** The component id the React renderer mounts (@a2ui/react A2uiSurface). */
const ROOT_COMPONENT_ID = "root";

/**
 * Catches render-time throws from a native a2ui surface and renders the
 * desugared fallback instead of a blank panel. Keyed on prompt.id by the
 * caller so the error state resets for each new prompt.
 */
class SurfaceErrorBoundary extends Component<
  { fallback: ReactNode; children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch(error: unknown) {
    console.error(
      "cenno: a2ui surface threw while rendering; falling back to the desugared prompt:",
      error,
    );
  }

  render() {
    return this.state.failed ? this.props.fallback : this.props.children;
  }
}

export default function PromptPanel({
  prompt,
  onAnswer,
  onDismiss,
  onStopReading,
}: {
  prompt: Prompt;
  onAnswer: (id: string, answer: string, via: Via) => void;
  /**
   * The ✕ chrome dismisses the prompt: App invokes dismiss_prompt(id), which
   * ends the parked ask() as a no-answer (TimedOut) — the same wire contract
   * the agent already handles on timeout. Optional so existing callers/tests
   * that only answer keep working.
   */
  onDismiss?: (id: string) => void;
  /**
   * Present only while sound-out is reading this prompt aloud. Renders a mute
   * control in the chrome that stops the speech without dismissing the prompt
   * (the user can still read and answer). Absent → no control shown.
   */
  onStopReading?: () => void;
}) {
  // Refs keep the action handler (created once per prompt.id) pointed at the
  // latest props without rebuilding the processor on every render.
  const onAnswerRef = useRef(onAnswer);
  onAnswerRef.current = onAnswer;
  const promptIdRef = useRef(prompt.id);
  promptIdRef.current = prompt.id;

  const { surface, fallback } = useMemo(
    () => {
      const build = (messages: unknown) => {
        const processor = new MessageProcessor([cennoCatalog], (action) => {
          // Only "submit*" actions complete the prompt (desugar contract; rich
          // a2ui payloads use the same action contract).
          if (!action.name.startsWith("submit")) return;
          // Harden against payload-authored contexts: via defaults to "text"
          // unless it is literally "choice" or "voice_text"; value is coerced
          // via String() with null/undefined → "" (ack).
          const ctx = action.context as
            | { value?: unknown; via?: unknown }
            | null
            | undefined;
          const via: Via =
            ctx?.via === "choice" || ctx?.via === "voice_text"
              ? ctx.via
              : "text";
          // /choice binding arrives as a 1-element array; unwrap it.
          const raw = Array.isArray(ctx?.value) ? ctx.value[0] : ctx?.value;
          // /scale arrives as a number; stringify. null/undefined → "" (ack).
          const answer = raw == null ? "" : String(raw);
          onAnswerRef.current(promptIdRef.current, answer, via);
        });
        processor.processMessages(
          messages as Parameters<typeof processor.processMessages>[0],
        );
        // Desugar always targets SURFACE_ID; native payloads may pick their
        // own surfaceId, so fall back to the first created surface.
        const created =
          processor.model.surfacesMap.get(SURFACE_ID) ??
          processor.model.surfacesMap.values().next().value;
        if (!created) throw new Error("payload created no surface");
        // The renderer mounts component id "root"; without it the surface
        // renders blank — treat that as a build failure so we fall back.
        if (!created.componentsModel.get(ROOT_COMPONENT_ID)) {
          throw new Error(
            `payload has no "${ROOT_COMPONENT_ID}" component to mount`,
          );
        }
        return created;
      };

      const desugared = () => build(desugar(prompt, getWidgets()));
      if (prompt.a2ui == null) {
        return { surface: desugared(), fallback: null };
      }
      try {
        // Eagerly build the desugared fallback too: the error boundary needs
        // it ready if the rich surface throws mid-render.
        return { surface: build(prompt.a2ui), fallback: desugared() };
      } catch (e) {
        console.error(
          "cenno: a2ui payload failed to build; falling back to the desugared prompt:",
          e,
        );
        return { surface: desugared(), fallback: null };
      }
    },
    // Keyed by prompt.id: a replacing prompt rebuilds processor + surface.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [prompt.id],
  );

  // Content-driven window height: after this prompt's surface mounts (and
  // whenever its content box changes — e.g. the error boundary swapping in
  // the fallback), measure the natural content height and ask Rust to fit
  // the window to it (resize_panel, clamped to [240, 560]). Keyed on
  // prompt.id like the surface memo: each prompt gets a fresh fit.
  const rootRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const root = rootRef.current;
    const content = contentRef.current;
    if (!root || !content) return;
    return observePanelContent(content, root);
  }, [prompt.id]);

  // The panel root owns the surface color (catalog components stay
  // transparent); data-flow switches the semantic theme.
  //
  // Dragging: data-tauri-drag-region only works on the EXACT element under
  // the cursor (Tauri 2 checks the mousedown target, no bubbling), so the
  // attribute on the root covers padding/empty areas while chips, inputs and
  // links stay clickable. Title/body text are attribute-less children, so an
  // invisible top strip (over padding + the title's first line — nothing
  // interactive lives up there) guarantees an always-grabbable handle.
  // Requires core:window:allow-start-dragging (capabilities/default.json).
  return (
    <div
      ref={rootRef}
      className="prompt-panel"
      data-flow={prompt.flow ?? "question"}
      data-tauri-drag-region
    >
      <div className="prompt-panel__drag-strip" data-tauri-drag-region aria-hidden="true" />
      {/* Chrome layer — OUTSIDE the A2UI surface so simple and rich payloads
          both get the wordmark + dismiss ✕. Absolutely positioned across the
          top (App.css); not a drag region (the ✕ is interactive — with
          acceptFirstMouse the single click both keys the panel and dismisses).
          z-index above the drag strip so the ✕ stays clickable. */}
      <div className="prompt-panel__chrome">
        <span className="prompt-panel__wordmark">cenno</span>
        {onStopReading && (
          <button
            type="button"
            className="prompt-panel__mute"
            aria-label="Stop reading aloud"
            title="Stop reading aloud"
            onClick={onStopReading}
          >
            {/* Monochrome speaker-off glyph; inherits chrome color like the ✕. */}
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <path
                d="M4 9v6h4l5 4V5L8 9H4z"
                fill="currentColor"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinejoin="round"
              />
              <path d="M17 9l5 5M22 9l-5 5" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
            </svg>
          </button>
        )}
        <button
          type="button"
          className="prompt-panel__dismiss"
          aria-label="Dismiss"
          onClick={() => onDismiss?.(prompt.id)}
        >
          ✕
        </button>
      </div>
      {/* Measurement wrapper (flex:none → always its natural height); the
          root itself is 100vh so its scrollHeight can't size DOWN.
          Deliberately NOT a drag region: with acceptFirstMouse the very
          first click lands here, and a draggable wrapper would turn a tap
          on body text into a window drag. Root + top strip cover dragging. */}
      <div ref={contentRef} className="prompt-panel__content">
        <SurfaceErrorBoundary
          key={prompt.id}
          fallback={fallback ? <A2uiSurface surface={fallback} /> : null}
        >
          <A2uiSurface surface={surface} />
        </SurfaceErrorBoundary>
      </div>
    </div>
  );
}
