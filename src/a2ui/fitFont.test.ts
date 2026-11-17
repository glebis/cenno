import { describe, expect, it } from "vitest";
import { MIN_FIT_FONT_PX, fitFontSize } from "./fitFont";

describe("fitFontSize", () => {
  it("leaves a value that already fits at the base size", () => {
    // The common case — a short token, or a widened panel. Returning `base`
    // exactly (not a rounded approximation) keeps the field from writing an
    // inline style that would then shadow the stylesheet.
    expect(fitFontSize(17, 300, 120)).toBe(17);
    expect(fitFontSize(17, 300, 300)).toBe(17);
  });

  it("scales a long key down proportionally", () => {
    // 600px of text into 300px: half the size.
    expect(fitFontSize(17, 300, 600)).toBe(8 < MIN_FIT_FONT_PX ? 11 : 8);
    // A realistic OpenAI key: ~51 chars at 17px ≈ 430px, into a 380px field.
    const size = fitFontSize(17, 380, 430);
    expect(size).toBeLessThan(17);
    expect(size).toBeGreaterThan(14);
  });

  it("never shrinks below the legibility floor", () => {
    // An absurd value must degrade to a horizontal scroll, not to 4px type.
    expect(fitFontSize(17, 100, 5000)).toBe(MIN_FIT_FONT_PX);
    expect(fitFontSize(17, 1, 5000)).toBe(MIN_FIT_FONT_PX);
  });

  it("never grows past the base size", () => {
    // Guard against a caller passing a stale tiny `needed`.
    expect(fitFontSize(17, 900, 10)).toBe(17);
  });

  it("is a no-op without layout information", () => {
    // First paint / detached node / jsdom: every width reads 0.
    expect(fitFontSize(17, 0, 500)).toBe(17);
    expect(fitFontSize(17, 300, 0)).toBe(17);
    expect(fitFontSize(0, 300, 500)).toBe(0);
  });

  it("honours a caller-supplied floor", () => {
    expect(fitFontSize(17, 100, 5000, 14)).toBe(14);
  });

  it("returns an integer so the DOM does not accumulate subpixel styles", () => {
    const size = fitFontSize(17, 377, 431);
    expect(Number.isInteger(size)).toBe(true);
  });
});
