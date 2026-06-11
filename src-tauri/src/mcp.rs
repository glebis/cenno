//! MCP server exposing `ask_user` over a Unix domain socket.
//!
//! Agents (MCP clients) connect to the socket, call `ask_user`, and the call
//! parks in the [`PromptRegistry`] until the human answers (or it times out).
//! The `notify` callback is how the UI layer learns a prompt appeared — the
//! Tauri layer passes an event emitter; tests pass a no-op.
//!
//! Idioms (UnixListener accept loop, `tool_router!`/`#[tool]`, manual
//! `ServerHandler::call_tool` delegating to the router) follow cull's
//! `src-tauri/src/mcp/{socket,tools}.rs`, which uses the same rmcp 1.7.

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Bundle identifier — must match `identifier` in `tauri.conf.json`
/// (asserted by the `identifier_matches_tauri_conf` unit test below).
pub const APP_IDENTIFIER: &str = "app.cenno";

/// Canonical per-user data directory for cenno.
///
/// macOS: `~/Library/Application Support/app.cenno/`
/// Linux: `~/.local/share/app.cenno/`
/// Windows: `%APPDATA%\app.cenno\`
///
/// This MUST agree with what Tauri's `app.path().app_data_dir()` resolves to.
/// In debug builds `lib.rs` asserts the socket paths match so any divergence
/// is caught at startup rather than silently.
pub fn data_dir() -> PathBuf {
    let base = dirs::data_dir().expect("could not determine user data directory");
    base.join(APP_IDENTIFIER)
}

/// Canonical path to the MCP Unix socket.
pub fn socket_path() -> PathBuf {
    data_dir().join("mcp.sock")
}

use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, wrapper::Parameters},
    model::{CallToolRequestParams, CallToolResult, ServerCapabilities, ServerInfo},
    service::RequestContext,
    tool, tool_router, ErrorData, RoleServer, ServerHandler, ServiceExt,
};
use tokio::net::UnixListener;

use chrono::Utc;

use crate::db::Db;
use crate::protocol::{AskRequest, Progress, SeqMeta, SequenceRequest, SequenceResponse};
use crate::registry::PromptRegistry;

/// Type-erased "a prompt appeared" callback (Task 6 passes a Tauri emitter).
///
/// The third arg carries optional sequence metadata: `None` for a plain
/// `ask_user` prompt, `Some(SeqMeta)` for each step of an `ask_sequence` run
/// so the UI can swap content instead of hiding between questions. The
/// single-ask path always passes `None`, keeping its `prompt` event wire shape
/// byte-identical (PromptEvent drops the field via `skip_serializing_if`).
pub type NotifyFn = Arc<dyn Fn(&str, &AskRequest, Option<SeqMeta>) + Send + Sync>;

#[derive(Clone)]
pub struct CennoServer {
    registry: PromptRegistry,
    notify: NotifyFn,
    db: Option<Db>,
    /// Default prompt timeout (from `~/.cenno` config, else the built-in 120)
    /// applied when an agent omits `timeout_s`.
    default_timeout_s: u64,
    tool_router: ToolRouter<Self>,
}

