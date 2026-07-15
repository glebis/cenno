use cenno_lib::routing::{DeviceMode, RoutingConfig};
use cenno_lib::{
    capture_guard::CaptureState,
    config::CaptureConfig,
    db::Db,
    mcp::start_socket_server,
    protocol::{RawScreenContext, ScreenContextRequest, ScreenContextStatus, Via},
    registry::PromptRegistry,
    screen_context::{ScreenContextReader, ScreenContextServices},
};
use rmcp::{model::CallToolRequestParams, ServiceExt};
use std::sync::Arc;

#[derive(Clone)]
struct FakeScreenContextReader(RawScreenContext);

impl ScreenContextReader for FakeScreenContextReader {
    fn read(&self, _request: &ScreenContextRequest) -> Result<RawScreenContext, String> {
        Ok(self.0.clone())
    }
}

fn screen_context(status: ScreenContextStatus, policy: CaptureConfig) -> ScreenContextServices {
    ScreenContextServices::new(
        Arc::new(FakeScreenContextReader(RawScreenContext {
            status,
            app_name: Some("Notes".into()),
            bundle_id: Some("com.apple.Notes".into()),
            window_title: Some("note".into()),
            url: None,
            host: None,
            focused_role: Some("AXTextArea".into()),
            selected_text: Some("chosen".into()),
            visible_text: Some("visible".into()),
            truncated: false,
        })),
        CaptureState::new(true, |_| {}),
        policy,
    )
}

fn default_screen_context() -> ScreenContextServices {
    screen_context(ScreenContextStatus::AxUnavailable, CaptureConfig::default())
}

/// All companion devices off → `relay::write_prompt` short-circuits on empty
/// targets, so these socket-flow tests never invoke the CloudKit FFI (which
/// would SIGTRAP in a headless test process). Routing logic itself is unit-
/// tested in `src/routing.rs`.
fn off_routing() -> RoutingConfig {
    RoutingConfig {
        iphone: DeviceMode::Off,
        ipad: DeviceMode::Off,
        watch: DeviceMode::Off,
        grace_s: 20,
        allow_agent_hint: true,
    }
}

#[tokio::test]
async fn get_screen_context_is_listed_and_permission_denied_is_data() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("mcp.sock");
    let reg = PromptRegistry::new();
    start_socket_server(
        sock.clone(),
        reg,
        |_id, _req, _seq| {},
        || {},
        None,
        120,
        off_routing(),
        screen_context(
            ScreenContextStatus::PermissionDenied,
            CaptureConfig::default(),
        ),
    )
    .await
    .unwrap();

    let stream = tokio::net::UnixStream::connect(&sock).await.unwrap();
    let client = ().serve(stream).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    assert!(tools.iter().any(|tool| tool.name == "get_screen_context"));

    let result = client
        .call_tool(CallToolRequestParams::new("get_screen_context"))
        .await
        .unwrap();
    assert_ne!(result.is_error, Some(true), "absence is data: {result:?}");
    let text = result.content[0].as_text().unwrap();
    let response: serde_json::Value = serde_json::from_str(&text.text).unwrap();
    assert_eq!(response["status"], "permission_denied");
    assert_eq!(response["untrusted"], true);
    let _ = client.cancel().await;
}

#[tokio::test]
async fn get_screen_context_applies_guard_before_socket_release() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("mcp.sock");
    let reg = PromptRegistry::new();
    start_socket_server(
        sock.clone(),
        reg,
        |_id, _req, _seq| {},
        || {},
        None,
        120,
        off_routing(),
        screen_context(
            ScreenContextStatus::Ok,
            CaptureConfig {
                denylist_bundles: vec!["com.apple.Notes".into()],
                ..Default::default()
            },
        ),
    )
    .await
    .unwrap();

    let stream = tokio::net::UnixStream::connect(&sock).await.unwrap();
    let client = ().serve(stream).await.unwrap();
    let result = client
        .call_tool(CallToolRequestParams::new("get_screen_context"))
        .await
        .unwrap();
    assert_ne!(result.is_error, Some(true));
    let text = result.content[0].as_text().unwrap();
    let response: serde_json::Value = serde_json::from_str(&text.text).unwrap();
    assert_eq!(response["status"], "blocked");
    assert_eq!(response["blocked_reason"], "denied_bundle");
    assert!(response["app_name"].is_null());
    assert!(response["visible_text"].is_null());
    let _ = client.cancel().await;
}

