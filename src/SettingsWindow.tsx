/**
 * SettingsWindow — cenno's main window (opened from the tray "cenno settings…").
 *
 * Four tabs:
 *   - Settings    — Voice/TTS (engine, voice, test), behavior toggles, defaults
 *   - Guardian    — read-only disk pressure and diagnostic observations
 *   - Integration — the MCP config snippet to wire cenno into an agent
 *   - About       — what cenno is, links (GitHub/LinkedIn/repo)
 *
 * Voice/TTS and defaults persist to ~/.cenno/config.json via the
 * `save_config_file` command; `tts_speak` reads that file fresh each call, so
 * changes take effect on the next spoken prompt without a restart. Behavior
 * toggles (launch-at-login, dock) apply immediately via dedicated commands.
 *
 * This is a first prototype — structure and wiring over polish.
 */
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import "./SettingsWindow.css";

type Tab = "settings" | "guardian" | "integration" | "about";

const SETTINGS_SELECT_TAB_EVENT = "settings:select-tab";
const SETTINGS_TABS: readonly Tab[] = ["settings", "guardian", "integration", "about"];

function parseSettingsTab(value: unknown): Tab | null {
  return typeof value === "string" && SETTINGS_TABS.includes(value as Tab)
    ? value as Tab
    : null;
}

function initialSettingsTab(): Tab {
  return parseSettingsTab(new URL(window.location.href).searchParams.get("tab")) ?? "settings";
}

type GuardianSeverity = "healthy" | "warning" | "critical" | "unknown";

interface GuardianStatusResponse {
  schemaVersion: 1;
  generatedAt: string;
  sample: {
    schemaVersion: 1;
    targetId: string;
    observedAt: string;
    totalBytes: string;
    availableBytes: string;
    freeBytes: string;
    blockSizeBytes: string;
  };
  evaluation: {
    severity: GuardianSeverity;
    reasons: string[];
    evaluatedAt: string;
    latestSampleAt: string;
    availableBytes: string;
    availableBasisPoints: number;
    trend?: {
      windowMs: number;
      dropBytes: string;
      timeToCriticalMs: string;
    };
  };
}

interface GuardianCandidate {
  scope: string;
  kind: "file" | "folder";
  relativePath: string;
  bytes: string;
}

interface GuardianDiagnosticsResponse {
  schemaVersion: 1;
  generatedAt: string;
  report: {
    schemaVersion: 1;
    observedAt: string;
    scopes: Array<{
      id: string;
      files: number;
      folders: number;
      bytes: string;
      readable: boolean;
    }>;
    totals: { files: number; folders: number; bytes: string };
    topCandidates: GuardianCandidate[];
    partial: boolean;
    limitsHit: string[];
    errorCount: number;
  };
  growth: null | {
    schemaVersion: 1;
    previousObservedAt: string;
    currentObservedAt: string;
    totals: { filesDelta: number; bytesDelta: string };
    candidates: Array<{
      scope: string;
      relativePath: string;
      previousBytes: string;
      currentBytes: string;
      deltaBytes: string;
    }>;
  };
  preview: {
    schemaVersion: 1;
    generatedFrom: string;
    previewRef: string;
    estimatedBytes: string;
    candidates: GuardianCandidate[];
  };
}

type GuardianRunStatus =
  | "imported"
  | "claimed"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "expired";

interface GuardianRun {
  authorizationId: string;
  profileId: string;
  profileVersion: number;
  expiresAt: string;
  status: GuardianRunStatus;
  progress: number;
  claimedAt?: string;
  startedAt?: string;
  finishedAt?: string;
  result?: Record<string, unknown>;
}

interface GuardianRunsResponse {
  schemaVersion: 1;
  sequence: number;
  runs: GuardianRun[];
  events: unknown[];
}

interface GuardianStartScanResponse {
  authorizationId: string;
  status: "running";
  startedAt: string;
}

interface GuardianCancelRunResponse {
  authorizationId: string;
  status: "cancelled";
}

interface GuardianMonitoringPolicy {
  warningBytes: string;
  criticalBytes: string;
  warningBasisPoints: number;
  criticalBasisPoints: number;
}

interface GuardianMonitoringPolicyResponse {
  schemaVersion: 1;
  revision: number;
  policy: GuardianMonitoringPolicy;
}

interface GuardianMonitoringPolicyDraft {
  warningBytes: string;
  criticalBytes: string;
  warningBasisPoints: string;
  criticalBasisPoints: string;
}

type GuardianSecuritySubsystemStatus =
  | "available"
  | "degraded"
  | "unavailable"
  | "uninitialized"
  | "active"
  | "pending_rotation"
  | "revoked";

interface GuardianSecurityKey {
  keyId: string;
  publicKeyFingerprint: string;
}

interface GuardianSecurityStatusResponse {
  schemaVersion: 1;
  reportedBy: "disk-guardian";
  execution: { status: GuardianSecuritySubsystemStatus; code?: string };
  personalContractsAuthority: {
    status: GuardianSecuritySubsystemStatus;
    code?: string;
    generation?: number;
    activeKey?: GuardianSecurityKey | null;
    pendingKey?: GuardianSecurityKey | null;
    retiredKeyCount?: number;
    revokedKeyCount?: number;
  };
  executorIdentity: {
    status: GuardianSecuritySubsystemStatus;
    code?: string;
    generation?: number;
    executorId?: string;
    activeKey?: GuardianSecurityKey | null;
    pendingKey?: GuardianSecurityKey | null;
  };
  profileLifecycle: {
    status: GuardianSecuritySubsystemStatus;
    code?: string;
    head?: { generation: number; digest: string };
    activeProfileCount?: number;
    retiredProfileCount?: number;
  };
}

type GuardianLoad<T> =
  | { state: "loading" }
  | { state: "ready"; data: T }
  | { state: "error"; message: string };

/** Mirror of Rust supertonic::ModelStatus. */
interface ModelStatus {
  present: boolean;
  dir: string;
  custom: boolean;
  missing: string[];
  total_bytes: number;
}

/** Mirror of the Rust Config (snake_case keys). Unknown blocks are preserved
 *  verbatim on the object so a round-trip save never drops them. */
interface RawConfig {
  panel?: unknown;
  defaults?: { timeout_s?: number; flow?: string };
  tts?: {
    enabled?: boolean;
    min_urgency?: string;
    voice?: string;
    engine?: string;
    model_path?: string;
    output_device?: string;
  };
  routing?: unknown;
  shortcuts?: { reopen?: string };
  widgets?: unknown;
  guardian?: { notifications_enabled?: boolean };
}

// Suggested default for the reopen-pending global shortcut (user-changeable).
const DEFAULT_REOPEN_SHORTCUT = "Cmd+Shift+C";

const FLOWS = ["question", "mood", "ema", "reminder", "ambient"] as const;
const URGENCIES = ["low", "normal", "high"] as const;

