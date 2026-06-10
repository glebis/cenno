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
 *
 * Adapters are intentionally untested here; they are covered by the Task 6
 * renderer integration.
 */
import { Fragment } from "react";
import { z } from "zod";
import {
  Catalog,
  ActionSchema,
  DynamicBooleanSchema,
  DynamicNumberSchema,
  DynamicStringSchema,
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
  ChipsView,
  TextFieldView,
  ButtonView,
  DotsView,
  SliderView,
  DateTimeView,
  ImageView,
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
      "Show the (stubbed until plan 3) voice input affordance.",
    ),
    submitAction: ActionSchema.optional().describe(
      "Action fired when the user submits the field with Enter.",
    ),
  }),
};

export const CennoTextField = createComponentImplementation(
  CennoTextFieldApi,
  ({ props }) => (
    <TextFieldView
      value={typeof props.value === "string" ? props.value : ""}
      label={typeof props.label === "string" ? props.label : undefined}
      voice={props.voice === true}
      onChange={(text) => props.setValue(text)}
      onSubmit={(text) => {
        // keep the bound value current, then notify the host
        props.setValue(text);
        props.submitAction?.();
      }}
    />
  ),
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
      <ChipsView
        choices={options}
        selected={values}
        variant={props.variant === "words" ? "words" : undefined}
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
    CennoDots,
  ],
  BASIC_FUNCTIONS,
);
