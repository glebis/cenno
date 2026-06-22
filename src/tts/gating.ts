/**
 * Decides whether an incoming prompt should be spoken aloud.
 *
 * Priority reuses the existing `urgency` field on AskRequest (Low/Normal/High)
 * rather than introducing a parallel P0/P1/P2 concept. `tts.enabled` is the
 * master switch (opt-in, default off). Within that, a prompt speaks when its
 * urgency is at or above the configured threshold — so High always speaks when
 * the feature is on, and the default threshold of `high` keeps everything else
 * silent until the user lowers it.
 */
// On the wire, AskRequest.urgency serializes lowercase ("low"/"normal"/"high");
// callers may also pass capitalized forms. Accept any string and fold case.
export type Urgency = string;

export interface TtsConfig {
  enabled: boolean;
  minUrgency: "low" | "normal" | "high";
}

const RANK: Record<string, number> = { low: 0, normal: 1, high: 2 };

export function shouldSpeak(urgency: Urgency | undefined, cfg: TtsConfig): boolean {
  if (!cfg.enabled) return false;
  // Agents that omit urgency default to Normal on the wire.
  const urgencyRank = RANK[(urgency ?? "Normal").toLowerCase()] ?? RANK.normal;
  return urgencyRank >= RANK[cfg.minUrgency];
}
