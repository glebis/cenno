//! `--mcp-stdio` bridge: pumps bytes between this process's stdin/stdout and
//! the app's MCP Unix socket, autolaunching the app (`--tray`) if needed.
//!
//! This is what MCP clients (Claude Code, etc.) actually exec — they speak
//! stdio, the app speaks a Unix socket, and this process is the dumb pipe in
//! between. Pattern follows cull's `run_stdio_bridge`.
//!
//! Invariant: stdout carries ONLY protocol bytes. All diagnostics go to stderr.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tokio::net::UnixStream;

/// How long to wait for the freshly launched app to bind its socket.
const LAUNCH_DEADLINE: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Connect to the cenno MCP socket (launching the app if it isn't running),
/// then forward stdin→socket and socket→stdout until either side closes.
pub async fn run_stdio_bridge(socket_path: PathBuf) -> anyhow::Result<()> {
    let stream = connect_or_launch(&socket_path).await?;

    let (mut sock_read, mut sock_write) = stream.into_split();
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();

    // Either copy finishing means the session is over:
    // - stdin EOF → the client hung up; nothing more to forward.
    // - socket EOF → the app went away; the client sees EOF and cleans up.
    tokio::select! {
        r = tokio::io::copy(&mut stdin, &mut sock_write) => { r?; }
        r = tokio::io::copy(&mut sock_read, &mut stdout) => { r?; }
    }
    Ok(())
}

/// Try to connect; on failure, spawn the app in tray mode and wait for a
/// fresh successful connect within [`LAUNCH_DEADLINE`].
///
/// Connect-first (rather than `socket_path.exists()`) deliberately collapses
/// two cases into one: socket file missing (app never ran) and socket file
/// present but dead (app crashed). Both fail the connect, and both have the
/// same fix — launch the app. Removing a stale socket file is NOT our job:
/// the app's `start_socket_server` unlinks it before binding, so the retry
/// loop below succeeds only against a fresh, live listener.
async fn connect_or_launch(socket_path: &Path) -> anyhow::Result<UnixStream> {
    match UnixStream::connect(socket_path).await {
        Ok(s) => return Ok(s),
        Err(e) => {
            eprintln!("cenno bridge: app not reachable ({e}); launching in tray mode...");
        }
    }

    let exe = std::env::current_exe()?;
    // Plain spawn, never wait(): the app must outlive this bridge process.
    // (The Child handle is dropped; the OS reparents the app when we exit.)
    std::process::Command::new(exe).arg("--tray").spawn()?;

    let deadline = Instant::now() + LAUNCH_DEADLINE;
    loop {
        match UnixStream::connect(socket_path).await {
            Ok(s) => return Ok(s),
            Err(e) if Instant::now() >= deadline => {
                anyhow::bail!(
                    "cenno failed to start within {}s (socket: {}): {e}",
                    LAUNCH_DEADLINE.as_secs(),
                    socket_path.display()
                );
            }
            Err(_) => tokio::time::sleep(POLL_INTERVAL).await,
        }
    }
}
