// Live widget gallery for the site. Every control below is the app's own view
// component, so what visitors click is exactly what the panel renders.
import { StrictMode, useState } from "react";
import type { ReactNode } from "react";
import { createRoot } from "react-dom/client";
import "../../src/styles/theme.css";
import {
  ChoicePickerView,
  ImageChoiceView,
  MatrixView,
  StepScoreView,
} from "../../src/a2ui/views";
import type { StepScoreAnswer } from "../../src/a2ui/views";
import "./widgets.css";

const IMG = "/widgets/img";

type Flow = "question" | "ema" | "mood" | "reminder";

/** One gallery entry: what it is for, the live panel, and what the agent gets. */
function Card({
  flow = "question",
  name,
  note,
  payload,
  question,
  answer,
  children,
}: {
  flow?: Flow;
  name: string;
  note: string;
  payload: string;
  question: string;
  answer: string | null;
  children: ReactNode;
}) {
  return (
    <article className="wg-card">
      <h3 className="wg-card__name">{name}</h3>
      <p className="wg-card__note">{note}</p>
      <div className="wg-panel" data-flow={flow}>
        <div className="wg-panel__chrome" aria-hidden="true">
          <span>cenno</span>
          <span>✕</span>
        </div>
        <p className="cenno-text cenno-text--question-m wg-panel__q">{question}</p>
        {children}
      </div>
      <dl className="wg-card__meta">
        <dt>Payload</dt>
        <dd><code>{payload}</code></dd>
        <dt>Agent gets</dt>
        <dd>
          <code className={answer === null ? "is-empty" : undefined} aria-live="polite">
            {answer === null ? "nothing yet — answer the panel" : JSON.stringify({ answer, via: "choice" })}
          </code>
        </dd>
      </dl>
    </article>
  );
}

const toggle = (list: string[], v: string) =>
  list.includes(v) ? list.filter((x) => x !== v) : [...list, v];

function RadioList() {
  const [sel, setSel] = useState<string | null>(null);
  return (
    <Card
      name="Radio list"
      note="Long labels, four to six options. Arrow keys move and select."
      payload={`ChoicePicker · "displayStyle": "checkbox"`}
      question="Which runtime for the build scripts?"
      answer={sel}
    >
      <ChoicePickerView
        choices={[
          { label: "Node 22 LTS — what CI already pins", value: "node22" },
          { label: "Bun 1.2 — faster installs", value: "bun" },
          { label: "Deno 2 — permissions model", value: "deno" },
          { label: "Keep the current setup", value: "keep" },
        ]}
        selected={sel ? [sel] : []}
        displayStyle="checkbox"
        variant="mutuallyExclusive"
        label="Runtime"
        onSelect={setSel}
      />
    </Card>
  );
}

function CheckList() {
  const [sel, setSel] = useState<string[]>(["tests"]);
  return (
    <Card
      name="Checklist"
      note="Several picks, comma-joined in the answer — so no option value may contain a comma."
      payload={`ChoicePicker · "checkbox" + "multipleSelection"`}
      question="Before I push, which checks should run?"
      answer={sel.length ? sel.join(",") : null}
    >
      <ChoicePickerView
        choices={[
          { label: "Run the full test suite", value: "tests" },
          { label: "Type-check the whole repo", value: "tsc" },
          { label: "Lint only the changed files", value: "lint" },
          { label: "Rebuild the Swift bridge", value: "swift" },
        ]}
        selected={sel}
        displayStyle="checkbox"
        variant="multipleSelection"
        label="Checks"
        onSelect={(v) => setSel((s) => toggle(s, v))}
      />
    </Card>
  );
}

function MultiChips() {
  const [sel, setSel] = useState<string[]>([]);
  return (
    <Card
      flow="ema"
      name="Multi-pick chips"
      note="The familiar pills, with more than one answer allowed. Best for short labels."
      payload={`ChoicePicker · "variant": "multipleSelection"`}
      question="What drained you today?"
      answer={sel.length ? sel.join(",") : null}
    >
      <ChoicePickerView
        choices={["meetings", "email", "noise", "context switching", "sleep"].map((l) => ({
          label: l,
          value: l.replace(/ /g, "_"),
        }))}
        selected={sel}
        variant="multipleSelection"
        onSelect={(v) => setSel((s) => toggle(s, v))}
      />
    </Card>
  );
}

