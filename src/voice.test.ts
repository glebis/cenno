import { describe, expect, it } from "vitest";
import { mergeTranscript } from "./voice";

describe("mergeTranscript", () => {
  it("empty base: transcript stands alone", () => {
    expect(mergeTranscript("", "hello there")).toBe("hello there");
  });

  it("typed base gets a separating space", () => {
    expect(mergeTranscript("Plan:", "ship it tomorrow")).toBe(
      "Plan: ship it tomorrow",
    );
  });

  it("base already ending in whitespace is not double-spaced", () => {
    expect(mergeTranscript("Plan: ", "ship it")).toBe("Plan: ship it");
  });

  it("each partial replaces the dictated tail, never the base", () => {
    const base = "Notes:";
    const first = mergeTranscript(base, "ship");
    const second = mergeTranscript(base, "ship it tomorrow");
    expect(first).toBe("Notes: ship");
    expect(second).toBe("Notes: ship it tomorrow");
  });
});