// Supertonic voice styles shipped with the model (voice_styles/*.json).
const SUPERTONIC_VOICES: { id: string; label: string }[] = [
  { id: "F1", label: "F1 — female, warm" },
  { id: "F2", label: "F2 — female, bright" },
  { id: "F3", label: "F3 — female, calm (default)" },
  { id: "F4", label: "F4 — female, soft" },
  { id: "F5", label: "F5 — female, clear" },
  { id: "M1", label: "M1 — male, warm" },
  { id: "M2", label: "M2 — male, bright" },
  { id: "M3", label: "M3 — male, calm" },
  { id: "M4", label: "M4 — male, soft" },
  { id: "M5", label: "M5 — male, clear" },
];

const MCP_SNIPPET = `{
  "mcpServers": {
    "cenno": {
      "command": "/Applications/cenno.app/Contents/MacOS/cenno",
      "args": ["--mcp-stdio"]
    }
  }
}`;

const SKILL_INSTALL =
  "npx skills add https://github.com/glebis/cenno/tree/main/skills/cenno";

export default function SettingsWindow() {
  const [tab, setTab] = useState<Tab>(initialSettingsTab);
  const [cfg, setCfg] = useState<RawConfig | null>(null);
  const [launchAtLogin, setLaunchAtLogin] = useState(false);
  const [hideFromDock, setHideFromDock] = useState(false);
  const [saved, setSaved] = useState(false);
  const [testing, setTesting] = useState(false);

  // Load current on-disk config + OS toggle state on mount.
  useEffect(() => {
    invoke<RawConfig>("read_config_file")
      .then(setCfg)
      .catch(() => setCfg({}));
    invoke<boolean>("get_launch_at_login")
      .then(setLaunchAtLogin)
      .catch(() => {});
  }, []);

  // A warm tray click targets this already-mounted window. Ignore unknown
  // payloads so no caller can navigate to a non-existent or unsafe surface.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;
    listen<unknown>(SETTINGS_SELECT_TAB_EVENT, ({ payload }) => {
      const selected = parseSettingsTab(payload);
      if (selected) setTab(selected);
    }).then((dispose) => {
      if (mounted) unlisten = dispose;
      else dispose();
    }).catch(() => {});
    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  // Persist the whole config (preserving unknown blocks) and flash "Saved".
  async function persist(next: RawConfig) {
    const previous = cfg;
    setCfg(next);
    try {
      await invoke("save_config_file", { config: next });
      setSaved(true);
      window.setTimeout(() => setSaved(false), 1400);
      return { ok: true as const };
    } catch (e) {
      console.error("cenno: save_config_file failed", e);
      setCfg((current) => current === next ? previous : current);
      return { ok: false as const, error: guardianError(e) };
    }
  }

  const tts = cfg?.tts ?? {};
  const engine = tts.engine === "supertonic" ? "supertonic" : "system";

  function patchTts(p: Partial<NonNullable<RawConfig["tts"]>>) {
    if (!cfg) return;
    persist({ ...cfg, tts: { ...cfg.tts, ...p } });
  }
  function patchDefaults(p: Partial<NonNullable<RawConfig["defaults"]>>) {
    if (!cfg) return;
    persist({ ...cfg, defaults: { ...cfg.defaults, ...p } });
  }
  function patchShortcuts(p: Partial<NonNullable<RawConfig["shortcuts"]>>) {
    if (!cfg) return;
    persist({ ...cfg, shortcuts: { ...cfg.shortcuts, ...p } });
  }

  async function testVoice() {
    setTesting(true);
    try {
      await invoke("tts_speak", {
        text: "Hi — this is how cenno will sound when it reads a prompt aloud.",
        voice: engine === "supertonic" ? tts.voice ?? "F3" : tts.voice ?? null,
      });
    } catch (e) {
      console.error("cenno: tts_speak failed", e);
    } finally {
      setTesting(false);
    }
  }

  async function toggleLaunchAtLogin(enabled: boolean) {
    setLaunchAtLogin(enabled);
    try {
      await invoke("set_launch_at_login", { enabled });
    } catch (e) {
      console.error(e);
      setLaunchAtLogin(!enabled);
    }
  }

  async function toggleDock(hidden: boolean) {
    setHideFromDock(hidden);
    try {
      await invoke("set_dock_visible", { visible: !hidden });
    } catch (e) {
      console.error(e);
      setHideFromDock(!hidden);
    }
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      setSaved(true);
      window.setTimeout(() => setSaved(false), 1400);
    } catch {
      /* clipboard blocked — no-op for the prototype */
    }
  }

  return (
    <div className="sw">
      <header className="sw__top">
        <div className="sw__brand">
          <CennoMark />
          <span className="sw__title">cenno</span>
        </div>
        <nav className="sw__tabs">
          {SETTINGS_TABS.map((t) => (
            <button
              key={t}
              className={`sw__tab${tab === t ? " sw__tab--active" : ""}`}
              onClick={() => setTab(t)}
            >
              {t[0].toUpperCase() + t.slice(1)}
            </button>
          ))}
        </nav>
        <span className={`sw__saved${saved ? " sw__saved--on" : ""}`}>Saved ✓</span>
      </header>

      <main className="sw__body">
        {tab === "settings" && (
          <SettingsTab
            cfg={cfg}
            tts={tts}
            engine={engine}
            testing={testing}
            launchAtLogin={launchAtLogin}
            hideFromDock={hideFromDock}
            onPatchTts={patchTts}
            onPatchDefaults={patchDefaults}
            onPatchShortcuts={patchShortcuts}
            onTest={testVoice}
            onToggleLaunch={toggleLaunchAtLogin}
            onToggleDock={toggleDock}
          />
        )}
        {tab === "integration" && (
          <IntegrationTab onCopy={copy} />
        )}
        {tab === "guardian" && cfg && <GuardianTab cfg={cfg} onPersist={persist} />}
        {tab === "about" && <AboutTab onOpen={openUrl} />}
      </main>
    </div>
  );
}

/* ─────────────────────────── Settings tab ─────────────────────────── */

