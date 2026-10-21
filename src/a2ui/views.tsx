/**
 * cenno A2UI views — plain React components, no A2UI imports.
 *
 * These are the rendering half of the catalog: catalog.tsx wraps each view in
 * a thin A2UI adapter (createComponentImplementation). Views are tested
 * directly (views.test.tsx); the adapters are covered by the Task 6
 * integration.
 *
 * Styling contract: catalog.css, consuming ONLY semantic theme vars
 * (--cenno-text, --cenno-text-dim, --cenno-line, --cenno-surface) and token
 * vars (--cenno-type-*, --cenno-space-*, --cenno-radius-*). Background stays
 * transparent — the panel root owns --cenno-surface.
 */
import { useEffect, useRef, useState } from "react";
import type {
  AnchorHTMLAttributes,
  KeyboardEvent as ReactKeyboardEvent,
  MouseEvent,
  ReactNode,
} from "react";
import ReactMarkdown from "react-markdown";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ImageLoadError, resolveImageUrl } from "./image";
import "./catalog.css";

export type TextRole = "question-l" | "question-m" | "body" | "caption";

/**
 * Markdown links must NEVER navigate the panel webview: a plain <a href>
 * click replaces the whole app with the linked page (CSP does not cover
 * top-level navigation — observed in Task 9 visual QA). Open externally
 * via the opener plugin instead.
 */
function ExternalLink(props: AnchorHTMLAttributes<HTMLAnchorElement>) {
  const { href, children, ...rest } = props;
  const onClick = (e: MouseEvent<HTMLAnchorElement>) => {
    e.preventDefault();
    if (href) {
      openUrl(href).catch((err) =>
        console.error("cenno: failed to open link externally:", err),
      );
    }
  };
  return (
    <a {...rest} href={href} onClick={onClick}>
      {children}
    </a>
  );
}

/** Markdown text. Role maps onto the type scale (TOKENS.md). */
export function TextView({
  markdown,
  role = "body",
}: {
  markdown: string;
  role?: TextRole;
}) {
  return (
    <div className={`cenno-text cenno-text--${role}`}>
      <ReactMarkdown components={{ a: ExternalLink }}>{markdown}</ReactMarkdown>
    </div>
  );
}

/**
 * EMA scale: a row of large bare numerals (no boxes), end labels in caption
 * size. Selected numeral = full opacity + underline.
 */
export function ScaleView({
  min,
  max,
  value,
  minLabel,
  maxLabel,
  onSelect,
}: {
  min: number;
  max: number;
  value?: number;
  minLabel?: string;
  maxLabel?: string;
  onSelect: (n: number) => void;
}) {
  const steps: number[] = [];
  for (let n = min; n <= max; n++) steps.push(n);
  const groupLabel =
    minLabel && maxLabel
      ? `${min} (${minLabel}) to ${max} (${maxLabel})`
      : `${min} to ${max}`;
  return (
    <div className="cenno-scale">
      <div className="cenno-scale__row" role="group" aria-label={groupLabel}>
        {steps.map((n) => (
          <button
            key={n}
            type="button"
            className={`cenno-scale__num${n === value ? " is-selected" : ""}`}
            aria-pressed={n === value}
            onClick={() => onSelect(n)}
          >
            {n}
          </button>
        ))}
      </div>
      {(minLabel || maxLabel) && (
        <div className="cenno-scale__labels" aria-hidden="true">
          <span className="cenno-scale__label">{minLabel}</span>
          <span className="cenno-scale__label">{maxLabel}</span>
        </div>
      )}
    </div>
  );
}

/**
 * Choice chips. Identity is the choice VALUE (labels may collide); a tap
 * reports the value. `selected` carries every selected value — single-select
 * callers pass a 1-element array.
 *
 * Default `variant` = outline pills. `variant: "words"` renders bare oversized
 * words in one centered row, no border/background (panel-mood-checkin.png);
 * pressed = underlined. Both keep a >= 44px tap target via vertical padding.
 */
