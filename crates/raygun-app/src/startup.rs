//! Startup sequence: resolve the data root, start logging, open the database, then create the
//! main window.
//!
//! The window is created here rather than from `tauri.conf.json` (where it is marked
//! `"create": false`) so the webview profile lands inside the data root instead of the
//! OS profile folder.

use std::error::Error;

use raygun_paths::{Environment, Paths, Resolution};
use tauri::{App, Manager, WebviewWindowBuilder};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

const MAIN_WINDOW: &str = "main";
const LOG_FILES_KEPT: usize = 7;

/// Resolved storage locations, available to commands as Tauri state.
pub struct AppPaths(pub Paths);

/// The open database, available to commands as Tauri state.
pub struct AppDb(pub raygun_db::Db);

/// Keeps the background log writer alive for the app's lifetime.
struct LogGuard(#[allow(dead_code)] WorkerGuard);

pub fn init(app: &mut App) -> Result<(), Box<dyn Error>> {
    let paths = resolve_paths(app)?;
    paths.ensure_base_dirs()?;
    let guard = init_logging(&paths)?;
    tracing::info!(
        mode = ?paths.mode(),
        root = %paths.root().display(),
        version = env!("CARGO_PKG_VERSION"),
        "storage resolved"
    );

    let db = tauri::async_runtime::block_on(raygun_db::Db::open(&paths.db()))?;
    tracing::info!(device_id = %db.device_id(), "database ready");

    create_main_window(app, &paths)?;

    app.manage(LogGuard(guard));
    app.manage(AppPaths(paths));
    app.manage(AppDb(db));
    Ok(())
}

fn resolve_paths(app: &App) -> Result<Paths, Box<dyn Error>> {
    // The one sanctioned use of Tauri's app-dir API (see clippy.toml): it supplies the
    // sandbox on mobile and the installed-mode location on desktop. Nothing else may
    // compute OS folders.
    #[allow(clippy::disallowed_methods)]
    let system_dir = app.path().app_local_data_dir().ok();
    let environment = Environment::from_process(system_dir)?;

    match raygun_paths::resolve(&environment)? {
        Resolution::Ready(paths) => Ok(paths),
        Resolution::NeedsChoice {
            reason,
            portable_root,
        } => {
            if ask_to_use_installed_mode(reason, &portable_root) {
                Ok(raygun_paths::choose_installed(&environment)?)
            } else {
                std::process::exit(0);
            }
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn ask_to_use_installed_mode(
    reason: raygun_paths::ChoiceReason,
    portable_root: &std::path::Path,
) -> bool {
    use raygun_paths::ChoiceReason;
    use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

    const USE_SYSTEM: &str = "Use system folders";
    let description = match reason {
        ChoiceReason::Unwritable => format!(
            "Raygun keeps all its data next to the app so it stays portable, but it can't write to:\n\n{}\n\nMove Raygun to a folder you can write to, or store data in your system's app-data folder instead.",
            portable_root.display()
        ),
        ChoiceReason::Translocated => "macOS is running Raygun from a temporary read-only copy because it was downloaded from the internet.\n\nMove Raygun.app to another folder (or clear its quarantine flag) and open it again to keep it portable, or store data in your system's app-data folder instead.".to_owned(),
    };

    let result = MessageDialog::new()
        .set_level(MessageLevel::Warning)
        .set_title("Raygun can't store data next to the app")
        .set_description(description)
        .set_buttons(MessageButtons::OkCancelCustom(
            USE_SYSTEM.to_owned(),
            "Quit".to_owned(),
        ))
        .show();
    result == MessageDialogResult::Custom(USE_SYSTEM.to_owned())
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn ask_to_use_installed_mode(
    _reason: raygun_paths::ChoiceReason,
    _portable_root: &std::path::Path,
) -> bool {
    unreachable!("mobile builds always resolve to the app sandbox")
}

fn init_logging(paths: &Paths) -> Result<WorkerGuard, Box<dyn Error>> {
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("raygun")
        .filename_suffix("log")
        .max_log_files(LOG_FILES_KEPT)
        .build(paths.logs())?;
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let filter = || EnvFilter::try_from_env("RAYGUN_LOG").unwrap_or_else(|_| "info".into());
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(writer)
        .with_filter(filter());
    let stderr_layer = cfg!(debug_assertions).then(|| {
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_filter(filter())
    });

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer)
        .try_init()?;
    Ok(guard)
}

fn create_main_window(app: &App, paths: &Paths) -> Result<(), Box<dyn Error>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == MAIN_WINDOW)
        .ok_or("tauri.conf.json has no `main` window")?
        .clone();

    // Honored by WebView2 (Windows) and WebKitGTK (Linux). Android keeps webview data in
    // the app sandbox anyway; macOS/iOS ignore it (documented limitation, ARCHITECTURE §3.3).
    WebviewWindowBuilder::from_config(app.handle(), &config)?
        .data_directory(paths.webview())
        .build()?;
    Ok(())
}
