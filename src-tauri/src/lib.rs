pub mod a2ui_guard;
pub mod bridge;
pub mod cli;
pub mod db;
pub mod mcp;
pub mod protocol;
pub mod registry;
pub mod suppress;
pub mod tray;

use tauri::{Emitter, Manager};

use crate::protocol::{AskRequest, Via};
use crate::registry::PromptRegistry;
use crate::suppress::SuppressionState;

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

/// The display-gating decision for an arriving (or replaying) prompt, plus
/// the persistence side-effect the lazy pause-expiry implies: when
/// `check()` reports it just cleared an expired pause, the stored
/// "pause_until" setting is cleared too so a restart doesn't resurrect it.
///
/// Public (not pub(crate)) so the socket-level integration test can exercise
/// the exact decision the notify closure uses.
pub fn should_display(
    suppress: &SuppressionState,
    db: Option<&crate::db::Db>,
    fullscreen_check: impl Fn() -> bool,
) -> bool {
    let check = suppress.check(fullscreen_check);
    if check.pause_cleared {
        if let Some(db) = db {
            if let Err(e) = db.set_setting(crate::tray::SETTING_PAUSE_UNTIL, "") {
                eprintln!("cenno: failed to clear persisted pause: {e}");
            }
        }
    }
    !check.suppress
}

/// Newest answerable prompt = highest numeric id suffix ("p_10" > "p_9").
/// Generic over the request payload so the test needs no AskRequest fixture.
fn pick_replay<T>(pending: Vec<(String, T, u64)>) -> Option<(String, T, u64)> {
    pending.into_iter().max_by_key(|(id, _, _)| {
        id.strip_prefix("p_").and_then(|n| n.parse::<u64>().ok()).unwrap_or(0)
    })
}

/// Re-show the newest still-answerable prompt after suppression lifts
/// (tray "Resume now", fullscreen checkbox toggled off, pause-expiry timer).
///
/// Re-checks suppression first: "Resume now" clicked while another app is
/// fullscreen (with hide-in-fullscreen on) must stay quiet — the pending
/// prompt then reappears on the next trigger or the next prompt arrival.
/// Replays via the same PromptEvent path as fresh prompts; remaining_s
/// carries what's left of the budget so the webview's auto-hide stays honest.
pub(crate) fn replay_pending(handle: &tauri::AppHandle) {
    let suppress = handle.state::<SuppressionState>();
    if suppress.should_suppress(crate::suppress::fullscreen_app_present) {
        eprintln!("cenno: replay skipped — still suppressed");
        return;
    }
    let registry = handle.state::<PromptRegistry>();
    if let Some((id, request, remaining_s)) = pick_replay(registry.pending()) {
        eprintln!("cenno: replaying pending prompt {id} ({remaining_s}s left)");
        let payload = PromptEvent { id, request, remaining_s };
        if let Err(e) = handle.emit("prompt", payload) {
            eprintln!("cenno: failed to emit replayed prompt: {e}");
        }
        show_prompt_window(handle);
    }
}

