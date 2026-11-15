pub mod commands;
pub mod companies;
pub mod db;
pub mod location;
pub mod models;
pub mod pay;
pub mod prefs;
pub mod rank;
pub mod resume;
pub mod scrapers;
pub mod skills;
pub mod sync;

use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Mutex;

use tauri::Manager;

pub struct AppState {
    /// Guarded by a plain mutex; the lock is never held across an await.
    pub db: Mutex<rusqlite::Connection>,
    pub refreshing: AtomicBool,
    /// Bumped by every write that could change what a ranked read returns.
    /// The ranked cache stores the generation it was built at and rebuilds
    /// when it no longer matches.
    pub(crate) generation: AtomicU64,
    /// The whole visible cache, scored and ordered. Rebuilt lazily, and only
    /// when the data or the reader's profile has actually moved — ranking
    /// 18,000 items is over a second of work that `loadAll()` used to pay for
    /// three times per refresh.
    ///
    /// Lock order is always `ranked` before `db`: the rebuild reads the
    /// database while holding this, and nothing takes them the other way
    /// round.
    pub(crate) ranked: Mutex<Option<commands::Ranked>>,
    /// How a running refresh is stopped. See [`commands::stop_refresh`].
    pub(crate) scan: commands::ScanControl,
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

/// The database could not be opened: hides the window that has nothing to
/// show, and says why in a native dialog whose dismissal quits.
///
/// Not `blocking_show`: `setup` runs on the main thread, where a blocking
/// dialog can deadlock the event loop it needs. The callback form runs inside
/// that loop instead.
fn startup_failed(app: &tauri::App, error: &anyhow::Error) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    let handle = app.handle().clone();
    app.dialog()
        .message(format!(
            "Anti-FOMO could not open its local database.\n\n{error:#}\n\n\
             Another copy of the app may be running and holding it, or the file \
             may be damaged. Close any other copy and try again. If it keeps \
             happening, moving the file aside will let the app start fresh — \
             your saved roles and résumés live in that file, so keep the copy."
        ))
        .title("Anti-FOMO could not start")
        .kind(MessageDialogKind::Error)
        .show(move |_| handle.exit(1));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    apply_linux_webview_workarounds();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                // Stdout is the terminal on desktop and logcat on Android. The
                // log directory is the one a packaged desktop build has: its
                // console is hidden on Windows and nobody runs a .dmg from a
                // shell, so without a file a scraper error or a UI crash on a
                // user's machine left nothing behind to attach to a report.
                // Settings → About opens the folder.
                .clear_targets()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir { file_name: None },
                ))
                .max_file_size(2_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(3))
                .build(),
        )
        .setup(|app| {
            // Per-platform app data dir, so the DB does not depend on the
            // working directory the way the old sqlite:///./antifomo.db did.
            let opened = app
                .path()
                .app_data_dir()
                .map_err(anyhow::Error::from)
                .map(|dir| dir.join("antifomo.db"))
                .and_then(|path| {
                    db::open(&path).map_err(|e| e.context(path.display().to_string()))
                });
            let conn = match opened {
                Ok(conn) => conn,
                Err(e) => {
                    // Returning the error would end in `.expect` below: an
                    // abort with nothing on screen, which on a packaged build
                    // looks like the app simply not opening. Say what failed
                    // and where, then leave.
                    log::error!("could not open the database: {e:#}");
                    startup_failed(app, &e);
                    return Ok(());
                }
            };

            app.manage(AppState {
                db: Mutex::new(conn),
                refreshing: AtomicBool::new(false),
                generation: AtomicU64::new(0),
                ranked: Mutex::new(None),
                scan: commands::ScanControl::default(),
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
            commands::get_last_call,
            commands::get_saved,
            commands::get_item_detail,
            commands::save_pasted_posting,
            commands::list_pasted_postings,
            commands::delete_pasted_posting,
            commands::search_descriptions,
            commands::set_application,
            commands::get_applications,
            commands::feed_status,
            commands::refresh,
            commands::stop_refresh,
            commands::get_scrape_prefs,
            commands::set_scrape_prefs,
            commands::get_onboarding_state,
            commands::complete_onboarding,
            commands::skip_onboarding,
            commands::list_setup_options,
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
            commands::list_resumes,
            commands::get_resume,
            commands::save_resume,
            commands::delete_resume,
            commands::set_default_resume,
            commands::get_resume_variant,
            commands::save_resume_variant,
            commands::clear_resume_variant,
            commands::layout_resume,
            commands::render_resume_pdf,
            commands::import_json_resume,
            commands::export_json_resume,
            commands::list_resume_themes,
            commands::open_log_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Anti-FOMO");
}
