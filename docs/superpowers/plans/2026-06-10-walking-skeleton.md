# Cenno Walking Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** End-to-end skeleton: `cenno ask "question"` (CLI or MCP stdio) shows a floating panel on macOS, the user types an answer, and the caller receives `{answer, via, elapsed_s}` as JSON.

**Architecture:** Single Tauri 2 process (cull pattern): Rust core owns an in-memory prompt registry and an rmcp MCP server on a Unix socket; the React webview renders prompts and resolves them via a Tauri command. CLI and `--mcp-stdio` bridge are alternate entry modes of the same binary. No SQLite, no voice, no A2UI desugaring yet — those are plans 2–4.

**Tech Stack:** Tauri 2, Rust (tokio, rmcp 1.7, serde, clap), React 18 + TypeScript + Vite, react-markdown. Reference implementation to crib from: `~/ai_projects/cull/src-tauri/src/` (same rmcp major, same socket/bridge pattern).

**Plan sequence:** 1 of 4. Next: rendering (A2UI + tokens), voice (whisper.cpp + BYOK), surfaces & policy (fullscreen, tray, SQLite history).

---

### Task 1: Scaffold Tauri 2 + React app

**Files:**
- Create: entire app via scaffold at repo root (`package.json`, `src/`, `src-tauri/`)
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Scaffold**

Run from `~/ai_projects/cenno`:
```bash
npm create tauri-app@latest . -- --template react-ts --manager npm --yes
npm install
```
If the scaffolder refuses a non-empty directory (we have `docs/` and `.git`), scaffold into `/tmp/cenno-scaffold` and `rsync -a /tmp/cenno-scaffold/ .` excluding `.git`.

- [ ] **Step 2: Set app identity**

In `src-tauri/tauri.conf.json` set:
```json
{
  "productName": "cenno",
  "identifier": "com.glebkalinin.cenno",
  "app": { "windows": [{ "title": "cenno", "width": 720, "height": 520, "visible": true }] }
}
```

- [ ] **Step 3: Verify dev build**