export function ChipsView({
  choices,
  selected = [],
  variant,
  onSelect,
}: {
  choices: { label: string; value: string }[];
  selected?: string[];
  variant?: "words";
  onSelect: (value: string) => void;
}) {
  const words = variant === "words";
  return (
    <div className={words ? "cenno-chips cenno-chips--words" : "cenno-chips"}>
      {choices.map(({ label, value }) => (
        <button
          key={value}
          type="button"
          className={words ? "cenno-word" : "cenno-chip"}
          aria-pressed={selected.includes(value)}
          onClick={() => onSelect(value)}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

/**
 * Choice list — the `displayStyle: "checkbox"` rendering: one row per choice,
 * each row a 44px tap target with a drawn mark (circle for single-select,
 * square for multi-select) and its label.
 *
 * Same identity rule as ChipsView: the VALUE is what gets reported (labels
 * may collide); `selected` carries every selected value and single-select
 * callers pass a 1-element array.
 *
 * Keyboard is explicit, not inherited. The rows are `<button>` elements with a
 * `role` override, so the browser hands them NONE of the native radio/checkbox
 * behaviour — everything below is the ARIA pattern implemented by hand:
 *
 * - `ArrowUp`/`ArrowDown` move focus, wrapping at the ends. In single-select
 *   they also select the neighbour, which is what makes the group a radiogroup
 *   rather than a list of unrelated buttons.
 * - `Space`/`Enter` select the focused row. A `role`-overridden `<button>` keeps
 *   its native click-on-Enter/Space keydown synthesis, so both keys are handled
 *   here with `preventDefault()` — that cancels the synthesised click and keeps
 *   exactly one report per press (jsdom does not synthesise it at all, so the
 *   double-fire would only ever show up in the real webview).
 * - Single-select keeps ONE tab stop (the selected row, else the first) so Tab
 *   skips past the group instead of walking every row; multi-select keeps every
 *   row tabbable, as independent checkboxes should.
 *
 * `label` (optional) names the group: a `radiogroup`/`group` with no accessible
 * name is announced as "group" by VoiceOver, which tells the user nothing.
 */
export function CheckListView({
  choices,
  selected = [],
  variant = "single",
  label,
  onSelect,
}: {
  choices: { label: string; value: string }[];
  selected?: string[];
  variant?: "single" | "multiple";
  /** Accessible name for the group (optional but recommended). */
  label?: string;
  onSelect: (value: string) => void;
}) {
  const multiple = variant === "multiple";
  const rowRefs = useRef<(HTMLButtonElement | null)[]>([]);

  // Tab stop for single-select: the selected row, falling back to the first
  // so the group is always reachable with a single Tab.
  const selectedIndex = choices.findIndex((c) => selected.includes(c.value));
  const tabStop = selectedIndex === -1 ? 0 : selectedIndex;

  const focusRow = (index: number) => {
    const row = rowRefs.current[index];
    if (!row) return;
    row.focus();
    // Single-select selects on focus move; multi-select only moves focus.
    if (!multiple) onSelect(choices[index].value);
  };

  const onKeyDown = (index: number) => (e: ReactKeyboardEvent) => {
    const last = choices.length - 1;
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        focusRow(index === last ? 0 : index + 1);
        return;
      case "ArrowUp":
        e.preventDefault();
        focusRow(index === 0 ? last : index - 1);
        return;
      case " ":
      case "Enter":
        // Do not double-fire: the native click path would report again.
        e.preventDefault();
        onSelect(choices[index].value);
        return;
      default:
    }
  };

  return (
    <div
      className="cenno-checklist"
      role={multiple ? "group" : "radiogroup"}
      aria-label={label}
    >
      {choices.map(({ label: choiceLabel, value }, i) => {
        const checked = selected.includes(value);
        return (
          <button
            key={value}
            ref={(el) => {
              rowRefs.current[i] = el;
            }}
            type="button"
            className="cenno-checklist__row"
            role={multiple ? "checkbox" : "radio"}
            aria-checked={checked}
            // Single-select: one tab stop. Multi-select: every row tabbable.
            tabIndex={multiple || i === tabStop ? 0 : -1}
            onClick={() => onSelect(value)}
            onKeyDown={onKeyDown(i)}
          >
            <span
              className={
                multiple
                  ? "cenno-checklist__mark cenno-checklist__mark--box"
                  : "cenno-checklist__mark"
              }
              aria-hidden="true"
            >
              {checked && (
                <svg viewBox="0 0 18 18" focusable="false" aria-hidden="true">
                  {multiple ? (
                    <path
                      d="M4 9.5 L7.5 13 L14 5.5"
                      fill="none"
                      stroke="var(--cenno-surface)"
                      strokeWidth="2"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  ) : (
                    <circle cx="9" cy="9" r="4" fill="var(--cenno-surface)" />
                  )}
                </svg>
              )}
            </span>
            <span className="cenno-checklist__label">{choiceLabel}</span>
          </button>
        );
      })}
    </div>
  );
}

/**
 * The ChoicePicker render decision, kept out of the A2UI adapter so it can be
 * tested without a surface.
 *
 * `displayStyle` reaches here exactly as the upstream zod schema parsed it:
 * `.enum([...]).default("checkbox").optional()` — the `.optional()` wraps the
 * `.default()`, so an OMITTED `displayStyle` parses to `undefined`, never to
 * the schema's `"checkbox"` default. cenno therefore treats `undefined` and
 * `"chips"` as chips: every payload shipped before this view existed keeps its
 * exact pill appearance. Only an explicit `"checkbox"` opts into the list.
 *
 * The cenno `"words"` variant always stays on chips — a bare-word mood row
 * must never turn into a list of rows.
 */
export function ChoicePickerView({
  choices,
  selected = [],
  displayStyle,
  variant,
  label,
  onSelect,
}: {
  choices: { label: string; value: string }[];
  selected?: string[];
  displayStyle?: "checkbox" | "chips";
  /** Selection behaviour plus the cenno "words" display variant. */
  variant?: "multipleSelection" | "mutuallyExclusive" | "words";
  /** Accessible name for the list group (list rendering only). */
  label?: string;
  onSelect: (value: string) => void;
}) {
  const words = variant === "words";
  if (displayStyle === "checkbox" && !words) {
    return (
      <CheckListView
        choices={choices}
        selected={selected}
        variant={variant === "multipleSelection" ? "multiple" : "single"}
        label={label}
        onSelect={onSelect}
      />
    );
  }
  return (
    <ChipsView
      choices={choices}
      selected={selected}
      variant={words ? "words" : undefined}
      onSelect={onSelect}
    />
  );
}

/**
 * Free-text input: bottom-border underline only (Reporter's "Alone ____"
 * pattern). Enter submits (IME composition guarded). When `voice` is set, a
 * mic circle toggles push-to-talk dictation: idle = circle with a mic glyph,
 * recording = pulsing dot (no waveforms — BRAND.md). A dictation failure
 * surfaces as a caption-size error line under the field.
 */
export function TextFieldView({
  value = "",
  label,
  placeholder,
  voice = false,
  recording = false,
  voiceError = null,
  onMicToggle,
  onChange,
  onSubmit,
}: {
  value?: string;
  /** Accessible name for the input (placeholder alone is a fragile accname). */
  label?: string;
  placeholder?: string;
  voice?: boolean;
  /** Live dictation in progress (drives the pulsing-dot state). */
  recording?: boolean;
  /** Graceful error state (permission denied, recognizer unavailable, …). */
  voiceError?: string | null;
  /** Mic tap handler; without one the mic renders disabled. */
  onMicToggle?: () => void;
  onChange?: (text: string) => void;
  onSubmit: (text: string) => void;
}) {
  // Internal draft seeded from (and re-synced to) the bound value, so the
  // input stays editable even when the host doesn't echo changes back.
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);

  // Auto-grow the textarea to fit its content (so dictated text is visible as
  // it streams in, not hidden off the right edge of a one-line field), then
  // scroll to the newest words once it hits its max height.
  const taRef = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    const el = taRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
    el.scrollTop = el.scrollHeight; // keep the latest line in view
  }, [draft]);

  return (
    <div className="cenno-field-wrap">
      <div className="cenno-field">
        <textarea
          ref={taRef}
          className="cenno-field__input"
          value={draft}
          rows={1}
          aria-label={label}
          placeholder={placeholder ?? label}
          onChange={(e) => {
            setDraft(e.target.value);
            onChange?.(e.target.value);
          }}
          onKeyDown={(e) => {
            // Enter submits; Shift+Enter inserts a newline. IME: Enter that
            // confirms a composition must not submit.
            if (
              e.key === "Enter" &&
              !e.shiftKey &&
              !e.nativeEvent.isComposing
            ) {
              e.preventDefault();
              onSubmit(e.currentTarget.value);
            }
          }}
        />
        {voice && (
          <button
            type="button"
            className={
              recording
                ? "cenno-field__mic cenno-field__mic--recording"
                : "cenno-field__mic"
            }
            disabled={!onMicToggle}
            aria-pressed={recording}
            aria-label={recording ? "Stop dictation" : "Dictate your answer"}
            onClick={onMicToggle}
          >
            {/* mic glyph (idle) — CSS swaps it for the pulsing dot while
                recording. Drawn inline so it follows currentColor. */}
            <svg
              className="cenno-field__mic-glyph"
              viewBox="0 0 24 24"
              width="18"
              height="18"
              aria-hidden="true"
            >
              <rect x="9" y="3" width="6" height="11" rx="3" fill="currentColor" />
              <path
                d="M 5.5 11.5 a 6.5 6.5 0 0 0 13 0"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.6"
                strokeLinecap="round"
              />
              <line
                x1="12"
                y1="18"
                x2="12"
                y2="21"
                stroke="currentColor"
                strokeWidth="1.6"
                strokeLinecap="round"
              />
            </svg>
          </button>
        )}
      </div>
      {voiceError && (
        <p className="cenno-field__voice-error" role="alert">
          {voiceError}
        </p>
      )}
    </div>
  );
}

/**
 * Continuous range slider: native input[type=range], end labels in caption
 * size (mirrors ScaleView). Dragging updates the bound value via onChange;
 * the commit (onCommit) fires on thumb release for pointer users and on
 * Enter for keyboard users — arrow keys only adjust the value, because
 * committing per keypress would answer on the FIRST arrow press, before
 * the user reaches their value. Enter matches the TextField/DateTimeInput
 * answer-on-Enter contract. The catalog adapter maps commit onto the
 * optional selectAction, so a slider can answer-on-commit or sit next to a
 * Send button, whichever the agent sends.
 */
export function SliderView({
  min,
  max,
  step,
  value,
  label,
  minLabel,
  maxLabel,
  onChange,
  onCommit,
}: {
  min: number;
  max: number;
  step?: number;
  value?: number;
  /** Accessible name; falls back to a range/end-label description. */
  label?: string;
  minLabel?: string;
  maxLabel?: string;
  onChange: (n: number) => void;
  onCommit: (n: number) => void;
}) {
  // Internal draft seeded from (and re-synced to) the bound value, same
  // pattern as TextFieldView: stays draggable when the host doesn't echo.
  const fallback = min + (max - min) / 2;
  const [draft, setDraft] = useState(value ?? fallback);
  useEffect(() => {
    if (value !== undefined) setDraft(value);
  }, [value]);
  const accName =
    label ??
    (minLabel && maxLabel
      ? `${min} (${minLabel}) to ${max} (${maxLabel})`
      : `${min} to ${max}`);
  return (
    <div className="cenno-slider">
      <input
        className="cenno-slider__input"
        type="range"
        min={min}
        max={max}
        step={step}
        value={draft}
        aria-label={accName}
        onChange={(e) => {
          const n = Number(e.target.value);
          setDraft(n);
          onChange(n);
        }}
        onPointerUp={(e) => onCommit(Number(e.currentTarget.value))}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            onCommit(Number(e.currentTarget.value));
          }
        }}
      />
      {(minLabel || maxLabel) && (
        <div className="cenno-slider__labels" aria-hidden="true">
          <span className="cenno-slider__label">{minLabel}</span>
          <span className="cenno-slider__label">{maxLabel}</span>
        </div>
      )}
    </div>
  );
}

