/**
 * cenno A2UI catalog — thin adapters mapping A2UI component APIs onto the
 * plain React views in views.tsx. Catalog id: `cenno:catalog/v1`.
 *
 * Standard @a2ui/web_core APIs are reused (with cenno rendering) wherever one
 * fits: Text, Row, Column, Button, TextField, ChoicePicker, Slider,
 * DateTimeInput, Image. TextField, ChoicePicker, Slider, and DateTimeInput
 * carry small schema extensions (submitAction / selectAction / voice /
 * minLabel / maxLabel) so the desugar layer can wire "answer on Enter / on
 * tap / on release" without a separate submit button.
 * Custom types exist only where no standard API fits:
 *
 * - `Scale`  — EMA 1..N numeral row with end labels (SliderApi has no
 *   minLabel/maxLabel and renders a continuous range, not discrete targets)
 * - `Dots`   — step pagination (no standard progress/pagination API)
 * - `Table`  — display-only rows (no Table exists in any A2UI v0.9 catalog;
 *   input cells are out of scope because every answer flattens to a string)
 * - `RatingMatrix` — several items rated in one panel, matrix or sliders
 *   (ChoicePicker and Slider both answer a single question)
 * - `ImageChoice` — choose between up to 10 images (ChoicePicker options are
 *   strictly {label, value}, so there is no place to put a picture)
 *
 * Adapters are intentionally untested here; they are covered by the Task 6
 * renderer integration.
 */
import { Fragment, useRef } from "react";
import { z } from "zod";
import { useVoiceDictation } from "../voice";
import {
  Catalog,
  ActionSchema,
  DynamicBooleanSchema,
  DynamicNumberSchema,
  DynamicStringSchema,
  DataBindingSchema,
  FunctionCallSchema,
} from "@a2ui/web_core/v0_9";
import {
  BASIC_FUNCTIONS,
  TextApi,
  RowApi,
  ColumnApi,
  ButtonApi,
  TextFieldApi,
  ChoicePickerApi,
  SliderApi,
  DateTimeInputApi,
  ImageApi,
} from "@a2ui/web_core/v0_9/basic_catalog";
import { createComponentImplementation } from "@a2ui/react/v0_9";
import {
  TextView,
  ScaleView,
  ChoicePickerView,
  TextFieldView,
  ButtonView,
  DotsView,
  SliderView,
  DateTimeView,
  ImageView,
  TableView,
  MatrixView,
  ImageChoiceView,
  type TextRole,
} from "./views";

/* ------------------------------------------------------------------ */
/* Shared schema fragments (reuse the standard CommonProps fields)     */
/* ------------------------------------------------------------------ */

const CommonProps = {
  accessibility: TextApi.schema.shape.accessibility,
  weight: TextApi.schema.shape.weight,
};

/* ------------------------------------------------------------------ */
/* Text — standard TextApi, markdown rendered by our TextView          */
/* ------------------------------------------------------------------ */

function roleFromVariant(variant: string | undefined): TextRole {
  switch (variant) {
    case "h1":
      return "question-l";
    case "h2":
    case "h3":
    case "h4":
    case "h5":
      return "question-m";
    case "caption":
      return "caption";
    default:
      return "body";
  }
}

export const CennoText = createComponentImplementation(
  TextApi,
  ({ props }) => (
    <TextView
      markdown={typeof props.text === "string" ? props.text : ""}
      role={roleFromVariant(props.variant)}
    />
  ),
);

/* ------------------------------------------------------------------ */
/* Row / Column — standard APIs, trivial flex containers               */
/* ------------------------------------------------------------------ */

type ChildRef = string | { id: string; basePath?: string };

function renderChildren(
  children: unknown,
  buildChild: (id: string, basePath?: string) => React.ReactNode,
) {
  if (!Array.isArray(children)) return null;
  return (children as ChildRef[]).map((item, i) => {
    if (typeof item === "string") {
      return <Fragment key={`${item}-${i}`}>{buildChild(item)}</Fragment>;
    }
    if (item && typeof item === "object" && "id" in item) {
      return (
        <Fragment key={`${item.id}-${i}`}>
          {buildChild(item.id, item.basePath)}
        </Fragment>
      );
    }
    return null;
  });
}

