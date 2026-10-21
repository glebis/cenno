import { invoke } from "@tauri-apps/api/core";

/**
 * Errors raised while turning an agent-supplied image reference into a usable
 * `src`. `reason` is shown verbatim in the panel, so it must stay a full
 * actionable sentence — never prefixed or decorated by callers.
 */
export class ImageLoadError extends Error {
  readonly reason: string;

  constructor(reason: string) {
    super(reason);
    this.name = "ImageLoadError";
    this.reason = reason;
  }
}

/** Inline raster formats a webview can render without help.
 *
 * Twin of `DATA_URI_OK` / `is_renderable_data_uri` in
 * `src-tauri/src/a2ui_guard.rs` — the two must stay identical; adding a format
 * here without adding it there lets the guard and the renderer disagree.
 */
const DATA_URI_OK = /^data:image\/(?:png|jpeg|gif|webp);base64,/;

const MALFORMED_DATA_URI =
  "malformed data: URI — expected data:image/<png|jpeg|gif|webp>;base64,…";

const REMOTE_NOT_ALLOWED =
  "remote images are not allowed — embed it as a data: URI or pass a local file path";

/**
 * One entry per path, holding the in-flight or settled read. Both cache hits
 * and concurrent tiles asking for the same file collapse onto a single
 * `load_image` call.
 */
const cache = new Map<string, Promise<string>>();

function loadLocalImage(path: string): Promise<string> {
  const raw = invoke<string>("load_image", { path, maxBytes: undefined });
  // Held in a `let` so the rejection handler can compare identity below; the
  // handler only runs after the assignment, so there is no TDZ hazard.
  let pending: Promise<string>;
  pending = raw.catch((cause: unknown) => {
    // Evict only MY entry. `clearImageCache` runs mid-render (PromptPanel), so
    // an older read can reject after a newer promise for the same path is
    // already cached — deleting by key alone would drop the newer one.
    if (cache.get(path) === pending) cache.delete(path);
    throw new ImageLoadError(typeof cause === "string" ? cause : String(cause));
  });
  cache.set(path, pending);
  return pending;
}

/**
 * Classify what an agent supplied as an image reference and return something
 * safe to put in an `<img src>`.
 *
 * Data URIs for raster formats pass through untouched; other data URIs and all
 * remote URLs are rejected (cenno never fetches); everything else is treated as
 * a local path and read through the `load_image` command, which stays camelCase
 * (`maxBytes`) because Tauri's default argument case is Camel.
 *
 * URL schemes are case-insensitive (RFC 3986) and agents do emit `DATA:` and
 * `HTTPS:`, so the scheme token is lowercased before classification. Only the
 * scheme is normalised — the payload and the local path are case-significant.
 */
export async function resolveImageUrl(url: string): Promise<string> {
  if (typeof url !== "string") {
    throw new ImageLoadError("image url must be a string");
  }
  const trimmed = url.trim();
  const colon = trimmed.indexOf(":");
  const probe =
    colon > 0
      ? trimmed.slice(0, colon).toLowerCase() + trimmed.slice(colon)
      : trimmed;

  if (DATA_URI_OK.test(probe)) return trimmed;
  if (probe.startsWith("data:")) throw new ImageLoadError(MALFORMED_DATA_URI);
  if (probe.startsWith("https:") || probe.startsWith("http:")) {
    throw new ImageLoadError(REMOTE_NOT_ALLOWED);
  }
  const cached = cache.get(trimmed);
  if (cached) return cached;
  return loadLocalImage(trimmed);
}

/** Drop every memoised read — used by tests and by retry affordances. */
export function clearImageCache(): void {
  cache.clear();
}
