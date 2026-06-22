import { describe, expect, it } from "vitest";
import { speechTextFor } from "./speechText";

describe("speechTextFor — say overrides body", () => {
  it("speaks the agent's `say` when present, not the body", () => {
    const out = speechTextFor({
      say: "Heads up: a database refactor is mid-flight. Commit it or leave it?",
      title: "Commit the in-progress db-refactor?",
      body_md: "This checkout is on **refactor/i5ly.4-split-db-rs** with ...",
    });
    expect(out).toBe("Heads up: a database refactor is mid-flight. Commit it or leave it?");
    expect(out).not.toContain("refactor/"); // body not read
  });

  it("still normalizes the `say` text (no raw markdown / identifiers voiced)", () => {
    const out = speechTextFor({ say: "Check `db.rs` and **commit**", title: "t", body_md: "b" });
    expect(out).not.toContain("`");
    expect(out).not.toContain("**");
    expect(out).toContain("db dot rs");
  });

  it("falls back to title + body when `say` is absent", () => {
    const out = speechTextFor({ title: "Commit?", body_md: "On **main** now" });
    expect(out).toContain("Commit?");
    expect(out).toContain("main");
    expect(out).not.toContain("**");
  });

  it("falls back to title + body when `say` is empty/whitespace", () => {
    const out = speechTextFor({ say: "   ", title: "Commit?", body_md: "body" });
    expect(out).toContain("Commit?");
    expect(out).toContain("body");
  });

  it("returns empty when nothing speakable", () => {
    expect(speechTextFor({ title: "", body_md: "" })).toBe("");
  });
});
