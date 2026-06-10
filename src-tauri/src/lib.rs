pub mod a2ui_guard;
pub mod bridge;
pub mod cli;
pub mod db;
pub mod mcp;
pub mod protocol;
pub mod registry;

use tauri::{Emitter, Manager};

use crate::protocol::{AskRequest, Via};
use crate::registry::PromptRegistry;

/// Payload of the `prompt` event emitted to the webview when an agent asks.
/// Also returned by the `pending_prompts` command (same wire shape) so the
/// webview can replay prompts it missed.
#[derive(Clone, serde::Serialize)]
struct PromptEvent {
    id: String,
    request: AskRequest,
    /// Seconds left before the Rust side times this prompt out. Fresh
    /// prompts carry the full timeout_s; prompts replayed via
    /// pending_prompts carry only what remains of their budget. The webview
    /// arms its auto-hide timer from this so a stale prompt never lingers
    /// past the moment the agent already received TimedOut.
    remaining_s: u64,
}

/// Cold-start race recovery: the agent's first ask can arrive (and emit the
/// `prompt` event) before the webview has mounted its listener. The webview
/// calls this right after registering `listen("prompt")` to pull anything
/// still answerable. Ordered oldest→newest.
#[tauri::command]
fn pending_prompts(state: tauri::State<PromptRegistry>) -> Vec<PromptEvent> {
    state
        .pending()
        .into_iter()
        .map(|(id, request, remaining_s)| PromptEvent { id, request, remaining_s })
        .collect()
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

    // NSWindowStyleMask.nonactivatingPanel (1 << 7). Replaces tao's
    // Borderless|Resizable|Miniaturizable mask; we intentionally drop
    // resize/miniaturize — the panel is fixed-size. Key-window eligibility
    // comes from RawNSPanel's canBecomeKeyWindow override, not the mask.
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
        // Counterpart: after answering, the webview hides itself via
        // window.hide() (= orderOut:) — pairs correctly with
        // order_front_regardless(), no key/activation state to undo.
        let queued = handle.run_on_main_thread(move || match h.get_webview_panel("main") {
            Ok(panel) => panel.order_front_regardless(),
            // {e:?}: tauri_nspanel::Error has no Display impl.
            Err(e) => eprintln!("cenno: main window was not converted to a panel: {e:?}"),
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
pub fn run(tray: bool) {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // Restore the panel's last position on launch (POSITION only — size
        // is fixed by design, and VISIBLE must stay out so the window keeps
        // its hidden-until-asked startup from tauri.conf.json).
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(tauri_plugin_window_state::StateFlags::POSITION)
                .build(),
        )
        // The plugin only persists on graceful exit / window close, but this
        // app lives until it's killed (no quit UI yet) — save on every move
        // instead so a drag survives a SIGTERM. Writes are a tiny JSON file;
        // move events are rare (user repositioning the panel by hand).
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Moved(_) = event {
                use tauri_plugin_window_state::AppHandleExt as _;
                if let Err(e) = window
                    .app_handle()
                    .save_window_state(tauri_plugin_window_state::StateFlags::POSITION)
                {
                    eprintln!("cenno: failed to save window position: {e}");
                }
            }
        });
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());
    builder
        .setup(move |app| {
            let registry = PromptRegistry::new();
            app.manage(registry.clone());

            // tray flag reserved for future tray-icon setup — panel conversion
            // must always happen: it shows nothing (hidden startup is already
            // guaranteed by visible:false in tauri.conf.json), and prompt
            // display depends on the window being a panel.
            let _ = tray;
            #[cfg(target_os = "macos")]
            convert_to_panel(app)?;

            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let sock_path = data_dir.join("mcp.sock");

            // Open (or create) the history database. Failure is non-fatal:
            // the app runs without history rather than refusing to launch.
            let db = match crate::db::Db::open(&data_dir.join("cenno.db")) {
                Ok(db) => {
                    eprintln!("cenno: history DB opened at {}/cenno.db", data_dir.display());
                    Some(db)
                }
                Err(e) => {
                    eprintln!("cenno: failed to open history DB: {e}");
                    None
                }
            };

            // Invariant: mcp::socket_path() must agree with what Tauri resolves.
            // Catch any divergence early in debug builds.
            #[cfg(debug_assertions)]
            {
                let canonical = mcp::socket_path();
                assert_eq!(
                    sock_path, canonical,
                    "socket path mismatch: lib.rs={sock_path:?} mcp::socket_path()={canonical:?}"
                );
            }

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let handle = app_handle.clone();
                let result = mcp::start_socket_server(
                    sock_path,
                    registry,
                    move |id, req| {
                        // Called from the socket server's tokio runtime; both
                        // emit() and window calls are thread-safe in Tauri 2.
                        let payload = PromptEvent {
                            id: id.to_string(),
                            request: req.clone(),
                            // A notify fires at ask() registration: nothing has
                            // elapsed yet, so the full budget remains.
                            remaining_s: req.timeout_s,
                        };
                        if let Err(e) = handle.emit("prompt", payload) {
                            eprintln!("cenno: failed to emit prompt event: {e}");
                        }
                        show_prompt_window(&handle);
                    },
                    db,
                )
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
        .invoke_handler(tauri::generate_handler![answer_prompt, pending_prompts])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
