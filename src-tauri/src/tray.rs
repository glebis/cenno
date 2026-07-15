//! Menu-bar tray: cenno's persistent home.
//!
//! The panel only appears when an agent asks something, so the tray icon is
//! the app's one always-visible surface — it carries the suppression controls
//! (pause / fullscreen quiet mode) and Quit. The icon is a template image
//! (monochrome black + alpha) generated via Codex CLI; macOS recolors it to
//! match the menu bar (light/dark/tinted).

use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle,
};

use tauri::Manager as _;

use crate::capture_guard::{CaptureSnapshot, CaptureState};
use crate::db::Db;
use crate::suppress::SuppressionState;

/// Handle to the "Show pending prompt" menu item, managed as Tauri state so
/// `refresh_pending_item` can update its label/enabled state after the menu
/// is built (registry changes arrive from arbitrary threads).
pub struct PendingMenuItem(MenuItem<tauri::Wry>);

pub struct CaptureMenuItem(CheckMenuItem<tauri::Wry>);

/// Label + enabled state for the show-pending item given the number of
/// answerable prompts. Disabled with an explicit "No pending prompt" when
/// empty — a greyed-out truthful label instead of a silent no-op (cenno-74i).
fn pending_item_state(count: usize) -> (String, bool) {
    match count {
        0 => ("No pending prompt".to_string(), false),
        1 => ("Show pending prompt".to_string(), true),
        n => (format!("Show pending prompts ({n})"), true),
    }
}

/// Sync the tray's show-pending item with the registry. Called from the
/// registry's change watcher (any thread) and once at setup to seed the
/// initial state; menu mutation is hopped onto the main thread, as AppKit
/// requires. No-ops before the tray exists (watcher can't fire earlier —
/// lib.rs installs it after setup_tray).
pub fn refresh_pending_item(app: &AppHandle) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        let Some(item) = app.try_state::<PendingMenuItem>() else {
            return;
        };
        let count = app
            .state::<crate::registry::PromptRegistry>()
            .pending()
            .len();
        let (label, enabled) = pending_item_state(count);
        if let Err(e) = item.0.set_text(&label) {
            eprintln!("cenno: failed to update show_pending label: {e}");
        }
        if let Err(e) = item.0.set_enabled(enabled) {
            eprintln!("cenno: failed to update show_pending enabled: {e}");
        }
    });
}

fn capture_item_state(snapshot: CaptureSnapshot) -> (&'static str, bool) {
    match (snapshot.enabled, snapshot.active) {
        (false, _) => ("Screen context off", false),
        (true, true) => ("Reading screen context…", true),
        (true, false) => ("Screen context allowed", true),
    }
}

pub fn refresh_capture_item(app: &AppHandle, snapshot: CaptureSnapshot) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        let Some(item) = app.try_state::<CaptureMenuItem>() else {
            return;
        };
        let (label, checked) = capture_item_state(snapshot);
        if let Err(e) = item.0.set_text(label) {
            eprintln!("cenno: failed to update capture label: {e}");
        }
        if let Err(e) = item.0.set_checked(checked) {
            eprintln!("cenno: failed to update capture checked state: {e}");
        }
    });
}

/// Settings keys shared with the startup loader in lib.rs.
pub const SETTING_PAUSE_UNTIL: &str = "pause_until";
pub const SETTING_HIDE_IN_FULLSCREEN: &str = "hide_in_fullscreen";
pub const SETTING_LAUNCH_AT_LOGIN: &str = "launch_at_login";
pub const SETTING_CAPTURE_ENABLED: &str = "capture_enabled";

/// (menu id, label, minutes) for the fixed pause durations.
const PAUSE_ITEMS: &[(&str, &str, i64)] = &[
    ("pause_15", "15 min", 15),
    ("pause_30", "30 min", 30),
    ("pause_60", "1 hour", 60),
    ("pause_120", "2 hours", 120),
    ("pause_300", "5 hours", 300),
    ("pause_480", "8 hours", 480),
];