export const CennoRow = createComponentImplementation(
  RowApi,
  ({ props, buildChild }) => (
    <div className="cenno-row">{renderChildren(props.children, buildChild)}</div>
  ),
);

export const CennoColumn = createComponentImplementation(
  ColumnApi,
  ({ props, buildChild }) => (
    <div className="cenno-column">
      {renderChildren(props.children, buildChild)}
    </div>
  ),
);

/* ------------------------------------------------------------------ */
/* Button — standard ButtonApi widened with the cenno "quiet" variant. */
/* borderless -> dim secondary; quiet -> text-only bottom-right Send    */
/* (panel-free-text.png); everything else -> primary pill.             */
/* ------------------------------------------------------------------ */

export const CennoButtonApi = {
  name: "Button",
  schema: ButtonApi.schema.extend({
    // Widen the stock primary/borderless/default hint with the cenno
    // "quiet" treatment: text-only Send aligned bottom-right.
    variant: z
      .enum(["default", "primary", "borderless", "quiet"])
      .optional()
      .describe(
        "Button style hint: 'primary' call-to-action pill, 'borderless' " +
          "link-like, or the cenno 'quiet' text-only Send (bottom-right).",
      ),
  }),
};

export const CennoButton = createComponentImplementation(
  CennoButtonApi,
  ({ props, buildChild }) => (
    <ButtonView
      variant={
        props.variant === "borderless"
          ? "secondary"
          : props.variant === "quiet"
            ? "quiet"
            : "primary"
      }
      disabled={props.isValid === false}
      onClick={props.action}
    >
      {props.child ? buildChild(props.child) : null}
    </ButtonView>
  ),
);

/* ------------------------------------------------------------------ */
/* TextField — standard TextFieldApi + {voice, submitAction}           */
/* ------------------------------------------------------------------ */

export const CennoTextFieldApi = {
  name: "TextField",
  schema: TextFieldApi.schema.extend({
    voice: DynamicBooleanSchema.optional().describe(
      "Show the push-to-talk dictation affordance (on-device Apple Speech).",
    ),
    submitAction: ActionSchema.optional().describe(
      "Action fired when the user submits the field with Enter.",
    ),
  }),
};

export const CennoTextField = createComponentImplementation(
  CennoTextFieldApi,
  ({ props }) => {
    const value = typeof props.value === "string" ? props.value : "";
    // Latest bound value for the dictation base, without re-arming the hook.
    const valueRef = useRef(value);
    valueRef.current = value;
    const dictation = useVoiceDictation({
      enabled: props.voice === true,
      getBase: () => valueRef.current,
      onText: (text) => props.setValue(text),
    });
    return (
      <TextFieldView
        value={value}
        label={typeof props.label === "string" ? props.label : undefined}
        voice={props.voice === true}
        recording={dictation.recording}
        voiceError={dictation.error}
        onMicToggle={dictation.toggle}
        onChange={(text) => props.setValue(text)}
        onSubmit={(text) => {
          // keep the bound value current, then notify the host
          props.setValue(text);
          props.submitAction?.();
        }}
      />
    );
  },
);

/* ------------------------------------------------------------------ */
/* ChoicePicker — standard ChoicePickerApi + {selectAction}; rendered  */
/* as outline pill chips                                               */
/* ------------------------------------------------------------------ */

export const CennoChoicePickerApi = {
  name: "ChoicePicker",
  schema: ChoicePickerApi.schema.extend({
    selectAction: ActionSchema.optional().describe(
      "Action fired after a choice is made (tap-to-answer flows).",
    ),
    // Widen the stock selection-behavior variant with a cenno display
    // variant: "words" renders options as bare oversized text (mood flow,
    // panel-mood-checkin.png) instead of outline pill chips.
    variant: z
      .enum(["multipleSelection", "mutuallyExclusive", "words"])
      .optional()
      .describe(
        "Selection behavior (multipleSelection / mutuallyExclusive) or the " +
          "cenno 'words' display variant for bare-word mood choices.",
      ),
  }),
};

