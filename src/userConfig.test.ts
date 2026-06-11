import { describe, expect, it } from "vitest";
import { tokensToCss } from "./userConfig";

describe("tokensToCss", () => {
  it("flattens a DTCG tree to --cenno-* variables, skipping $-keys", () => {
    const css = tokensToCss({
      $description: "ignored",
      color: {
        $type: "color",
        flow: {
          mood: { $value: "#FF0000", $description: "red" },
          ema: { $value: "#00FF00" },
        },
        text: { default: { $value: "#FFFFFF" } },
      },
      space: { 2: { $value: "16px" } },
    });
    expect(css).toContain("--cenno-color-flow-mood: #FF0000;");
    expect(css).toContain("--cenno-color-flow-ema: #00FF00;");
    expect(css).toContain("--cenno-color-text-default: #FFFFFF;");
    expect(css).toContain("--cenno-space-2: 16px;");
    expect(css).not.toContain("$description");
    expect(css.startsWith(":root {")).toBe(true);
  });

  it("joins font-family arrays, quoting multi-word entries", () => {
    const css = tokensToCss({
      font: {
        family: {
          default: { $value: ["-apple-system", "SF Pro Text", "sans-serif"] },
        },
      },
    });
    expect(css).toContain(
      "--cenno-font-family-default: -apple-system, 'SF Pro Text', sans-serif;",
    );
  });

  it("kebab-cases camelCase segments", () => {
    const css = tokensToCss({ fontWeight: { bold: { $value: "700" } } });
    expect(css).toContain("--cenno-font-weight-bold: 700;");
  });

  it("returns empty string when there are no token leaves", () => {
    expect(tokensToCss({ $description: "nothing here" })).toBe("");
    expect(tokensToCss(null)).toBe("");
  });
});
