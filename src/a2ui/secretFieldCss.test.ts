// @vitest-environment node
import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";

/**
 * The masked field's layout and masking contract.
 *
 * A key is a single token. Wrapped onto a second line it reads as two values,
 * and a partly-off-screen one cannot be glance-checked — which is the whole
 * point of the reveal toggle. jsdom cannot measure layout, and under jsdom
 * `import.meta.url` is not a `file://` URL (so a plain readFileSync by URL
 * throws `TypeError: The URL must be of scheme file`), hence the node
 * environment here, matching src/styles/theme.test.ts.
 */
const css = readFileSync(new URL("./catalog.css", import.meta.url), "utf8");

/** The declaration block for one selector (first match). */
function block(selector: string): string {
  const start = css.indexOf(`${selector} {`);
  expect(start, `${selector} missing from catalog.css`).toBeGreaterThan(-1);
  return css.slice(start, css.indexOf("}", start));
}

describe("masked field: one line, no wrap", () => {
  it("refuses to wrap a credential", () => {
    const rules = block(".cenno-field__input--secret");
    expect(rules).toContain("white-space: pre");
    expect(rules).toContain("word-break: normal");
    expect(rules).toContain("overflow-wrap: normal");
    // Horizontal only: vertical scrolling inside a 44px box would hide the
    // start of the key just as effectively as wrapping.
    expect(rules).toContain("overflow-x: auto");
    expect(rules).toContain("overflow-y: hidden");
  });

  it("masks by default and unmasks only while revealed", () => {
    expect(block(".cenno-field__input--secret")).toContain(
      "-webkit-text-security: disc",
    );
    expect(block(".cenno-field__input--revealed")).toContain(
      "-webkit-text-security: none",
    );
  });

  it("hides the scrollbar so it cannot eat the baseline", () => {
    expect(block(".cenno-field__input--secret")).toContain(
      "scrollbar-width: none",
    );
    expect(css).toContain(".cenno-field__input--secret::-webkit-scrollbar");
  });

  it("keeps the reveal control an icon button, not a text pill", () => {
    const rules = block(".cenno-field__reveal");
    // A round 28px target: it must not grow into a text-width pill.
    expect(rules).toContain("width: 28px");
    expect(rules).toContain("border-radius: 50%");
    // No glyph fallback text is styled here, so the label cannot leak back in.
    expect(rules).not.toContain("font-size");
  });
});