function IconPick() {
  const [sel, setSel] = useState<string | null>(null);
  return (
    <Card
      name="Pick an image"
      note="Up to ten candidates in a grid that never leaves an orphan row. For generated art, icons, screenshots."
      payload={`ImageChoice · "variant": "mutuallyExclusive"`}
      question="Which icon ships with the release?"
      answer={sel}
    >
      <ImageChoiceView
        options={[
          { value: "ink", label: "ink drop", image: `${IMG}/icon-ink.jpg` },
          { value: "quill", label: "quill", image: `${IMG}/icon-quill.jpg` },
          { value: "stack", label: "cards", image: `${IMG}/icon-stack.jpg` },
        ]}
        selected={sel ? [sel] : []}
        variant="mutuallyExclusive"
        onSelect={setSel}
      />
    </Card>
  );
}

function StylePick() {
  const [sel, setSel] = useState<string[]>([]);
  return (
    <Card
      flow="reminder"
      name="Keep several images"
      note="Multi-select over the same grid. The last image is a web URL, which cenno never fetches: the tile says why and stays selectable."
      payload={`ImageChoice · "variant": "multipleSelection"`}
      question="Keep which header styles?"
      answer={sel.length ? sel.join(",") : null}
    >
      <ImageChoiceView
        options={[
          { value: "line", label: "line", image: `${IMG}/style-line.jpg` },
          { value: "watercolor", label: "watercolor", image: `${IMG}/style-watercolor.jpg` },
          { value: "flat", label: "flat", image: `${IMG}/style-flat.jpg` },
          { value: "iso", label: "isometric", image: `${IMG}/style-iso.jpg` },
          { value: "photo", label: "photo", image: `${IMG}/style-photo.jpg` },
          { value: "remote", label: "remote", image: "https://example.com/header.png" },
        ]}
        selected={sel}
        variant="multipleSelection"
        onSelect={(v) => setSel((s) => toggle(s, v))}
      />
    </Card>
  );
}

function Matrix({ mode }: { mode: "matrix" | "sliders" }) {
  const [draft, setDraft] = useState<Record<string, number>>({});
  const [done, setDone] = useState<string | null>(null);
  const matrix = mode === "matrix";
  return (
    <Card
      flow={matrix ? "question" : "ema"}
      name={matrix ? "Rate several things" : "Rate on sliders"}
      note={
        matrix
          ? "Up to three rows on one scale. Click a column number to rate every row at once. Nothing is sent until Done, and skipping a row is fine."
          : "Same answer, one continuous slider per row."
      }
      payload={`RatingMatrix · "mode": "${mode}"`}
      question="How did today's session go?"
      answer={done}
    >
      <MatrixView
        rows={[
          { value: "pace", label: "Pace" },
          { value: "useful", label: "Usefulness" },
          { value: "clarity", label: "Clarity" },
        ]}
        mode={mode}
        min={1}
        max={5}
        minLabel="low"
        maxLabel="high"
        value={draft}
        onSelect={(all) => {
          setDraft(all);
          setDone(null);
        }}
        onDone={(all) => setDone(JSON.stringify(all))}
      />
    </Card>
  );
}

function StepScore() {
  const [draft, setDraft] = useState<StepScoreAnswer>({});
  const [done, setDone] = useState<string | null>(null);
  return (
    <Card
      flow="mood"
      name="Score one step"
      note="One item, several 0–3 scores and on/off flags. Next unlocks once every score is set — 0 counts."
      payload="StepScore"
      question="Score this step (0 none · 3 high)"
      answer={done}
    >
      <StepScoreView
        step="Review PRs"
        dimensions={[
          { value: "sensitivity", label: "If this step were 2× faster, would throughput rise?", max: 3, decisive: true },
          { value: "wait", label: "wait before", max: 3 },
          { value: "starve", label: "starves next", max: 3 },
          { value: "cap", label: "capacity gap", max: 3 },
        ]}
        flags={[
          { value: "necessary", label: "necessary" },
          { value: "policy_gate", label: "policy gate" },
        ]}
        value={draft}
        doneLabel="Next"
        onSelect={(a) => {
          setDraft(a);
          setDone(null);
        }}
        onDone={(a) => setDone(JSON.stringify(a))}
      />
    </Card>
  );
}

function Gallery() {
  return (
    <div className="wg-grid">
      <RadioList />
      <CheckList />
      <IconPick />
      <StylePick />
      <Matrix mode="matrix" />
      <StepScore />
      <MultiChips />
      <Matrix mode="sliders" />
    </div>
  );
}

const mount = document.getElementById("widget-gallery");
if (mount) {
  mount.replaceChildren();
  createRoot(mount).render(
    <StrictMode>
      <Gallery />
    </StrictMode>,
  );
}