export const CennoChoicePicker = createComponentImplementation(
  CennoChoicePickerApi,
  ({ props }) => {
    const values = Array.isArray(props.value) ? (props.value as string[]) : [];
    const options = (props.options ?? []).map((o) => ({
      label: typeof o.label === "string" ? o.label : String(o.label ?? ""),
      value: o.value,
    }));
    return (
      <ChoicePickerView
        choices={options}
        selected={values}
        // `undefined` here is an OMITTED displayStyle (the upstream
        // `.optional()` wraps `.default("checkbox")`, so the default never
        // materialises). ChoicePickerView keeps chips for undefined/"chips".
        displayStyle={props.displayStyle}
        variant={props.variant}
        // The schema's own `label` names the group in the list rendering
        // (a radiogroup with no accessible name is announced as "group").
        // Chips keep their existing markup and appearance, as shipped.
        label={typeof props.label === "string" ? props.label : undefined}
        onSelect={(value) => {
          if (props.variant === "multipleSelection") {
            props.setValue(
              values.includes(value)
                ? values.filter((v) => v !== value)
                : [...values, value],
            );
          } else {
            props.setValue([value]);
          }
          props.selectAction?.();
        }}
      />
    );
  },
);

/* ------------------------------------------------------------------ */
/* Slider — standard SliderApi + {minLabel, maxLabel, selectAction};   */
/* continuous range, answer-on-release when selectAction is present    */
/* ------------------------------------------------------------------ */

export const CennoSliderApi = {
  name: "Slider",
  schema: SliderApi.schema.extend({
    minLabel: DynamicStringSchema.optional().describe(
      "Caption under the low end (e.g. 'not at all').",
    ),
    maxLabel: DynamicStringSchema.optional().describe(
      "Caption under the high end (e.g. 'completely').",
    ),
    selectAction: ActionSchema.optional().describe(
      "Action fired when the user commits a value: releasing the thumb " +
        "(pointer) or pressing Enter (keyboard). Include it for " +
        "answer-on-commit flows; omit it and pair the slider with a " +
        "Button to require an explicit confirm.",
    ),
  }),
};

export const CennoSlider = createComponentImplementation(
  CennoSliderApi,
  ({ props }) => (
    <SliderView
      min={typeof props.min === "number" ? props.min : 0}
      max={typeof props.max === "number" ? props.max : 10}
      step={typeof props.step === "number" ? props.step : undefined}
      value={typeof props.value === "number" ? props.value : undefined}
      label={typeof props.label === "string" ? props.label : undefined}
      minLabel={typeof props.minLabel === "string" ? props.minLabel : undefined}
      maxLabel={typeof props.maxLabel === "string" ? props.maxLabel : undefined}
      onChange={(n) => props.setValue(n)}
      onCommit={(n) => {
        // keep the bound value current, then notify the host
        props.setValue(n);
        props.selectAction?.();
      }}
    />
  ),
);

/* ------------------------------------------------------------------ */
/* DateTimeInput — standard API + {submitAction}; native system picker */
/* ------------------------------------------------------------------ */

export const CennoDateTimeInputApi = {
  name: "DateTimeInput",
  schema: DateTimeInputApi.schema.extend({
    submitAction: ActionSchema.optional().describe(
      "Action fired when the user submits the field with Enter. Include " +
        "it for answer-on-Enter flows; omit it and pair with a Button to " +
        "require an explicit confirm.",
    ),
  }),
};

export const CennoDateTimeInput = createComponentImplementation(
  CennoDateTimeInputApi,
  ({ props }) => {
    const date = props.enableDate === true;
    const time = props.enableTime === true;
    return (
      <DateTimeView
        kind={date && time ? "datetime" : time ? "time" : "date"}
        value={typeof props.value === "string" ? props.value : ""}
        label={typeof props.label === "string" ? props.label : undefined}
        min={typeof props.min === "string" ? props.min : undefined}
        max={typeof props.max === "string" ? props.max : undefined}
        onChange={(v) => props.setValue(v)}
        onSubmit={(v) => {
          // keep the bound value current, then notify the host
          props.setValue(v);
          props.submitAction?.();
        }}
      />
    );
  },
);

