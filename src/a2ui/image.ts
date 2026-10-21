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

/** Inline raster formats a webview can render without help. */
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
  const pending = invoke<string>("load_image", { path, maxBytes: undefined }).catch(
    (cause: unknown) => {
      // A failed read must not poison the cache: a retry after the file appears
      // has to reach the backend again.
      cache.delete(path);
      throw new ImageLoadError(
        typeof cause === "string" ? cause : String(cause),
      );
    },
  );
  cache.set(path, pending);
  return pending;
}

/**
 * Classify what an agent supplied as an image reference and return something
 * safe to put in an `<img src>`.
 *
 * Data URIs for raster formats pass through untouched; other data URIs and all
 * remote URLs are rejected (cenno never fetches); everything else is treated as
 * a local path and read through the `load_image` command.
 */
export async function resolveImageUrl(url: string): Promise<string> {
  if (DATA_URI_OK.test(url)) return url;
  if (url.startsWith("data:")) throw new ImageLoadError(MALFORMED_DATA_URI);
  if (url.startsWith("https:") || url.startsWith("http:")) {
    throw new ImageLoadError(REMOTE_NOT_ALLOWED);
  }
  const cached = cache.get(url);
  if (cached) return cached;
  return loadLocalImage(url);
}

/** Drop every memoised read — used by tests and by retry affordances. */
export function clearImageCache(): void {
  cache.clear();
}
