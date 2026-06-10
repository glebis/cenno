//! "Check for updates" flow, triggered from the tray menu.
//!
//! Checks the GitHub releases endpoint (tauri.conf.json `plugins.updater`),
//! and walks the user through install + relaunch via native dialogs — the
//! tray is the app's only persistent surface, so dialogs are the only place
//! "you're up to date" / "install now?" feedback can live.
//!
//! Restart caveat: prompts parked in the registry die with the process; the
//! agent sees its normal timeout. That's the same contract as the user
//! quitting from the tray, and the "Install & restart" button label makes
//! the restart explicit.

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

/// Blocking dialogs must stay off the main thread; callers run this on the
/// async runtime (tray.rs spawns it).
pub async fn check_and_install(app: AppHandle) {
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            eprintln!("cenno: updater unavailable: {e}");
            show(
                &app,
                MessageDialogKind::Error,
                "Update check failed",
                &format!("The updater is unavailable: {e}"),
            );
            return;
        }
    };

    match updater.check().await {
        Ok(Some(update)) => {
            let proceed = app
                .dialog()
                .message(format!(
                    "cenno {} is available (you have {}).\n\n\
                     Installing will restart cenno; any prompt currently \
                     on screen times out for its agent as usual.",
                    update.version, update.current_version
                ))
                .title("Update available")
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "Install & restart".into(),
                    "Later".into(),
                ))
                .blocking_show();
            if !proceed {
                eprintln!("cenno: update {} postponed", update.version);
                return;
            }
            match update.download_and_install(|_, _| {}, || {}).await {
                Ok(()) => {
                    eprintln!("cenno: update installed; restarting");
                    app.restart();
                }
                Err(e) => {
                    eprintln!("cenno: update install failed: {e}");
                    show(
                        &app,
                        MessageDialogKind::Error,
                        "Update failed",
                        &format!(
                            "Downloading or installing the update failed: {e}\n\n\
                             You can retry from the tray menu, or download the \
                             latest DMG from GitHub releases."
                        ),
                    );
                }
            }
        }
        Ok(None) => {
            show(
                &app,
                MessageDialogKind::Info,
                "Up to date",
                &format!(
                    "cenno {} is the latest version.",
                    app.package_info().version
                ),
            );
        }
        Err(e) => {
            eprintln!("cenno: update check failed: {e}");
            show(
                &app,
                MessageDialogKind::Error,
                "Update check failed",
                &format!("Could not reach the update server: {e}"),
            );
        }
    }
}

fn show(app: &AppHandle, kind: MessageDialogKind, title: &str, message: &str) {
    app.dialog()
        .message(message)
        .title(title)
        .kind(kind)
        .blocking_show();
}
