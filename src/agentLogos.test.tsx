import { describe, expect, it } from "vitest";
import { agentGlyph } from "./agentLogos";

describe("agentGlyph", () => {
  it("matches confirmed client names, case- and space-insensitively", () => {
    expect(agentGlyph("claude-code")).not.toBeNull();
    expect(agentGlyph("  Claude Code ")).not.toBeNull();
    expect(agentGlyph("codex-mcp-client")).not.toBeNull();
  });

  it("gives Claude Code and Codex different marks", () => {
    expect(agentGlyph("claude-code")).not.toBe(agentGlyph("codex-mcp-client"));
  });

  it("does not lend a mark to look-alike or unknown names", () => {
    expect(agentGlyph("claude-code-but-not-really")).toBeNull();
    expect(agentGlyph("my-claude")).toBeNull();
    expect(agentGlyph("deploy-script")).toBeNull();
    expect(agentGlyph(undefined)).toBeNull();
    expect(agentGlyph("")).toBeNull();
  });
});