impl CennoServer {
    pub fn new(
        registry: PromptRegistry,
        notify: NotifyFn,
        db: Option<Db>,
        default_timeout_s: u64,
    ) -> Self {
        Self {
            registry,
            notify,
            db,
            default_timeout_s,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl CennoServer {
    #[tool(
        description = "Ask the human user a question and wait for their answer. \
                       Returns JSON: {answer, via, elapsed_s} when answered, or \
                       {answered: false, prompt_id} on timeout."
    )]
    async fn ask_user(
        &self,
        Parameters(mut params): Parameters<AskRequest>,
    ) -> Result<String, String> {
        // Resolve the timeout against config now, so the registry and the
        // panel's auto-hide budget all see the same concrete value.
        params.timeout_s = Some(params.timeout_secs(Some(self.default_timeout_s)));
        // Boundary guard: the web renderer silently drops malformed/mis-versioned
        // a2ui messages, so reject HERE — before a prompt is registered — and
        // hand the agent an actionable error instead of a surface that never
        // renders. Err(String) becomes a CallToolResult with is_error: true
        // (rmcp's IntoCallToolResult impl for Result<T, E>).
        if let Some(a2ui) = &params.a2ui {
            crate::a2ui_guard::validate_a2ui(a2ui)
                .map_err(|msg| format!("invalid a2ui payload: {msg}"))?;
        }
        // TODO(plan4): observe context.ct (client cancellation) so a dead agent
        // unparks the prompt instead of burning the full timeout_s.
        let created_at = Utc::now();

        // Capture the prompt_id assigned by the registry via the notify callback.
        // The notify fires synchronously inside registry.ask() before parking.
        let captured_id: Arc<parking_lot::Mutex<String>> =
            Arc::new(parking_lot::Mutex::new(String::new()));
        let captured_id2 = captured_id.clone();

        // Serialise the params once for the CloudKit payload (fire-and-forget).
        let payload_for_relay = serde_json::to_string(&params).unwrap_or_default();
        let timeout_for_relay = params.timeout_secs(Some(self.default_timeout_s));

        let resp = self
            .registry
            .ask(params.clone(), |id, req| {
                *captured_id2.lock() = id.to_string();
                (self.notify)(id, req, None);
                // Publish to CloudKit so the Watch/iPhone companion can pick it up.
                crate::relay::write_prompt(id, &payload_for_relay, timeout_for_relay);
            })
            .await;

        // Record the outcome — failures are non-fatal: log and continue.
        {
            let prompt_id = match &resp {
                crate::protocol::AskResponse::Answered { .. } => captured_id.lock().clone(),
                crate::protocol::AskResponse::TimedOut { prompt_id, .. } => prompt_id.clone(),
            };
            if let Some(db) = &self.db {
                if let Err(e) = db.record_prompt(&params, &prompt_id, &resp, created_at) {
                    eprintln!("cenno db: failed to record prompt {prompt_id}: {e}");
                }
            }
            // Update CloudKit state so the companion hides the prompt.
            let (state, answer_json) = match &resp {
                crate::protocol::AskResponse::Answered { .. } => {
                    let j = serde_json::to_string(&resp).unwrap_or_default();
                    ("answered", Some(j))
                }
                crate::protocol::AskResponse::TimedOut { .. } => ("timed_out", None),
            };
            crate::relay::update_state(&prompt_id, state, answer_json.as_deref());
        }

        Ok(serde_json::to_string(&resp).expect("AskResponse is always serializable"))
    }

    #[tool(
        description = "Ask the human user several questions back-to-back in one panel. \
                       Each answered question is immediately replaced by the next (no \
                       hide/reshow gap); the panel hides only after the last. \
                       Auto-fills progress dots (step/total) and applies the optional \
                       sequence `flow` to any question lacking one. A per-question \
                       timeout ends the run early. Returns JSON: \
                       {answers: [...]} — one entry per question that ran, in order. \
                       The final entry is {answered: false, prompt_id} if that question \
                       timed out (and the run then stops)."
    )]
    async fn ask_sequence(
        &self,
        Parameters(params): Parameters<SequenceRequest>,
    ) -> Result<String, String> {
        let total = params.questions.len() as u32;

        // Boundary guard: validate EVERY a2ui payload up front — before any
        // prompt is registered — so a malformed step doesn't run the earlier
        // questions and then hand the agent a half-finished sequence. Naming
        // the offending index makes the error actionable. Mirrors ask_user's
        // guard, just per-question.
        for (i, q) in params.questions.iter().enumerate() {
            if let Some(a2ui) = &q.a2ui {
                crate::a2ui_guard::validate_a2ui(a2ui)
                    .map_err(|msg| format!("invalid a2ui payload in question {i}: {msg}"))?;
            }
        }

        let mut answers = Vec::with_capacity(params.questions.len());
        for (i, question) in params.questions.into_iter().enumerate() {
            let index = i as u32;
            let last = index + 1 == total;

            // Apply the sequence-level defaults to questions that lack them:
            // inherit `flow`, auto-fill progress dots (1-based step), and
            // resolve the timeout against config.
            let mut question = question;
            if question.flow.is_none() {
                question.flow = params.flow.clone();
            }
            if question.progress.is_none() {
                question.progress = Some(Progress { step: index + 1, total });
            }
            question.timeout_s = Some(question.timeout_secs(Some(self.default_timeout_s)));

            let created_at = Utc::now();
            let captured_id: Arc<parking_lot::Mutex<String>> =
                Arc::new(parking_lot::Mutex::new(String::new()));
            let captured_id2 = captured_id.clone();

            let seq = SeqMeta { index, total, last };
            let resp = self
                .registry
                .ask(question.clone(), |id, req| {
                    *captured_id2.lock() = id.to_string();
                    (self.notify)(id, req, Some(seq.clone()));
                })
                .await;

            // One history row per question (mirror ask_user).
            if let Some(db) = &self.db {
                let prompt_id = match &resp {
                    crate::protocol::AskResponse::Answered { .. } => captured_id.lock().clone(),
                    crate::protocol::AskResponse::TimedOut { prompt_id, .. } => prompt_id.clone(),
                };
                if let Err(e) = db.record_prompt(&question, &prompt_id, &resp, created_at) {
                    eprintln!("cenno db: failed to record prompt {prompt_id}: {e}");
                }
            }

            let timed_out = matches!(resp, crate::protocol::AskResponse::TimedOut { .. });
            answers.push(resp);
            // Timeout ends the run early: include the TimedOut entry, then stop.
            if timed_out {
                break;
            }
        }

        Ok(serde_json::to_string(&SequenceResponse { answers })
            .expect("SequenceResponse is always serializable"))
    }
}