/* ------------------------------------------------------------------ */
/* Image — standard ImageApi, display-only                             */
/* ------------------------------------------------------------------ */

export const CennoImage = createComponentImplementation(
  ImageApi,
  ({ props }) => (
    <ImageView
      url={typeof props.url === "string" ? props.url : ""}
      description={
        typeof props.description === "string" ? props.description : undefined
      }
      fit={props.fit}
      variant={props.variant}
    />
  ),
);

/* ------------------------------------------------------------------ */
/* Scale — custom: discrete numeral row with end labels                */
/* ------------------------------------------------------------------ */

export const ScaleApi = {
  name: "Scale",
  schema: z
    .object({
      ...CommonProps,
      min: DynamicNumberSchema.optional().describe(
        "Lowest target value (defaults to 1).",
      ),
      max: DynamicNumberSchema.describe("Highest target value."),
      minLabel: DynamicStringSchema.optional().describe(
        "Caption under the low end (e.g. 'not at all').",
      ),
      maxLabel: DynamicStringSchema.optional().describe(
        "Caption under the high end (e.g. 'completely').",
      ),
      value: DynamicNumberSchema.optional().describe(
        "The selected value; bind to the data model.",
      ),
      selectAction: ActionSchema.optional().describe(
        "Action fired after a target is tapped.",
      ),
    })
    .strict(),
};

export const CennoScale = createComponentImplementation(
  ScaleApi,
  ({ props }) => (
    <ScaleView
      min={typeof props.min === "number" ? props.min : 1}
      max={typeof props.max === "number" ? props.max : 7}
      value={typeof props.value === "number" ? props.value : undefined}
      minLabel={typeof props.minLabel === "string" ? props.minLabel : undefined}
      maxLabel={typeof props.maxLabel === "string" ? props.maxLabel : undefined}
      onSelect={(n) => {
        props.setValue(n);
        props.selectAction?.();
      }}
    />
  ),
);

/* ------------------------------------------------------------------ */
/* Table — custom: display-only rows with per-column alignment         */
/* ------------------------------------------------------------------ */

/**
 * No standard Table exists in any A2UI v0.9 basic catalog (verified against
 * @a2ui/web_core 0.10.7 and 0.14.0), so this is a cenno custom type alongside
 * Scale and Dots. Display-only: `rows` are literal cells, never bindings,
 * because an answer must flatten to one string anyway. Caps (4 columns,
 * 12 rows) live in the guard (src-tauri/src/a2ui_guard.rs), which returns an
 * actionable error before a payload reaches this schema.
 *
 * Body cells widen the DynamicString union with `z.number()`, because the
 * design doc allows a JSON number ("numbers may be sent as JSON numbers and
 * are stringified on render", e.g. `["Pro", 9]`). Without it the zod parse
 * would reject the table and the webview would silently drop it, while the
 * guard accepts it.
 */
const TableCellSchema = z.union([DynamicStringSchema, z.number()]);

export const TableApi = {
  name: "Table",
  schema: z
    .object({
      ...CommonProps,
      header: DynamicBooleanSchema.optional().describe(
        "Render the header row of column labels (default true). Set false " +
          "for a legend-less key/value block.",
      ),
      columns: z
        .array(
          z
            .object({
              label: DynamicStringSchema.describe("Column header text."),
              align: z
                .enum(["start", "end"])
                .optional()
                .describe(
                  "Cell alignment for the whole column: 'start' (default) " +
                    "or 'end' for numbers, which also uses tabular numerals.",
                ),
            })
            .strict(),
        )
        .min(1)
        .describe("Column definitions, at most 4 (a panel is ~320-420px wide)."),
      rows: z
        .array(z.array(TableCellSchema))
        .describe(
          "Rows of literal cell strings, at most 12. One entry per column; a " +
            "short row is padded with an em dash, a longer row is rejected. " +
            "Numbers are allowed and are stringified on render.",
        ),
      dense: DynamicBooleanSchema.optional().describe(
        "Tighten the vertical cell padding (default false).",
      ),
      caption: DynamicStringSchema.optional().describe(
        "Accessible name for the table, rendered above it in caption size.",
      ),
    })
    .strict(),
};