/// Arm a one-shot timer that fires at `until` and, if THIS pause is still the
/// active one (generation unchanged — no re-pause/resume happened meanwhile),
/// clears it, persists the clear, and replays any pending prompt.
///
/// Called when a pause is set via the tray and when a persisted pause is
/// restored at startup. A stale timer (generation moved) is a no-op.
pub(crate) fn arm_pause_expiry_timer(
    handle: &tauri::AppHandle,
    db: Option<crate::db::Db>,
    until: chrono::DateTime<chrono::Utc>,
) {
    let suppress = handle.state::<SuppressionState>().inner().clone();
    let generation = suppress.pause_generation();
    let handle = handle.clone();
    tauri::async_runtime::spawn(async move {
        let wait = (until - chrono::Utc::now()).to_std().unwrap_or_default();
        tokio::time::sleep(wait).await;
        if suppress.pause_generation() != generation {
            return; // re-paused or resumed meanwhile — not our pause anymore
        }
        suppress.resume(); // clears the (just-expired) pause, bumps generation
        if let Some(db) = &db {
            if let Err(e) = db.set_setting(crate::tray::SETTING_PAUSE_UNTIL, "") {
                eprintln!("cenno: failed to clear persisted pause: {e}");
            }
        }
        eprintln!("cenno: pause expired — replaying pending prompts if any");
        replay_pending(&handle);
    });
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

            // The --tray flag no longer gates the tray icon — the menu-bar
            // presence IS the app's home, so setup_tray runs in both modes.
            // Panel conversion must also always happen: it shows nothing
            // (hidden startup is already guaranteed by visible:false in
            // tauri.conf.json), and prompt display depends on the window
            // being a panel.
            let _ = tray;
            #[cfg(target_os = "macos")]
            convert_to_panel(app)?;

            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let sock_path = data_dir.join("mcp.sock");

            // Open (or create) the history database. Failure is non-fatal:
            // the app runs without history rather than refusing to launch.
            let db_path = data_dir.join("cenno.db");
            let db = match crate::db::Db::open(&db_path) {
                Ok(db) => {
                    // Defense in depth: answers are stored as plaintext; restrict
                    // file access to the current user only.
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt as _;
                        if let Err(e) = std::fs::set_permissions(
                            &db_path,
                            std::fs::Permissions::from_mode(0o600),
                        ) {
                            eprintln!("cenno: could not set DB permissions: {e}");
                        }
                    }
                    eprintln!("cenno: history DB opened at {}/cenno.db", data_dir.display());
                    Some(db)
                }
                Err(e) => {
                    eprintln!("cenno: failed to open history DB: {e}");
                    None
                }
            };

            // Suppression state, seeded from persisted settings when the DB
            // is available. Defaults: not paused, hide-in-fullscreen ON.
            // `restored_until` (a still-future persisted pause) is carried
            // out so its expiry timer can be armed once state is managed.
            let (suppress, restored_until) = {
                use crate::tray::{SETTING_HIDE_IN_FULLSCREEN, SETTING_PAUSE_UNTIL};

                // hide_in_fullscreen: absent → default true → write it back
                // so the settings row exists from first launch on.
                let hide_in_fullscreen = match db
                    .as_ref()
                    .and_then(|db| db.get_setting(SETTING_HIDE_IN_FULLSCREEN).ok().flatten())
                {
                    Some(v) => v == "true",
                    None => {
                        if let Some(db) = &db {
                            if let Err(e) = db.set_setting(SETTING_HIDE_IN_FULLSCREEN, "true") {
                                eprintln!("cenno: failed to seed hide_in_fullscreen: {e}");
                            }
                        }
                        true
                    }
                };
                let suppress = crate::suppress::SuppressionState::new(hide_in_fullscreen);

                // pause_until: restore only if it parses AND is still in the
                // future — an expired (or cleared/garbled) value means no pause.
                let mut restored_until = None;
                if let Some(raw) = db
                    .as_ref()
                    .and_then(|db| db.get_setting(SETTING_PAUSE_UNTIL).ok().flatten())
                {
                    if let Ok(until) = chrono::DateTime::parse_from_rfc3339(&raw) {
                        let until = until.with_timezone(&chrono::Utc);
                        if until > chrono::Utc::now() {
                            suppress.restore_pause_until(until);
                            restored_until = Some(until);
                            eprintln!("cenno: restored pause until {until}");
                        }
                    }
                }
                (suppress, restored_until)
            };
            app.manage(suppress.clone());

            // Tray icon + menu — always, in both windowed and --tray modes.
            tray::setup_tray(app.handle(), suppress.clone(), db.clone())?;

            // A restored pause needs its expiry timer re-armed, otherwise
            // prompts suppressed after this restart would only replay
            // lazily (next prompt arrival) instead of at pause end.
            if let Some(until) = restored_until {
                arm_pause_expiry_timer(app.handle(), db.clone(), until);
            }

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
            let suppress_gate = suppress.clone();
            let db_gate = db.clone();
            tauri::async_runtime::spawn(async move {
                let handle = app_handle.clone();
                let result = mcp::start_socket_server(
                    sock_path,
                    registry,
                    move |id, req| {
                        // Display gate: paused or fullscreen → no emit, no
                        // show. The prompt stays pending (registry already
                        // registered it; agent timeout contract unchanged)
                        // and replays when suppression lifts.
                        if !should_display(&suppress_gate, db_gate.as_ref(), crate::suppress::fullscreen_app_present) {
                            eprintln!("cenno: prompt {id} suppressed (paused or fullscreen) — queued for replay");
                            return;
                        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_replay_picks_newest_by_numeric_id() {
        // Numeric, not lexicographic: p_10 beats p_9.
        let pending = vec![("p_2".to_string(), (), 5), ("p_10".to_string(), (), 9), ("p_9".to_string(), (), 1)];
        assert_eq!(pick_replay(pending).unwrap().0, "p_10");
        assert!(pick_replay::<()>(vec![]).is_none());
    }

    #[test]
    fn should_display_true_when_unsuppressed() {
        let s = SuppressionState::new(true);
        assert!(should_display(&s, None, || false));
    }

    #[test]
    fn should_display_false_when_paused_or_fullscreen() {
        let s = SuppressionState::new(true);
        s.pause_for(15);
        assert!(!should_display(&s, None, || false));
        s.resume();
        assert!(!should_display(&s, None, || true));
        s.set_hide_in_fullscreen(false);
        assert!(should_display(&s, None, || true));
    }

    #[test]
    fn should_display_clears_expired_pause_from_db() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::Db::open(&dir.path().join("t.db")).unwrap();
        db.set_setting(crate::tray::SETTING_PAUSE_UNTIL, "2020-01-01T00:00:00Z").unwrap();

        let s = SuppressionState::new(false);
        s.restore_pause_until(chrono::Utc::now() - chrono::Duration::seconds(1));

        assert!(should_display(&s, Some(&db), || false), "expired pause must not suppress");
        assert_eq!(s.snapshot().0, None, "in-memory pause cleared");
        assert_eq!(
            db.get_setting(crate::tray::SETTING_PAUSE_UNTIL).unwrap().as_deref(),
            Some(""),
            "persisted pause cleared so a restart can't resurrect it"
        );
    }
}
