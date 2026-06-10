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
pub const APP_IDENTIFIER: &str = "com.glebkalinin.cenno";

/// Canonical path to the MCP Unix socket.
///
/// This MUST return the same directory that Tauri's `app.path().app_data_dir()`
/// resolves to on macOS (`~/Library/Application Support/com.glebkalinin.cenno/`).
/// In debug builds, `lib.rs`'s setup function asserts the two paths match so
/// any divergence is caught at startup rather than silently.
///
/// Using `dirs::data_dir()` here avoids a Tauri dependency in the CLI path.
pub fn socket_path() -> PathBuf {
    // macOS: ~/Library/Application Support
    // Linux: ~/.local/share
    // Windows: %APPDATA%
    let base = dirs::data_dir().expect("could not determine user data directory");
    base.join(APP_IDENTIFIER).join("mcp.sock")
}

use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, wrapper::Parameters},
    model::{CallToolRequestParams, CallToolResult, ServerCapabilities, ServerInfo},
    service::RequestContext,
    tool, tool_router, ErrorData, RoleServer, ServerHandler, ServiceExt,
};
use tokio::net::UnixListener;

use crate::protocol::AskRequest;
use crate::registry::PromptRegistry;

/// Type-erased "a prompt appeared" callback (Task 6 passes a Tauri emitter).
pub type NotifyFn = Arc<dyn Fn(&str, &AskRequest) + Send + Sync>;

#[derive(Clone)]
pub struct CennoServer {
    registry: PromptRegistry,
    notify: NotifyFn,
    tool_router: ToolRouter<Self>,
}

impl CennoServer {
    pub fn new(registry: PromptRegistry, notify: NotifyFn) -> Self {
        Self {
            registry,
            notify,
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
    async fn ask_user(&self, Parameters(params): Parameters<AskRequest>) -> String {
        // TODO(plan4): observe context.ct (client cancellation) so a dead agent
        // unparks the prompt instead of burning the full timeout_s.
        let resp = self
            .registry
            .ask(params, |id, req| (self.notify)(id, req))
            .await;
        serde_json::to_string(&resp).expect("AskResponse is always serializable")
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
    notify: impl Fn(&str, &AskRequest) + Send + Sync + 'static,
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
            let server = CennoServer::new(registry.clone(), notify.clone());
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
