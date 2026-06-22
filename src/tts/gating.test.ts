import { describe, expect, it } from "vitest";
import { shouldSpeak, type TtsConfig } from "./gating";

const cfg = (over: Partial<TtsConfig> = {}): TtsConfig => ({
  enabled: true,
  minUrgency: "high",
  ...over,
});

describe("shouldSpeak — master switch", () => {
  it("never speaks when disabled, even for High", () => {
    expect(shouldSpeak("High", cfg({ enabled: false }))).toBe(false);
  });
});

describe("shouldSpeak — default threshold (high)", () => {
  it("speaks High by default", () => {
    expect(shouldSpeak("High", cfg())).toBe(true);
  });
  it("stays silent for Normal by default", () => {
    expect(shouldSpeak("Normal", cfg())).toBe(false);
  });
  it("stays silent for Low by default", () => {
    expect(shouldSpeak("Low", cfg())).toBe(false);
  });
});

describe("shouldSpeak — lowered threshold", () => {
  it("threshold=normal speaks Normal and High but not Low", () => {
    const c = cfg({ minUrgency: "normal" });
    expect(shouldSpeak("High", c)).toBe(true);
    expect(shouldSpeak("Normal", c)).toBe(true);
    expect(shouldSpeak("Low", c)).toBe(false);
  });
  it("threshold=low speaks everything", () => {
    const c = cfg({ minUrgency: "low" });
    expect(shouldSpeak("High", c)).toBe(true);
    expect(shouldSpeak("Normal", c)).toBe(true);
    expect(shouldSpeak("Low", c)).toBe(true);
  });
});

describe("shouldSpeak — High is always at/above any threshold", () => {
  it("High speaks at every threshold when enabled", () => {
    for (const minUrgency of ["low", "normal", "high"] as const) {
      expect(shouldSpeak("High", cfg({ minUrgency }))).toBe(true);
    }
  });
});

describe("shouldSpeak — robustness", () => {
  it("treats a missing/unknown urgency as Normal", () => {
    // agents that don't set urgency default to Normal on the wire
    expect(shouldSpeak(undefined, cfg({ minUrgency: "normal" }))).toBe(true);
    expect(shouldSpeak(undefined, cfg({ minUrgency: "high" }))).toBe(false);
  });
});
