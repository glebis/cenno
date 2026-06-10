/**
 * Table tests for desugar(req) — one test per mapping rule.
 * Shape under test: the three-message v0.9 envelope from spike/a2ui/src/messages.ts
 * (createSurface + updateComponents + updateDataModel).
 */
import { describe, expect, it } from "vitest";
import {
  TextApi,
  RowApi,
  ColumnApi,
  ButtonApi,
} from "@a2ui/web_core/v0_9/basic_catalog";
import { desugar } from "./desugar";
import {
  CennoTextFieldApi,
  CennoChoicePickerApi,
  ScaleApi,
  DotsApi,
} from "./catalog";
import type { Prompt } from "../PromptPanel";

type AnyComponent = Record<string, any>;

function prompt(over: Partial<Prompt> = {}): Prompt {
  return {
    id: "p_1",
    title: "How focused are you?",
    body_md: "Be honest.",
    input: { kind: "text" },
    ...over,
  };
}

/** Unpack the envelope into the parts the table tests assert on. */
function parts(req: Prompt) {
  const messages = desugar(req) as any[];
  const create = messages[0]?.createSurface;
  const components: AnyComponent[] =
    messages[1]?.updateComponents?.components ?? [];
  const dataModel = messages[2]?.updateDataModel?.value ?? {};
  const byId = new Map(components.map((c) => [c.id, c]));
  const col = byId.get("col") as AnyComponent;
  return { messages, create, components, byId, col, dataModel };
}

describe("desugar envelope", () => {
  it("emits createSurface + updateComponents + updateDataModel for surface 'main'", () => {
    const { messages, create } = parts(prompt());
    expect(messages).toHaveLength(3);
    for (const m of messages) expect(m.version).toBe("v0.9");
    expect(create).toEqual({
      surfaceId: "main",
      catalogId: "cenno:catalog/v1",
    });
    expect(messages[1].updateComponents.surfaceId).toBe("main");
    expect(messages[2].updateDataModel).toMatchObject({
      surfaceId: "main",
      path: "/",
    });
  });

  it("is deterministic (same request, same output)", () => {
    const req = prompt({ input: { kind: "choice" }, choices: ["a", "b"] });
    expect(desugar(req)).toEqual(desugar(req));
  });
});