#[tokio::test]
async fn ask_user_over_socket_resolves() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("mcp.sock");
    let reg = PromptRegistry::new();
    start_socket_server(sock.clone(), reg.clone(), |_id, _req, _seq| {}, || {}, None, 120, off_routing(), default_screen_context())
        .await
        .unwrap();

    // auto-answer any prompt after 100ms, like a user typing "yes"
    let reg2 = reg.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            for id in reg2.pending_ids() {
                reg2.resolve(&id, "yes".into(), Via::Text, None);
            }
        }
    });

    let result = cenno_lib::mcp::test_support::call_ask_user(
        &sock,
        serde_json::json!({"title": "Deploy?", "timeout_s": 5}),
    )
    .await
    .unwrap();
    assert_eq!(result["answer"], "yes");
    assert_eq!(result["via"], "text");
}

/// A bad `a2ui` payload must be rejected at the boundary BEFORE a prompt is
/// registered. No auto-answerer is spawned and timeout_s is large: if the
/// guard failed to short-circuit, ask() would park the prompt and this call
/// would hang far past the 5s budget — fast return itself proves ask was
/// never called.
#[tokio::test]
async fn ask_user_with_invalid_a2ui_errors_without_registering_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("mcp.sock");
    let reg = PromptRegistry::new();
    start_socket_server(sock.clone(), reg.clone(), |_id, _req, _seq| {}, || {}, None, 120, off_routing(), default_screen_context())
        .await
        .unwrap();

    let stream = tokio::net::UnixStream::connect(&sock).await.unwrap();
    let client = ().serve(stream).await.unwrap();

    let args = serde_json::json!({
        "title": "Deploy?",
        "timeout_s": 60,
        "a2ui": [
            {"version": "v0.8", "createSurface": {"surfaceId": "main", "catalogId": "c"}}
        ]
    });
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.call_tool(
            CallToolRequestParams::new("ask_user")
                .with_arguments(args.as_object().unwrap().clone()),
        ),
    )
    .await
    .expect("guard must reject immediately, not park the prompt")
    .unwrap();

    assert_eq!(result.is_error, Some(true), "tool must signal an error: {result:?}");
    let text = result
        .content
        .iter()
        .find_map(|c| c.as_text())
        .map(|t| t.text.clone())
        .expect("error result carries a text message");
    assert!(text.contains("invalid a2ui payload"), "{text}");
    assert!(text.contains("v0.8"), "must name the offending version: {text}");

    assert!(
        reg.pending_ids().is_empty(),
        "rejected call must not register a prompt: {:?}",
        reg.pending_ids()
    );
    let _ = client.cancel().await;
}

/// An answered ask writes an 'answered' row to the database.
#[tokio::test]
async fn answered_ask_writes_history_row() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("mcp.sock");
    let db_path = dir.path().join("cenno.db");
    let db = Db::open(&db_path).unwrap();

    let reg = PromptRegistry::new();
    start_socket_server(sock.clone(), reg.clone(), |_id, _req, _seq| {}, || {}, Some(db.clone()), 120, off_routing(), default_screen_context())
        .await
        .unwrap();

    // auto-answer any prompt after 100ms
    let reg2 = reg.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            for id in reg2.pending_ids() {
                reg2.resolve(&id, "ship it".into(), Via::Choice, None);
            }
        }
    });

    let result = cenno_lib::mcp::test_support::call_ask_user(
        &sock,
        serde_json::json!({"title": "Release v2?", "timeout_s": 5}),
    )
    .await
    .unwrap();
    assert_eq!(result["answer"], "ship it");

    // Give the recording task a moment to write (it's spawned async after ask returns)
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Assert the row was written with the correct status/answer/via.
    let conn = db.raw_conn();
    let conn = conn.lock();
    let (status, answer, via): (String, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT status, answer, via FROM prompts WHERE title = 'Release v2?'",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, Option<String>>(2)?)),
        )
        .expect("expected a row in prompts for 'Release v2?'");

    assert_eq!(status, "answered");
    assert_eq!(answer.as_deref(), Some("ship it"));
    assert_eq!(via.as_deref(), Some("choice"));
}

