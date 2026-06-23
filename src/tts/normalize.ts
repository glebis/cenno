/**
 * Convert a prompt's title / markdown body into text that a TTS engine can
 * speak intelligibly — without losing any of the substance.
 *
 * Two jobs:
 *   1. Strip markdown syntax (bold/italic/code/headings/lists/links) so the
 *      engine never reads "star star" or "backtick" aloud.
 *   2. Voice code-like identifiers sensibly: file paths, dotted filenames and
 *      snake_case become spoken words ("db dot rs", "refactor slash ...")
 *      rather than raw punctuation — while ordinary prose (sentence periods,
 *      hyphenated words) is left untouched.
 *
 * Guard: this only ever rewrites punctuation; it never drops prose words.
 */
export function normalizeForSpeech(text: string): string {
  if (!text || !text.trim()) return "";

  let out = text;

  // 1. Line-level markers: headings, blockquotes, list bullets.
  out = out
    .replace(/^[ \t]*#{1,6}[ \t]+/gm, "") // # Heading
    .replace(/^[ \t]*>[ \t]?/gm, "") // > blockquote
    .replace(/^[ \t]*[-*+][ \t]+/gm, "") // - bullet
    .replace(/^[ \t]*\d+\.[ \t]+/gm, ""); // 1. ordered item

  // 2. Links: keep the visible text, drop the URL.
  out = out.replace(/\[([^\]]+)\]\(([^)]+)\)/g, "$1");

  // 3. Inline emphasis / code markers. Asterisks and backticks never appear in
  //    our identifiers, so they are safe to strip globally. Double underscore
  //    (bold) is stripped; single underscore is left for the token step, where
  //    snake_case is handled.
  out = out
    .replace(/\*+/g, "")
    .replace(/__/g, "")
    .replace(/`/g, "");

  // 3b. Pronunciation fixes: "cenno" is Italian ("a nod/sign"), said "CHEN-no",
  //     but English TTS reads it "SEN-no". Respell it phonetically for speech
  //     only (this text is never shown). Word-boundary + case-insensitive.
  out = out.replace(/\bcenno\b/gi, "chenno");

  // 4. Tokenize and voice identifier-shaped tokens.
  const spoken = out
    .split(/\s+/)
    .map((token) => (isIdentifier(token) ? voiceIdentifier(token) : token))
    .join(" ");

  return spoken.replace(/\s+/g, " ").trim();
}

/**
 * A token is "identifier-shaped" if it contains a path separator, a snake_case
 * underscore, or a dot flanked by word characters (a dotted filename) — but NOT
 * a sentence-ending period (those have no word char after the dot).
 */
function isIdentifier(token: string): boolean {
  return token.includes("/") || token.includes("_") || /\w\.\w/.test(token);
}

function voiceIdentifier(token: string): string {
  return token
    .replace(/\//g, " slash ")
    .replace(/_/g, " ")
    .replace(/\./g, " dot ")
    .replace(/-/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}
