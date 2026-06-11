/**
 * desugar — pure mapping from the simple `ask_user` form (AskRequest /
 * Prompt) to the A2UI v0.9 message envelope the renderer consumes: the same
 * three-message shape the spike validated (spike/a2ui/src/messages.ts):
 * createSurface + updateComponents (flat component list) + updateDataModel.
 *
 * Deterministic component ids — tests and the Task 6 renderer reference them
 * by name: "root", "col", "title", "body", "input", "send", "dots",
 * "choices", "scale" (plus button-label children "sendLabel" / "yesLabel" /
 * "noLabel", and "actions" / "yes" / "no" for confirm).
 *
 * Action contract (Task 6 builds on this):
 * - Every user-completing interaction fires an event action whose name
 *   starts with "submit": `submit` (text/voice), `submit-choice`,
 *   `submit-scale`, `submit-yes`, `submit-no`.
 * - The action context always carries `{ value, via }`. `via` is encoded as
 *   a LITERAL in the context — "text" for typed answers, "choice" for
 *   tap-to-answer (chips, scale, confirm) — so the Task 6 action handler
 *   reads it straight from context without parsing action names.
 * - `value` is either a data-model binding ({path: ...}) or a literal
 *   (confirm buttons). Bound values resolve at fire time: /draft is a
 *   string, /choice is the ChoicePicker's string[] selection (single-select
 *   → one element; Task 6 unwraps), /scale is a number (Task 6 stringifies
 *   it before answering — scale answers are the stringified numeral).
 *
 * Deviations from the original sketch, forced by the real catalog
 * (src/a2ui/catalog.tsx, `cenno:catalog/v1`):
 * - No Card component exists in the cenno catalog (the panel root owns the
 *   surface), so "root" is a Column wrapping "col" rather than a Card.
 *
 * The `a2ui` passthrough field is NOT handled here — Task 7 short-circuits
 * before desugar is called.
 */
import type { Prompt } from "../PromptPanel";

// Plain JSON shapes matching @a2ui/web_core v0.9 server-to-client messages.
// Kept structural (not imported zod-inferred types) so this module stays a
// dependency-free pure function; the renderer validates on ingest.
export interface A2uiComponent {
  id: string;
  component: string;
  [prop: string]: unknown;
}

export type A2uiMessage =
  | { version: "v0.9"; createSurface: { surfaceId: string; catalogId: string } }
  | {
      version: "v0.9";
      updateComponents: { surfaceId: string; components: A2uiComponent[] };
    }
  | {
      version: "v0.9";
      updateDataModel: {
        surfaceId: string;
        path: string;
        value: Record<string, unknown>;
      };
    };

export type A2uiMessages = A2uiMessage[];

export const SURFACE_ID = "main";
/** Must match the Catalog id in src/a2ui/catalog.tsx. */
export const CATALOG_ID = "cenno:catalog/v1";

type Via = "text" | "choice" | "voice_text";

function submitAction(name: string, value: unknown, via: Via) {
  return { event: { name, context: { value, via } } };
}

function button(
  id: string,
  label: string,
  variant: "primary" | "borderless" | "quiet",
  action: unknown,
): A2uiComponent[] {
  return [
    { id, component: "Button", variant, child: `${id}Label`, action },
    { id: `${id}Label`, component: "Text", text: label },
  ];
}

/**
 * A custom widget template from `~/.cenno/config.json` — the same shape
 * `desugarInput` returns, so a configured widget name slots straight in. Its
 * components are validated against the catalog at render time like any payload.
 */
export interface WidgetTemplate {
  childIds: string[];
  components: A2uiComponent[];
  dataModel?: Record<string, unknown>;
}

