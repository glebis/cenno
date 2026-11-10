import { describe, expect, it } from "vitest";
import { comboFromKeyEvent, comboGlyphs, type KeyLike } from "./shortcutRecorder";

const key = (code: string, mods: Partial<KeyLike> = {}): KeyLike => ({
  code,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
});

describe("comboFromKeyEvent", () => {
  it("builds the default combo", () => {
    expect(comboFromKeyEvent(key("KeyC", { metaKey: true, shiftKey: true }))).toEqual({
      kind: "combo",
      combo: "Cmd+Shift+C",
    });
  });

  it("uses the physical key, so Option doesn't turn C into ç", () => {
    expect(comboFromKeyEvent(key("KeyC", { altKey: true, ctrlKey: true }))).toEqual({
      kind: "combo",
      combo: "Ctrl+Alt+C",
    });
  });

  it("maps digits, punctuation, arrows and F-keys", () => {
    const c = (code: string) =>
      comboFromKeyEvent(key(code, { metaKey: true }));
    expect(c("Digit1")).toEqual({ kind: "combo", combo: "Cmd+1" });
    expect(c("Comma")).toEqual({ kind: "combo", combo: "Cmd+Comma" });
    expect(c("ArrowUp")).toEqual({ kind: "combo", combo: "Cmd+ArrowUp" });
    expect(c("F5")).toEqual({ kind: "combo", combo: "Cmd+F5" });
  });

  it("waits while only modifiers are held", () => {
    expect(comboFromKeyEvent(key("ShiftLeft", { shiftKey: true, metaKey: true }))).toEqual({
      kind: "pending",
      held: ["Cmd", "Shift"],
    });
  });

  it("bare Escape cancels, bare Backspace/Delete clears", () => {
    expect(comboFromKeyEvent(key("Escape")).kind).toBe("cancel");
    expect(comboFromKeyEvent(key("Backspace")).kind).toBe("clear");
    expect(comboFromKeyEvent(key("Delete")).kind).toBe("clear");
    // With a modifier they're ordinary keys.
    expect(comboFromKeyEvent(key("Escape", { metaKey: true }))).toEqual({
      kind: "combo",
      combo: "Cmd+Escape",
    });
  });

  it("rejects combos that would hijack typing", () => {
    expect(comboFromKeyEvent(key("KeyA")).kind).toBe("invalid");
    expect(comboFromKeyEvent(key("KeyA", { shiftKey: true })).kind).toBe("invalid");
  });

  it("rejects keys the accelerator parser can't read", () => {
    expect(comboFromKeyEvent(key("IntlBackslash", { metaKey: true })).kind).toBe("invalid");
  });
});

describe("comboGlyphs", () => {
  it("renders in macOS modifier order", () => {
    expect(comboGlyphs("Cmd+Shift+C")).toEqual(["⇧", "⌘", "C"]);
    expect(comboGlyphs("Shift+Alt+Ctrl+Cmd+K")).toEqual(["⌃", "⌥", "⇧", "⌘", "K"]);
  });

  it("accepts hand-written config aliases", () => {
    expect(comboGlyphs("CmdOrCtrl+Option+p")).toEqual(["⌥", "⌘", "P"]);
    expect(comboGlyphs("Command+ArrowUp")).toEqual(["⌘", "↑"]);
    expect(comboGlyphs("Cmd+Digit1")).toEqual(["⌘", "1"]);
  });

  it("blank → no keycaps", () => {
    expect(comboGlyphs("")).toEqual([]);
    expect(comboGlyphs("  ")).toEqual([]);
  });
});