Run: `npm run tauri dev` — expect the default window to open. Quit it.
Run: `cd src-tauri && cargo test` — expect: 0 tests, exit 0.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "chore: scaffold Tauri 2 + React + TS app"
```

---

### Task 2: A2UI React renderer spike (timeboxed: half a day)

Research task, not TDD. Purpose: validate `@a2ui/react@0.10` custom-catalog + theming API before plan 2 commits to it.

**Files:**
- Create: `spike/a2ui/` (throwaway Vite page, not wired into the app)
- Create: `docs/superpowers/research/2026-06-a2ui-react-spike.md`

- [ ] **Step 1: Minimal harness**

```bash
mkdir -p spike/a2ui && cd spike/a2ui && npm init -y && npm i react react-dom @a2ui/react @a2ui/web_core vite @vitejs/plugin-react
```
Render a hardcoded A2UI flat component list (Card → Text → TextField → Button) using the renderer's documented entry point. Consult the package README (`node_modules/@a2ui/react/README.md`) and https://github.com/google/A2UI samples.

- [ ] **Step 2: Answer the four spike questions in the findings doc**

1. Can we register **our own React component** for a catalog type (e.g. replace `Button`)? Show the working code.
2. Do CSS custom properties on a wrapper element style the rendered components (token pipeline viability)?
3. Can the payload be **patched incrementally** (streaming update of one component by id)?
4. Does the renderer pin a spec version, and what breaks if we feed v0.9 payloads?

- [ ] **Step 3: Write decision in findings doc**

One of: **(a)** adopt `@a2ui/react` with custom catalog, or **(b)** fallback — write our own thin interpreter that walks the flat list (the spec names this fallback). Include a 5-line rationale.

- [ ] **Step 4: Commit**

```bash
git add spike docs/superpowers/research && git commit -m "research: A2UI React renderer spike findings"
```

---

### Task 3: Protocol types

**Files:**
- Create: `src-tauri/src/protocol.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod protocol;`)

- [ ] **Step 1: Write the failing test** (bottom of `protocol.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ask_request_roundtrip_with_defaults() {
        let json = r#"{"title":"Check-in","body_md":"How is **focus**?"}"#;
        let req: AskRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title, "Check-in");
        assert!(matches!(req.input.kind, InputKind::Text));
        assert!(matches!(req.urgency, Urgency::Normal));
        assert_eq!(req.timeout_s, 120);
        let back = serde_json::to_string(&req).unwrap();
        assert!(back.contains("\"urgency\":\"normal\""));
    }

    #[test]
    fn answered_response_serializes() {
        let resp = AskResponse::Answered { answer: "ok".into(), via: Via::Text, elapsed_s: 3.2 };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"via\":\"text\""));
    }

    #[test]
    fn timeout_response_serializes() {
        let resp = AskResponse::TimedOut { answered: false, prompt_id: "p_1".into() };
        assert!(serde_json::to_string(&resp).unwrap().contains("\"answered\":false"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd src-tauri && cargo test protocol` — expect: compile error, types not defined.

- [ ] **Step 3: Implement**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Urgency { Low, Normal, High }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind { Text, Voice, VoiceText, Choice, Scale, Confirm, None }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputSpec { #[serde(default = "default_kind")] pub kind: InputKind }
fn default_kind() -> InputKind { InputKind::Text }
impl Default for InputSpec { fn default() -> Self { Self { kind: default_kind() } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskRequest {
    pub title: String,
    #[serde(default)] pub body_md: String,
    #[serde(default)] pub input: InputSpec,
    #[serde(default)] pub choices: Option<Vec<String>>,
    #[serde(default = "default_urgency")] pub urgency: Urgency,
    #[serde(default = "default_timeout")] pub timeout_s: u64,
    #[serde(default)] pub a2ui: Option<serde_json::Value>,
}
fn default_urgency() -> Urgency { Urgency::Normal }
fn default_timeout() -> u64 { 120 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Via { Voice, Text, Choice }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AskResponse {
    Answered { answer: String, via: Via, elapsed_s: f64 },
    TimedOut { answered: bool, prompt_id: String },
}
```

- [ ] **Step 4: Run tests** — `cargo test protocol` — expect: 3 passed.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: protocol types for ask_user contract"`

---

### Task 4: Prompt registry

In-memory pending-prompt store. `ask()` registers a prompt and awaits its oneshot receiver with timeout; `resolve()` is called by the UI.

**Files:**
- Create: `src-tauri/src/registry.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod registry;`)

- [ ] **Step 1: Write the failing tests** (bottom of `registry.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::*;

    fn req() -> AskRequest { serde_json::from_str(r#"{"title":"t","timeout_s":1}"#).unwrap() }

    #[tokio::test]
    async fn resolve_completes_ask() {
        let reg = PromptRegistry::new();
        let reg2 = reg.clone();
        let task = tokio::spawn(async move { reg2.ask(req(), |_id, _req| {}).await });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let id = reg.pending_ids()[0].clone();
        assert!(reg.resolve(&id, "hello".into(), Via::Text));
        match task.await.unwrap() {
            AskResponse::Answered { answer, .. } => assert_eq!(answer, "hello"),
            _ => panic!("expected Answered"),
        }
    }

    #[tokio::test]
    async fn timeout_returns_timed_out_and_keeps_pending() {
        let reg = PromptRegistry::new();
        let resp = reg.ask(req(), |_id, _req| {}).await; // timeout_s = 1
        match resp {
            AskResponse::TimedOut { prompt_id, .. } => assert!(reg.pending_ids().contains(&prompt_id)),
            _ => panic!("expected TimedOut"),
        }
    }

    #[tokio::test]
    async fn resolve_unknown_id_is_false() {
        assert!(!PromptRegistry::new().resolve("nope", "x".into(), Via::Text));
    }
}
```

- [ ] **Step 2: Run** — `cargo test registry` — expect: compile failure.

- [ ] **Step 3: Implement**

```rust
use crate::protocol::*;
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc, time::{Duration, Instant}};
use tokio::sync::oneshot;

#[derive(Clone)]
pub struct PromptRegistry {
    inner: Arc<Mutex<HashMap<String, Pending>>>,
    counter: Arc<std::sync::atomic::AtomicU64>,
}

struct Pending { tx: Option<oneshot::Sender<(String, Via)>>, pub request: AskRequest }

impl PromptRegistry {
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(HashMap::new())), counter: Arc::new(0.into()) }
    }

    /// Registers the prompt, calls `notify(id, req)` (used to emit to the UI),
    /// then awaits the answer or times out. On timeout the prompt STAYS pending
    /// (tray inbox semantics, plan 4 reads these via get_response).
    pub async fn ask(&self, req: AskRequest, notify: impl FnOnce(&str, &AskRequest)) -> AskResponse {
        let n = self.counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let id = format!("p_{n}");
        let (tx, rx) = oneshot::channel();
        self.inner.lock().insert(id.clone(), Pending { tx: Some(tx), request: req.clone() });
        notify(&id, &req);
        let started = Instant::now();
        match tokio::time::timeout(Duration::from_secs(req.timeout_s), rx).await {
            Ok(Ok((answer, via))) => {
                self.inner.lock().remove(&id);
                AskResponse::Answered { answer, via, elapsed_s: started.elapsed().as_secs_f64() }
            }
            _ => AskResponse::TimedOut { answered: false, prompt_id: id },
        }
    }

    pub fn resolve(&self, id: &str, answer: String, via: Via) -> bool {
        let mut map = self.inner.lock();
        match map.get_mut(id).and_then(|p| p.tx.take()) {
            Some(tx) => { let _ = tx.send((answer, via)); true }
            None => false,
        }
    }

    pub fn pending_ids(&self) -> Vec<String> { self.inner.lock().keys().cloned().collect() }
}
```
Add to `src-tauri/Cargo.toml`: `parking_lot = "0.12"`, `tokio = { version = "1", features = ["full"] }`.

- [ ] **Step 4: Run** — `cargo test registry` — expect: 3 passed.

- [ ] **Step 5: Commit** — `git commit -am "feat: in-memory prompt registry with timeout-to-pending semantics"`

---

### Task 5: MCP server on Unix socket

**Files:**
- Create: `src-tauri/src/mcp.rs`
- Modify: `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml` (`rmcp = { version = "1.7", features = ["server", "transport-io"] }`)
- Reference: `~/ai_projects/cull/src-tauri/src/mcp/socket.rs` and `mcp/tools.rs` — same rmcp major; copy the `tool_router!`/serve-on-UnixStream idioms exactly, do not invent macro syntax.

- [ ] **Step 1: Write the failing integration test** (`src-tauri/tests/mcp_socket.rs`)

```rust
use cenno_lib::{mcp::start_socket_server, registry::PromptRegistry, protocol::Via};

#[tokio::test]
async fn ask_user_over_socket_resolves() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("mcp.sock");
    let reg = PromptRegistry::new();
    start_socket_server(sock.clone(), reg.clone()).await.unwrap();

    // auto-answer any prompt after 100ms, like a user typing "yes"
    let reg2 = reg.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            for id in reg2.pending_ids() { reg2.resolve(&id, "yes".into(), Via::Text); }
        }
    });

    // rmcp client over the same socket
    let result = cenno_lib::mcp::test_support::call_ask_user(&sock,
        serde_json::json!({"title": "Deploy?", "timeout_s": 5})).await.unwrap();
    assert_eq!(result["answer"], "yes");
    assert_eq!(result["via"], "text");
}
```
`test_support::call_ask_user` is a small helper in `mcp.rs` behind `#[cfg(any(test, feature = "test-support"))]`: connects an rmcp client to the UnixStream and calls the tool (crib client setup from rmcp's own examples; cull's `--mcp-stdio` bridge shows the stream wiring).

- [ ] **Step 2: Run** — `cargo test --test mcp_socket` — expect: compile failure.

- [ ] **Step 3: Implement `mcp.rs`**

Shape (exact macro syntax from cull's `tools.rs`):

```rust
use crate::{protocol::*, registry::PromptRegistry};
use std::path::PathBuf;

#[derive(Clone)]
pub struct CennoServer { pub registry: PromptRegistry }

// tool_router! / #[tool] block defining exactly one tool:
//   ask_user(params: AskRequest) -> serde_json::Value
// body:
//   let resp = self.registry.ask(params, |id, req| notify_ui(id, req)).await;
//   Ok(serde_json::to_value(resp)?)
// For the skeleton, notify_ui is a callback field on CennoServer
// (set to a Tauri event emitter in Task 6, a no-op in tests).

pub async fn start_socket_server(path: PathBuf, registry: PromptRegistry) -> anyhow::Result<()> {
    if path.exists() { std::fs::remove_file(&path)?; }
    let listener = tokio::net::UnixListener::bind(&path)?;
    tokio::spawn(async move {
        loop {
            if let Ok((stream, _)) = listener.accept().await {
                let server = CennoServer { registry: registry.clone() };
                tokio::spawn(async move { /* rmcp .serve(stream) per cull socket.rs */ });
            }
        }
    });
    Ok(())
}
```
Also: rename the lib crate target to `cenno_lib` in `Cargo.toml` (`[lib] name = "cenno_lib"`) so integration tests can import it. Add `tempfile = "3"` and `anyhow = "1"` to dev/main deps.

- [ ] **Step 4: Run** — `cargo test --test mcp_socket` — expect: 1 passed.

- [ ] **Step 5: Commit** — `git commit -am "feat: MCP server with ask_user tool on Unix socket"`

---

### Task 6: Panel window + React prompt UI

**Files:**
- Modify: `src-tauri/src/lib.rs` (Tauri setup: socket server start, panel window, `answer_prompt` command)
- Create: `src/PromptPanel.tsx`, `src/PromptPanel.test.tsx`
- Modify: `src/App.tsx`, `package.json` (add `react-markdown`, `vitest`, `@testing-library/react`, `jsdom`)

- [ ] **Step 1: Write the failing component test** (`src/PromptPanel.test.tsx`)

```tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import PromptPanel from "./PromptPanel";

describe("PromptPanel", () => {
  const prompt = { id: "p_1", title: "Check-in", body_md: "How is **focus**?", input: { kind: "text" } };

  it("renders title and markdown body", () => {
    render(<PromptPanel prompt={prompt} onAnswer={() => {}} />);
    expect(screen.getByText("Check-in")).toBeTruthy();
    expect(screen.getByText("focus").tagName).toBe("STRONG");
  });

  it("submits typed answer", () => {
    const onAnswer = vi.fn();
    render(<PromptPanel prompt={prompt} onAnswer={onAnswer} />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "deep" } });
    fireEvent.click(screen.getByRole("button", { name: /send/i }));
    expect(onAnswer).toHaveBeenCalledWith("p_1", "deep", "text");
  });
});
```

- [ ] **Step 2: Run** — `npx vitest run` (configure `environment: "jsdom"` in `vite.config.ts` test block) — expect: FAIL, module missing.

- [ ] **Step 3: Implement `PromptPanel.tsx`**

```tsx
import { useState } from "react";
import ReactMarkdown from "react-markdown";

export interface Prompt { id: string; title: string; body_md: string; input: { kind: string } }

export default function PromptPanel({ prompt, onAnswer }:
  { prompt: Prompt; onAnswer: (id: string, answer: string, via: "text") => void }) {
  const [text, setText] = useState("");
  return (
    <div className="prompt-panel">
      <h1>{prompt.title}</h1>
      <ReactMarkdown>{prompt.body_md}</ReactMarkdown>
      <input role="textbox" value={text} onChange={e => setText(e.target.value)}
        onKeyDown={e => e.key === "Enter" && onAnswer(prompt.id, text, "text")} autoFocus />
      <button onClick={() => onAnswer(prompt.id, text, "text")}>Send</button>
    </div>
  );
}
```

- [ ] **Step 4: Run** — `npx vitest run` — expect: 2 passed.

- [ ] **Step 5: Wire Rust side in `lib.rs`**

```rust
#[tauri::command]
fn answer_prompt(state: tauri::State<PromptRegistry>, id: String, answer: String, via: String) -> bool {
    let via = match via.as_str() { "voice" => Via::Voice, "choice" => Via::Choice, _ => Via::Text };
    state.resolve(&id, answer, via)
}
```
In `setup`: create the registry, manage it as state, start the socket server at `app.path().app_data_dir()?.join("mcp.sock")`, and set the server's notify callback to emit `app.emit("prompt", PromptEvent { id, request })`. Frontend `App.tsx` listens with `listen("prompt", ...)`, shows `PromptPanel`, and `invoke("answer_prompt", { id, answer, via })` on submit, then hides the panel. Panel window config for the skeleton: `decorations: false`, `alwaysOnTop: true`, `width: 420`, `height: 240` on the main window (nspanel conversion is Task 7).

- [ ] **Step 6: Manual verification**

`npm run tauri dev`, then from another terminal run the Task 5 test-support binary or:
```bash
cargo test --test mcp_socket -- --nocapture
```
against the live socket path (`~/Library/Application Support/com.glebkalinin.cenno/mcp.sock`) by temporarily pointing the test at it. Expect: panel shows the prompt, typing an answer returns it to the caller.

- [ ] **Step 7: Commit** — `git add -A && git commit -m "feat: panel UI renders prompts and resolves them via answer_prompt"`

---

### Task 7: Non-activating NSPanel

**Files:**
- Modify: `src-tauri/Cargo.toml`: `tauri-nspanel = { git = "https://github.com/ahkohd/tauri-nspanel", branch = "v2" }`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Convert window to NSPanel**

Per the tauri-nspanel README: register the plugin, then in setup convert the prompt window with `window.to_panel()`, set style mask to non-activating (`NSWindowStyleMask::NonactivatingPanel`), level above normal windows, and `collection_behavior` to join all spaces. Follow the README example verbatim — this crate's API moves; pin the rev that compiles and record it in Cargo.lock.

- [ ] **Step 2: Manual verification (the acceptance test for this task)**

With focus in another app (e.g. a terminal with a cursor blinking): trigger a prompt. Expect: panel appears above all windows, **the terminal keeps keyboard focus** until you click into the panel's input. Record the result in the commit message.

- [ ] **Step 3: Fallback if the plugin fights Tauri 2.x**

Keep `alwaysOnTop` window from Task 6 and file the nspanel conversion as a known issue in README — do not burn more than 2 hours here; the skeleton's value is the end-to-end loop.

- [ ] **Step 4: Commit** — `git commit -am "feat: prompt panel as non-activating NSPanel"`

---

### Task 8: CLI mode — `cenno ask`

**Files:**
- Modify: `src-tauri/src/main.rs`, `src-tauri/Cargo.toml` (`clap = { version = "4", features = ["derive"] }`)
- Create: `src-tauri/src/cli.rs`

- [ ] **Step 1: Write the failing test** (in `cli.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_ask() {
        let cli = Cli::try_parse_from(["cenno", "ask", "How deep?", "--timeout", "30"]).unwrap();
        match cli.command {
            Some(Command::Ask { title, timeout, .. }) => { assert_eq!(title, "How deep?"); assert_eq!(timeout, 30); }
            _ => panic!(),
        }
    }
    #[test]
    fn no_args_means_gui() {
        assert!(Cli::try_parse_from(["cenno"]).unwrap().command.is_none());
    }
}
```

- [ ] **Step 2: Run** — `cargo test cli` — expect: compile failure.

- [ ] **Step 3: Implement**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cenno")]
pub struct Cli {
    #[command(subcommand)] pub command: Option<Command>,
    /// Run headless with surfaces ready (no main window)
    #[arg(long)] pub tray: bool,
    /// Bridge stdin/stdout to the MCP socket (launching the app if needed)
    #[arg(long)] pub mcp_stdio: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Ask the user a question and print the JSON result
    Ask {
        title: String,
        #[arg(long, default_value = "")] body: String,
        #[arg(long, default_value = "120")] timeout: u64,
    },
}
```
`main.rs`: parse first; `Command::Ask` connects an rmcp client to the socket (reuse `test_support::call_ask_user`, promoted to a `client` module — feature-gate removed), prints the JSON result, exits 0 (or exits 2 with `{"answered":false,...}` on timeout). If the socket is missing, print a clear error: `cenno is not running — start it or use 'cenno --mcp-stdio'`. No subcommand → run the Tauri app (with `--tray` skipping window creation via `tauri::Builder` `.setup` checking the flag).

- [ ] **Step 4: Run tests** — `cargo test cli` — expect: 2 passed.

- [ ] **Step 5: Manual E2E** — with the app running: `./src-tauri/target/debug/cenno ask "Ship it?"` → answer in panel → JSON on stdout.

- [ ] **Step 6: Commit** — `git commit -am "feat: cenno ask CLI via MCP socket client"`

---

### Task 9: `--mcp-stdio` bridge with autolaunch

**Files:**
- Create: `src-tauri/src/bridge.rs`
- Modify: `src-tauri/src/main.rs`
- Reference: cull's `run_stdio_bridge()` in `~/ai_projects/cull/src-tauri/src/lib.rs:135-191` — same logic, copy the structure.

- [ ] **Step 1: Implement (no unit test — covered by E2E in Step 2)**

```rust
pub async fn run_stdio_bridge(socket_path: std::path::PathBuf) -> anyhow::Result<()> {
    if !socket_path.exists() {
        let exe = std::env::current_exe()?;
        std::process::Command::new(exe).arg("--tray").spawn()?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while !socket_path.exists() {
            if std::time::Instant::now() > deadline { anyhow::bail!("cenno failed to start"); }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }
    let stream = tokio::net::UnixStream::connect(&socket_path).await?;
    let (mut sr, mut sw) = stream.into_split();
    let (mut stdin, mut stdout) = (tokio::io::stdin(), tokio::io::stdout());
    tokio::select! {
        r = tokio::io::copy(&mut stdin, &mut sw) => { r?; }
        r = tokio::io::copy(&mut sr, &mut stdout) => { r?; }
    }
    Ok(())
}
```

- [ ] **Step 2: E2E verification with a real MCP client**

Add to a test project's `.mcp.json`:
```json
{ "mcpServers": { "cenno": { "command": "/Users/glebkalinin/ai_projects/cenno/src-tauri/target/debug/cenno", "args": ["--mcp-stdio"] } } }
```
From Claude Code in that project: call the `ask_user` tool. Expect: cenno launches in tray mode if not running, panel appears, the answer comes back as the tool result.

- [ ] **Step 3: Commit** — `git commit -am "feat: --mcp-stdio bridge with tray autolaunch"`

---

### Task 10: Smoke script + README

**Files:**
- Create: `scripts/smoke.sh`, `README.md`

- [ ] **Step 1: Smoke script**

```bash
#!/usr/bin/env bash
# E2E: requires the app running. Asks a question, auto-answers via a second
# prompt is NOT possible headlessly yet — this script verifies the socket
# round-trip with a short timeout and accepts both outcomes as "wired".
set -euo pipefail
BIN="src-tauri/target/debug/cenno"
OUT=$("$BIN" ask "Smoke test — press Enter in the panel" --timeout 10 || true)
echo "$OUT" | jq -e '(.answer != null) or (.answered == false)' >/dev/null \
  && echo "SMOKE OK: $OUT" || { echo "SMOKE FAIL: $OUT"; exit 1; }
```

- [ ] **Step 2: README** — quickstart (build, run, `.mcp.json` snippet from Task 9), the four tools table (note: only `ask_user` implemented in skeleton), plan sequence pointer to `docs/superpowers/`.

- [ ] **Step 3: Commit** — `git add -A && git commit -m "chore: smoke script and README"`

---

## Self-review notes

- **Spec coverage (this phase):** architecture modes (app/tray/stdio/CLI) ✓, `ask_user` ✓, panel surface ✓ (nspanel Task 7), timeout-stays-pending ✓ (registry), spike ✓ (Task 2). Deliberately out (later plans): `show_surface`/`dismiss_surface`/`get_response` tools, fullscreen, tray popover UI, urgency policy, SQLite, voice, A2UI desugaring, tokens.
- **Known approximation:** rmcp macro syntax in Task 5 is intentionally referenced to cull's working code rather than spelled out — copying a working idiom from the same major version beats inventing macro syntax that may not compile.
- **Type consistency:** `AskRequest/AskResponse/Via` defined in Task 3 are the only contract types; Tasks 4, 5, 6, 8 all import from `crate::protocol`.
