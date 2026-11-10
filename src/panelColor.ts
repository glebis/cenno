/**
 * Per-agent panel colors.
 *
 * Resolution order for a panel's surface color:
 *   1. an explicit per-agent override from `~/.cenno/config.json`
 *      (`agent_colors`) — user config beats everything;
 *   2. the agent's own `flow` choice (mood/question/ema/…), unchanged;
 *   3. when the agent sent no `flow`, a color for that agent — a built-in hue
 *      near its mark for known agents, or a stable hash into a fixed palette
 *      for everyone else;
 *   4. no agent and no flow → the `question` blue.
 *
 * A configured default flow (`defaults.flow`) also counts as an explicit
 * user choice, so it keeps the flow color instead of enabling (3).
 *
 * Every default color here is checked against the token text color
 * (`color.text.default`, white) for WCAG AA normal-text contrast (≥ 4.5:1).
 * `panelColor.test.ts` guards that invariant.
 */

export type Flow = "mood" | "question" | "ema" | "reminder" | "ambient";

export const FLOWS: readonly Flow[] = [
  "mood",
  "question",
  "ema",
  "reminder",
  "ambient",
];

/** `color.text.default` — all panel text sits on the surface in this ink. */
export const PANEL_TEXT_COLOR = "#FFFFFF";
/** WCAG AA normal-text contrast floor. */
export const AA_CONTRAST = 4.5;

/**
 * Hues near each known agent's mark (src/agentLogos.tsx), darkened to clear
 * AA against white text. Names are the exact self-reported clientInfo values
 * the marks use; matching is case/space-insensitive like `agentGlyph`.
 */
export const KNOWN_AGENT_COLORS: Record<string, string> = {
  "claude-code": "#A8482B", // Claude — clay/orange
  "claude code": "#A8482B",
  codex: "#0B6B57", // OpenAI/Codex — deep green
  "codex-mcp-client": "#0B6B57",
  deepseek: "#1B4BA8", // DeepSeek — blue
  kimi: "#6A3FA0", // Kimi — violet
  qwen: "#7A2F6B", // Qwen — plum
  "nous research": "#3B5A2E", // Nous — forest
  nous: "#3B5A2E",
  pi: "#4A3B8C", // pi — indigo
};

/**
 * Fixed palette for unknown agents. All entries pass AA against white; the
 * hash makes the assignment stable across launches. Order is append-only —
 * reordering changes every existing unknown agent's color.
 */
export const UNKNOWN_AGENT_PALETTE: readonly string[] = [
  "#1E4FD8", // cobalt
  "#0E7C6B", // teal
  "#8A2E5C", // wine
  "#6B4E2E", // bronze
  "#3D4B8C", // slate blue
  "#7A3B1E", // rust
  "#2E5A2E", // moss
  "#5A2E6B", // aubergine
];

export interface PanelColor {
  /** `data-flow` value: layout/semantics plus the flow hue when surface is null. */
  flow: Flow;
  /** Inline `--cenno-surface` override; null → let data-flow pick the hue. */
  surface: string | null;
  /** True when the color came from the agent rather than a flow theme. */
  fromAgent: boolean;
}

export interface PanelColorInput {
  agent?: string;
  /**
   * Effective flow. Undefined means *no flow was given at all* — keep that
   * distinct from `"question"`, so the per-agent color can apply.
   */
  flow?: Flow;
  /** `agent_colors` from `~/.cenno/config.json`. */
  overrides?: Record<string, string>;
}

/** `#RGB` or `#RRGGBB`, case-insensitive. Mirrors config.rs validation. */
export function isHexColor(value: string): boolean {
  return /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i.test(value);
}

function agentKey(agent: string | undefined): string | null {
  const key = agent?.trim().toLowerCase();
  return key ? key : null;
}

function hashAgent(key: string): number {
  // FNV-1a: tiny, stable forever, well distributed for short names.
  let hash = 0x811c9dc5;
  for (let i = 0; i < key.length; i++) {
    hash ^= key.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

/** Built-in color for an agent (known hue or stable palette pick), if any. */
export function defaultAgentColor(agent: string | undefined): string | null {
  const key = agentKey(agent);
  if (!key) return null;
  const known = KNOWN_AGENT_COLORS[key];
  if (known) return known;
  return UNKNOWN_AGENT_PALETTE[hashAgent(key) % UNKNOWN_AGENT_PALETTE.length];
}

/** Explicit `agent_colors` entry for this agent, ignoring invalid values. */
export function agentColorOverride(
  agent: string | undefined,
  overrides: Record<string, string> | undefined,
): string | null {
  const key = agentKey(agent);
  if (!key || !overrides) return null;
  for (const [name, color] of Object.entries(overrides)) {
    if (name.trim().toLowerCase() === key && isHexColor(color)) return color;
  }
  return null;
}

/** Apply the documented resolution order to one prompt. */
export function resolvePanelColor(input: PanelColorInput): PanelColor {
  const override = agentColorOverride(input.agent, input.overrides);
  if (override) {
    return { flow: input.flow ?? "question", surface: override, fromAgent: true };
  }

  // Agent-chosen flow (or an explicit configured default) keeps its flow hue.
  if (input.flow) {
    return { flow: input.flow, surface: null, fromAgent: false };
  }

  const agentColor = defaultAgentColor(input.agent);
  if (agentColor) {
    return { flow: "question", surface: agentColor, fromAgent: true };
  }

  return { flow: "question", surface: null, fromAgent: false };
}

function channel(value: number): number {
  const c = value / 255;
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

function parseHex(hex: string): [number, number, number] {
  let h = hex.replace("#", "");
  if (h.length === 3) h = h.split("").map((ch) => ch + ch).join("");
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}

/** WCAG relative luminance of an `#RGB`/`#RRGGBB` color. */
export function relativeLuminance(hex: string): number {
  const [r, g, b] = parseHex(hex);
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

/** WCAG contrast ratio between two valid hex colors (1–21). */
export function contrastRatio(a: string, b: string = PANEL_TEXT_COLOR): number {
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  const [hi, lo] = la >= lb ? [la, lb] : [lb, la];
  return (hi + 0.05) / (lo + 0.05);
}
