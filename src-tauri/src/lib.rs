pub mod mcp;
pub mod protocol;
pub mod registry;

use tauri::{Emitter, Manager};

use crate::protocol::{AskRequest, Via};
use crate::registry::PromptRegistry;

/// Payload of the `prompt` event emitted to the webview when an agent asks.
#[derive(Clone, serde::Serialize)]
struct PromptEvent {
    id: String,
    request: AskRequest,
}

#[tauri::command]
fn answer_prompt(state: tauri::State<PromptRegistry>, id: String, answer: String, via: String) -> bool {
    let via = match via.as_str() {
        "voice" => Via::Voice,
        "choice" => Via::Choice,
        _ => Via::Text,
    };
    state.resolve(&id, answer, via)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let registry = PromptRegistry::new();
            app.manage(registry.clone());

            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let sock_path = data_dir.join("mcp.sock");

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let handle = app_handle.clone();
                let result = mcp::start_socket_server(sock_path, registry, move |id, req| {
                    // Called from the socket server's tokio runtime; both
                    // emit() and window calls are thread-safe in Tauri 2.
                    let payload = PromptEvent { id: id.to_string(), request: req.clone() };
                    if let Err(e) = handle.emit("prompt", payload) {
                        eprintln!("cenno: failed to emit prompt event: {e}");
                    }
                    if let Some(win) = handle.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                })
                .await;
                if let Err(e) = result {
                    eprintln!("cenno: failed to start MCP socket server: {e}");
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![answer_prompt])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