/**
 * Date / time input: native picker (system UI), styled like the text-field
 * underline. `kind` maps onto the input type; values are the input's native
 * ISO-ish strings ("2026-06-15", "14:30", "2026-06-15T14:30") — the agent
 * receives them verbatim. Enter submits, same contract as TextFieldView.
 */
export function DateTimeView({
  kind,
  value = "",
  label,
  min,
  max,
  onChange,
  onSubmit,
}: {
  kind: "date" | "time" | "datetime";
  value?: string;
  /** Accessible name for the input. */
  label?: string;
  min?: string;
  max?: string;
  onChange: (value: string) => void;
  onSubmit?: (value: string) => void;
}) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  return (
    <div className="cenno-field">
      <input
        className="cenno-field__input cenno-datetime__input"
        type={kind === "datetime" ? "datetime-local" : kind}
        value={draft}
        aria-label={label}
        min={min}
        max={max}
        onChange={(e) => {
          setDraft(e.target.value);
          onChange(e.target.value);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.nativeEvent.isComposing) {
            onSubmit?.(e.currentTarget.value);
          }
        }}
      />
    </div>
  );
}

/**
 * Display-only image with three states in a fixed-size box, so the panel never
 * jumps between them: loading (shimmer), ok (the <img>), error (a caption-size
 * line naming the reason). A broken preview must not read as a missing option,
 * so the failed box stays put with the reason visible.
 *
 * Without a description it is presentational (alt="" → hidden from assistive
 * tech). Note for agents: bundled builds enforce CSP, so prefer app-served
 * assets or local file paths over arbitrary remote URLs — remote URLs are
 * rejected by resolveImageUrl with a named reason.
 */
