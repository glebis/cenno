/**
 * Content-driven panel height — the webview side of the `resize_panel`
 * Tauri command (src-tauri/src/lib.rs).
 *
 * The window is created 420x240, but real prompts (the interview's EMA
 * scale questions, long bodies) routinely need more than 240 logical px;
 * clipping + a scrollbar is not the Reporter feel. After a prompt's surface
 * mounts, PromptPanel measures the content's natural height and asks Rust
 * for a window that fits it. Rust clamps to [240, 560] and keeps width 420.
 *
 * Geometry: `.prompt-panel` is height:100vh, so its own scrollHeight can
 * never shrink below the viewport — useless for sizing DOWN. Instead the
 * surface is wrapped in `.prompt-panel__content` (flex:none → always its
 * natural height) and the desired window height is that wrapper's
 * scrollHeight plus the panel root's vertical padding. CSS px == Tauri
 * logical px, no scale conversion needed.
 *
 * Loop safety: clamping happens here too (mirroring Rust's band) so the
 * |desired − current| ≤ 4px guard compares achievable numbers — otherwise a
 * 700px prompt would re-request 700 against a 560 window forever. Calls are
 * additionally rAF-debounced, and ResizeObserver only fires when the
 * CONTENT box changes (the wrapper's natural height does not depend on the
 * window height), so a resize cannot re-trigger itself.
 */
import { invoke } from "@tauri-apps/api/core";

/** Mirror of the Rust band (PANEL_MIN_HEIGHT / PANEL_MAX_HEIGHT). */
export const PANEL_MIN_HEIGHT = 240;
export const PANEL_MAX_HEIGHT = 560;

/** Ignore sub-4px differences — not worth a native window resize. */
const RESIZE_EPSILON = 4;

/** Clamp a measured height to the panel band (JS mirror of Rust's clamp). */
export function clampPanelHeight(height: number): number {
  if (!Number.isFinite(height)) return PANEL_MIN_HEIGHT;
  return Math.min(Math.max(height, PANEL_MIN_HEIGHT), PANEL_MAX_HEIGHT);
}

/**
 * The window height that would fit `content` without scrolling: the
 * wrapper's natural height plus the panel root's vertical padding, clamped
 * to the band.
 */
export function desiredPanelHeight(
  content: HTMLElement,
  root: HTMLElement,
): number {
  const style = getComputedStyle(root);
  // jsdom returns "" for unresolved styles → NaN → 0.
  const padding =
    (parseFloat(style.paddingTop) || 0) + (parseFloat(style.paddingBottom) || 0);
  return clampPanelHeight(content.scrollHeight + padding);
}

/**
 * Measure and, when it differs from the current window height by more than
 * the epsilon, ask Rust to resize. Returns whether a resize was requested.
 *
 * `currentHeight` defaults to window.innerHeight — the webview fills the
 * window (autoresizing mask installed by the nspanel conversion), so the
 * viewport height IS the current logical window height.
 */
export function syncPanelHeight(
  content: HTMLElement,
  root: HTMLElement,
  currentHeight: number = window.innerHeight,
): boolean {
  const height = desiredPanelHeight(content, root);
  if (Math.abs(height - currentHeight) <= RESIZE_EPSILON) return false;
  invoke("resize_panel", { height }).catch((e) => {
    // Non-fatal: the panel still works at its old size (root scrolls).
    console.error("resize_panel failed:", e);
  });
  return true;
}

/**
 * Keep the window fitted to `content` for the lifetime of one prompt:
 * one rAF-debounced measure right after mount, then re-measure whenever the
 * content box changes (a2ui fallback swap after a render throw, async
 * layout settling). Returns a cleanup function for the effect.
 */
export function observePanelContent(
  content: HTMLElement,
  root: HTMLElement,
): () => void {
  let raf = 0;
  const schedule = () => {
    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(() => syncPanelHeight(content, root));
  };
  schedule(); // initial fit for this prompt
  let observer: ResizeObserver | undefined;
  if (typeof ResizeObserver !== "undefined") {
    observer = new ResizeObserver(schedule);
    observer.observe(content);
  }
  return () => {
    cancelAnimationFrame(raf);
    observer?.disconnect();
  };
}
