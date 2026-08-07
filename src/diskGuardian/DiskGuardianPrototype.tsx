import { useEffect, useId, useRef, useState } from "react";
import "./DiskGuardianPrototype.css";

type Candidate = {
  id: string;
  name: string;
  scope: string;
  estimate: string;
  limits: string;
};

const CANDIDATES: Candidate[] = [
  {
    id: "browser-caches",
    name: "Browser cache files",
    scope: "Safari and Chromium cache folders for this user",
    estimate: "About 3.8 GB could be reclaimed",
    limits: "Keeps cookies, history, passwords, downloads, and open tabs",
  },
  {
    id: "developer-builds",
    name: "Old developer build output",
    scope: "Build artefacts not modified in the last 30 days",
    estimate: "About 6.4 GB could be reclaimed",
    limits: "Excludes source files, archives, simulators, and active projects",
  },
  {
    id: "large-downloads",
    name: "Large Downloads to review",
    scope: "Files larger than 1 GB in this user's Downloads folder",
    estimate: "About 2.1 GB to review",
    limits: "Review only; nothing would be selected or removed automatically",
  },
];

export default function DiskGuardianPrototype() {
  const [warningThreshold, setWarningThreshold] = useState("20");
  const [criticalThreshold, setCriticalThreshold] = useState("10");
  const [thresholdNotice, setThresholdNotice] = useState("");
  const [pendingCandidate, setPendingCandidate] = useState<Candidate | null>(null);
  const [approvedIds, setApprovedIds] = useState<Set<string>>(() => new Set());
  const dialogTitleId = useId();
  const dialogDescriptionId = useId();
  const actionButtons = useRef(new Map<string, HTMLButtonElement>());

  useEffect(() => {
    if (!pendingCandidate) return;

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") setPendingCandidate(null);
    }

    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [pendingCandidate]);

  function saveExampleThresholds(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const warning = Number(warningThreshold);
    const critical = Number(criticalThreshold);
    if (!Number.isFinite(warning) || !Number.isFinite(critical) || critical >= warning) {
      setThresholdNotice("Critical must be lower than warning.");
      return;
    }
    setThresholdNotice(
      `Example updated locally: warning at ${warningThreshold} GB, critical at ${criticalThreshold} GB. These values are not saved.`,
    );
  }

  function closeConfirmation() {
    const candidateId = pendingCandidate?.id;
    setPendingCandidate(null);
    if (candidateId) {
      window.requestAnimationFrame(() => actionButtons.current.get(candidateId)?.focus());
    }
  }

  function confirmCandidate() {
    if (!pendingCandidate) return;
    const candidateId = pendingCandidate.id;
    setApprovedIds((current) => new Set(current).add(candidateId));
    setPendingCandidate(null);
    window.requestAnimationFrame(() => actionButtons.current.get(candidateId)?.focus());
  }

  return (
    <section className="dg" aria-labelledby="dg-title">
      <header className="dg__intro">
        <div>
          <p className="dg__eyebrow">Disk Guardian · interactive prototype</p>
          <h1 id="dg-title">Storage has room, but it is tightening</h1>
          <p className="dg__lede">
            A read-only preview of how cenno could surface pressure and offer narrowly
            scoped cleanup choices.
          </p>
        </div>
        <span className="dg__risk" aria-label="Current risk: warning">
          Warning
        </span>
      </header>

      <div className="dg__metrics" aria-label="Current disk status">
        <article className="dg__metric">
          <span className="dg__metric-label">Free space</span>
          <strong>18.6 GB</strong>
          <span>of 494 GB available</span>
        </article>
        <article className="dg__metric">
          <span className="dg__metric-label">30-day trend</span>
          <strong>−11.2 GB</strong>
          <span>about 370 MB used per day</span>
        </article>
        <article className="dg__metric">
          <span className="dg__metric-label">Projected critical</span>
          <strong>23 days</strong>
          <span>if the current trend continues</span>
        </article>
      </div>

      <section className="dg__section" aria-labelledby="dg-thresholds-title">
        <div className="dg__section-heading">
          <div>
            <p className="dg__eyebrow">Example controls</p>
            <h2 id="dg-thresholds-title">Alert thresholds</h2>
          </div>
          <span className="dg__demo-badge">Demo only · not persisted</span>
        </div>
        <form className="dg__threshold-form" onSubmit={saveExampleThresholds}>
          <label>
            <span>Warning below</span>
            <span className="dg__number-field">
              <input
                aria-label="Warning threshold in gigabytes"
                type="number"
                min="1"
                max="500"
                step="1"
                value={warningThreshold}
                onChange={(event) => setWarningThreshold(event.target.value)}
              />
              <span aria-hidden="true">GB</span>
            </span>
          </label>
          <label>
            <span>Critical below</span>
            <span className="dg__number-field">
              <input
                aria-label="Critical threshold in gigabytes"
                type="number"
                min="1"
                max="500"
                step="1"
                value={criticalThreshold}
                onChange={(event) => setCriticalThreshold(event.target.value)}
              />
              <span aria-hidden="true">GB</span>
            </span>
          </label>
          <button className="dg__button dg__button--secondary" type="submit">
            Apply to this demo
          </button>
        </form>
        <p className="dg__notice" role="status" aria-live="polite">
          {thresholdNotice || "Changing these examples does not update cenno settings."}
        </p>
      </section>

      <section className="dg__section" aria-labelledby="dg-candidates-title">
        <div className="dg__section-heading">
          <div>
            <p className="dg__eyebrow">Candidate preview</p>
            <h2 id="dg-candidates-title">Possible cleanup actions</h2>
          </div>
          <p className="dg__section-note">Estimates are fixture data, not a disk scan.</p>
        </div>

        <div className="dg__candidate-list">
          {CANDIDATES.map((candidate) => {
            const approved = approvedIds.has(candidate.id);
            return (
              <article className="dg__candidate" key={candidate.id}>
                <div className="dg__candidate-copy">
                  <div className="dg__candidate-title-row">
                    <h3>{candidate.name}</h3>
                    {approved && <span className="dg__approved">Approved in demo</span>}
                  </div>
                  <dl>
                    <div>
                      <dt>Scope</dt>
                      <dd>{candidate.scope}</dd>
                    </div>
                    <div>
                      <dt>Estimate</dt>
                      <dd>{candidate.estimate}</dd>
                    </div>
                    <div>
                      <dt>Limits</dt>
                      <dd>{candidate.limits}</dd>
                    </div>
                  </dl>
                </div>
                <button
                  className="dg__button dg__button--primary"
                  type="button"
                  ref={(node) => {
                    if (node) actionButtons.current.set(candidate.id, node);
                    else actionButtons.current.delete(candidate.id);
                  }}
                  onClick={() => setPendingCandidate(candidate)}
                >
                  {approved ? "Review approval" : "Review action"}
                  <span className="dg__sr-only">: {candidate.name}</span>
                </button>
              </article>
            );
          })}
        </div>
      </section>

      {pendingCandidate && (
        <div className="dg__dialog-backdrop">
          <div
            className="dg__dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby={dialogTitleId}
            aria-describedby={dialogDescriptionId}
          >
            <p className="dg__eyebrow">Separate action confirmation</p>
            <h2 id={dialogTitleId}>Approve “{pendingCandidate.name}”?</h2>
            <p id={dialogDescriptionId}>
              This marks only this candidate as approved in the prototype. It will not
              inspect, change, or remove any files.
            </p>
            <div className="dg__dialog-scope">
              <span>Exact scope</span>
              <strong>{pendingCandidate.scope}</strong>
            </div>
            <div className="dg__dialog-actions">
              <button
                className="dg__button dg__button--secondary"
                type="button"
                onClick={closeConfirmation}
              >
                Cancel
              </button>
              <button
                className="dg__button dg__button--primary"
                type="button"
                autoFocus
                onClick={confirmCandidate}
              >
                Confirm demo approval
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
