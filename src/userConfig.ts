/**
 * userConfig — loads the optional external configuration from `~/.cenno`
 * (delivered by the Rust `get_user_config` / `get_user_tokens` commands) and
 * applies the parts the webview owns:
 *
 *   - design tokens: `~/.cenno/tokens.json` (W3C DTCG) is flattened to
 *     `--cenno-*` CSS variables and injected as a stylesheet that overrides the
 *     built-in theme.
 *   - custom widgets: declarative templates that compose built-in controls;
 *     the desugar layer expands them by name (see a2ui/desugar.ts).
 *   - prompt defaults: e.g. the default flow theme.
 *
 * Everything is optional and best-effort: outside Tauri (tests, plain browser)
 * the invokes fail and we fall back to built-in defaults.
 */
import { invoke } from "@tauri-apps/api/core";

/** A custom widget template — the desugar `input` shape it expands to. */
export interface WidgetTemplate {
  childIds: string[];
  components: Array<{ id: string; component: string; [prop: string]: unknown }>;
  dataModel?: Record<string, unknown>;
}

/** Raw `tts` block from `~/.cenno/config.json` (snake_case from Rust). */
export interface RawTtsConfig {
  enabled?: boolean;
  min_urgency?: string;
  voice?: string;
  engine?: string;
  model_path?: string;
}

export interface UserConfig {
  panel?: unknown;
  defaults?: { timeout_s?: number; flow?: string };
  widgets?: Record<string, WidgetTemplate>;
  tts?: RawTtsConfig;
}

/** Resolved voice-out config the player consumes (camelCase, defaults applied). */
export interface ResolvedTtsConfig {
  enabled: boolean;
  minUrgency: "low" | "normal" | "high";
  /** Optional on-device voice id / Supertonic style; undefined → engine default. */
  voice?: string;
  /** "system" (AVSpeech, default) or "supertonic". */
  engine: "system" | "supertonic";
  /** Optional custom Supertonic model dir; undefined → default cache. */
  modelPath?: string;
}

let cachedWidgets: Record<string, WidgetTemplate> = {};
let cachedDefaults: UserConfig["defaults"] = {};
// Opt-in, default off; default threshold "high" so only High-urgency speaks.
let cachedTts: ResolvedTtsConfig = { enabled: false, minUrgency: "high", engine: "system" };

export function getWidgets(): Record<string, WidgetTemplate> {
  return cachedWidgets;
}
export function getDefaults(): UserConfig["defaults"] {
  return cachedDefaults;
}
export function getTts(): ResolvedTtsConfig {
  return cachedTts;
}

function resolveTts(raw: RawTtsConfig | undefined): ResolvedTtsConfig {
  const min = (raw?.min_urgency ?? "high").toLowerCase();
  const minUrgency = min === "low" || min === "normal" ? min : "high";
  const voice = raw?.voice && raw.voice.trim() ? raw.voice : undefined;
  const engine = raw?.engine === "supertonic" ? "supertonic" : "system";
  const modelPath = raw?.model_path && raw.model_path.trim() ? raw.model_path : undefined;
  return { enabled: raw?.enabled === true, minUrgency, voice, engine, modelPath };
}

/** Convert a kebab/camel segment list into a `--cenno-…` variable name. */
function tokenVarName(path: string[]): string {
  const kebab = path.map((seg) =>
    seg.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase(),
  );
  return `--cenno-${kebab.join("-")}`;
}

/** Walk a DTCG token tree, emitting `--cenno-…: value;` for every `$value` leaf. */
function flattenTokens(
  node: unknown,
  path: string[],
  out: string[],
): void {
  if (!node || typeof node !== "object") return;
  const obj = node as Record<string, unknown>;
  if ("$value" in obj) {
    const v = obj.$value;
    const value = Array.isArray(v)
      ? v.map((x) => (/\s/.test(String(x)) ? `'${x}'` : String(x))).join(", ")
      : String(v);
    out.push(`${tokenVarName(path)}: ${value};`);
    return;
  }
  for (const key of Object.keys(obj)) {
    if (key.startsWith("$")) continue;
    flattenTokens(obj[key], [...path, key], out);
  }
}

/** Build the override stylesheet text from a DTCG token document. */
export function tokensToCss(tokens: unknown): string {
  const decls: string[] = [];
  flattenTokens(tokens, [], decls);
  if (decls.length === 0) return "";
  return `:root {\n  ${decls.join("\n  ")}\n}`;
}

function injectTokenStyles(css: string): void {
  if (!css) return;
  const id = "cenno-user-tokens";
  let el = document.getElementById(id) as HTMLStyleElement | null;
  if (!el) {
    el = document.createElement("style");
    el.id = id;
    // Appended last so it overrides the built-in tokens.css/theme.css.
    document.head.appendChild(el);
  }
  el.textContent = css;
}

/**
 * Fetch and apply external config. Safe to call once at startup; never throws.
 */
export async function loadUserConfig(): Promise<void> {
  try {
    const config = await invoke<UserConfig>("get_user_config");
    cachedWidgets = config.widgets ?? {};
    cachedDefaults = config.defaults ?? {};
    cachedTts = resolveTts(config.tts);
  } catch {
    /* not in Tauri, or command missing → built-in defaults */
  }
  try {
    const tokens = await invoke<unknown>("get_user_tokens");
    if (tokens) injectTokenStyles(tokensToCss(tokens));
  } catch {
    /* no user tokens → built-in theme stands alone */
  }
}
