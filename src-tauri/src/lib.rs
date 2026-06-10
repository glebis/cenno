pub mod a2ui_guard;
pub mod bridge;
pub mod cli;
pub mod db;
pub mod mcp;
pub mod protocol;
pub mod registry;
pub mod suppress;
pub mod tray;
pub mod updater;

use tauri::{Emitter, Manager};

use crate::protocol::{AskRequest, SeqMeta, Via};
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
    /// Set only for prompts emitted by an `ask_sequence` run; tells the
    /// frontend to swap content (not hide) between steps and to hide only
    /// after `last`. Absent for plain `ask_user` (and replayed) prompts —
    /// `skip_serializing_if` keeps the single-ask wire shape byte-identical.
    #[serde(skip_serializing_if = "Option::is_none")]
    seq: Option<SeqMeta>,
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
        .map(|(id, request, remaining_s)| PromptEvent { id, request, remaining_s, seq: None })
        .collect()
}

/// Panel geometry: width is fixed by design; height adapts to the prompt's
/// content (the webview measures itself and calls `resize_panel`). The band
/// keeps a degenerate measurement from collapsing the panel or letting a
/// runaway payload cover the screen.
const PANEL_WIDTH: f64 = 420.0;
const PANEL_MIN_HEIGHT: f64 = 240.0;
const PANEL_MAX_HEIGHT: f64 = 560.0;

/// Clamp a webview-reported content height to the allowed panel band.
/// Non-finite input (NaN/∞ — nothing sane measures to that) falls back to
/// the design minimum instead of poisoning the clamp.
fn clamp_panel_height(height: f64) -> f64 {
    if !height.is_finite() {
        return PANEL_MIN_HEIGHT;
    }
    height.clamp(PANEL_MIN_HEIGHT, PANEL_MAX_HEIGHT)
}