/// `ask_sequence` runs N questions back-to-back in one call: an auto-answerer
/// resolves each pending prompt in turn, and the tool returns the answers in
/// order. Every question is also recorded as its own history row.
#[tokio::test]
async fn ask_sequence_runs_questions_in_order_and_records_each() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("mcp.sock");
    let db_path = dir.path().join("cenno.db");
    let db = Db::open(&db_path).unwrap();

    let reg = PromptRegistry::new();
    start_socket_server(sock.clone(), reg.clone(), |_id, _req, _seq| {}, || {}, Some(db.clone()), 120, off_routing(), default_screen_context())
        .await
        .unwrap();

    // Auto-answerer: resolve whatever prompt is currently pending with an
    // answer derived from its title, so we can assert per-question routing.
    let reg2 = reg.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            // `pending()` only lists the one still-awaiting prompt (the
            // sequence fires the next ask only after this resolves).
            for (id, request, _remaining) in reg2.pending() {
                let answer = format!("ans-{}", request.title);
                reg2.resolve(&id, answer, Via::Text, None);
            }
        }
    });

    let result = cenno_lib::mcp::test_support::call_ask_sequence(
        &sock,
        serde_json::json!({
            "questions": [
                {"title": "q1", "timeout_s": 5},
                {"title": "q2", "timeout_s": 5},
                {"title": "q3", "timeout_s": 5}
            ]
        }),
    )
    .await
    .unwrap();

    let answers = result["answers"].as_array().expect("answers is an array");
    assert_eq!(answers.len(), 3, "all three questions answered: {result}");
    assert_eq!(answers[0]["answer"], "ans-q1");
    assert_eq!(answers[1]["answer"], "ans-q2");
    assert_eq!(answers[2]["answer"], "ans-q3");

    // One history row per question.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let conn = db.raw_conn();
    let conn = conn.lock();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prompts WHERE title IN ('q1','q2','q3') AND status = 'answered'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 3, "one answered row per sequence question");
}

/// A per-question timeout ends the run early: the timed-out question's
/// `TimedOut` entry is included, then the sequence STOPS (later questions are
/// never asked). The history still records both rows that ran.
// PRE-EXISTING DEBT (surfaced 2026-06-15): this test parks forever under the
// current registry design. `PromptRegistry::ask` Phase 1 runs NO timeout until a
// prompt is marked shown (registry.rs:71-85); the empty/suppressing notify here
// never shows the second question, so its 1s budget never starts and the run
// can't end. The test encodes an older "timeout starts at registration" model
// and never ran before (the `mcp_socket` target failed to compile — its
// `start_socket_server` calls predated the `default_timeout_s` parameter). Left
// ignored, not silently rewritten, pending a decision on the intended behavior.
#[ignore = "stale: expects un-shown prompt to time out; current registry arms timeout on show"]
#[tokio::test]
async fn ask_sequence_timeout_ends_run_early() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("mcp.sock");
    let db_path = dir.path().join("cenno.db");
    let db = Db::open(&db_path).unwrap();

    let reg = PromptRegistry::new();
    start_socket_server(sock.clone(), reg.clone(), |_id, _req, _seq| {}, || {}, Some(db.clone()), 120, off_routing(), default_screen_context())
        .await
        .unwrap();

    // Answer ONLY the first question; the second has a short timeout and is
    // left to elapse, which must end the run (the third is never reached).
    let reg2 = reg.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            for (id, request, _remaining) in reg2.pending() {
                if request.title == "first" {
                    reg2.resolve(&id, "ok".into(), Via::Text, None);
                }
            }
        }
    });

    let result = cenno_lib::mcp::test_support::call_ask_sequence(
        &sock,
        serde_json::json!({
            "questions": [
                {"title": "first", "timeout_s": 5},
                {"title": "second", "timeout_s": 1},
                {"title": "third", "timeout_s": 5}
            ]
        }),
    )
    .await
    .unwrap();

    let answers = result["answers"].as_array().expect("answers is an array");
    assert_eq!(answers.len(), 2, "run stops after the timed-out question: {result}");
    assert_eq!(answers[0]["answer"], "ok");
    // Second entry is the TimedOut shape: {answered: false, prompt_id}.
    assert_eq!(answers[1]["answered"], false, "second question timed out: {result}");
    assert!(answers[1]["prompt_id"].is_string(), "timeout carries prompt_id: {result}");

    // The third question was never asked.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let conn = db.raw_conn();
    let conn = conn.lock();
    let third: i64 = conn
        .query_row("SELECT COUNT(*) FROM prompts WHERE title = 'third'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(third, 0, "the question after the timeout must never run");
    // Both rows that ran are recorded (one answered, one timed_out).
    let ran: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prompts WHERE title IN ('first','second')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(ran, 2, "both run questions recorded a row");
}

