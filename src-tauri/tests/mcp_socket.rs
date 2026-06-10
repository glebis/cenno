use cenno_lib::{db::Db, mcp::start_socket_server, protocol::Via, registry::PromptRegistry};
use rmcp::{model::CallToolRequestParams, ServiceExt};

#[tokio::test]
async fn ask_user_over_socket_resolves() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("mcp.sock");
    let reg = PromptRegistry::new();
    start_socket_server(sock.clone(), reg.clone(), |_id, _req| {}, None)
        .await
        .unwrap();

    // auto-answer any prompt after 100ms, like a user typing "yes"
    let reg2 = reg.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            for id in reg2.pending_ids() {
                reg2.resolve(&id, "yes".into(), Via::Text);
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
    start_socket_server(sock.clone(), reg.clone(), |_id, _req| {}, None)
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
    start_socket_server(sock.clone(), reg.clone(), |_id, _req| {}, Some(db.clone()))
        .await
        .unwrap();

    // auto-answer any prompt after 100ms
    let reg2 = reg.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            for id in reg2.pending_ids() {
                reg2.resolve(&id, "ship it".into(), Via::Choice);
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
