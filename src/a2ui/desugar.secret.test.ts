/**
 * Secret panels are the one place where the "free-form invariant" (every
 * prompt gets a voice-capable TextField + Send) MUST NOT hold: an ordinary
 * TextField would mail the credential back to the agent as a plain answer, and
 * a mic would have a third-party speech service transcribe it.
 *
 * These tests pin that exception so a future refactor of `withFreeform` cannot
 * quietly re-add the escape hatch.
 */
import { describe, expect, it } from "vitest";
import { desugar } from "./desugar";
import type { Prompt } from "../PromptPanel";

type AnyComponent = Record<string, any>;

function prompt(over: Partial<Prompt> = {}): Prompt {
  return {
    id: "p_1",
    title: "OpenAI API key",
    body_md: "Paste the key. It is stored encrypted and never leaves this Mac.",
    input: { kind: "secret", secret_name: "openai", secret_format: "openai_api_key" },
    ...over,
  };
}

function parts(req: Prompt) {
  const messages = desugar(req) as any[];
  const components: AnyComponent[] =
    messages[1]?.updateComponents?.components ?? [];
  return { components, byId: new Map(components.map((c) => [c.id, c])) };
}

describe("secret desugar", () => {
  it("renders one obscured, mic-less TextField and a Save button", () => {
    const { byId } = parts(prompt());
    expect(byId.get("input")).toMatchObject({
      component: "TextField",
      obscured: true,
      voice: false,
      value: { path: "/draft" },
    });
    expect(byId.get("send")).toMatchObject({ component: "Button" });
  });

  it("carries the declared format so the panel can validate live", () => {
    const { byId } = parts(prompt());
    expect(byId.get("input")?.secretFormat).toBe("openai_api_key");
  });

  it("uses the agent's label, falling back to a neutral prompt", () => {
    expect(parts(prompt()).byId.get("input")?.label).toBe("Paste it here");
    const labelled = parts(
      prompt({
        input: {
          kind: "secret",
          secret_name: "openai",
          secret_label: "Paste your OpenAI key",
        },
      }),
    );
    expect(labelled.byId.get("input")?.label).toBe("Paste your OpenAI key");
  });

  it("submits with via 'secret' so the value never travels as an answer", () => {
    const { byId } = parts(prompt());
    // Both affordances (Enter on the field, click on Save) must use the secret
    // via; a stray "text" here would hand the value to the agent.
    expect(JSON.stringify(byId.get("input")?.submitAction)).toContain("secret");
    expect(JSON.stringify(byId.get("send")?.action)).toContain("secret");
  });

  it("adds no second free-form field (no bypass around the obscured one)", () => {
    const { components } = parts(prompt());
    const fields = components.filter((c) => c.component === "TextField");
    expect(fields).toHaveLength(1);
    for (const f of fields) expect(f.obscured).toBe(true);
  });

  it("ignores a custom widget registered under 'secret'", () => {
    // A user template named "secret" must not be able to replace the credential
    // field with something that renders plaintext.
    const messages = desugar(prompt(), {
      secret: {
        childIds: ["input"],
        components: [
          { id: "input", component: "TextField", label: "Key", value: { path: "/draft" } },
        ],
      },
    }) as any[];
    const components: AnyComponent[] =
      messages[1]?.updateComponents?.components ?? [];
    const field = components.find((c) => c.component === "TextField");
    expect(field?.obscured).toBe(true);
    expect(field?.voice).toBe(false);
  });
});