/**
 * Cells take literal strings or JSON numbers (the design doc allows
 * `["Pro", 9]` and stringifies numbers on render). Anything else — in
 * practice a data-model binding, which the guard rejects — degrades to an
 * empty string, so TableView renders its em dash instead of "[object
 * Object]".
 */
function literal(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number") return String(value);
  return "";
}

export const CennoTable = createComponentImplementation(
  TableApi,
  ({ props }) => (
    <TableView
      header={props.header !== false}
      columns={(props.columns ?? []).map((col) => ({
        label: literal(col.label),
        align: col.align,
      }))}
      rows={(props.rows ?? []).map((row) => row.map(literal))}
      dense={props.dense === true}
      caption={typeof props.caption === "string" ? props.caption : undefined}
    />
  ),
);

/* ------------------------------------------------------------------ */
/* RatingMatrix — custom: several items rated in one panel             */
/* ------------------------------------------------------------------ */

/**
 * No standard A2UI component rates several items at once (Slider and
 * ChoicePicker are both single-question), so this is a cenno custom type
 * alongside Scale, Table and Dots. One component with `mode` picking the
 * presentation: both modes are the same data (a set of keyed numbers), and two
 * components would duplicate the caps and the styling.
 *
 * `value` binds to the data model as an object keyed by row value (not an
 * array): each row writes `path + "/" + row.value`, so reordering rows cannot
 * corrupt an answer. The object is what makes the answer self-describing —
 * `{"pace":4,"usefulness":3}` — instead of a position-dependent list.
 *
 * It is written as JSON TEXT, not as a live object, because that is exactly
 * what the answer path forwards: PromptPanel delivers `String(raw)`
 * (src/PromptPanel.tsx:161), so a live object would reach the agent as
 * "[object Object]" (verified against a real payload through the real
 * catalog). The text form is what the design doc promises the agent, and it
 * parses with one JSON.parse on the agent side.
 *
 * Caps (3 rows, 7 points) live in the guard (src-tauri/src/a2ui_guard.rs),
 * which returns an actionable error pointing at `ask_sequence` before a
 * payload ever reaches this schema.
 *
 * `value` is spelled out as `Record<string, number> | string | DataBinding |
 * FunctionCall` rather than `DynamicValueSchema`: the binder derives the type
 * of `setValue` from the union minus its dynamic arms, and DynamicValueSchema's
 * remaining arms are scalars/arrays, so an object write would not typecheck.
 */
const RatingMatrixValueSchema = z.union([
  z.record(z.number()),
  z.string(),
  DataBindingSchema,
  FunctionCallSchema,
]);

/**
 * The bound value as a row->number map. Accepts the JSON text the adapter
 * writes, plus a real object (a payload may pre-seed one, and an echoed
 * `updateDataModel` keeps that shape). Anything else degrades to empty rather
 * than throwing inside the surface.
 */
function parseMatrixValue(bound: unknown): Record<string, number> {
  const value: Record<string, number> = {};
  const source =
    typeof bound === "string"
      ? (() => {
          try {
            return JSON.parse(bound) as unknown;
          } catch {
            return null;
          }
        })()
      : bound;
  if (!source || typeof source !== "object" || Array.isArray(source)) {
    return value;
  }
  for (const [key, n] of Object.entries(source)) {
    if (typeof n === "number") value[key] = n;
  }
  return value;
}

