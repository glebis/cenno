//! Probe a RUNNING cenno app instance over its real socket.
//!
//! `#[ignore]`d because it needs the GUI app up:
//!   1. launch the app (`npm run tauri dev` or the debug binary)
//!   2. `cargo test --test live_socket_probe -- --ignored`
//!
//! Nobody answers the prompt, so ask_user must come back as the TimedOut
//! JSON shape — proving the app started the socket server, registered the
//! prompt, emitted, and timed out cleanly.

#[tokio::test]
#[ignore = "requires the cenno app to be running"]
async fn live_ask_user_times_out_cleanly() {
    let sock = dirs_path();
    assert!(
        sock.exists(),
        "socket not found at {} — is the cenno app running?",
        sock.display()
    );

    // Override for tests that need the prompt to stay pending longer
    // (e.g. the NSPanel focus-stealing check observes the app mid-ask).
    let timeout_s: u64 = std::env::var("CENNO_PROBE_TIMEOUT_S")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);

    let result = cenno_lib::mcp::test_support::call_ask_user(
        &sock,
        serde_json::json!({"title": "Live probe", "body_md": "automated E2E check", "timeout_s": timeout_s}),
    )
    .await
    .expect("ask_user call over live socket failed");

    assert_eq!(result["answered"], false, "expected TimedOut shape, got: {result}");
    assert!(
        result["prompt_id"].as_str().is_some_and(|s| s.starts_with("p_")),
        "expected prompt_id, got: {result}"
    );
}

fn dirs_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    std::path::PathBuf::from(home)
        .join("Library/Application Support/com.glebkalinin.cenno/mcp.sock")
}
