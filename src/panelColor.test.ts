import { describe, expect, it } from "vitest";
import tokens from "../tokens/tokens.json";
import {
  AA_CONTRAST,
  KNOWN_AGENT_COLORS,
  PANEL_TEXT_COLOR,
  UNKNOWN_AGENT_PALETTE,
  agentColorOverride,
  contrastRatio,
  defaultAgentColor,
  deltaE,
  isHexColor,
  relativeLuminance,
  resolvePanelColor,
} from "./panelColor";

describe("default agent colors are legible white-on-color", () => {
  // The panel's text token is white; every fallback color must clear WCAG AA
  // normal text (4.5:1) so the prompt stays readable.
  it("checks against the actual color.text.default token", () => {
    expect(PANEL_TEXT_COLOR.toLowerCase()).toBe(
      tokens.color.text.default.$value.toLowerCase(),
    );
  });

  it("every known-agent hue passes AA against color.text.default", () => {
    for (const [agent, color] of Object.entries(KNOWN_AGENT_COLORS)) {
      expect(contrastRatio(color, PANEL_TEXT_COLOR), `${agent} ${color}`).toBeGreaterThanOrEqual(
        AA_CONTRAST,
      );
    }
  });

  it("every unknown-agent palette entry passes AA against color.text.default", () => {
    for (const color of UNKNOWN_AGENT_PALETTE) {
      expect(contrastRatio(color, PANEL_TEXT_COLOR), color).toBeGreaterThanOrEqual(
        AA_CONTRAST,
      );
    }
  });

  // Distinct from flows: an agent color that looks like a flow color makes a
  // no-flow prompt indistinguishable from a flow-themed one.
  const flowColors = Object.entries(tokens.color.flow)
    .filter(([, v]) => typeof v === "object" && v !== null && "$value" in v)
    .map(([k, v]) => [k, (v as { $value: string }).$value] as const);
  const MIN_DELTA_E = 20;

  it("reads all five flow colors from tokens", () => {
    expect(flowColors.map(([k]) => k).sort()).toEqual(
      ["ambient", "ema", "mood", "question", "reminder"],
    );
  });

  it("no agent color is a look-alike of a flow color", () => {
    const agentColors = [
      ...Object.entries(KNOWN_AGENT_COLORS),
      ...UNKNOWN_AGENT_PALETTE.map((c) => ["palette", c] as const),
    ];
    for (const [who, color] of agentColors) {
      for (const [flow, flowColor] of flowColors) {
        expect(deltaE(color, flowColor), `${who} ${color} vs ${flow}`).toBeGreaterThanOrEqual(
          MIN_DELTA_E,
        );
      }
    }
  });

  it("unknown-agent colors don't mimic known agents or each other", () => {
    const known = [...new Set(Object.values(KNOWN_AGENT_COLORS))];
    UNKNOWN_AGENT_PALETTE.forEach((c, i) => {
      for (const k of known) {
        expect(deltaE(c, k), `${c} vs known ${k}`).toBeGreaterThanOrEqual(MIN_DELTA_E);
      }
      for (const other of UNKNOWN_AGENT_PALETTE.slice(i + 1)) {
        expect(deltaE(c, other), `${c} vs ${other}`).toBeGreaterThanOrEqual(MIN_DELTA_E);
      }
    });
  });

  it("computes ΔE sanely", () => {
    expect(deltaE("#123456", "#123456")).toBe(0);
    expect(deltaE("#000000", "#FFFFFF")).toBeCloseTo(100, 0);
  });

  it("computes WCAG extremes correctly", () => {
    expect(contrastRatio("#000000", "#FFFFFF")).toBeCloseTo(21, 5);
    expect(contrastRatio("#FFFFFF", "#FFFFFF")).toBeCloseTo(1, 5);
    expect(relativeLuminance("#000000")).toBe(0);
    expect(relativeLuminance("#FFFFFF")).toBeCloseTo(1, 5);
  });
});