describe("desugar mapping table", () => {
  it("always: root Column -> col Column -> [title, body]", () => {
    const { byId, col } = parts(prompt());
    expect(byId.get("root")).toMatchObject({
      component: "Column",
      children: ["col"],
    });
    expect(col.component).toBe("Column");
    expect(col.children.slice(0, 2)).toEqual(["title", "body"]);
    // h2 -> question-m (22px): TOKENS.md maps type.question.m to PANEL
    // questions; h1/question-l is the fullscreen size (Task 9 visual QA).
    expect(byId.get("title")).toMatchObject({
      component: "Text",
      variant: "h2",
      text: "How focused are you?",
    });
    expect(byId.get("body")).toMatchObject({
      component: "Text",
      text: "Be honest.",
    });
  });

  it("empty body_md: body component omitted entirely", () => {
    const { byId, col } = parts(prompt({ body_md: "" }));
    expect(byId.has("body")).toBe(false);
    expect(col.children).not.toContain("body");
  });

  it("text: TextField (no voice) + primary Send button, both firing submit {value, via:'text'}", () => {
    const { byId, col, dataModel } = parts(prompt({ input: { kind: "text" } }));
    const submit = {
      event: {
        name: "submit",
        context: { value: { path: "/draft" }, via: "text" },
      },
    };
    expect(col.children).toEqual(["title", "body", "input", "send"]);
    expect(byId.get("input")).toMatchObject({
      component: "TextField",
      value: { path: "/draft" },
      submitAction: submit,
    });
    expect(byId.get("input").voice).toBeUndefined();
    expect(byId.get("send")).toMatchObject({
      component: "Button",
      variant: "primary",
      child: "sendLabel",
      action: submit,
    });
    expect(byId.get("sendLabel")).toMatchObject({
      component: "Text",
      text: "Send",
    });
    expect(dataModel.draft).toBe("");
  });

  it.each(["voice", "voice_text"])(
    "%s: TextField with voice: true + Send button",
    (kind) => {
      const { byId, col } = parts(prompt({ input: { kind } }));
      expect(byId.get("input")).toMatchObject({
        component: "TextField",
        voice: true,
      });
      expect(col.children).toContain("send");
    },
  );

  it("choice: ChoicePicker with {label,value} options and submit-choice tap-to-answer; no send button", () => {
    const { byId, col, dataModel } = parts(
      prompt({ input: { kind: "choice" }, choices: ["Calm", "Tense"] }),
    );
    expect(col.children).toEqual(["title", "body", "choices"]);
    expect(byId.get("choices")).toMatchObject({
      component: "ChoicePicker",
      options: [
        { label: "Calm", value: "Calm" },
        { label: "Tense", value: "Tense" },
      ],
      value: { path: "/choice" },
      selectAction: {
        event: {
          name: "submit-choice",
          context: { value: { path: "/choice" }, via: "choice" },
        },
      },
    });
    expect(byId.has("send")).toBe(false);
    expect(dataModel.choice).toEqual([]);
  });

  it("scale: Scale 1..7 with end labels and submit-scale tap-to-answer; no send button", () => {
    const { byId, col } = parts(prompt({ input: { kind: "scale" } }));
    expect(col.children).toEqual(["title", "body", "scale"]);
    expect(byId.get("scale")).toMatchObject({
      component: "Scale",
      min: 1,
      max: 7,
      minLabel: "not at all",
      maxLabel: "completely",
      value: { path: "/scale" },
      selectAction: {
        event: {
          name: "submit-scale",
          context: { value: { path: "/scale" }, via: "choice" },
        },
      },
    });
    expect(byId.has("send")).toBe(false);
  });

  it("confirm: actions Row of Yes (primary, submit-yes) and No (borderless, submit-no); no text input", () => {
    const { byId, col } = parts(prompt({ input: { kind: "confirm" } }));
    // Row so the buttons hug their labels side by side (panel-reminder.png)
    // instead of stacking as full-width slabs (Task 9 visual QA).
    expect(col.children).toEqual(["title", "body", "actions"]);
    expect(byId.get("actions")).toMatchObject({
      component: "Row",
      children: ["yes", "no"],
    });
    expect(byId.get("yes")).toMatchObject({
      component: "Button",
      variant: "primary",
      child: "yesLabel",
      action: {
        event: { name: "submit-yes", context: { value: "yes", via: "choice" } },
      },
    });
    expect(byId.get("no")).toMatchObject({
      component: "Button",
      variant: "borderless",
      child: "noLabel",
      action: {
        event: { name: "submit-no", context: { value: "no", via: "choice" } },
      },
    });
    expect(byId.get("yesLabel")).toMatchObject({ text: "Yes" });
    expect(byId.get("noLabel")).toMatchObject({ text: "No" });
    expect(byId.has("input")).toBe(false);
  });

  it("none: just title and body, no input components", () => {
    const { byId, col } = parts(prompt({ input: { kind: "none" } }));
    expect(col.children).toEqual(["title", "body"]);
    for (const id of ["input", "send", "choices", "scale", "actions", "yes", "no"]) {
      expect(byId.has(id)).toBe(false);
    }
  });

  it("progress: Dots appended last", () => {
    const { byId, col } = parts(
      prompt({ input: { kind: "scale" }, progress: { step: 2, total: 5 } }),
    );
    expect(col.children).toEqual(["title", "body", "scale", "dots"]);
    expect(byId.get("dots")).toMatchObject({
      component: "Dots",
      step: 2,
      total: 5,
    });
  });

  it("unknown input kind falls back to text", () => {
    const { byId, col } = parts(prompt({ input: { kind: "hologram" } }));
    expect(col.children).toEqual(["title", "body", "input", "send"]);
    expect(byId.get("input").component).toBe("TextField");
  });

  it("every desugared component validates against its cenno catalog schema", () => {
    // Extended cenno APIs shadow the stock TextField/ChoicePicker entries.
    const schemaByName = new Map(
      [
        TextApi,
        RowApi,
        ColumnApi,
        ButtonApi,
        CennoTextFieldApi,
        CennoChoicePickerApi,
        ScaleApi,
        DotsApi,
      ].map((api) => [api.name, api.schema]),
    );
    const kinds = ["text", "voice", "voice_text", "choice", "scale", "confirm", "none"];
    for (const kind of kinds) {
      const { components } = parts(
        prompt({
          input: { kind },
          choices: ["a", "b"],
          progress: { step: 1, total: 3 },
        }),
      );
      for (const { id, component, ...props } of components) {
        const schema = schemaByName.get(component);
        expect(schema, `no catalog schema for ${component}`).toBeDefined();
        const result = schema!.safeParse(props);
        expect(
          result.success,
          `${kind}/${id}: ${result.success ? "" : result.error}`,
        ).toBe(true);
      }
    }
  });

  it("contract: every action name starts with 'submit' and context carries {value, via}", () => {
    const kinds = ["text", "voice", "voice_text", "choice", "scale", "confirm"];
    for (const kind of kinds) {
      const { components } = parts(
        prompt({ input: { kind }, choices: ["a"] }),
      );
      const actions = components.flatMap((c) =>
        [c.action, c.submitAction, c.selectAction].filter(Boolean),
      );
      expect(actions.length).toBeGreaterThan(0);
      for (const a of actions) {
        expect(a.event.name).toMatch(/^submit/);
        expect(a.event.context).toHaveProperty("value");
        expect(["text", "choice"]).toContain(a.event.context.via);
      }
    }
  });
});
