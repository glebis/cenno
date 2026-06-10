use cenno_lib::{mcp::start_socket_server, protocol::Via, registry::PromptRegistry};

#[tokio::test]
async fn ask_user_over_socket_resolves() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("mcp.sock");
    let reg = PromptRegistry::new();
    start_socket_server(sock.clone(), reg.clone(), |_id, _req| {})
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