export function ImageView({
  url,
  description,
  fit = "fill",
  variant = "mediumFeature",
}: {
  url: string;
  description?: string;
  fit?: "contain" | "cover" | "fill" | "none" | "scaleDown";
  variant?:
    | "icon"
    | "avatar"
    | "smallFeature"
    | "mediumFeature"
    | "largeFeature"
    | "header";
}) {
  const [state, setState] = useState<"loading" | "ok" | "error">("loading");
  const [src, setSrc] = useState<string | null>(null);
  const [reason, setReason] = useState("");

  useEffect(() => {
    // Results that land after a re-render for a different url (or after
    // unmount) must not overwrite the newer state.
    let live = true;
    setState("loading");
    setSrc(null);
    setReason("");

    if (!url) {
      setReason("no image URL was given");
      setState("error");
      return () => {
        live = false;
      };
    }

    resolveImageUrl(url).then(
      (resolved) => {
        if (!live) return;
        setSrc(resolved);
        setState("ok");
      },
      (err: unknown) => {
        if (!live) return;
        setReason(imageReason(err));
        setState("error");
      },
    );

    return () => {
      live = false;
    };
  }, [url]);

  const box = `cenno-image-box cenno-image-box--${variant}`;

  if (state === "ok" && src !== null) {
    return (
      <div className={box}>
        <img
          className="cenno-image"
          src={src}
          alt={description ?? ""}
          style={{ objectFit: fit === "scaleDown" ? "scale-down" : fit }}
          onError={() => {
            // A well-formed data: URI can still fail to decode in the webview.
            setReason("failed to decode");
            setState("error");
          }}
        />
      </div>
    );
  }

  if (state === "error") {
    return (
      <div
        className={box}
        data-testid="image-error"
        title={reason}
        // The description is dropped from the visible text on failure, so it
        // must ride on the label: a screen-reader user still needs to know
        // WHICH image failed. `role="img"` makes the box an accessible node.
        role="img"
        aria-label={
          description
            ? `${description}: image failed to load — ${reason}`
            : `image failed to load — ${reason}`
        }
      >
        <span className="cenno-image-box__error">
          <span className="cenno-image-box__error-glyph" aria-hidden="true">
            ⚠
          </span>
          <span className="cenno-image-box__error-text">
            {`image failed to load — ${reason}`}
          </span>
        </span>
      </div>
    );
  }

  return (
    <div className={box}>
      <div
        className="cenno-image-box__loading"
        data-testid="image-loading"
        // Same accessibility contract as the <img>: presentational without a
        // description, labelled by it when there is one.
        role={description ? "img" : undefined}
        aria-hidden={description ? undefined : true}
        aria-label={description ? `Loading ${description}` : undefined}
      />
    </div>
  );
}