/// Build the tray icon + menu and install the menu-event handlers.
///
/// Called from setup in BOTH windowed and `--tray` modes — the menu-bar
/// presence IS the app's home; the panel is just its transient prompt UI.
///
/// Concrete (Wry) `AppHandle`, not generic over `Runtime`: the handlers call
/// `crate::replay_pending`, which goes through the NSPanel show path — that
/// machinery is Wry-only, and lib.rs is the sole caller anyway.
pub fn setup_tray(
    app: &AppHandle,
    suppress: SuppressionState,
    db: Option<Db>,
    launch_at_login: bool,
    capture_state: CaptureState,
) -> tauri::Result<()> {
    // Template icon: ship the @2x (44px) bytes; macOS downscales to menu-bar
    // size crisply on retina. icon_as_template(true) tells AppKit to treat
    // it as a mask (alpha only) and recolor for the current appearance.
    let icon = Image::from_bytes(include_bytes!("../icons/tray/trayTemplate@2x.png"))?;

    let pause_items: Vec<MenuItem<tauri::Wry>> = PAUSE_ITEMS
        .iter()
        .map(|(id, label, _)| MenuItem::with_id(app, *id, *label, true, None::<&str>))
        .collect::<Result<_, _>>()?;
    let pause_tomorrow =
        MenuItem::with_id(app, "pause_tomorrow", "Until tomorrow", true, None::<&str>)?;
    let pause_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = pause_items
        .iter()
        .map(|i| i as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
        .chain(std::iter::once(
            &pause_tomorrow as &dyn tauri::menu::IsMenuItem<tauri::Wry>,
        ))
        .collect();
    let pause_menu = Submenu::with_id_and_items(app, "pause_for", "Pause for", true, &pause_refs)?;

    // Always present; a no-op when nothing is paused (simplest correct UX).
    let resume = MenuItem::with_id(app, "resume", "Resume now", true, None::<&str>)?;

    // Checked state mirrors SuppressionState, which lib.rs seeded from
    // persisted settings (default ON).
    let (_, hide_fs) = suppress.snapshot();
    let hide_fullscreen = CheckMenuItem::with_id(
        app,
        "hide_fullscreen",
        "Don't show in fullscreen",
        true,
        hide_fs,
        None::<&str>,
    )?;

    // Checked state seeded from the reconciled setting (lib.rs ran the
    // default-on/reconcile logic before building the tray).
    let launch_login = CheckMenuItem::with_id(
        app,
        "launch_at_login",
        "Launch at login",
        true,
        launch_at_login,
        None::<&str>,
    )?;

    let (capture_label, capture_checked) = capture_item_state(capture_state.snapshot());
    let capture_enabled = CheckMenuItem::with_id(
        app,
        "capture_enabled",
        capture_label,
        true,
        capture_checked,
        None::<&str>,
    )?;
    app.manage(CaptureMenuItem(capture_enabled.clone()));

    let settings = MenuItem::with_id(app, "open_settings", "cenno settings…", true, None::<&str>)?;

    // Manual recovery: bring a parked (suppressed or hidden) prompt back on
    // screen. NOT for dismissed/answered/timed-out prompts — those ended their
    // ask() and are gone for good. The item mirrors the registry (see
    // refresh_pending_item): disabled with a "No pending prompt" label when
    // there is nothing answerable, so a dead click is visible as a dead item.
    let (label, enabled) = pending_item_state(0);
    let show_pending = MenuItem::with_id(app, "show_pending", &label, enabled, None::<&str>)?;
    app.manage(PendingMenuItem(show_pending.clone()));

    let check_updates = MenuItem::with_id(
        app,
        "check_updates",
        "Check for updates…",
        true,
        None::<&str>,
    )?;

    let quit = MenuItem::with_id(app, "quit", "Quit cenno", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &settings,
            &show_pending,
            &PredefinedMenuItem::separator(app)?,
            &capture_enabled,
            &PredefinedMenuItem::separator(app)?,
            &pause_menu,
            &resume,
            &PredefinedMenuItem::separator(app)?,
            &hide_fullscreen,
            &launch_login,
            &PredefinedMenuItem::separator(app)?,
            &check_updates,
            &quit,
        ],
    )?;

    // CheckMenuItem toggles its own checked state on click; the handler reads
    // it back. Keep a handle alive inside the closure for that.
    let hide_fullscreen_handle = hide_fullscreen.clone();
    let launch_login_handle = launch_login.clone();
    let capture_enabled_handle = capture_enabled.clone();

    TrayIconBuilder::with_id("cenno-tray")
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();

            // Persist helper — settings writes must never break the menu.
            let persist = |key: &str, value: &str| {
                if let Some(db) = &db {
                    if let Err(e) = db.set_setting(key, value) {
                        eprintln!("cenno: failed to persist {key}: {e}");
                    }
                }
            };

            if let Some((_, _, minutes)) = PAUSE_ITEMS.iter().find(|(pid, _, _)| *pid == id) {
                let until = suppress.pause_for(*minutes);
                persist(SETTING_PAUSE_UNTIL, &until.to_rfc3339());
                // Expiry timer: replays any still-pending prompt when the
                // pause runs out (no-op if re-paused/resumed meanwhile).
                crate::arm_pause_expiry_timer(app, db.clone(), until);
                eprintln!("cenno: paused until {until}");
                return;
            }

            match id {
                "capture_enabled" => {
                    let checked = capture_enabled_handle.is_checked().unwrap_or(false);
                    capture_state.set_enabled(checked);
                    persist(
                        SETTING_CAPTURE_ENABLED,
                        if checked { "true" } else { "false" },
                    );
                    eprintln!("cenno: screen context allowed = {checked}");
                }
                "pause_tomorrow" => {
                    let until = suppress.pause_until_tomorrow();
                    persist(SETTING_PAUSE_UNTIL, &until.to_rfc3339());
                    crate::arm_pause_expiry_timer(app, db.clone(), until);
                    eprintln!("cenno: paused until tomorrow ({until})");
                }
                "resume" => {
                    suppress.resume();
                    // Empty value = no pause (loader treats unparseable as none).
                    persist(SETTING_PAUSE_UNTIL, "");
                    eprintln!("cenno: resumed");
                    // Re-show anything that arrived while paused. (Internally
                    // re-checks suppression — a fullscreen app keeps it quiet.)
                    crate::replay_pending(app, false);
                }
                "hide_fullscreen" => {
                    let checked = hide_fullscreen_handle.is_checked().unwrap_or(true);
                    suppress.set_hide_in_fullscreen(checked);
                    persist(
                        SETTING_HIDE_IN_FULLSCREEN,
                        if checked { "true" } else { "false" },
                    );
                    eprintln!("cenno: hide_in_fullscreen = {checked}");
                    if !checked {
                        // Quiet mode just turned off — surface whatever queued.
                        crate::replay_pending(app, false);
                    }
                }
                "launch_at_login" => {
                    let checked = launch_login_handle.is_checked().unwrap_or(true);
                    use tauri_plugin_autostart::ManagerExt as _;
                    let autolaunch = app.autolaunch();
                    let result = if checked {
                        autolaunch.enable()
                    } else {
                        autolaunch.disable()
                    };
                    if let Err(e) = result {
                        let verb = if checked { "enable" } else { "disable" };
                        eprintln!("cenno: failed to {verb} launch at login: {e}");
                    }
                    // Persist regardless: the setting records intent, and the
                    // startup reconcile self-heals a failed plugin call.
                    persist(
                        SETTING_LAUNCH_AT_LOGIN,
                        if checked { "true" } else { "false" },
                    );
                    eprintln!("cenno: launch_at_login = {checked}");
                }
                "open_settings" => {
                    crate::open_settings_window(app);
                }
                "show_pending" => {
                    // Re-show the front-of-queue parked prompt (restores its
                    // draft on the frontend). Explicit user gesture → force
                    // past suppression: clicking this WHILE paused/fullscreen
                    // is precisely a request to see the prompt anyway. The
                    // pause itself stays in effect for future prompts.
                    crate::replay_pending(app, true);
                }
                "check_updates" => {
                    // Off the menu-event (main) thread: the flow blocks on
                    // dialogs and the download. Repeated clicks just queue
                    // further checks — harmless, the dialog serializes them.
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::updater::check_and_install(app).await;
                    });
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{capture_item_state, pending_item_state};
    use crate::capture_guard::CaptureSnapshot;

    /// Empty queue → disabled item that says so; one prompt → the classic
    /// label; several → the label carries the count.
    #[test]
    fn pending_item_state_reflects_queue_depth() {
        assert_eq!(
            pending_item_state(0),
            ("No pending prompt".to_string(), false)
        );
        assert_eq!(
            pending_item_state(1),
            ("Show pending prompt".to_string(), true)
        );
        assert_eq!(
            pending_item_state(3),
            ("Show pending prompts (3)".to_string(), true)
        );
    }

    #[test]
    fn capture_item_state_reflects_disabled_idle_and_active() {
        assert_eq!(
            capture_item_state(CaptureSnapshot {
                enabled: false,
                active: false
            }),
            ("Screen context off", false)
        );
        assert_eq!(
            capture_item_state(CaptureSnapshot {
                enabled: true,
                active: false
            }),
            ("Screen context allowed", true)
        );
        assert_eq!(
            capture_item_state(CaptureSnapshot {
                enabled: true,
                active: true
            }),
            ("Reading screen context…", true)
        );
    }
}