describe("agent color lookup", () => {
  it("gives known agents a built-in hue and no mark-less agent", () => {
    expect(defaultAgentColor("claude-code")).toBe(KNOWN_AGENT_COLORS["claude-code"]);
    expect(defaultAgentColor("codex")).toBe(KNOWN_AGENT_COLORS["codex"]);
    expect(defaultAgentColor(undefined)).toBeNull();
    expect(defaultAgentColor("   ")).toBeNull();
  });

  it("is stable and case/space-insensitive for unknown agents", () => {
    const first = defaultAgentColor("my-custom-agent");
    expect(first).not.toBeNull();
    expect(defaultAgentColor("  My-Custom-Agent ")).toBe(first);
  });

  it("spreads different unknown names across the palette", () => {
    const a = defaultAgentColor("agent-alpha");
    const b = defaultAgentColor("agent-beta");
    const c = defaultAgentColor("agent-gamma");
    expect(new Set([a, b, c]).size).toBeGreaterThan(1);
  });

  it("treats known and unknown agents as different colors", () => {
    expect(defaultAgentColor("claude-code")).not.toBe(defaultAgentColor("codex"));
  });
});

describe("resolution order", () => {
  it("an explicit override beats the agent's own flow", () => {
    expect(
      resolvePanelColor({
        agent: "claude-code",
        flow: "mood",
        overrides: { "claude-code": "#123456" },
      }),
    ).toEqual({ flow: "mood", surface: "#123456", fromAgent: true });
  });

  it("override matching is case-insensitive; invalid values are ignored", () => {
    expect(agentColorOverride("Claude-Code", { "claude-code": "#123456" })).toBe(
      "#123456",
    );
    expect(agentColorOverride("claude-code", { "claude-code": "red" })).toBeNull();
    expect(
      resolvePanelColor({
        agent: "claude-code",
        flow: "mood",
        overrides: { "claude-code": "red" },
      }),
    ).toEqual({ flow: "mood", surface: null, fromAgent: false });
  });

  it("an agent-chosen flow keeps its flow color", () => {
    expect(resolvePanelColor({ agent: "claude-code", flow: "ema" })).toEqual({
      flow: "ema",
      surface: null,
      fromAgent: false,
    });
  });

  it("a configured default flow (passed as flow) keeps its flow color", () => {
    expect(resolvePanelColor({ agent: "claude-code", flow: "question" })).toEqual({
      flow: "question",
      surface: null,
      fromAgent: false,
    });
  });

  it("distinguishes an explicit flow=question from no flow at all", () => {
    expect(
      resolvePanelColor({ agent: "claude-code", flow: "question" }).surface,
    ).toBeNull();
    expect(resolvePanelColor({ agent: "claude-code" }).surface).toBe(
      KNOWN_AGENT_COLORS["claude-code"],
    );
  });

  it("without a flow, a known or unknown agent gets its own color", () => {
    expect(resolvePanelColor({ agent: "claude-code" })).toEqual({
      flow: "question",
      surface: KNOWN_AGENT_COLORS["claude-code"],
      fromAgent: true,
    });
    const unknown = resolvePanelColor({ agent: "some-new-agent" });
    expect(unknown.fromAgent).toBe(true);
    expect(UNKNOWN_AGENT_PALETTE).toContain(unknown.surface);
  });

  it("without an agent or a flow, falls back to question blue", () => {
    expect(resolvePanelColor({})).toEqual({
      flow: "question",
      surface: null,
      fromAgent: false,
    });
  });
});

describe("isHexColor", () => {
  it("accepts short and long hex only", () => {
    for (const good of ["#fff", "#FFF", "#a8482b", "#A8482B"]) {
      expect(isHexColor(good)).toBe(true);
    }
    for (const bad of ["fff", "#ff", "#ffff", "#gggggg", "red", "#12345678"]) {
      expect(isHexColor(bad)).toBe(false);
    }
  });
});
