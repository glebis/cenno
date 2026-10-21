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
 * Trim ASCII whitespace only, mirroring `check_image_url` in
 * `src-tauri/src/a2ui_guard.rs`. `String.prototype.trim` also strips U+FEFF and
 * U+00A0 (which Rust's `str::trim` does not) while missing U+0085 (which Rust
 * strips), so the two sides would classify the same input differently. ASCII
 * trimming is byte-identical on both.
 */
function trimAscii(s: string): string {
  return s.replace(/^[\t\n\v\f\r ]+|[\t\n\v\f\r ]+$/g, "");
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
  const trimmed = trimAscii(url);
  // A leading non-ASCII whitespace (U+00A0, U+0085, U+FEFF, …) would leave the
  // scheme undetectable while looking invisible — reject it explicitly rather
  // than misrouting the value into the local-path branch.
  const first = trimmed.charAt(0);
  if (first && first.charCodeAt(0) > 0x7f && (/\s/.test(first) || first === "\u0085")) {
    throw new ImageLoadError(
      "image url starts with a non-ASCII whitespace character — remove it; a data: URI or a local path must start at the first character",
    );
  }
  const probe = lowerScheme(trimmed);

  if (DATA_URI_OK.test(probe)) return trimmed;
  if (probe.startsWith("data:")) throw new ImageLoadError(MALFORMED_DATA_URI);
  if (probe.startsWith("https:") || probe.startsWith("http:")) {
    throw new ImageLoadError(REMOTE_NOT_ALLOWED);
  }
  const cached = cache.get(trimmed);
  if (cached) return cached;
  return loadLocalImage(trimmed);
}

/** Lowercase ONLY the leading scheme token, never the payload or the path. */
function lowerScheme(value: string): string {
  const colon = value.indexOf(":");
  if (colon <= 0) return value;
  return value.slice(0, colon).toLowerCase() + value.slice(colon);
}

/** Drop every memoised read — used by tests and by retry affordances. */
export function clearImageCache(): void {
  cache.clear();
}
