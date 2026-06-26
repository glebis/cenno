/**
 * SettingsWindow — cenno's main window (opened from the tray "cenno settings…").
 *
 * Three tabs:
 *   - Settings    — Voice/TTS (engine, voice, test), behavior toggles, defaults
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
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import "./SettingsWindow.css";

type Tab = "settings" | "integration" | "about";

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
  const [tab, setTab] = useState<Tab>("settings");
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

  // Persist the whole config (preserving unknown blocks) and flash "Saved".
  async function persist(next: RawConfig) {
    setCfg(next);
    try {
      await invoke("save_config_file", { config: next });
      setSaved(true);
      window.setTimeout(() => setSaved(false), 1400);
    } catch (e) {
      console.error("cenno: save_config_file failed", e);
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
          {(["settings", "integration", "about"] as Tab[]).map((t) => (
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
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="sw__toggle">
      <input
        type="checkbox"
        checked={checked}
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
