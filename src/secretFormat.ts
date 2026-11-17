/**
 * Live format validation for `kind: "secret"` panels.
 *
 * The pattern catalogue lives in Rust (`src-tauri/src/secret_formats.rs`) and is
 * fetched once per session — NOT duplicated here. Two reasons:
 *
 * 1. One source of truth. A pattern that drifts between the panel's caption and
 *    the CLI's warning would tell the user two different things about the same
 *    key.
 * 2. The Rust `regex` crate is the narrower dialect (linear, no lookaround, no
 *    backreferences). Every pattern in the catalogue is written to be valid in
 *    BOTH that crate and JS `RegExp`, so shipping the same string here is safe
 *    by construction — see the portability note in `secret_formats.rs`.
 *
 * Validation is ADVISORY and never blocks a submit. A rotated key, a
 * private-endpoint key, or an in-house key with a `sk-` prefix legitimately does
 * not match the published shape; refusing it would be worse than accepting a
 * typo the user can see.
 *
 * A fetch failure degrades to "no opinion" (no caption) rather than a false
 * warning: an unavailable catalogue is not evidence that the key is wrong.
 */
import { invoke } from "@tauri-apps/api/core";

/** One row of the built-in catalogue, as serialized by `secret_formats`. */
export interface SecretFormatInfo {
  id: string;
  label: string;
  /** Regex source, valid in both Rust `regex` and JS `RegExp`. */
  pattern: string;
  /** Human hint shown alongside a mismatch ("OpenAI keys start sk-"). */
  hint: string;
}

/**
 * `match`     — the value satisfies the pattern.
 * `mismatch`  — it does not; warn, still allow submit.
 * `unknown`   — no format declared, catalogue unavailable, or the pattern did
 *               not compile; say nothing rather than guess.
 * `empty`     — nothing typed yet; the caption stays quiet.
 */
export type FormatState = "match" | "mismatch" | "unknown" | "empty";

export interface FormatVerdict {
  state: FormatState;
  /** Ready-to-render sentence. Empty when the caption should not appear. */
  message: string;
}

/**
 * The receipt for a stored credential, as returned by `store_secret`. By
 * construction it has no field that can carry the value — `hint` is the masked
 * shape (`sk-pro…1234`) and is all the asking agent ever sees.
 */
export interface SecretReceipt {
  stored: boolean;
  name: string;
  backend: string;
  format: string | null;
  hint: string;
  path: string;
}

const UNKNOWN: FormatVerdict = { state: "unknown", message: "" };

let catalogue: Map<string, SecretFormatInfo> | null = null;
/** In-flight fetch, so N panels opening at once issue one invoke. */
let inFlight: Promise<Map<string, SecretFormatInfo> | null> | null = null;
/** Compiled patterns, keyed by format id. A failed compile caches `null`. */
const compiled = new Map<string, RegExp | null>();

/**
 * Fetch (and cache) the catalogue. Resolves to null on any failure — callers
 * treat that as "cannot judge".
 */
export async function loadSecretFormats(): Promise<Map<
  string,
  SecretFormatInfo
> | null> {
  if (catalogue) return catalogue;
  if (inFlight) return inFlight;
  inFlight = invoke<SecretFormatInfo[]>("secret_formats")
    .then((list) => {
      catalogue = new Map(list.map((f) => [f.id, f]));
      return catalogue;
    })
    .catch((e) => {
      // Not console.error: on a plain-browser preview (vitest, `npm run dev`
      // without Tauri) there is no backend at all and this is expected.
      console.debug("cenno: secret format catalogue unavailable:", e);
      return null;
    })
    .finally(() => {
      inFlight = null;
    });
  return inFlight;
}

/** Test seam: drop the cache so a fresh catalogue can be fetched. */
export function resetSecretFormatsForTests(): void {
  catalogue = null;
  inFlight = null;
  compiled.clear();
}

/**
 * Pure verdict for a (format id, value) pair against an already-loaded
 * catalogue. Kept separate from the fetching so it is trivially testable.
 */
export function verdictFor(
  formats: Map<string, SecretFormatInfo> | null,
  formatId: string | null | undefined,
  value: string,
): FormatVerdict {
  if (!value) return { state: "empty", message: "" };
  if (!formatId || !formats) return UNKNOWN;
  const format = formats.get(formatId);
  if (!format) return UNKNOWN;

  let re = compiled.get(format.id);
  if (re === undefined) {
    try {
      re = new RegExp(format.pattern);
    } catch {
      // A pattern the JS engine rejects must not turn into a scary warning.
      re = null;
    }
    compiled.set(format.id, re);
  }
  if (!re) return UNKNOWN;

  if (re.test(value)) {
    return {
      state: "match",
      message: `Looks like a ${format.label}.`,
    };
  }
  return {
    state: "mismatch",
    // Always say the value will still be saved — the user must not think a
    // warning has blocked them, or they will go chase a key format instead of
    // pressing Save.
    message: `Not the usual shape for a ${format.label} — ${format.hint}. Saving anyway; check for a typo or a copied-in space.`,
  };
}