export const RatingMatrixApi = {
  name: "RatingMatrix",
  schema: z
    .object({
      ...CommonProps,
      mode: z
        .enum(["matrix", "sliders"])
        .describe(
          "'matrix' = one row of numeral targets per item (discrete points, " +
            "one tap answers the row); 'sliders' = one labelled slider per item.",
        ),
      rows: z
        .array(
          z
            .object({
              value: z
                .string()
                .describe(
                  "The key this row's answer is filed under; returned to the " +
                    "agent, never displayed.",
                ),
              label: DynamicStringSchema.optional().describe(
                "Display name for the row; falls back to the row value.",
              ),
            })
            .strict(),
        )
        .min(1)
        .describe("Items to rate, at most 3 (a panel fits 240-560px)."),
      min: DynamicNumberSchema.describe("Lowest point on the scale."),
      max: DynamicNumberSchema.describe(
        "Highest point on the scale; at most 7 points from min to max.",
      ),
      minLabel: DynamicStringSchema.optional().describe(
        "Caption under the low end (e.g. 'not at all').",
      ),
      maxLabel: DynamicStringSchema.optional().describe(
        "Caption under the high end (e.g. 'completely').",
      ),
      value: RatingMatrixValueSchema.optional().describe(
        "Object binding keyed by row value, e.g. {\"path\": \"/items\"}. " +
          "Each row writes its own key beneath it.",
      ),
      selectAction: ActionSchema.optional().describe(
        "Action fired after a row is answered. Its context should carry " +
          "{ value: <the same path as `value`>, via: \"choice\" }.",
      ),
    })
    .strict(),
};

export const CennoRatingMatrix = createComponentImplementation(
  RatingMatrixApi,
  ({ props }) => {
    const value = parseMatrixValue(props.value);
    return (
      <MatrixView
        rows={(props.rows ?? []).map((row) => ({
          value: row.value,
          label: typeof row.label === "string" ? row.label : row.value,
        }))}
        mode={props.mode}
        min={typeof props.min === "number" ? props.min : 1}
        max={typeof props.max === "number" ? props.max : 5}
        minLabel={typeof props.minLabel === "string" ? props.minLabel : undefined}
        maxLabel={typeof props.maxLabel === "string" ? props.maxLabel : undefined}
        value={value}
        onSelect={(key, n) => {
          // Spread, not a positional array: the object is keyed by row value, so
          // a reordered row list still maps onto the right answer.
          props.setValue(JSON.stringify({ ...value, [key]: n }));
          // Sliders must not ANSWER while the thumb is still moving — that is
          // SliderView's change/commit split (src/a2ui/views.tsx:473), so in
          // sliders mode the action waits for onCommit below. A matrix tap is
          // the whole gesture, so it answers immediately.
          if (props.mode !== "sliders") props.selectAction?.();
        }}
        onCommit={
          props.mode === "sliders"
            ? (key, n) => {
                props.setValue(JSON.stringify({ ...value, [key]: n }));
                props.selectAction?.();
              }
            : undefined
        }
      />
    );
  },
);

/* ------------------------------------------------------------------ */
/* ImageChoice — custom: choose from up to 10 images                   */
/* ------------------------------------------------------------------ */

/**
 * No standard A2UI component puts an image on an option: `ChoicePicker`
 * options are strictly `{label, value}` in every published release, and its
 * chips layout has nowhere to draw a picture. A new type (rather than an
 * extra option field) keeps a shipped `ChoicePicker` payload from silently
 * changing shape.
 *
 * `value` binds as a `string[]` exactly like `ChoicePicker` — the adapter
 * below copies its selection logic line for line — so `multipleSelection`
 * answers flatten to a comma-joined string through `String(raw)` in
 * `src/PromptPanel.tsx` unchanged, and `mutuallyExclusive` is unwrapped to its
 * one token. That flattening is WHY the guard rejects a comma inside an option
 * value in the multi-select case (see the `ImageChoice` arm of
 * src-tauri/src/a2ui_guard.rs); nothing here may write an object, which would
 * reach the agent as "[object Object]".
 *
 * Caps (10 options, 5 columns) live in the guard.
 */
