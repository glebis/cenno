/**
 * The post-answer confirmation vocabulary — 30 quiet, deadpan, occasionally
 * ironic ways to say "noted." (per the user: "maybe somewhat ironically").
 * Reporter style: lowercase, terminal period, never more than four words.
 * Irony escalates sparingly — most entries are plain clerk-speak; a few
 * admit the machine is enjoying itself.
 *
 * Rotation is a SEQUENTIAL cycle, not random: consecutive answers must
 * differ, and randomness repeats often enough to feel broken. The cursor
 * persists in localStorage so the cycle survives panel restarts.
 */

export const NOTED_WORDS: readonly string[] = [
  "noted.",
  "logged.",
  "filed.",
  "archived.",
  "inscribed.",
  "etched.",
  "committed.",
  "recorded.",
  "stamped.",
  "catalogued.",
  "indexed.",
  "preserved.",
  "duly noted.",
  "for the record.",
  "so it is written.",
  "entered into evidence.",
  "the ledger grows.",
  "it is known.",
  "saved to disk.",
  "one for the books.",
  "chiseled in stone.",
  "data point acquired.",
  "your truth is safe.",
  "history will judge.",
  "the archive thanks you.",
  "remembered forever.",
  "received and understood.",
  "acknowledged, human.",
  "beep. stored.",
  "immortalized.",
];

/** localStorage key for the rotation cursor (the NEXT index to serve). */
export const NOTED_INDEX_KEY = "cenno.notedIndex";

/**
 * The next confirmation word in the cycle; advances and persists the
 * cursor. Storage failures (private mode, quota, no window) degrade to
 * always serving the first word — the confirmation must never throw.
 */
export function nextNotedWord(): string {
  let index = 0;
  try {
    const raw = window.localStorage.getItem(NOTED_INDEX_KEY);
    const parsed = raw == null ? Number.NaN : Number(raw);
    // Garbled/out-of-range values reset the cycle instead of NaN-ing it.
    if (Number.isInteger(parsed) && parsed >= 0) {
      index = parsed % NOTED_WORDS.length;
    }
  } catch {
    // unreadable storage → start of cycle
  }
  try {
    window.localStorage.setItem(
      NOTED_INDEX_KEY,
      String((index + 1) % NOTED_WORDS.length),
    );
  } catch {
    // unwritable storage → same word every time, still fine
  }
  return NOTED_WORDS[index];
}