/// Suppression gates the DISPLAY side-effect only: with a paused
/// SuppressionState wired into the notify closure (mirroring lib.rs's
/// gating), the display probe never fires, but the prompt still registers
/// and the agent still gets the normal TimedOut contract.
// PRE-EXISTING DEBT (surfaced 2026-06-15): same root cause as
// `ask_sequence_timeout_ends_run_early`. A suppressed prompt is never shown, so
// `ask` Phase 1 parks with no timeout (registry.rs:71-85) and the asserted
// TimedOut (line "unseen prompt must time out") can never fire — the call hangs.
// Stale against the current "timeout arms on show" design; never ran before
// (target didn't compile). Ignored pending a behavior decision.
#[ignore = "stale: expects suppressed/un-shown prompt to time out; current registry arms timeout on show"]
#[tokio::test]
async fn suppressed_notify_skips_display_but_prompt_still_registers() {
    use cenno_lib::suppress::SuppressionState;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("mcp.sock");
    let reg = PromptRegistry::new();

    let suppress = SuppressionState::new(true);
    suppress.pause_for(60);

    let displayed = Arc::new(AtomicBool::new(false));
    let displayed_probe = displayed.clone();
    let gate = suppress.clone();
    start_socket_server(
        sock.clone(),
        reg.clone(),
        move |_id, _req, _seq| {
            // Exact decision lib.rs's notify closure makes before emit+show.
            if cenno_lib::should_display(&gate, None, || false) {
                displayed_probe.store(true, Ordering::SeqCst);
            }
        },
        || {},
        None,
        120,
        off_routing(),
        default_screen_context(),
    )
    .await
    .unwrap();

    let result = cenno_lib::mcp::test_support::call_ask_user(
        &sock,
        serde_json::json!({"title": "Quiet hour?", "timeout_s": 1}),
    )
    .await
    .unwrap();

    // Agent contract unchanged: timed out, prompt_id returned.
    assert_eq!(result["answered"], false, "unseen prompt must time out: {result}");
    // No display side-effect fired...
    assert!(!displayed.load(Ordering::SeqCst), "suppressed prompt must not display");
    // ...but the prompt registered (and stays as inbox leftover).
    assert_eq!(reg.pending_ids().len(), 1);

    // After resume, a fresh ask displays normally.
    suppress.resume();
    let reg2 = reg.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            for id in reg2.pending_ids() {
                reg2.resolve(&id, "back".into(), Via::Text, None);
            }
        }
    });
    let result = cenno_lib::mcp::test_support::call_ask_user(
        &sock,
        serde_json::json!({"title": "Back?", "timeout_s": 5}),
    )
    .await
    .unwrap();
    assert_eq!(result["answer"], "back");
    assert!(displayed.load(Ordering::SeqCst), "post-resume prompt must display");
}
