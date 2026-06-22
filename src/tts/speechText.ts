import { normalizeForSpeech } from "./normalize";

export interface SpeakableFields {
  /** Optional short, ear-friendly summary the agent wrote to be spoken. */
  say?: string;
  title: string;
  body_md: string;
}

/**
 * The text sound-out actually speaks. When the agent provides `say`, that is
 * spoken instead of the full prompt — the body is often too long/structured to
 * listen to. Otherwise we fall back to the title plus body. Either way the
 * result is normalized (markdown stripped, identifiers voiced).
 */
export function speechTextFor({ say, title, body_md }: SpeakableFields): string {
  if (say && say.trim()) return normalizeForSpeech(say);
  return [normalizeForSpeech(title), normalizeForSpeech(body_md)].filter(Boolean).join(". ");
}
