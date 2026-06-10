import { beforeEach, describe, expect, it } from "vitest";
import { NOTED_INDEX_KEY, NOTED_WORDS, nextNotedWord } from "./notedWords";

beforeEach(() => {
  window.localStorage.clear();
});

describe("NOTED_WORDS", () => {
  it("has exactly 30 entries", () => {
    expect(NOTED_WORDS).toHaveLength(30);
  });

  it("entries are nonempty, lowercase, period-terminated, ≤4 words", () => {
    for (const word of NOTED_WORDS) {
      expect(word.trim().length).toBeGreaterThan(0);
      expect(word).toBe(word.toLowerCase());
      expect(word.endsWith(".")).toBe(true);
      expect(word.split(/\s+/).length).toBeLessThanOrEqual(4);
    }
  });

  it("has no duplicates", () => {
    expect(new Set(NOTED_WORDS).size).toBe(NOTED_WORDS.length);
  });
});

describe("nextNotedWord", () => {
  it("cycles sequentially through all 30 and wraps", () => {
    const seen = Array.from({ length: NOTED_WORDS.length }, () =>
      nextNotedWord(),
    );
    expect(seen).toEqual([...NOTED_WORDS]);
    // 31st call wraps back to the start.
    expect(nextNotedWord()).toBe(NOTED_WORDS[0]);
  });

  it("consecutive calls never repeat", () => {
    let prev = nextNotedWord();
    for (let i = 0; i < 2 * NOTED_WORDS.length; i++) {
      const next = nextNotedWord();
      expect(next).not.toBe(prev);
      prev = next;
    }
  });

  it("persists the cursor in localStorage between calls", () => {
    nextNotedWord(); // serves index 0, stores 1
    expect(window.localStorage.getItem(NOTED_INDEX_KEY)).toBe("1");
    // A "restarted" session (fresh module call, same storage) resumes.
    expect(nextNotedWord()).toBe(NOTED_WORDS[1]);
    expect(window.localStorage.getItem(NOTED_INDEX_KEY)).toBe("2");
  });

  it("recovers from a garbled stored index", () => {
    window.localStorage.setItem(NOTED_INDEX_KEY, "definitely not a number");
    expect(nextNotedWord()).toBe(NOTED_WORDS[0]);
    window.localStorage.setItem(NOTED_INDEX_KEY, "-7");
    expect(nextNotedWord()).toBe(NOTED_WORDS[0]);
  });

  it("falls back to the first word when storage throws", () => {
    const original = window.localStorage;
    Object.defineProperty(window, "localStorage", {
      configurable: true,
      get() {
        throw new Error("private mode");
      },
    });
    try {
      expect(nextNotedWord()).toBe(NOTED_WORDS[0]);
      expect(nextNotedWord()).toBe(NOTED_WORDS[0]); // no cursor → no advance
    } finally {
      Object.defineProperty(window, "localStorage", {
        configurable: true,
        value: original,
      });
    }
  });
});
