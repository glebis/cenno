// @vitest-environment node
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";

// The Help & About window renders NOTICE verbatim (`../NOTICE?raw`), so the
// file is the single source of truth. This test fails when the required
// disclosures drift out of it — the drift the help window would inherit.
const notice = readFileSync(new URL("../NOTICE", import.meta.url), "utf8");

describe("NOTICE acknowledgements", () => {
  it("states the license and the AI-assistance disclosure", () => {
    expect(notice).toContain("Apache License, Version 2.0");
    expect(notice).toMatch(/assisted by AI coding tools/i);
    expect(notice).toContain("AUTHORSHIP.md");
  });

  it("credits the vendored agent marks and their licenses", () => {
    expect(notice).toContain("simple-icons");
    expect(notice).toContain("CC0-1.0");
    expect(notice).toContain("@lobehub/icons-static-svg");
    expect(notice).toContain("MIT License");
  });

  it("names the major runtime dependencies", () => {
    expect(notice).toContain("Tauri");
    expect(notice).toContain("React");
    expect(notice).toContain("SpeechTranscriber");
  });
});