/// Content-driven panel height: the webview measures the rendered prompt
/// (see src/panelResize.ts) and asks for a window that fits it. Width stays
/// fixed. Works on the swizzled NSPanel too — setClass doesn't change how
/// tao applies frame sizes, and the nspanel conversion installed autoresizing
/// masks so the webview follows the new frame.
///
/// The window-state plugin persists POSITION only (see the builder below),
/// so this resize never fights a restored size.
#[tauri::command]
fn resize_panel(window: tauri::WebviewWindow, height: f64) {
    let height = clamp_panel_height(height);
    if let Err(e) = window.set_size(tauri::LogicalSize::new(PANEL_WIDTH, height)) {
        eprintln!("cenno: resize_panel({height}) failed: {e}");
    }
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

/// User dismissed the panel (clicked ✕): end the parked `ask()` as a
/// no-answer (TimedOut), the same wire shape the agent already handles on
/// timeout — no protocol change. Returns false if the prompt already
/// resolved/timed out (unknown or already-consumed id).
#[tauri::command]
fn dismiss_prompt(state: tauri::State<PromptRegistry>, id: String) -> bool {
    state.dismiss(&id)
}

/// Startup decision for launch-at-login: `(enabled, persist_default)`.
/// Pure — the actual plugin call is glue (see `reconcile_launch_at_login`).
///
/// Absent setting → ON and write the default back so the settings row exists
/// from first launch on (mirror of the hide_in_fullscreen seeding pattern).
/// Present → honor it verbatim ("true" → ON, anything else → OFF).
fn launch_at_login_decision(stored: Option<&str>) -> (bool, bool) {
    match stored {
        Some(v) => (v == "true", false),
        None => (true, true),
    }
}

/// Reconcile the persisted `launch_at_login` setting with the OS autostart
/// registration at startup, and return the resulting enabled state (used to
/// seed the tray checkbox).
///
/// `apply` performs the actual OS-level enable/disable (the autostart plugin
/// in production, a probe in tests). Calling it unconditionally on every
/// startup is idempotent and self-heals an entry the user removed (or an
/// enable that never happened because the DB write raced a crash).
pub fn reconcile_launch_at_login(
    db: Option<&crate::db::Db>,
    apply: impl FnOnce(bool) -> Result<(), String>,
) -> bool {
    let stored = db.and_then(|db| {
        db.get_setting(crate::tray::SETTING_LAUNCH_AT_LOGIN)
            .ok()
            .flatten()
    });
    let (enabled, persist_default) = launch_at_login_decision(stored.as_deref());
    if persist_default {
        if let Some(db) = db {
            if let Err(e) = db.set_setting(crate::tray::SETTING_LAUNCH_AT_LOGIN, "true") {
                eprintln!("cenno: failed to seed launch_at_login: {e}");
            }
        }
    }
    if let Err(e) = apply(enabled) {
        let verb = if enabled { "enable" } else { "disable" };
        eprintln!("cenno: failed to {verb} launch at login: {e}");
    }
    enabled
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

/// The display target for the fullscreen check: where would the panel
/// appear right now? Resolved at CHECK time (not startup) so a panel the
/// user dragged to another monitor is honored.
///
/// Preferred: the panel window's current monitor, as a logical-points rect —
/// the same top-left-origin global space as `CGDisplay::bounds()`, which is
/// what `fullscreen_on_display` compares against. Fallback: the panel's own
/// frame (an ordered-out NSPanel can report no screen); the detector maps
/// its center to a display. Returns None when even the window lookup fails —
/// the detector then falls back to cursor position, then the main display.
fn panel_display_target(handle: &tauri::AppHandle) -> Option<crate::suppress::Rect> {
    let win = handle.get_webview_window("main")?;
    if let Ok(Some(mon)) = win.current_monitor() {
        let scale = mon.scale_factor();
        let pos = mon.position().to_logical::<f64>(scale);
        let size = mon.size().to_logical::<f64>(scale);
        return Some(crate::suppress::Rect { x: pos.x, y: pos.y, w: size.width, h: size.height });
    }
    let scale = win.scale_factor().ok()?;
    let pos = win.outer_position().ok()?.to_logical::<f64>(scale);
    let size = win.outer_size().ok()?.to_logical::<f64>(scale);
    Some(crate::suppress::Rect { x: pos.x, y: pos.y, w: size.width, h: size.height })
}

/// The production fullscreen check: scoped to the display the panel lives
/// on. Built fresh per call so every suppression decision re-resolves the
/// panel's display.
fn fullscreen_on_panel_display(handle: &tauri::AppHandle) -> bool {
    crate::suppress::fullscreen_on_display(panel_display_target(handle))
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
    if suppress.should_suppress(|| fullscreen_on_panel_display(handle)) {
        eprintln!("cenno: replay skipped — still suppressed");
        return;
    }
    let registry = handle.state::<PromptRegistry>();
    if let Some((id, request, remaining_s)) = pick_replay(registry.pending()) {
        eprintln!("cenno: replaying pending prompt {id} ({remaining_s}s left)");
        let payload = PromptEvent { id, request, remaining_s, seq: None };
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
///
/// First-click answers: while the panel is not key, AppKit would normally
/// swallow the first mouse-down (it only makes the window key); the user's
/// first click on a chip then does nothing — the answer needed a SECOND
/// click. `acceptFirstMouse: true` in tauri.conf.json fixes that: wry's
/// WryWebView subclass overrides `acceptsFirstMouse:` from an ivar captured
/// at webview creation, so the same click that keys the panel also reaches
/// the button. The override lives on the webview VIEW, not the window, so
/// the `object_setClass` panel swap below cannot undo it.
#[cfg(target_os = "macos")]
#[allow(deprecated)] // tauri-nspanel v2 re-exports the deprecated `cocoa` crate; its own code does the same.
fn convert_to_panel(app: &tauri::App) -> tauri::Result<()> {
    use tauri_nspanel::cocoa::appkit::NSWindowCollectionBehavior;
    use tauri_nspanel::WebviewWindowExt as _;

    // NSWindowStyleMask.nonactivatingPanel (1 << 7). Replaces tao's
    // Borderless|Resizable|Miniaturizable mask; we intentionally drop
    // resize/miniaturize — the USER can't resize the panel (resize_panel
    // sets the frame programmatically, which needs no style-mask bit).
    // Key-window eligibility comes from RawNSPanel's canBecomeKeyWindow
    // override, not the mask.
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
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // GitHub-releases updater + the dialogs its tray-menu flow shows
        // (updater.rs). Endpoint/pubkey live in tauri.conf.json `plugins.updater`.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        // Launch-at-login registration. LaunchAgent (not AppleScript): a
        // plist in ~/Library/LaunchAgents survives the app not being in
        // /Applications and needs no Automation permission. `--tray` keeps
        // the login launch headless (panel hidden until a prompt arrives).
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--tray"]),
        ))
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

            // The tray icon always runs — the menu-bar presence IS the app's
            // home, regardless of how cenno was launched. Panel conversion
            // must also always happen: it shows nothing (hidden startup is
            // already guaranteed by visible:false in tauri.conf.json), and
            // prompt display depends on the window being a panel.
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

            // Launch at login: default ON. Decide from the persisted setting
            // (absent → enable + write back) and reconcile the OS state to
            // match — idempotent on every startup, so a login item the user
            // deleted out-of-band comes back while the setting says ON.
            let launch_at_login = {
                use tauri_plugin_autostart::ManagerExt as _;
                let manager = app.autolaunch();
                reconcile_launch_at_login(db.as_ref(), |enable| {
                    if enable { manager.enable() } else { manager.disable() }
                        .map_err(|e| e.to_string())
                })
            };

            // Tray icon + menu — always, in both windowed and --tray modes.
            tray::setup_tray(app.handle(), suppress.clone(), db.clone(), launch_at_login)?;

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
                    move |id, req, seq| {
                        // Display gate: paused or fullscreen ON THE PANEL'S
                        // display → no emit, no show. The prompt stays
                        // pending (registry already registered it; agent
                        // timeout contract unchanged) and replays when
                        // suppression lifts.
                        if !should_display(&suppress_gate, db_gate.as_ref(), || fullscreen_on_panel_display(&handle)) {
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
                            // None for plain ask_user; Some for ask_sequence steps.
                            seq,
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
        .invoke_handler(tauri::generate_handler![answer_prompt, dismiss_prompt, pending_prompts, resize_panel])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_panel_height_clamps_to_band() {
        assert_eq!(clamp_panel_height(100.0), PANEL_MIN_HEIGHT);
        assert_eq!(clamp_panel_height(240.0), 240.0);
        assert_eq!(clamp_panel_height(381.5), 381.5);
        assert_eq!(clamp_panel_height(560.0), 560.0);
        assert_eq!(clamp_panel_height(10_000.0), PANEL_MAX_HEIGHT);
    }

    #[test]
    fn clamp_panel_height_rejects_non_finite() {
        // NaN/∞ can only come from a buggy or hostile webview — fall back to
        // the design minimum rather than letting NaN through f64::clamp.
        assert_eq!(clamp_panel_height(f64::NAN), PANEL_MIN_HEIGHT);
        assert_eq!(clamp_panel_height(f64::INFINITY), PANEL_MIN_HEIGHT);
        assert_eq!(clamp_panel_height(f64::NEG_INFINITY), PANEL_MIN_HEIGHT);
    }

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
    fn launch_at_login_decision_defaults_on_and_persists() {
        // Absent → ON, write the default back (default-on requirement).
        assert_eq!(launch_at_login_decision(None), (true, true));
        // Present → honored verbatim, no re-persist.
        assert_eq!(launch_at_login_decision(Some("true")), (true, false));
        assert_eq!(launch_at_login_decision(Some("false")), (false, false));
        // Garbled value fails closed (no autostart) rather than guessing.
        assert_eq!(launch_at_login_decision(Some("yes")), (false, false));
    }

    #[test]
    fn reconcile_launch_at_login_seeds_default_and_enables() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::Db::open(&dir.path().join("t.db")).unwrap();

        let applied = std::cell::Cell::new(None);
        let enabled = reconcile_launch_at_login(Some(&db), |on| {
            applied.set(Some(on));
            Ok(())
        });

        assert!(enabled);
        assert_eq!(applied.get(), Some(true), "OS autostart enabled");
        assert_eq!(
            db.get_setting(crate::tray::SETTING_LAUNCH_AT_LOGIN).unwrap().as_deref(),
            Some("true"),
            "default written back so the settings row exists from first launch"
        );
    }

    #[test]
    fn reconcile_launch_at_login_honors_stored_setting() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::Db::open(&dir.path().join("t.db")).unwrap();

        // Stored OFF → disable on startup (reconciles a stale OS entry).
        db.set_setting(crate::tray::SETTING_LAUNCH_AT_LOGIN, "false").unwrap();
        let applied = std::cell::Cell::new(None);
        assert!(!reconcile_launch_at_login(Some(&db), |on| {
            applied.set(Some(on));
            Ok(())
        }));
        assert_eq!(applied.get(), Some(false));
        assert_eq!(
            db.get_setting(crate::tray::SETTING_LAUNCH_AT_LOGIN).unwrap().as_deref(),
            Some("false"),
            "stored setting not overwritten"
        );

        // Stored ON → enable (idempotent self-heal of a removed entry).
        db.set_setting(crate::tray::SETTING_LAUNCH_AT_LOGIN, "true").unwrap();
        let applied = std::cell::Cell::new(None);
        assert!(reconcile_launch_at_login(Some(&db), |on| {
            applied.set(Some(on));
            Ok(())
        }));
        assert_eq!(applied.get(), Some(true));
    }

    #[test]
    fn reconcile_launch_at_login_survives_no_db_and_apply_failure() {
        // No DB → default ON still applied (app runs without history).
        let applied = std::cell::Cell::new(None);
        assert!(reconcile_launch_at_login(None, |on| {
            applied.set(Some(on));
            Ok(())
        }));
        assert_eq!(applied.get(), Some(true));

        // A failing plugin call must not panic and still reports the
        // intended state (the tray checkbox shows intent, not OS truth).
        assert!(reconcile_launch_at_login(None, |_| Err("nope".into())));
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
