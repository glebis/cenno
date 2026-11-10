/**
 * Pure helpers behind the settings window's shortcut recorder.
 *
 * `comboFromKeyEvent` turns a keydown into the accelerator string the Rust
 * side parses (`parse_reopen_shortcut` → tauri-plugin-global-shortcut), e.g.
 * "Cmd+Shift+C". The key comes from `KeyboardEvent.code` (physical key), not
 * `.key`: with Option held, `.key` is "ç" instead of "c", and on non-US
 * layouts it is a localized character the accelerator parser can't read.
 *
 * `comboGlyphs` renders a stored combo as macOS keycaps (⌃⌥⇧⌘ order).
 */

export const DEFAULT_REOPEN_SHORTCUT = "Cmd+Shift+C";

/** Minimal slice of KeyboardEvent we read — keeps tests DOM-free. */
export interface KeyLike {
  code: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

export type RecordResult =
  /** Only modifiers held so far — keep listening; `held` previews them. */
  | { kind: "pending"; held: string[] }
  /** Bare Escape — abandon recording, keep the old shortcut. */
  | { kind: "cancel" }
  /** Bare Backspace/Delete — disable the shortcut. */
  | { kind: "clear" }
  /** A key we can't (or shouldn't) bind; `reason` is shown to the user. */
  | { kind: "invalid"; reason: string }
  | { kind: "combo"; combo: string };

const MODIFIER_CODES = new Set([
  "MetaLeft", "MetaRight", "ControlLeft", "ControlRight",
  "AltLeft", "AltRight", "ShiftLeft", "ShiftRight", "CapsLock", "Fn",
]);

// Non-alphanumeric codes the global-hotkey parser accepts verbatim
// (case-insensitive). Anything outside this list is rejected up front so we
// never persist a combo that silently fails to register.
const PASSTHROUGH_CODES = new Set([
  "Space", "Enter", "Tab", "Backspace", "Delete", "Escape",
  "Comma", "Period", "Slash", "Backslash", "Semicolon", "Quote",
  "BracketLeft", "BracketRight", "Minus", "Equal", "Backquote",
  "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight",
  "Home", "End", "PageUp", "PageDown",
]);

/** KeyboardEvent.code → accelerator key token, or null if unsupported. */
export function keyToken(code: string): string | null {
  let m = /^Key([A-Z])$/.exec(code);
  if (m) return m[1];
  m = /^Digit([0-9])$/.exec(code);
  if (m) return m[1];
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;
  if (/^Numpad([0-9])$/.test(code)) return code;
  return PASSTHROUGH_CODES.has(code) ? code : null;
}

function heldModifiers(e: KeyLike): string[] {
  const mods: string[] = [];
  if (e.metaKey) mods.push("Cmd");
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  return mods;
}

export function comboFromKeyEvent(e: KeyLike): RecordResult {
  const mods = heldModifiers(e);
  if (MODIFIER_CODES.has(e.code)) return { kind: "pending", held: mods };

  if (mods.length === 0 && e.code === "Escape") return { kind: "cancel" };
  if (mods.length === 0 && (e.code === "Backspace" || e.code === "Delete")) {
    return { kind: "clear" };
  }

  const key = keyToken(e.code);
  if (!key) return { kind: "invalid", reason: "That key can’t be used in a shortcut." };

  // A global hotkey steals the keystroke from every app. Without ⌘/⌃/⌥ it
  // would hijack plain typing (Shift+letter is just a capital letter).
  if (!e.metaKey && !e.ctrlKey && !e.altKey) {
    return { kind: "invalid", reason: "Include ⌘, ⌃ or ⌥ — otherwise it would hijack typing." };
  }

  return { kind: "combo", combo: [...mods, key].join("+") };
}

const MOD_GLYPH: Record<string, string> = {
  CMD: "⌘", COMMAND: "⌘", SUPER: "⌘", META: "⌘",
  CMDORCTRL: "⌘", COMMANDORCONTROL: "⌘", CMDORCONTROL: "⌘", COMMANDORCTRL: "⌘",
  CTRL: "⌃", CONTROL: "⌃",
  ALT: "⌥", OPTION: "⌥",
  SHIFT: "⇧",
};
// macOS menu order: Control, Option, Shift, Command.
const MOD_ORDER = ["⌃", "⌥", "⇧", "⌘"];

const KEY_GLYPH: Record<string, string> = {
  ARROWUP: "↑", UP: "↑", ARROWDOWN: "↓", DOWN: "↓",
  ARROWLEFT: "←", LEFT: "←", ARROWRIGHT: "→", RIGHT: "→",
  ENTER: "↩", RETURN: "↩", TAB: "⇥", BACKSPACE: "⌫", DELETE: "⌦",
  ESCAPE: "⎋", SPACE: "Space", HOME: "↖", END: "↘",
  PAGEUP: "⇞", PAGEDOWN: "⇟",
  COMMA: ",", PERIOD: ".", SLASH: "/", BACKSLASH: "\\", SEMICOLON: ";",
  QUOTE: "'", BRACKETLEFT: "[", BRACKETRIGHT: "]", MINUS: "-", EQUAL: "=",
  BACKQUOTE: "`",
};

/** "Cmd+Shift+C" → ["⇧", "⌘", "C"]. Empty/blank → []. */
export function comboGlyphs(combo: string): string[] {
  const parts = combo.split("+").map((p) => p.trim()).filter(Boolean);
  const mods: string[] = [];
  const keys: string[] = [];
  for (const p of parts) {
    const up = p.toUpperCase();
    if (MOD_GLYPH[up]) {
      if (!mods.includes(MOD_GLYPH[up])) mods.push(MOD_GLYPH[up]);
      continue;
    }
    const tok = keyToken(p) ?? p;
    keys.push(KEY_GLYPH[tok.toUpperCase()] ?? tok.toUpperCase());
  }
  mods.sort((a, b) => MOD_ORDER.indexOf(a) - MOD_ORDER.indexOf(b));
  return [...mods, ...keys];
}