function SettingsTab(props: {
  cfg: RawConfig | null;
  tts: NonNullable<RawConfig["tts"]>;
  engine: "system" | "supertonic";
  testing: boolean;
  launchAtLogin: boolean;
  hideFromDock: boolean;
  onPatchTts: (p: Partial<NonNullable<RawConfig["tts"]>>) => void;
  onPatchDefaults: (p: Partial<NonNullable<RawConfig["defaults"]>>) => void;
  onPatchShortcuts: (p: Partial<NonNullable<RawConfig["shortcuts"]>>) => void;
  onTest: () => void;
  onToggleLaunch: (v: boolean) => void;
  onToggleDock: (v: boolean) => void;
}) {
  const { cfg, tts, engine } = props;
  if (!cfg) return <p className="sw__muted">Loading…</p>;
  const enabled = tts.enabled === true;
  const reopenShortcut = cfg.shortcuts?.reopen ?? "";

  return (
    <>
      <section className="sw__section">
        <h2>Voice — read prompts aloud</h2>
        <p className="sw__muted">
          Sound-out speaks a prompt when it arrives. Off by default.
        </p>

        <Toggle
          label="Speak prompts aloud"
          checked={enabled}
          onChange={(v) => props.onPatchTts({ enabled: v })}
        />

        <div className={`sw__sub${enabled ? "" : " sw__sub--off"}`}>
          <Field label="Engine">
            <select
              value={engine}
              onChange={(e) => props.onPatchTts({ engine: e.target.value })}
            >
              <option value="system">System (macOS, fast)</option>
              <option value="supertonic">Supertonic (on-device neural)</option>
            </select>
          </Field>

          {engine === "supertonic" && (
            <>
              <Field label="Voice">
                <select
                  value={tts.voice ?? "F3"}
                  onChange={(e) => props.onPatchTts({ voice: e.target.value })}
                >
                  {SUPERTONIC_VOICES.map((v) => (
                    <option key={v.id} value={v.id}>
                      {v.label}
                    </option>
                  ))}
                </select>
              </Field>
              <SupertonicModel
                customPath={tts.model_path ?? ""}
                onPatchTts={props.onPatchTts}
              />
            </>
          )}

          <Field label="Read aloud when urgency is at least">
            <select
              value={tts.min_urgency ?? "high"}
              onChange={(e) => props.onPatchTts({ min_urgency: e.target.value })}
            >
              {URGENCIES.map((u) => (
                <option key={u} value={u}>
                  {u}
                </option>
              ))}
            </select>
          </Field>

          <OutputDevicePicker
            value={tts.output_device ?? ""}
            onChange={(v) => props.onPatchTts({ output_device: v || undefined })}
          />

          <button className="sw__btn" onClick={props.onTest} disabled={props.testing}>
            {props.testing ? "Speaking…" : "▶ Test voice"}
          </button>
        </div>
      </section>

      <section className="sw__section">
        <h2>Behavior</h2>
        <Toggle
          label="Launch at login"
          checked={props.launchAtLogin}
          onChange={props.onToggleLaunch}
        />
        <Toggle
          label="Hide from Dock (menu-bar only)"
          checked={props.hideFromDock}
          onChange={props.onToggleDock}
        />
        <p className="sw__hint">
          Dock change applies now; persistence across restarts is coming.
        </p>
      </section>

      <section className="sw__section">
        <h2>Shortcut — reopen a pending prompt</h2>
        <p className="sw__muted">
          A global hotkey (and the “Show pending prompt” tray item) brings a
          dismissed or hidden prompt back — with whatever you’d typed restored.
        </p>
        <Field label="Reopen shortcut">
          <input
            type="text"
            value={reopenShortcut}
            placeholder={`${DEFAULT_REOPEN_SHORTCUT} (leave blank to disable)`}
            spellCheck={false}
            onChange={(e) =>
              props.onPatchShortcuts({ reopen: e.target.value || undefined })
            }
          />
        </Field>
        <p className="sw__hint">
          Format like <code>{DEFAULT_REOPEN_SHORTCUT}</code> or{" "}
          <code>CmdOrCtrl+Alt+P</code>. Takes effect after restarting cenno.
        </p>
      </section>

      <section className="sw__section">
        <h2>Defaults</h2>
        <Field label="Default flow (when an agent doesn't set one)">
          <select
            value={cfg.defaults?.flow ?? "question"}
            onChange={(e) => props.onPatchDefaults({ flow: e.target.value })}
          >
            {FLOWS.map((f) => (
              <option key={f} value={f}>
                {f}
              </option>
            ))}
          </select>
        </Field>
      </section>
    </>
  );
}

/* ─────────────────────────── Guardian tab ────────────────────────── */

const BYTE_UNITS = ["B", "kB", "MB", "GB", "TB", "PB", "EB"] as const;

/** Format an IPC decimal byte string without crossing Number's safe-integer boundary. */
export function formatGuardianBytes(raw: string): string {
  let bytes: bigint;
  try {
    bytes = BigInt(raw);
  } catch {
    return "Unavailable";
  }

  const negative = bytes < 0n;
  const magnitude = negative ? -bytes : bytes;
  let divisor = 1n;
  let unit = 0;
  while (unit < BYTE_UNITS.length - 1 && magnitude >= divisor * 1000n) {
    divisor *= 1000n;
    unit += 1;
  }

  if (unit === 0) return `${bytes.toString()} B`;

  const hundredths = (magnitude * 100n + divisor / 2n) / divisor;
  const whole = hundredths / 100n;
  const fraction = (hundredths % 100n).toString().padStart(2, "0").replace(/0+$/, "");
  return `${negative ? "−" : ""}${whole.toString()}${fraction ? `.${fraction}` : ""} ${BYTE_UNITS[unit]}`;
}

function formatBasisPoints(basisPoints: number): string {
  if (!Number.isFinite(basisPoints)) return "Unavailable";
  return `${(basisPoints / 100).toFixed(2).replace(/\.00$/, "")}%`;
}

