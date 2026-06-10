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
 * Choice chips: outline pills. Identity is the choice VALUE (labels may
 * collide); a tap reports the value. `selected` carries every selected value
 * — single-select callers pass a 1-element array.
 */
export function ChipsView({
  choices,
  selected = [],
  onSelect,
}: {
  choices: { label: string; value: string }[];
  selected?: string[];
  onSelect: (value: string) => void;
}) {
  return (
    <div className="cenno-chips">
      {choices.map(({ label, value }) => (
        <button
          key={value}
          type="button"
          className="cenno-chip"
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

/** Primary = solid white on flow color; secondary = text-only dim. */
export function ButtonView({
  variant = "primary",
  disabled = false,
  onClick,
  children,
}: {
  variant?: "primary" | "secondary";
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
