import { describe, expect, it } from "vitest";
import { normalizeForSpeech } from "./normalize";

describe("normalizeForSpeech — markdown stripping", () => {
  it("removes bold markers but keeps the words", () => {
    const out = normalizeForSpeech("Commit the **db-refactor** now");
    expect(out).not.toContain("**");
    expect(out).toContain("db-refactor"); // hyphenated word left readable
  });

  it("removes inline-code backticks", () => {
    const out = normalizeForSpeech("the file `db.rs` changed");
    expect(out).not.toContain("`");
  });

  it("strips heading markers", () => {
    expect(normalizeForSpeech("# Commit?")).toBe("Commit?");
  });

  it("strips list bullets", () => {
    const out = normalizeForSpeech("- one\n- two");
    expect(out).not.toMatch(/^[-*]\s/m);
    expect(out).toContain("one");
    expect(out).toContain("two");
  });

  it("keeps link text and drops the URL", () => {
    const out = normalizeForSpeech("see [privacy](https://example.com/x) page");
    expect(out).toContain("privacy");
    expect(out).not.toContain("https");
    expect(out).not.toContain("example.com");
  });
});

describe("normalizeForSpeech — code identifiers voiced intelligibly", () => {
  it("voices slashes in a branch path", () => {
    // the real db-refactor example
    const out = normalizeForSpeech("refactor/i5ly.4-split-db-rs");
    expect(out).toContain("slash");
    expect(out).not.toContain("/");
    expect(out).not.toContain("-"); // identifier hyphens become spaces
  });

  it("voices dotted filenames as 'dot'", () => {
    expect(normalizeForSpeech("db.rs")).toBe("db dot rs");
    expect(normalizeForSpeech("schema.sql")).toBe("schema dot sql");
  });

  it("voices underscores in snake_case as spaces", () => {
    expect(normalizeForSpeech("dump_schema.rs")).toBe("dump schema dot rs");
  });

  it("does NOT turn a sentence-ending period into 'dot'", () => {
    const out = normalizeForSpeech("It looks mid-flight, not complete. Do you agree?");
    expect(out).not.toContain("dot");
    expect(out).toContain("complete.");
  });
});

describe("normalizeForSpeech — substance preservation (the anxiety guard)", () => {
  it("never empties a non-empty body", () => {
    const out = normalizeForSpeech("This checkout is on **refactor/i5ly.4-split-db-rs**");
    expect(out.trim().length).toBeGreaterThan(0);
  });

  it("preserves every prose word from the real prompt body", () => {
    const body =
      "This checkout is on **refactor/i5ly.4-split-db-rs** with another " +
      "session's unfinished work (schema.sql rewrite, db.rs split, an unwired " +
      "dump_schema.rs helper). It looks mid-flight, not complete.";
    const out = normalizeForSpeech(body);
    for (const word of ["checkout", "session", "unfinished", "rewrite", "split", "helper", "complete"]) {
      expect(out).toContain(word);
    }
    // identifiers got voiced, not dropped
    expect(out).toContain("slash");
    expect(out).toContain("dot");
  });

  it("collapses redundant whitespace", () => {
    expect(normalizeForSpeech("a    b\n\n\nc")).toBe("a b c");
  });

  it("returns empty for empty input", () => {
    expect(normalizeForSpeech("")).toBe("");
    expect(normalizeForSpeech("   ")).toBe("");
  });
});
