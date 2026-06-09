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

/// macOS: swizzle the (hidden) main window into a non-activating NSPanel.
///
/// This is the product's core trick: the panel can be ordered on-screen —
/// and even take key window status when the user clicks into it — WITHOUT
/// activating the app, so the user's frontmost app never loses focus.
#[cfg(target_os = "macos")]
#[allow(deprecated)] // tauri-nspanel v2 re-exports the deprecated `cocoa` crate; its own code does the same.
fn convert_to_panel(app: &tauri::App) -> tauri::Result<()> {
    use tauri_nspanel::cocoa::appkit::NSWindowCollectionBehavior;
    use tauri_nspanel::WebviewWindowExt as _;

    // NSWindowStyleMask.nonactivatingPanel (1 << 7). The window is created
    // with decorations:false (= borderless, mask 0), so this is the only
    // style bit we need.
    const STYLE_MASK_NONACTIVATING_PANEL: i32 = 1 << 7;
    // NSFloatingWindowLevel — above normal windows, below the menu bar.
    // (Replaces the alwaysOnTop window-level from tauri.conf.json.)
    const FLOATING_WINDOW_LEVEL: i32 = 3;

    let window = app
        .get_webview_window("main")
        .expect("main window declared in tauri.conf.json");
    let panel = window.to_panel()?;
    panel.set_style_mask(STYLE_MASK_NONACTIVATING_PANEL);
    panel.set_level(FLOATING_WINDOW_LEVEL);
    // Follow the user to whatever Space/fullscreen app they're in.
    panel.set_collection_behaviour(
        NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary,
    );
    Ok(())
}

/// Bring the prompt UI on screen when an agent asks.
///
/// macOS: order the panel front WITHOUT making it key — keyboard focus stays
/// with the user's current app until they click into the panel (which the
/// nonactivating style permits without app activation). Deliberately NOT
/// `panel.show()`: that calls makeKeyWindow and would grab keystrokes.
fn show_prompt_window(handle: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt as _;
        let h = handle.clone();
        // AppKit ordering calls must run on the main thread; the notify
        // callback fires on the socket server's tokio runtime.
        let queued = handle.run_on_main_thread(move || match h.get_webview_panel("main") {
            Ok(panel) => panel.order_front_regardless(),
            Err(_) => eprintln!("cenno: main window was not converted to a panel"),
        });
        if let Err(e) = queued {
            eprintln!("cenno: failed to dispatch panel show: {e}");
        }
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(win) = handle.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());
    builder
        .setup(|app| {
            let registry = PromptRegistry::new();
            app.manage(registry.clone());

            #[cfg(target_os = "macos")]
            convert_to_panel(app)?;

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
                    show_prompt_window(&handle);
                })
                .await;
                if let Err(e) = result {
                    // Without the socket server this app is an invisible zombie
                    // (window starts hidden, no tray yet) — exit loudly instead.
                    eprintln!("cenno: failed to start MCP socket server: {e}");
                    app_handle.exit(1);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![answer_prompt])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