/** Input-kind specific components + initial data model. */
function desugarInput(
  req: Prompt,
  widgets: Record<string, WidgetTemplate>,
): {
  childIds: string[];
  components: A2uiComponent[];
  dataModel: Record<string, unknown>;
} {
  const kind = req.input?.kind;
  // A configured custom widget takes precedence over the built-in kinds: the
  // agent invokes it by name (input.kind) and we expand its declarative
  // template (a composition of built-in controls — no code).
  if (kind && widgets[kind]) {
    const t = widgets[kind];
    return {
      childIds: t.childIds,
      components: t.components,
      dataModel: t.dataModel ?? {},
    };
  }
  switch (kind) {
    case "choice": {
      const options = (req.choices ?? []).map((c) => ({ label: c, value: c }));
      // Mood flow renders choices as bare oversized words in one row
      // (panel-mood-checkin.png), not outline pills. Other flows omit the
      // variant and keep the default chip rendering.
      const wordsVariant = req.flow === "mood" ? { variant: "words" } : {};
      return {
        childIds: ["choices"],
        components: [
          {
            id: "choices",
            component: "ChoicePicker",
            ...wordsVariant,
            options,
            value: { path: "/choice" },
            selectAction: submitAction(
              "submit-choice",
              { path: "/choice" },
              "choice",
            ),
          },
        ],
        dataModel: { choice: [] },
      };
    }
    case "scale":
      return {
        childIds: ["scale"],
        components: [
          {
            id: "scale",
            component: "Scale",
            min: 1,
            max: 7,
            minLabel: "not at all",
            maxLabel: "completely",
            value: { path: "/scale" },
            selectAction: submitAction(
              "submit-scale",
              { path: "/scale" },
              "choice",
            ),
          },
        ],
        dataModel: {},
      };
    case "confirm":
      // Buttons sit in a Row ("actions") so they hug their labels side by
      // side (panel-reminder.png) instead of stacking as full-width slabs.
      return {
        childIds: ["actions"],
        components: [
          { id: "actions", component: "Row", children: ["yes", "no"] },
          ...button(
            "yes",
            "Yes",
            "primary",
            submitAction("submit-yes", "yes", "choice"),
          ),
          ...button(
            "no",
            "No",
            "borderless",
            submitAction("submit-no", "no", "choice"),
          ),
        ],
        dataModel: {},
      };
    case "none":
      return { childIds: [], components: [], dataModel: {} };
    case "voice":
    case "voice_text": // serde rename: protocol's VoiceText is "voice_text"
    case "text":
    default: {
      // Unknown/missing kind: defensive default to text.
      const voice =
        req.input?.kind === "voice" || req.input?.kind === "voice_text";
      // voice_text answers report via "voice_text" whether the user dictated,
      // edited, or typed — the panel kind names the modality offered, so the
      // caller-visible shape stays constant (spec: "same shape as text").
      const submit = submitAction(
        "submit",
        { path: "/draft" },
        voice ? "voice_text" : "text",
      );
      return {
        childIds: ["input", "send"],
        components: [
          {
            id: "input",
            component: "TextField",
            label: "Your reply",
            value: { path: "/draft" },
            ...(voice ? { voice: true } : {}),
            submitAction: submit,
          },
          // Quiet text Send, bottom-right (panel-free-text.png) — not a
          // white primary pill. Confirm Yes/No stay pills (panel-reminder.png).
          ...button("send", "Send", "quiet", submit),
        ],
        dataModel: { draft: "" },
      };
    }
  }
}

/**
 * Pure: AskRequest/Prompt -> A2UI v0.9 message envelope. `widgets` are the
 * custom templates from `~/.cenno/config.json`; an `input.kind` matching one
 * expands the template instead of a built-in control.
 */
export function desugar(
  req: Prompt,
  widgets: Record<string, WidgetTemplate> = {},
): A2uiMessages {
  const input = desugarInput(req, widgets);
  const hasBody = req.body_md !== "";

  const childIds = [
    "title",
    ...(hasBody ? ["body"] : []),
    ...input.childIds,
    ...(req.progress ? ["dots"] : []),
  ];

  const components: A2uiComponent[] = [
    { id: "root", component: "Column", children: ["col"] },
    { id: "col", component: "Column", children: childIds },
    // h2 -> question-m (22px): TOKENS.md maps type.question.m to PANEL
    // questions; question-l (44px, h1) is reserved for fullscreen surfaces
    // which don't exist yet. Visual QA (Task 9) showed h1 titles eating
    // half the 420x240 panel and clipping the input/button below.
    { id: "title", component: "Text", variant: "h2", text: req.title },
    ...(hasBody
      ? [{ id: "body", component: "Text", text: req.body_md }]
      : []),
    ...input.components,
    ...(req.progress
      ? [
          {
            id: "dots",
            component: "Dots",
            step: req.progress.step,
            total: req.progress.total,
          },
        ]
      : []),
  ];

  return [
    {
      version: "v0.9",
      createSurface: { surfaceId: SURFACE_ID, catalogId: CATALOG_ID },
    },
    {
      version: "v0.9",
      updateComponents: { surfaceId: SURFACE_ID, components },
    },
    {
      version: "v0.9",
      updateDataModel: {
        surfaceId: SURFACE_ID,
        path: "/",
        value: input.dataModel,
      },
    },
  ];
}
