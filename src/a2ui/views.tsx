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
import { useEffect, useState } from "react";
import type { AnchorHTMLAttributes, MouseEvent, ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import { openUrl } from "@tauri-apps/plugin-opener";
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
 * Free-text input: bottom-border underline only (Reporter's "Alone ____"
 * pattern). Enter submits (IME composition guarded). When `voice` is set, a
 * disabled mic stub circle is shown — voice arrives in plan 3.
 */
export function TextFieldView({
  value = "",
  label,
  placeholder,
  voice = false,
  onChange,
  onSubmit,
}: {
  value?: string;
  /** Accessible name for the input (placeholder alone is a fragile accname). */
  label?: string;
  placeholder?: string;
  voice?: boolean;
  onChange?: (text: string) => void;
  onSubmit: (text: string) => void;
}) {
  // Internal draft seeded from (and re-synced to) the bound value, so the
  // input stays editable even when the host doesn't echo changes back.
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  return (
    <div className="cenno-field">
      <input
        className="cenno-field__input"
        type="text"
        value={draft}
        aria-label={label}
        placeholder={placeholder ?? label}
        onChange={(e) => {
          setDraft(e.target.value);
          onChange?.(e.target.value);
        }}
        onKeyDown={(e) => {
          // IME: Enter confirms the composition, not the answer
          if (e.key === "Enter" && !e.nativeEvent.isComposing) {
            onSubmit(e.currentTarget.value);
          }
        }}
      />
      {voice && (
        <button
          type="button"
          className="cenno-field__mic"
          disabled
          title="voice arrives in plan 3"
          aria-label="voice input (coming soon)"
        />
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
 * Display-only image. Without a description it is presentational (alt="" →
 * hidden from assistive tech). Note for agents: bundled builds enforce CSP,
 * so prefer data: URIs or app-served assets over arbitrary remote URLs.
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
  return (
    <img
      className={`cenno-image cenno-image--${variant}`}
      src={url}
      alt={description ?? ""}
      style={{ objectFit: fit === "scaleDown" ? "scale-down" : fit }}
    />
  );
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