export const ImageChoiceApi = {
  name: "ImageChoice",
  schema: z
    .object({
      ...CommonProps,
      options: z
        .array(
          z
            .object({
              value: z
                .string()
                .describe(
                  "The value returned to you when this image is chosen; " +
                    "never displayed. Must not contain a comma when variant " +
                    "is multipleSelection (the answer is comma-joined).",
                ),
              label: DynamicStringSchema.optional().describe(
                "Display name for the tile; falls back to the value. Rendered " +
                  "UNDER the image, never over it.",
              ),
              image: DynamicStringSchema.describe(
                "A data: URI (data:image/<png|jpeg|gif|webp>;base64,…) or a " +
                  "local file path. Remote URLs are rejected.",
              ),
            })
            .strict(),
        )
        .min(1)
        .max(10)
        .describe("Images to choose between, 1 to 10."),
      variant: z
        .enum(["mutuallyExclusive", "multipleSelection"])
        .optional()
        .describe(
          "Selection behavior (default mutuallyExclusive). The selection mark " +
            "is a circle for one answer, a square for several.",
        ),
      fit: z
        .enum(["cover", "contain"])
        .optional()
        .describe(
          "How a tile fills its square: 'cover' (default) crops to an even " +
            "grid; 'contain' letterboxes, for screenshots or UI diffs where " +
            "cropping destroys the comparison.",
        ),
      columns: z
        .number()
        .int()
        .min(1)
        .max(5)
        .optional()
        .describe(
          "Columns in the grid, 1 to 5. Omit it to use the adaptive table " +
            "(which keeps tiles at least ~58px and never orphans one tile).",
        ),
      value: z
        .union([z.array(z.string()), DataBindingSchema, FunctionCallSchema])
        .optional()
        .describe("The chosen value(s); bind to the data model."),
      selectAction: ActionSchema.optional().describe(
        "Action fired after an image is chosen (tap-to-answer flows). Its " +
          "context should carry { value: <the same path as `value`>, " +
          "via: \"choice\" }.",
      ),
    })
    .strict(),
};

export const CennoImageChoice = createComponentImplementation(
  ImageChoiceApi,
  ({ props }) => {
    const values = Array.isArray(props.value) ? (props.value as string[]) : [];
    const options = (props.options ?? []).map((o) => ({
      value: o.value,
      label: typeof o.label === "string" ? o.label : undefined,
      image: typeof o.image === "string" ? o.image : "",
    }));
    return (
      <ImageChoiceView
        options={options}
        selected={values}
        variant={props.variant}
        fit={props.fit}
        columns={props.columns}
        onSelect={(value) => {
          // Same selection logic as CennoChoicePicker, deliberately: one
          // flattening path, so the two components cannot disagree about what
          // a multi-select answer looks like.
          if (props.variant === "multipleSelection") {
            props.setValue(
              values.includes(value)
                ? values.filter((v) => v !== value)
                : [...values, value],
            );
          } else {
            props.setValue([value]);
          }
          props.selectAction?.();
        }}
      />
    );
  },
);

/* ------------------------------------------------------------------ */
/* Dots — custom: EMA step pagination                                  */
/* ------------------------------------------------------------------ */

export const DotsApi = {
  name: "Dots",
  schema: z
    .object({
      ...CommonProps,
      step: DynamicNumberSchema.describe("Current step, 1-based."),
      total: DynamicNumberSchema.describe("Total number of steps."),
    })
    .strict(),
};

export const CennoDots = createComponentImplementation(DotsApi, ({ props }) => (
  <DotsView
    step={typeof props.step === "number" ? props.step : 1}
    total={typeof props.total === "number" ? props.total : 1}
  />
));

/* ------------------------------------------------------------------ */
/* Catalog                                                             */
/* ------------------------------------------------------------------ */

export const cennoCatalog = new Catalog(
  "cenno:catalog/v1",
  [
    CennoText,
    CennoRow,
    CennoColumn,
    CennoButton,
    CennoTextField,
    CennoChoicePicker,
    CennoSlider,
    CennoDateTimeInput,
    CennoImage,
    CennoScale,
    CennoTable,
    CennoRatingMatrix,
    CennoImageChoice,
    CennoDots,
  ],
  BASIC_FUNCTIONS,
);
