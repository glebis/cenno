// @vitest-environment node
import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
const css = readFileSync(new URL("./theme.css", import.meta.url), "utf8");
describe("theme.css", () => {
  for (const flow of ["mood", "question", "ema", "reminder", "ambient"] as const)
    it(`maps data-flow=${flow} to its hue`, () =>
      expect(css).toContain(`[data-flow="${flow}"]`));
  it("defines the semantic surface var default", () =>
    expect(css).toContain("--cenno-surface: var(--cenno-color-flow-question)"));
  it("defines semantic text vars", () => {
    expect(css).toContain("--cenno-text: var(--cenno-color-text-default)");
    expect(css).toContain("--cenno-text-dim: var(--cenno-color-text-dim)");
  });
  it("defines the semantic line var", () =>
    expect(css).toContain("--cenno-line: var(--cenno-color-line)"));
});