function formatObservation(value: string): string {
  const observed = new Date(value);
  if (Number.isNaN(observed.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(observed);
}

function guardianError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Guardian data is unavailable.";
}

function isSafeGuardianRelativePath(value: string): boolean {
  return value.length > 0 &&
    !value.startsWith("/") &&
    !value.startsWith("~") &&
    !/^[A-Za-z]:[\\/]/.test(value) &&
    !value.split(/[\\/]+/).includes("..");
}

function GuardianTab({
  cfg,
  onPersist,
}: {
  cfg: RawConfig;
  onPersist: (next: RawConfig) => Promise<
    { ok: true } | { ok: false; error: string }
  >;
}) {
  const [status, setStatus] = useState<GuardianLoad<GuardianStatusResponse>>({
    state: "loading",
  });
  const [diagnostics, setDiagnostics] = useState<
    GuardianLoad<GuardianDiagnosticsResponse>
  >({ state: "loading" });
  const [runs, setRuns] = useState<GuardianLoad<GuardianRunsResponse>>({
    state: "loading",
  });
  const [monitoringPolicy, setMonitoringPolicy] = useState<
    GuardianLoad<GuardianMonitoringPolicyResponse>
  >({ state: "loading" });
  const [securityStatus, setSecurityStatus] = useState<
    GuardianLoad<GuardianSecurityStatusResponse>
  >({ state: "loading" });
  const [policyDraft, setPolicyDraft] = useState<GuardianMonitoringPolicyDraft | null>(null);
  const [policySaving, setPolicySaving] = useState(false);
  const [policyMessage, setPolicyMessage] = useState<
    { kind: "error" | "saved"; text: string } | null
  >(null);
  const [runAction, setRunAction] = useState<{
    authorizationId: string;
    kind: "start" | "cancel";
  } | null>(null);
  const [runActionError, setRunActionError] = useState<string | null>(null);
  const [notificationError, setNotificationError] = useState<string | null>(null);
  const [notificationSaving, setNotificationSaving] = useState(false);
  const notificationSaveInFlight = useRef(false);
  const refreshGeneration = useRef(0);
  const runActionInFlight = useRef(new Set<string>());

  const refresh = useCallback(async () => {
    const generation = ++refreshGeneration.current;
    const current = () => refreshGeneration.current === generation;
    setStatus({ state: "loading" });
    setDiagnostics({ state: "loading" });
    setRuns({ state: "loading" });
    setMonitoringPolicy({ state: "loading" });
    setSecurityStatus({ state: "loading" });

    const statusRequest = invoke<GuardianStatusResponse>("guardian_get_status")
      .then((data) => current() && setStatus({ state: "ready", data }))
      .catch((error) => current() && setStatus({ state: "error", message: guardianError(error) }));
    const diagnosticsRequest = invoke<GuardianDiagnosticsResponse>("guardian_get_diagnostics")
      .then((data) => current() && setDiagnostics({ state: "ready", data }))
      .catch((error) => current() && setDiagnostics({ state: "error", message: guardianError(error) }));
    const runsRequest = invoke<GuardianRunsResponse>("guardian_get_runs")
      .then((data) => current() && setRuns({ state: "ready", data }))
      .catch((error) => current() && setRuns({ state: "error", message: guardianError(error) }));
    const policyRequest = invoke<GuardianMonitoringPolicyResponse>("guardian_get_monitoring_policy")
      .then((data) => {
        if (!current()) return;
        setMonitoringPolicy({ state: "ready", data });
        setPolicyDraft({
          warningBytes: data.policy.warningBytes,
          criticalBytes: data.policy.criticalBytes,
          warningBasisPoints: String(data.policy.warningBasisPoints),
          criticalBasisPoints: String(data.policy.criticalBasisPoints),
        });
      })
      .catch((error) => {
        if (!current()) return;
        setMonitoringPolicy({ state: "error", message: guardianError(error) });
        setPolicyDraft(null);
      });
    const securityRequest = invoke<GuardianSecurityStatusResponse>("guardian_get_security_status")
      .then((data) => current() && setSecurityStatus(data
        ? { state: "ready", data }
        : { state: "error", message: "Guardian security status is unavailable." }))
      .catch((error) => current() && setSecurityStatus({ state: "error", message: guardianError(error) }));
    if (current()) setPolicyMessage(null);
    await Promise.all([statusRequest, diagnosticsRequest, runsRequest, policyRequest, securityRequest]);
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const refreshing =
    status.state === "loading" ||
    diagnostics.state === "loading" ||
    runs.state === "loading" ||
    monitoringPolicy.state === "loading" ||
    securityStatus.state === "loading";

  async function saveMonitoringPolicy() {
    if (monitoringPolicy.state !== "ready" || !policyDraft) return;
    const validation = validateMonitoringPolicyDraft(policyDraft);
    if (typeof validation === "string") {
      setPolicyMessage({ kind: "error", text: validation });
      return;
    }
    setPolicySaving(true);
    setPolicyMessage(null);
    try {
      const updated = await invoke<GuardianMonitoringPolicyResponse>(
        "guardian_update_monitoring_policy",
        { expectedRevision: monitoringPolicy.data.revision, policy: validation },
      );
      setMonitoringPolicy({ state: "ready", data: updated });
      setPolicyDraft({
        warningBytes: updated.policy.warningBytes,
        criticalBytes: updated.policy.criticalBytes,
        warningBasisPoints: String(updated.policy.warningBasisPoints),
        criticalBasisPoints: String(updated.policy.criticalBasisPoints),
      });
      setPolicyMessage({ kind: "saved", text: `Saved as Guardian policy revision ${updated.revision}.` });
    } catch (error) {
      const message = guardianError(error);
      setPolicyMessage({
        kind: "error",
        text: /revision conflict/i.test(message)
          ? `${message}. Reload Guardian settings and try again.`
          : message,
      });
    } finally {
      setPolicySaving(false);
    }
  }

  async function runCommand(
    kind: "start" | "cancel",
    authorizationId: string,
  ) {
    const key = `${kind}:${authorizationId}`;
    if (runActionInFlight.current.has(key)) return;
    const run = runs.state === "ready"
      ? runs.data.runs.find((candidate) => candidate.authorizationId === authorizationId)
      : undefined;
    const expectedStatus = kind === "start" ? "imported" : "running";
    if (!run || run.status !== expectedStatus) {
      setRunActionError("The Guardian run state changed. Refresh the journal before retrying.");
      await refresh();
      return;
    }
    runActionInFlight.current.add(key);
    setRunAction({ authorizationId, kind });
    setRunActionError(null);
    try {
      if (kind === "start") {
        await invoke<GuardianStartScanResponse>("guardian_start_scan", {
          authorizationId,
        });
      } else {
        await invoke<GuardianCancelRunResponse>("guardian_cancel_run", {
          authorizationId,
        });
      }
      await refresh();
    } catch (error) {
      setRunActionError(`${guardianError(error)} The run was not retried automatically; refresh the Guardian journal before trying again.`);
      await refresh();
    } finally {
      runActionInFlight.current.delete(key);
      setRunAction(null);
    }
  }

  async function toggleNotifications(enabled: boolean) {
    if (notificationSaveInFlight.current) return;
    notificationSaveInFlight.current = true;
    setNotificationSaving(true);
    setNotificationError(null);
    try {
      if (enabled) {
        const granted = await invoke<boolean>("guardian_request_notification_permission");
        if (!granted) {
          setNotificationError("macOS notification permission was not granted.");
          return;
        }
      }
      const result = await onPersist({
        ...cfg,
        guardian: { ...cfg.guardian, notifications_enabled: enabled },
      });
      if (!result.ok) {
        setNotificationError(
          `Could not save notification preference: ${result.error}. The previous setting is still active.`,
        );
      }
    } catch (error) {
      setNotificationError(
        enabled
          ? guardianError(error)
          : `Could not save notification preference: ${guardianError(error)}`,
      );
    } finally {
      notificationSaveInFlight.current = false;
      setNotificationSaving(false);
    }
  }

  return (
    <div className="guardian" aria-labelledby="guardian-title">
      <header className="guardian__intro">
        <div>
          <p className="guardian__eyebrow">Read-only disk observation</p>
          <h1 id="guardian-title">Disk Guardian</h1>
          <p className="guardian__lede">
            Current storage pressure and bounded diagnostics. Guardian cannot remove or
            change files from this screen.
          </p>
        </div>
        <button
          type="button"
          className="sw__btn sw__btn--ghost guardian__refresh"
          onClick={() => void refresh()}
          disabled={refreshing}
        >
          {refreshing ? "Refreshing…" : "Refresh"}
        </button>
      </header>

      <section className="guardian__section guardian__preferences">
        <h2>Notifications</h2>
        <Toggle
          label="Notify me when disk status changes to warning or critical"
          checked={cfg.guardian?.notifications_enabled === true}
          disabled={notificationSaving}
          onChange={(enabled) => void toggleNotifications(enabled)}
        />
        <p className="sw__hint">
          This is a Cenno presentation preference. Disk thresholds remain owned by
          Guardian and are not changed here. The initial status never triggers an alert.
        </p>
        {notificationError && <p className="sw__err">{notificationError}</p>}
      </section>

      <GuardianMonitoringPolicyEditor
        result={monitoringPolicy}
        draft={policyDraft}
        saving={policySaving}
        message={policyMessage}
        onChange={setPolicyDraft}
        onSave={() => void saveMonitoringPolicy()}
      />

      <GuardianSecurityStatusPanel result={securityStatus} />
      <GuardianStatusPanel result={status} />
      <GuardianDiagnosticsPanel result={diagnostics} />
      <GuardianPreviewEvidencePanel result={diagnostics} />
      <GuardianRunsPanel
        result={runs}
        action={runAction}
        actionError={runActionError}
        onStart={(authorizationId) => void runCommand("start", authorizationId)}
        onCancel={(authorizationId) => void runCommand("cancel", authorizationId)}
      />
    </div>
  );
}

function securityLabel(value: GuardianSecuritySubsystemStatus): string {
  return value.replace(/_/g, " ");
}

function GuardianSecurityKeyFact({ label, value }: {
  label: string;
  value?: GuardianSecurityKey | null;
}) {
  if (!value) return null;
  return <div><dt>{label}</dt><dd><code>{value.publicKeyFingerprint}</code></dd></div>;
}

function GuardianSecurityStatusPanel({
  result,
}: {
  result: GuardianLoad<GuardianSecurityStatusResponse>;
}) {
  if (result.state === "loading") {
    return <GuardianNotice title="Security status" message="Loading Guardian-reported security state…" />;
  }
  if (result.state === "error") {
    return <GuardianNotice title="Security status unavailable" message={result.message} error />;
  }
  const { execution, personalContractsAuthority, executorIdentity, profileLifecycle } = result.data;
  return (
    <section className="guardian__section guardian__security" aria-labelledby="guardian-security-heading">
      <div className="guardian__heading">
        <div>
          <p className="guardian__eyebrow">Guardian-reported local state</p>
          <h2 id="guardian-security-heading">Security status</h2>
        </div>
        <span className="guardian__completeness">Read only</span>
      </div>
      <p className="sw__muted guardian__security-copy">
        These facts describe what Disk Guardian currently sees. They do not show whether
        Personal Contracts has pinned Guardian or how far its run journal cursor has advanced.
        Open Personal Contracts to verify its Guardian trust and synchronization state.
      </p>
      <div className="guardian__security-grid">
        <article>
          <h3>Execution</h3>
          <p className={`guardian__security-state guardian__security-state--${execution.status}`}>
            {securityLabel(execution.status)}
          </p>
          {execution.code && <code>{execution.code}</code>}
        </article>
        <article>
          <h3>Personal Contracts issuer trusted by Guardian</h3>
          <p className={`guardian__security-state guardian__security-state--${personalContractsAuthority.status}`}>
            {securityLabel(personalContractsAuthority.status)}
          </p>
          {personalContractsAuthority.code && <code>{personalContractsAuthority.code}</code>}
          {personalContractsAuthority.generation !== undefined && (
            <dl className="guardian__security-facts">
              <div><dt>Trust generation</dt><dd>{personalContractsAuthority.generation}</dd></div>
              <GuardianSecurityKeyFact label="Active fingerprint" value={personalContractsAuthority.activeKey} />
              <GuardianSecurityKeyFact label="Pending fingerprint" value={personalContractsAuthority.pendingKey} />
              <div><dt>Historical keys</dt><dd>{personalContractsAuthority.retiredKeyCount} retired · {personalContractsAuthority.revokedKeyCount} revoked</dd></div>
            </dl>
          )}
        </article>
        <article>
          <h3>Guardian executor identity</h3>
          <p className={`guardian__security-state guardian__security-state--${executorIdentity.status}`}>
            {securityLabel(executorIdentity.status)}
          </p>
          {executorIdentity.code && <code>{executorIdentity.code}</code>}
          {executorIdentity.generation !== undefined && (
            <dl className="guardian__security-facts">
              <div><dt>Identity generation</dt><dd>{executorIdentity.generation}</dd></div>
              <div><dt>Executor</dt><dd>{executorIdentity.executorId}</dd></div>
              <GuardianSecurityKeyFact label="Active fingerprint" value={executorIdentity.activeKey} />
              <GuardianSecurityKeyFact label="Pending fingerprint" value={executorIdentity.pendingKey} />
            </dl>
          )}
        </article>
        <article>
          <h3>Accepted profile lifecycle</h3>
          <p className={`guardian__security-state guardian__security-state--${profileLifecycle.status}`}>
            {securityLabel(profileLifecycle.status)}
          </p>
          {profileLifecycle.code && <code>{profileLifecycle.code}</code>}
          {profileLifecycle.head && (
            <dl className="guardian__security-facts">
              <div><dt>Head generation</dt><dd>{profileLifecycle.head.generation}</dd></div>
              <div><dt>Head digest</dt><dd><code>{profileLifecycle.head.digest}</code></dd></div>
              <div><dt>Profiles</dt><dd>{profileLifecycle.activeProfileCount} active · {profileLifecycle.retiredProfileCount} retired</dd></div>
            </dl>
          )}
        </article>
      </div>
    </section>
  );
}

function validateMonitoringPolicyDraft(
  draft: GuardianMonitoringPolicyDraft,
): GuardianMonitoringPolicy | string {
  const canonicalBytes = /^(0|[1-9]\d*)$/;
  if (
    draft.warningBytes.length > 40 ||
    draft.criticalBytes.length > 40 ||
    !canonicalBytes.test(draft.warningBytes) ||
    !canonicalBytes.test(draft.criticalBytes)
  ) {
    return "Byte thresholds must be canonical decimal strings without signs, spaces, or leading zeroes.";
  }
  if (BigInt(draft.criticalBytes) > BigInt(draft.warningBytes)) {
    return "Critical bytes must be no greater than warning bytes.";
  }
  const canonicalInteger = /^(0|[1-9]\d*)$/;
  if (
    !canonicalInteger.test(draft.warningBasisPoints) ||
    !canonicalInteger.test(draft.criticalBasisPoints)
  ) {
    return "Basis points must be integers from 0 through 10000.";
  }
  const warningBasisPoints = Number(draft.warningBasisPoints);
  const criticalBasisPoints = Number(draft.criticalBasisPoints);
  if (
    warningBasisPoints > 10_000 ||
    criticalBasisPoints > 10_000 ||
    !Number.isSafeInteger(warningBasisPoints) ||
    !Number.isSafeInteger(criticalBasisPoints)
  ) {
    return "Basis points must be integers from 0 through 10000.";
  }
  if (criticalBasisPoints > warningBasisPoints) {
    return "Critical basis points must be no greater than warning basis points.";
  }
  return {
    warningBytes: draft.warningBytes,
    criticalBytes: draft.criticalBytes,
    warningBasisPoints,
    criticalBasisPoints,
  };
}

function GuardianMonitoringPolicyEditor({
  result,
  draft,
  saving,
  message,
  onChange,
  onSave,
}: {
  result: GuardianLoad<GuardianMonitoringPolicyResponse>;
  draft: GuardianMonitoringPolicyDraft | null;
  saving: boolean;
  message: { kind: "error" | "saved"; text: string } | null;
  onChange: (draft: GuardianMonitoringPolicyDraft) => void;
  onSave: () => void;
}) {
  if (result.state === "loading") {
    return <GuardianNotice title="Monitoring thresholds" message="Loading Guardian policy…" />;
  }
  if (result.state === "error" || !draft) {
    return <GuardianNotice
      title="Monitoring thresholds unavailable"
      message={result.state === "error" ? result.message : "Guardian policy is unavailable."}
      error
    />;
  }
  const field = (key: keyof GuardianMonitoringPolicyDraft, label: string) => (
    <label>
      <span>{label}</span>
      <input
        type="text"
        inputMode="numeric"
        aria-label={label}
        value={draft[key]}
        onChange={(event) => onChange({ ...draft, [key]: event.target.value })}
        autoComplete="off"
        spellCheck={false}
      />
    </label>
  );
  return (
    <section className="guardian__section guardian__policy" aria-labelledby="guardian-policy-heading">
      <div className="guardian__heading">
        <div>
          <p className="guardian__eyebrow">Guardian-owned policy</p>
          <h2 id="guardian-policy-heading">Monitoring thresholds</h2>
        </div>
        <span className="guardian__completeness">Revision {result.data.revision}</span>
      </div>
      <p className="sw__muted guardian__policy-copy">
        Guardian owns and enforces this monitoring policy. Cenno can edit only these four
        threshold values; targets, paths, schedules, retention, trend rules, and cleanup
        remain unavailable here.
      </p>
      <div className="guardian__policy-grid">
        {field("warningBytes", "Warning bytes")}
        {field("criticalBytes", "Critical bytes")}
        {field("warningBasisPoints", "Warning basis points")}
        {field("criticalBasisPoints", "Critical basis points")}
      </div>
      <div className="guardian__policy-actions">
        <button type="button" className="sw__btn" disabled={saving} onClick={onSave}>
          {saving ? "Saving…" : "Save thresholds"}
        </button>
        <span className={message?.kind === "error" ? "sw__err" : "sw__muted"}>
          {message?.text ?? "10,000 basis points equals 100%."}
        </span>
      </div>
    </section>
  );
}

function GuardianStatusPanel({
  result,
}: {
  result: GuardianLoad<GuardianStatusResponse>;
}) {
  if (result.state === "loading") {
    return <GuardianNotice title="Disk status" message="Loading current observation…" />;
  }
  if (result.state === "error") {
    return <GuardianNotice title="Disk status unavailable" message={result.message} error />;
  }

  const { sample, evaluation } = result.data;
  return (
    <section className="guardian__section" aria-labelledby="guardian-status-heading">
      <div className="guardian__heading">
        <div>
          <p className="guardian__eyebrow">Disk status</p>
          <h2 id="guardian-status-heading">Latest observation</h2>
        </div>
        <span className={`guardian__severity guardian__severity--${evaluation.severity}`}>
          {evaluation.severity}
        </span>
      </div>

      <div className="guardian__metrics" aria-label="Current disk status">
        <GuardianMetric
          label="Available space"
          value={formatGuardianBytes(sample.availableBytes)}
          detail={`of ${formatGuardianBytes(sample.totalBytes)} total`}
        />
        <GuardianMetric
          label="Available percent"
          value={formatBasisPoints(evaluation.availableBasisPoints)}
          detail="Available to the current user"
        />
        <GuardianMetric
          label="Observed"
          value={formatObservation(sample.observedAt)}
          detail={`Target ${sample.targetId}`}
        />
      </div>

      <div className="guardian__reasons">
        <h3>Reasons</h3>
        {evaluation.reasons.length ? (
          <ul>{evaluation.reasons.map((reason) => <li key={reason}>{reason}</li>)}</ul>
        ) : (
          <p>No pressure reasons reported.</p>
        )}
      </div>

      {evaluation.trend && (
        <div className="guardian__trend" aria-label="Status trend">
          <span>Recent drop</span>
          <strong>{formatGuardianBytes(evaluation.trend.dropBytes)}</strong>
          <span>across {formatDuration(evaluation.trend.windowMs)}</span>
          <span>· estimated time to critical {formatDurationString(evaluation.trend.timeToCriticalMs)}</span>
        </div>
      )}
    </section>
  );
}

function GuardianDiagnosticsPanel({
  result,
}: {
  result: GuardianLoad<GuardianDiagnosticsResponse>;
}) {
  if (result.state === "loading") {
    return <GuardianNotice title="Diagnostics" message="Loading bounded diagnostics…" />;
  }
  if (result.state === "error") {
    return <GuardianNotice title="Diagnostics unavailable" message={result.message} error />;
  }

  const { report, growth } = result.data;
  return (
    <section className="guardian__section" aria-labelledby="guardian-diagnostics-heading">
      <div className="guardian__heading">
        <div>
          <p className="guardian__eyebrow">Diagnostics</p>
          <h2 id="guardian-diagnostics-heading">Bounded scan summary</h2>
        </div>
        <span className={`guardian__completeness${report.partial ? " guardian__completeness--partial" : ""}`}>
          {report.partial ? "Partial" : "Complete"}
        </span>
      </div>

      <div className="guardian__metrics guardian__metrics--diagnostics">
        <GuardianMetric label="Observed size" value={formatGuardianBytes(report.totals.bytes)} detail="Across configured scopes" />
        <GuardianMetric label="Files" value={report.totals.files.toLocaleString()} detail={`${report.totals.folders.toLocaleString()} folders`} />
        <GuardianMetric label="Errors" value={report.errorCount.toLocaleString()} detail={report.errorCount ? "Some entries could not be read" : "No scan errors"} />
      </div>

      <dl className="guardian__facts">
        <div><dt>Observed</dt><dd>{formatObservation(report.observedAt)}</dd></div>
        <div><dt>Limits hit</dt><dd>{report.limitsHit.length ? report.limitsHit.join(", ") : "None"}</dd></div>
        <div><dt>Readable scopes</dt><dd>{report.scopes.filter((scope) => scope.readable).length} of {report.scopes.length}</dd></div>
      </dl>

      <GuardianGrowth growth={growth} />

      <div className="guardian__candidates">
        <div className="guardian__subheading">
          <h3>Largest observed candidates</h3>
          <span>Relative paths only</span>
        </div>
        {report.topCandidates.length ? (
          <ol>
            {report.topCandidates.map((candidate, index) => (
              <li key={`${candidate.scope}:${candidate.relativePath}:${index}`}>
                <div>
                  <span className="guardian__candidate-kind">{candidate.kind}</span>
                  <strong>{candidate.relativePath}</strong>
                  <span>{candidate.scope}</span>
                </div>
                <b>{formatGuardianBytes(candidate.bytes)}</b>
              </li>
            ))}
          </ol>
        ) : (
          <p className="sw__muted">No candidates were reported.</p>
        )}
      </div>
    </section>
  );
}

function GuardianGrowth({ growth }: { growth: GuardianDiagnosticsResponse["growth"] }) {
  if (!growth) {
    return <div className="guardian__growth"><h3>Growth</h3><p>No earlier observation is available for comparison.</p></div>;
  }
  return (
    <div className="guardian__growth">
      <h3>Growth since {formatObservation(growth.previousObservedAt)}</h3>
      <p>
        <strong>{formatGuardianBytes(growth.totals.bytesDelta)}</strong> · {growth.totals.filesDelta.toLocaleString()} files
      </p>
      {growth.candidates.length > 0 && (
        <ul>
          {growth.candidates.map((candidate, index) => (
            <li key={`${candidate.scope}:${candidate.relativePath}:${index}`}>
              <span>{candidate.relativePath}</span>
              <strong>{formatGuardianBytes(candidate.deltaBytes)}</strong>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function GuardianPreviewEvidencePanel({
  result,
}: {
  result: GuardianLoad<GuardianDiagnosticsResponse>;
}) {
  if (result.state !== "ready") return null;

  const { preview } = result.data;
  // The Guardian protocol is bounded, but keep the presentation bounded too so
  // an unexpectedly large response cannot turn this review surface into a log viewer.
  const candidates = preview.candidates
    .filter((candidate) => isSafeGuardianRelativePath(candidate.relativePath))
    .slice(0, 10);

  return (
    <section className="guardian__section" aria-labelledby="guardian-preview-heading">
      <div className="guardian__heading">
        <div>
          <p className="guardian__eyebrow">Read-only evidence</p>
          <h2 id="guardian-preview-heading">Diagnostic preview</h2>
        </div>
      </div>
      <p className="sw__muted guardian__preview-copy">
        This immutable diagnostic evidence is not approval and does not perform cleanup.
      </p>
      <dl className="guardian__facts guardian__preview-facts">
        <div><dt>Generated from</dt><dd>{formatObservation(preview.generatedFrom)}</dd></div>
        <div><dt>Canonical preview reference</dt><dd><code className="guardian__preview-ref" title={preview.previewRef}>{preview.previewRef}</code></dd></div>
        <div><dt>Estimated bytes</dt><dd>{formatGuardianBytes(preview.estimatedBytes)}</dd></div>
      </dl>
      <div className="guardian__candidates guardian__preview-candidates">
        <div className="guardian__subheading">
          <h3>Preview candidates</h3>
          <span>Relative paths only</span>
        </div>
        {candidates.length ? (
          <ol>
            {candidates.map((candidate, index) => (
              <li key={`${candidate.scope}:${candidate.relativePath}:${index}`}>
                <div>
                  <span className="guardian__candidate-kind">{candidate.kind}</span>
                  <strong>{candidate.relativePath}</strong>
                  <span>{candidate.scope}</span>
                </div>
                <b>{formatGuardianBytes(candidate.bytes)}</b>
              </li>
            ))}
          </ol>
        ) : (
          <p className="sw__muted">This diagnostic preview contains no safe relative candidates.</p>
        )}
      </div>
    </section>
  );
}

function GuardianRunsPanel({
  result,
  action,
  actionError,
  onStart,
  onCancel,
}: {
  result: GuardianLoad<GuardianRunsResponse>;
  action: { authorizationId: string; kind: "start" | "cancel" } | null;
  actionError: string | null;
  onStart: (authorizationId: string) => void;
  onCancel: (authorizationId: string) => void;
}) {
  if (result.state === "loading") {
    return <GuardianNotice title="Authorized runs" message="Loading run journal…" />;
  }
  if (result.state === "error") {
    return <GuardianNotice title="Run journal unavailable" message={result.message} error />;
  }

  return (
    <section className="guardian__section" aria-labelledby="guardian-runs-heading">
      <div className="guardian__heading">
        <div>
          <p className="guardian__eyebrow">Authorized activity</p>
          <h2 id="guardian-runs-heading">Run journal</h2>
        </div>
      </div>
      <p className="sw__muted guardian__approval-copy">
        Review and approve new scan authorizations in Personal Contracts. Cenno only
        starts authorizations already imported by Guardian.
      </p>
      {actionError && <p className="sw__err guardian__run-error">{actionError}</p>}
      {result.data.runs.length ? (
        <ol className="guardian__runs">
          {result.data.runs.map((run) => {
            const pending = action?.authorizationId === run.authorizationId;
            return (
              <li key={run.authorizationId}>
                <div className="guardian__run-main">
                  <div className="guardian__run-title">
                    <strong>{run.profileId}</strong>
                    <span className={`guardian__run-status guardian__run-status--${run.status}`}>
                      {run.status}
                    </span>
                  </div>
                  <dl className="guardian__run-facts">
                    <div><dt>Authorization</dt><dd>{run.authorizationId}</dd></div>
                    <div><dt>Profile version</dt><dd>{String(run.profileVersion)}</dd></div>
                    <div><dt>Progress</dt><dd>{String(run.progress)}%</dd></div>
                    <div><dt>Expires</dt><dd>{formatObservation(run.expiresAt)}</dd></div>
                    {run.status === "expired" && run.finishedAt && (
                      <div><dt>Observed expired</dt><dd>{formatObservation(run.finishedAt)}</dd></div>
                    )}
                    {run.status === "expired" && (
                      <div><dt>Reason</dt><dd>Authorization expired before it could start.</dd></div>
                    )}
                  </dl>
                  {run.status === "imported" && (
                    <p className="sw__muted guardian__approval-copy">
                      Delivered to Guardian and signature-verified. An explicit
                      Start scan action is still required.
                    </p>
                  )}
                </div>
                {run.status === "imported" && (
                  <button
                    type="button"
                    className="sw__btn guardian__run-action"
                    disabled={pending}
                    onClick={() => onStart(run.authorizationId)}
                  >
                    {pending && action.kind === "start" ? "Starting…" : "Start scan"}
                  </button>
                )}
                {run.status === "running" && (
                  <button
                    type="button"
                    className="sw__btn sw__btn--ghost guardian__run-action"
                    disabled={pending}
                    onClick={() => onCancel(run.authorizationId)}
                  >
                    {pending && action.kind === "cancel" ? "Cancelling…" : "Cancel"}
                  </button>
                )}
              </li>
            );
          })}
        </ol>
      ) : (
        <p className="sw__muted">No imported authorizations or recorded runs.</p>
      )}
    </section>
  );
}

function GuardianMetric({ label, value, detail }: { label: string; value: string; detail: string }) {
  return <article className="guardian__metric"><span>{label}</span><strong>{value}</strong><small>{detail}</small></article>;
}

function GuardianNotice({ title, message, error = false }: { title: string; message: string; error?: boolean }) {
  return <section className={`guardian__notice${error ? " guardian__notice--error" : ""}`}><h2>{title}</h2><p>{message}</p></section>;
}

function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "an unknown window";
  if (ms >= 86_400_000) return `${Math.round(ms / 86_400_000)} days`;
  if (ms >= 3_600_000) return `${Math.round(ms / 3_600_000)} hours`;
  return `${Math.round(ms / 60_000)} minutes`;
}

function formatDurationString(raw: string): string {
  try {
    const ms = BigInt(raw);
    if (ms < 0n) return "unknown";
    const day = 86_400_000n;
    const hour = 3_600_000n;
    if (ms >= day) return `${((ms + day / 2n) / day).toString()} days`;
    if (ms >= hour) return `${((ms + hour / 2n) / hour).toString()} hours`;
    return `${((ms + 30_000n) / 60_000n).toString()} minutes`;
  } catch {
    return "unknown";
  }
}

/* ─────────────────────── Output device picker ─────────────────────── */

function OutputDevicePicker({
  value,
  onChange,
}: {
  value: string;
  onChange: (v: string) => void;
}) {
  const [devices, setDevices] = useState<string[]>([]);

  useEffect(() => {
    invoke<string[]>("list_audio_outputs")
      .then(setDevices)
      .catch(() => setDevices([]));
  }, []);

  // A previously-saved device that isn't currently present (unplugged) should
  // still appear, so the user sees what's configured rather than a silent reset.
  const options = value && !devices.includes(value) ? [value, ...devices] : devices;

  return (
    <Field label="Output device (Supertonic voice)">
      <select value={value} onChange={(e) => onChange(e.target.value)}>
        <option value="">System default output</option>
        {options.map((d) => (
          <option key={d} value={d}>
            {d}
            {value === d && !devices.includes(d) ? " (not connected)" : ""}
          </option>
        ))}
      </select>
    </Field>
  );
}

/* ─────────────────────── Supertonic model block ───────────────────── */

function SupertonicModel({
  customPath,
  onPatchTts,
}: {
  customPath: string;
  onPatchTts: (p: Partial<NonNullable<RawConfig["tts"]>>) => void;
}) {
  const [status, setStatus] = useState<ModelStatus | null>(null);
  const [pct, setPct] = useState<number | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const refresh = useCallback(() => {
    invoke<ModelStatus>("tts_model_status").then(setStatus).catch(() => {});
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    const un = listen<{ status: string; pct?: number; message?: string }>(
      "tts-download-progress",
      (e) => {
        const p = e.payload;
        if (p.status === "downloading") {
          setPct(p.pct ?? 0);
          setErr(null);
        } else if (p.status === "done") {
          setPct(100);
          window.setTimeout(() => {
            setPct(null);
            refresh();
          }, 600);
        } else if (p.status === "error") {
          setErr(p.message ?? "download failed");
          setPct(null);
        }
      },
    );
    return () => {
      un.then((f) => f());
    };
  }, [refresh]);

  const downloading = pct !== null;
  const sizeGb = status ? (status.total_bytes / 1e9).toFixed(2) : "0.40";

  async function download() {
    setErr(null);
    setPct(0);
    try {
      await invoke("tts_download_model");
    } catch {
      /* the error arrives via the progress event */
    }
  }

  async function deleteModel() {
    setErr(null);
    try {
      await invoke("tts_delete_model");
      refresh();
    } catch (e) {
      setErr(typeof e === "string" ? e : "delete failed");
    }
  }

  return (
    <div className="sw__model">
      <div className="sw__model-status">
        {status?.present ? (
          <span className="sw__ok">
            ✓ Voice model installed{status.custom ? " (custom path)" : ` · ${sizeGb} GB`}
          </span>
        ) : (
          <span className="sw__warn">
            Model not installed — Supertonic falls back to the system voice until
            you download it.
          </span>
        )}
      </div>

      {!status?.present && !downloading && !status?.custom && (
        <button className="sw__btn" onClick={download}>
          Download voice model (~{sizeGb} GB)
        </button>
      )}

      {status?.present && !status.custom && !downloading && (
        <div className="sw__model-actions">
          <button className="sw__btn sw__btn--ghost" onClick={deleteModel}>
            Delete model
          </button>
          <button className="sw__btn sw__btn--ghost" onClick={download}>
            Re-download
          </button>
        </div>
      )}

      {downloading && (
        <div className="sw__progress" role="progressbar" aria-valuenow={pct ?? 0}>
          <div className="sw__progress-bar" style={{ width: `${pct ?? 0}%` }} />
          <span className="sw__progress-pct">{pct ?? 0}%</span>
        </div>
      )}

      {err && <p className="sw__err">Download failed: {err}</p>}

      <Field label="Custom model path (optional)">
        <input
          type="text"
          placeholder="default: ~/.cenno/models/supertonic-3"
          defaultValue={customPath}
          onBlur={(e) => {
            const v = e.target.value.trim();
            onPatchTts({ model_path: v || undefined });
            window.setTimeout(refresh, 150);
          }}
        />
      </Field>
    </div>
  );
}

/* ───────────────────────── Integration tab ────────────────────────── */

function IntegrationTab({ onCopy }: { onCopy: (t: string) => void }) {
  return (
    <>
      <section className="sw__section">
        <h2>Add cenno to your agent</h2>
        <p className="sw__muted">
          Drop this into your MCP config (e.g. Claude Desktop / Claude Code).
          cenno exposes an <code>ask_user</code> tool.
        </p>
        <CodeBlock text={MCP_SNIPPET} onCopy={onCopy} />
      </section>

      <section className="sw__section">
        <h2>Or install the cenno skill</h2>
        <p className="sw__muted">
          Teaches your agent to ask well — right input kinds, flows, graceful
          timeouts — and can wire cenno into a project for you.
        </p>
        <CodeBlock text={SKILL_INSTALL} onCopy={onCopy} />
      </section>

      <section className="sw__section">
        <h2>Try it from the shell</h2>
        <CodeBlock text="cenno --mcp-stdio" onCopy={onCopy} />
      </section>
    </>
  );
}

/* ───────────────────────────── About tab ──────────────────────────── */

function AboutTab({ onOpen }: { onOpen: (url: string) => Promise<void> }) {
  return (
    <>
      <section className="sw__section">
        <h2>What is cenno?</h2>
        <p className="sw__muted">
          cenno is a macOS menu-bar app that lets MCP-capable AI agents ask you
          questions through small floating panels — without stealing keyboard
          focus. The agent calls <code>ask_user</code>, a panel slides in, and
          your answer comes back as structured data. Every exchange is recorded
          locally.
        </p>
      </section>

      <section className="sw__section">
        <h2>Links</h2>
        <ul className="sw__links">
          <li>
            <button className="sw__link" onClick={() => onOpen("https://github.com/glebis/cenno")}>
              cenno on GitHub →
            </button>
          </li>
          <li>
            <button className="sw__link" onClick={() => onOpen("https://github.com/glebis")}>
              github.com/glebis →
            </button>
          </li>
          <li>
            <button
              className="sw__link"
              onClick={() => onOpen("https://www.linkedin.com/in/glebkalinin/")}
            >
              linkedin.com/in/glebkalinin →
            </button>
          </li>
        </ul>
      </section>

      <footer className="sw__footer">
        Made by Gleb Kalinin · cenno
      </footer>
    </>
  );
}

/* ───────────────────────────── Primitives ─────────────────────────── */

// The cenno wordmark glyph from the site: an arc over a dot (a head nodding).
function CennoMark() {
  return (
    <svg className="sw__mark" width="18" height="20" viewBox="0 0 22 20" aria-hidden="true">
      <path
        d="M 5.6874 7.5765 A 5.5 5.5 0 0 1 16.3126 7.5765"
        fill="none"
        stroke="currentColor"
        strokeWidth="2.5"
      />
      <circle cx="11" cy="16.25" r="2.75" fill="currentColor" />
    </svg>
  );
}

function Toggle({
  label,
  checked,
  disabled = false,
  onChange,
}: {
  label: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="sw__toggle">
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className="sw__track">
        <span className="sw__knob" />
      </span>
      <span className="sw__toggle-label">{label}</span>
    </label>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="sw__field">
      <span className="sw__field-label">{label}</span>
      {children}
    </label>
  );
}

function CodeBlock({ text, onCopy }: { text: string; onCopy: (t: string) => void }) {
  return (
    <div className="sw__code">
      <pre>{text}</pre>
      <button className="sw__copy" onClick={() => onCopy(text)}>
        Copy
      </button>
    </div>
  );
}
