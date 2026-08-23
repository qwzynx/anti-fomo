pub mod commands;
pub mod db;
pub mod location;
pub mod models;
pub mod rank;
pub mod scrapers;
pub mod skills;

use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use tauri::Manager;

pub struct AppState {
    /// Guarded by a plain mutex; the lock is never held across an await.
    pub db: Mutex<rusqlite::Connection>,
    pub refreshing: AtomicBool,
}

/// WebKitGTK's DMABUF renderer crashes the webview on several Wayland driver
/// combinations with "Error 71 (Protocol error) dispatching to Wayland display",
/// taking the window down before it paints. Disabling it costs some GPU
/// compositing but makes the app start reliably. Must run before GTK
/// initialises, and an explicit user setting still wins.
#[cfg(target_os = "linux")]
fn apply_linux_webview_workarounds() {
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    apply_linux_webview_workarounds();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                // A single stdout target: on desktop that is the terminal, on
                // Android the plugin routes it to logcat — the only way to see
                // scraper errors on a device.
                .clear_targets()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .build(),
        )
        .setup(|app| {
            // Per-platform app data dir, so the DB does not depend on the
            // working directory the way the old sqlite:///./antifomo.db did.
            let dir = app.path().app_data_dir()?;
            let conn = db::open(&dir.join("antifomo.db"))?;

            app.manage(AppState {
                db: Mutex::new(conn),
                refreshing: AtomicBool::new(false),
            });

            // Refresh in the background so the window opens immediately on the
            // cached feed instead of blocking on a full scrape.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = commands::refresh(handle, false).await {
                    log::warn!("startup refresh failed: {e}");
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_feed,
            commands::get_internships,
            commands::get_saved,
            commands::feed_status,
            commands::refresh,
            commands::set_saved,
            commands::set_dismissed,
            commands::mark_seen,
            commands::clear_dismissed,
            commands::clear_data,
            commands::list_interests,
            commands::get_interests,
            commands::set_interests,
            commands::list_skills,
            commands::get_skills,
            commands::set_skills,
            commands::get_setting,
            commands::set_setting,
            commands::list_sources,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Anti-FOMO");
}