impl ServerHandler for CennoServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("cenno — ask the human user questions via popup prompts")
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, ErrorData> {
        Ok(rmcp::model::ListToolsResult {
            tools: self.tool_router.list_all(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let call_context = ToolCallContext::new(self, request, context);
        self.tool_router.call(call_context).await
    }
}

/// Bind the socket and start accepting MCP connections in a background task.
///
/// Removes a stale (non-connectable) socket file first; errors if a live
/// server already listens there. Returns once the listener is bound, so
/// callers can connect immediately after this resolves.
pub async fn start_socket_server(
    sock_path: PathBuf,
    registry: PromptRegistry,
    notify: impl Fn(&str, &AskRequest, Option<SeqMeta>) + Send + Sync + 'static,
    db: Option<Db>,
    default_timeout_s: u64,
) -> anyhow::Result<()> {
    // TODO(plan4): two concurrent launches can unlink each other's live socket —
    // enforce single instance (tauri-plugin-single-instance) instead of smarter
    // socket dancing.
    if sock_path.exists() {
        match tokio::net::UnixStream::connect(&sock_path).await {
            Ok(_) => anyhow::bail!(
                "another MCP server is already listening on {}",
                sock_path.display()
            ),
            Err(_) => {
                let _ = std::fs::remove_file(&sock_path);
            }
        }
    }

    let listener = UnixListener::bind(&sock_path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&sock_path, std::fs::Permissions::from_mode(0o600))?;
    }

    let notify: NotifyFn = Arc::new(notify);
    tokio::spawn(async move {
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("cenno mcp: accept failed: {e}");
                    // Persistent accept errors (e.g. EMFILE under fd pressure)
                    // are recoverable — back off instead of busy-spinning.
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
            };
            let server =
                CennoServer::new(registry.clone(), notify.clone(), db.clone(), default_timeout_s);
            tokio::spawn(async move {
                let (read, write) = tokio::io::split(stream);
                match server.serve((read, write)).await {
                    Ok(running) => {
                        if let Err(e) = running.waiting().await {
                            eprintln!("cenno mcp: session ended with error: {e:?}");
                        }
                    }
                    Err(e) => eprintln!("cenno mcp: session failed to start: {e:?}"),
                }
            });
        }
    });

    Ok(())
}

/// MCP client helpers — used by `cenno ask`, integration tests, and (Task 9) the stdio bridge.
///
/// Always compiled (not `#[cfg(test)]`): integration tests in `tests/` build
/// the lib WITHOUT `cfg(test)`, and gating this behind a `test-support`
/// feature would need a dev-dependency self-reference to enable it. Keeping
/// it unconditional is the simplest thing that compiles everywhere; the only
/// extra cost is rmcp's small "client" feature.
pub mod client {
    use super::*;

    /// Connect to the cenno MCP socket, call `ask_user` with `params`, and
    /// parse the tool's text content as JSON.
    pub async fn call_ask_user(
        sock_path: &Path,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let args = match params {
            serde_json::Value::Object(map) => map,
            other => anyhow::bail!("ask_user params must be a JSON object, got: {other}"),
        };

        let stream = tokio::net::UnixStream::connect(sock_path).await?;
        // `()` is rmcp's no-op ClientHandler; serve() runs the initialize handshake.
        let client = ().serve(stream).await?;

        let result = client
            .call_tool(CallToolRequestParams::new("ask_user").with_arguments(args))
            .await?;

        let text = result
            .content
            .iter()
            .find_map(|c| c.as_text())
            .map(|t| t.text.clone())
            .ok_or_else(|| anyhow::anyhow!("ask_user returned no text content: {result:?}"))?;

        let value = serde_json::from_str(&text)?;
        let _ = client.cancel().await;
        Ok(value)
    }

    /// Connect to the cenno MCP socket, call `ask_sequence` with `params`, and
    /// parse the tool's text content as JSON (`{answers: [...]}`).
    pub async fn call_ask_sequence(
        sock_path: &Path,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let args = match params {
            serde_json::Value::Object(map) => map,
            other => anyhow::bail!("ask_sequence params must be a JSON object, got: {other}"),
        };

        let stream = tokio::net::UnixStream::connect(sock_path).await?;
        let client = ().serve(stream).await?;

        let result = client
            .call_tool(CallToolRequestParams::new("ask_sequence").with_arguments(args))
            .await?;

        let text = result
            .content
            .iter()
            .find_map(|c| c.as_text())
            .map(|t| t.text.clone())
            .ok_or_else(|| anyhow::anyhow!("ask_sequence returned no text content: {result:?}"))?;

        let value = serde_json::from_str(&text)?;
        let _ = client.cancel().await;
        Ok(value)
    }
}

/// Backward-compat alias so existing integration tests keep compiling without edits.
/// (Prefer `mcp::client` in new code.)
#[doc(hidden)]
pub use client as test_support;

#[cfg(test)]
mod tests {
    use super::*;

    /// `socket_path()` derives the app-data dir from APP_IDENTIFIER while the
    /// running app derives it from tauri.conf.json — if the conf's identifier
    /// ever changes, this turns silent release drift into a test failure.
    #[test]
    fn identifier_matches_tauri_conf() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
            .expect("tauri.conf.json is valid JSON");
        assert_eq!(conf["identifier"], APP_IDENTIFIER);
    }
}
