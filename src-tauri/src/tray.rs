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

use crate::db::Db;
use crate::suppress::SuppressionState;

/// Settings keys shared with the startup loader in lib.rs.
pub const SETTING_PAUSE_UNTIL: &str = "pause_until";
pub const SETTING_HIDE_IN_FULLSCREEN: &str = "hide_in_fullscreen";
pub const SETTING_LAUNCH_AT_LOGIN: &str = "launch_at_login";

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
        .chain(std::iter::once(&pause_tomorrow as &dyn tauri::menu::IsMenuItem<tauri::Wry>))
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

    let settings =
        MenuItem::with_id(app, "open_settings", "cenno settings…", true, None::<&str>)?;

    // Manual recovery: bring a parked/hidden prompt back on screen (e.g. after
    // it was dismissed or hidden). No-op when nothing is pending — same
    // always-present, harmless-when-empty pattern as "Resume now".
    let show_pending =
        MenuItem::with_id(app, "show_pending", "Show pending prompt", true, None::<&str>)?;

    let check_updates =
        MenuItem::with_id(app, "check_updates", "Check for updates…", true, None::<&str>)?;

    let quit = MenuItem::with_id(app, "quit", "Quit cenno", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &settings,
            &show_pending,
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
                    crate::replay_pending(app);
                }
                "hide_fullscreen" => {
                    let checked = hide_fullscreen_handle.is_checked().unwrap_or(true);
                    suppress.set_hide_in_fullscreen(checked);
                    persist(SETTING_HIDE_IN_FULLSCREEN, if checked { "true" } else { "false" });
                    eprintln!("cenno: hide_in_fullscreen = {checked}");
                    if !checked {
                        // Quiet mode just turned off — surface whatever queued.
                        crate::replay_pending(app);
                    }
                }
                "launch_at_login" => {
                    let checked = launch_login_handle.is_checked().unwrap_or(true);
                    use tauri_plugin_autostart::ManagerExt as _;
                    let autolaunch = app.autolaunch();
                    let result =
                        if checked { autolaunch.enable() } else { autolaunch.disable() };
                    if let Err(e) = result {
                        let verb = if checked { "enable" } else { "disable" };
                        eprintln!("cenno: failed to {verb} launch at login: {e}");
                    }
                    // Persist regardless: the setting records intent, and the
                    // startup reconcile self-heals a failed plugin call.
                    persist(SETTING_LAUNCH_AT_LOGIN, if checked { "true" } else { "false" });
                    eprintln!("cenno: launch_at_login = {checked}");
                }
                "open_settings" => {
                    crate::open_settings_window(app);
                }
                "show_pending" => {
                    // Re-show the front-of-queue parked prompt (restores its
                    // draft on the frontend). Internally re-checks suppression,
                    // so a fullscreen/paused state still keeps it quiet.
                    crate::replay_pending(app);
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
