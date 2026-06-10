// @vitest-environment node
import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";

const css = readFileSync(new URL("./tokens.css", import.meta.url), "utf8");

describe("generated tokens.css", () => {
  it("contains every flow color from TOKENS.md", () => {
    for (const [name, hex] of [
      ["--cenno-color-flow-mood", "#FF6250"],
      ["--cenno-color-flow-question", "#1E4FD8"],
      ["--cenno-color-flow-ema", "#0E7C6B"],
      ["--cenno-color-flow-reminder", "#4A5568"],
      ["--cenno-color-flow-ambient", "#14171A"],
    ] as const) expect(css.toLowerCase()).toContain(`${name.toLowerCase()}: ${hex.toLowerCase()}`);
  });
  it("contains type scale and spacing", () => {
    expect(css).toContain("--cenno-type-question-l: 44px");
    expect(css).toContain("--cenno-type-caption: 13px");
    expect(css).toContain("--cenno-space-3: 24px");
    expect(css).toContain("--cenno-radius-control: 10px");
  });
});