/** `reason` is shown verbatim in the panel; never decorate it here. */
function imageReason(err: unknown): string {
  if (err instanceof ImageLoadError) return err.reason;
  if (typeof err === "string" && err.trim()) return err;
  if (err instanceof Error && err.message) return err.message;
  return "unknown error";
}

/**
 * primary  = solid white pill on the flow color.
 * secondary = text-only dim (borderless link-like).
 * quiet    = text-only Send, --cenno-text, pinned bottom-right
 *            (panel-free-text.png) — no pill background, no border.
 */
export function ButtonView({
  variant = "primary",
  disabled = false,
  onClick,
  children,
}: {
  variant?: "primary" | "secondary" | "quiet";
  disabled?: boolean;
  onClick: () => void;
  children?: ReactNode;
}) {
  return (
    <button
      type="button"
      className={`cenno-button cenno-button--${variant}`}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

/**
 * Display-only table: real `<table>` semantics with an optional `<caption>`
 * (which supplies the accessible name), a `columns`-driven header row, and
 * one `<tr>` per record. No input cells, no sorting, no filtering — an
 * answer must flatten to one string (protocol.rs), so an editable cell would
 * need cell→binding validation for no gain.
 *
 * Design rules (TOKENS.md / the rich-panel spec): hairlines only — a bottom
 * border per row, never an outer border, zebra striping, or a background; the
 * header sits at caption size, dim, uppercase, with `.08em` tracking, the same
 * treatment as the ScaleView end labels. Per-column `align` puts numbers in an
 * `is-end` cell with tabular numerals, so digits line up.
 *
 * A row SHORTER than `columns` renders `—` for the missing cells rather than a
 * hole; the guard allows short rows on purpose and rejects long ones (see the
 * `Table` arm in a2ui_guard.rs). `header: false` drops the header row and keeps
 * the body (a legend-less key/value block).
 */
export function TableView({
  header = true,
  columns,
  rows,
  dense = false,
  caption,
}: {
  /** Render the header row (default true). */
  header?: boolean;
  columns: { label: string; align?: "start" | "end" }[];
  rows: string[][];
  /** Halve the vertical cell padding (~40px rows → ~32px); hairlines stay. */
  dense?: boolean;
  /** Accessible name for the table; rendered as a real `<caption>`. */
  caption?: string;
}) {
  return (
    <table className={`cenno-table${dense ? " is-dense" : ""}`}>
      {caption && <caption className="cenno-table__caption">{caption}</caption>}
      {header && (
        <thead>
          <tr>
            {columns.map((col, i) => (
              <th
                key={i}
                scope="col"
                className={`cenno-table__head${col.align === "end" ? " is-end" : ""}`}
              >
                {col.label}
              </th>
            ))}
          </tr>
        </thead>
      )}
      <tbody>
        {rows.map((row, r) => (
          <tr key={r}>
            {columns.map((col, c) => (
              <td
                key={c}
                className={`cenno-table__cell${col.align === "end" ? " is-end" : ""}`}
                // `overflow-wrap: break-word` inline as well as in catalog.css:
                // a long unbroken token (a hash, a url) must wrap inside its
                // column, never widen the panel.
                style={{ overflowWrap: "break-word" }}
              >
                {/* A blank cell is the same hole as a missing one. */}
                {row[c]?.trim() ? row[c] : "—"}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** EMA step pagination: 6px dots, active full white, inactive 40%. */
export function DotsView({ step, total }: { step: number; total: number }) {
  return (
    <ol className="cenno-dots" aria-label={`step ${step} of ${total}`}>
      {Array.from({ length: total }, (_, i) => (
        <li
          key={i}
          className={`cenno-dot${i + 1 === step ? " is-active" : ""}`}
          aria-current={i + 1 === step ? "step" : undefined}
        />
      ))}
    </ol>
  );
}
