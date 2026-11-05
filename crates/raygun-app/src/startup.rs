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
use tracing_subscriber::filter::FilterExt;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

const MAIN_WINDOW: &str = "main";
const LOG_FILES_KEPT: usize = 7;

/// Resolved storage locations, available to commands as Tauri state.
pub struct AppPaths(pub Paths);

/// The open database, available to commands as Tauri state.
pub struct AppDb(pub std::sync::Arc<raygun_db::Db>);

/// Keeps the background log writer alive for the app's lifetime.
struct LogGuard(#[allow(dead_code)] WorkerGuard);

pub fn init(app: &mut App) -> Result<(), Box<dyn Error>> {
    let paths = resolve_paths(app)?;
    // Erase everything (M46.51) happens here, before anything opens the data root.
    let erased = crate::backup::erase::finish_pending(paths.root(), paths.mode());
    paths.ensure_base_dirs()?;
    let guard = init_logging(&paths)?;
    tracing::info!(
        mode = ?paths.mode(),
        root = %paths.root().display(),
        version = env!("CARGO_PKG_VERSION"),
        "storage resolved"
    );
    if let Some(erased) = erased {
        tracing::warn!(
            removed = erased.removed,
            failed = ?erased.failed,
            "erased everything but the backups"
        );
    }

    let db = tauri::async_runtime::block_on(raygun_db::Db::open(&paths.db()))?;
    tracing::info!(device_id = %db.device_id(), "database ready");
    // Before the services: some read the database from background tasks right away.
    app.manage(AppDb(std::sync::Arc::new(db)));
    // Incognito (M47), known before anything records.
    let privacy =
        tauri::async_runtime::block_on(crate::privacy::Privacy::load(&app.state::<AppDb>().0));
    app.manage(privacy);

    let art_dir = paths.cache(raygun_paths::CacheKind::Images);
    // Image caches with their limits from settings (M46.2).
    let startup_settings =
        tauri::async_runtime::block_on(crate::settings::load(&app.state::<AppDb>().0))
            .unwrap_or_default();
    let cache_prefs = startup_settings.cache;
    crate::device_state::set_downloads_allowed(startup_settings.downloads_in_battery_saver);
    let caches = crate::caches::Caches::new(&paths, &cache_prefs);
    let proxy = tauri::async_runtime::block_on(raygun_net::StreamProxy::start_with(
        raygun_net::http_client()?,
        raygun_net::ProxyStorage {
            artwork: Some(raygun_net::StaticFiles {
                dir: art_dir.clone(),
                is_allowed: raygun_library::is_artwork_name,
            }),
            pages: Some(caches.pages.clone()),
            covers: Some(caches.covers.clone()),
            local: Some(paths.downloads()),
            port_file: Some(paths.root().join("proxy-port")),
        },
    ))?;
    app.manage(caches);
    app.manage(std::sync::Arc::new(crate::websites::WebsiteService::new(
        app.handle().clone(),
        paths.root().join("web"),
    )?));
    app.manage(crate::browser_check::BrowserChecks::new(
        app.handle().clone(),
        paths.root().join("web").join("checks"),
    ));
    crate::caches::sweep_later(app.handle());
    app.manage(proxy.images());
    app.manage(proxy.media());
    app.manage(proxy.lan());
    app.manage(crate::library::LibraryService::new(
        app.handle().clone(),
        art_dir,
        proxy.address(),
    ));
    app.manage(crate::uploads::Uploads::new(
        paths.downloads().join(".uploads"),
    ));
    app.manage(crate::manga::MangaService::new(
        app.handle().clone(),
        proxy.images(),
        paths.downloads(),
    ));
    app.manage(crate::novel::NovelService::new(
        app.handle().clone(),
        proxy.images(),
        paths.downloads(),
    ));
    app.manage(crate::video_session::VideoSessions::new(
        app.handle().clone(),
    ));
    app.manage(crate::video_sources::VideoSourceService::new(
        app.handle().clone(),
        proxy.images(),
    ));
    let downloads = crate::manga_downloads::MangaDownloadService::new(
        app.handle().clone(),
        paths.downloads(),
        proxy.images(),
        raygun_net::http_client()?,
    );
    app.manage(downloads.clone());
    let video_downloads = crate::video_downloads::VideoDownloadService::new(
        app.handle().clone(),
        paths.downloads(),
        proxy.images(),
        // No overall timeout (films are large), but a stalled connection gives up.
        raygun_net::client_builder()
            .connect_timeout(std::time::Duration::from_secs(20))
            .read_timeout(std::time::Duration::from_secs(45))
            .build()?,
    );
    app.manage(video_downloads.clone());
    app.manage(std::sync::Arc::new(crate::export::ExportService::new(
        app.handle().clone(),
        crate::export::Outputs::new(
            app.handle().clone(),
            paths.exports(),
            paths.cache(raygun_paths::CacheKind::Share),
        ),
    )?));
    let podcasts = std::sync::Arc::new(crate::podcasts::PodcastService::new(
        app.handle().clone(),
        raygun_net::http_client()?,
        proxy.images(),
        paths.downloads(),
    ));
    app.manage(podcasts.clone());
    // Captions for podcasts and audiobooks (M46.32), and songs' words waiting from the last run.
    let captions = crate::captions::CaptionService::new(
        app.handle().clone(),
        raygun_net::http_client()?,
        paths.downloads(),
        paths.cache(raygun_paths::CacheKind::Captions),
    );
    app.manage(captions.clone());
    captions.resume_songs();
    app.manage(crate::local_network::LocalNetwork::new(
        app.handle().clone(),
    ));
    app.manage(crate::cast::CastService::new(app.handle().clone()));
    app.manage(std::sync::Arc::new(crate::stremio::StremioService::new(
        app.handle().clone(),
        raygun_net::http_client()?,
    )));
    let ai = tauri::async_runtime::block_on(crate::ai::AiService::open(
        app.handle().clone(),
        paths.secrets(),
        paths.models(),
        raygun_net::http_client()?,
    ))?;
    app.manage(ai);
    app.manage(crate::backup::BackupService::new(
        app.handle().clone(),
        paths.root().join("backups"),
        crate::export::Outputs::new(
            app.handle().clone(),
            paths.exports(),
            paths.cache(raygun_paths::CacheKind::Share),
        ),
    ));
    app.manage(crate::playback::PlaybackService::new(
        app.handle().clone(),
        proxy,
        native_engine(app),
    ));
    crate::playback::PlaybackService::start_saving(app.handle().clone());
    crate::playback::PlaybackService::start_watchdog(app.handle().clone());

    let extensions = crate::extensions::ExtensionService::start(app.handle().clone(), &paths)?;
    app.manage(crate::extension_installs::ExtensionInstalls::start_in(
        app.handle(),
        extensions.clone(),
    ));
    app.manage(extensions);
    app.manage(crate::servers::ServerService::new(app.handle().clone()));
    app.manage(std::sync::Arc::new(crate::resolver::ResolverService::new(
        app.handle().clone(),
    )));
    app.manage(crate::collections::CollectionsService::new(
        app.handle().clone(),
    ));
    let discovery = crate::discovery::DiscoveryService::new(app.handle().clone());
    app.manage(discovery.clone());
    app.manage(crate::made_for_you::MadeForYouService::new(
        app.handle().clone(),
    ));
    // Local Files (M46.27): folders scanned again (changed files only) a little after start.
    let local_files = crate::local_files::LocalFilesService::new(app.handle().clone());
    app.manage(local_files.clone());
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        if let Err(error) = local_files.rescan(None, false).await {
            tracing::warn!(%error, "couldn't scan the local folders");
        }
    });
    // Followed artists and their new releases (M46.42).
    let follows = crate::follows::ArtistFollows::new(app.handle().clone());
    follows.start();
    app.manage(follows);
    // Lyrics kept on the device, forgotten when not played for a while (M46.46).
    let lyrics = crate::lyrics_store::LyricsStore::new(app.handle().clone());
    lyrics.start();
    app.manage(lyrics);
    // Translations under the lyrics (M46.48).
    app.manage(crate::lyrics_translation::LyricsTranslations::new(
        app.handle().clone(),
    ));
    // Covers from other sources when one's missing or fails (M46.40).
    app.manage(crate::cover_fallback::CoverFallbacks::new(
        app.handle().clone(),
    ));
    // The user's own details for songs, albums and artists (M46.30).
    let details = crate::details::DetailService::new(app.handle().clone(), paths.downloads());
    app.manage(details.clone());
    tauri::async_runtime::block_on(details.start());
    // Copies of a song saved under several names merged, once (M46.48); it reads the details.
    let repairing = app.handle().clone();
    tauri::async_runtime::spawn(async move { crate::collections::repair(&repairing).await });
    // Song lengths (M46.26): learned from playback, sources and lookups.
    let durations = crate::durations::DurationService::new(app.handle().clone());
    durations.start();
    app.manage(durations);
    app.manage(crate::import::ImportService::new(
        app.handle().clone(),
        raygun_net::http_client()?,
    ));
    // Raygun's own updates from its GitHub releases (M46.47).
    let updates = crate::updates::UpdateService::new(
        app.handle().clone(),
        paths.root().join("cache"),
        raygun_net::http_client()?,
    );
    updates.start();
    app.manage(updates);
    // yt-dlp (M46.47): YouTube downloads on desktop, and songs YouTube's own files didn't give.
    app.manage(crate::ytdlp::YtDlpService::new(
        app.handle().clone(),
        paths.root(),
        raygun_net::http_client()?,
    ));
    // Files saved from sites in the built-in browser.
    let website_downloads = crate::website_downloads::WebsiteDownloadService::new(
        app.handle().clone(),
        paths.downloads().join("files"),
        raygun_net::http_client()?,
    );
    website_downloads.start();
    app.manage(website_downloads);
    let songs = crate::music_downloads::SongDownloadService::new(
        app.handle().clone(),
        paths.downloads(),
        app.state::<raygun_net::ImageLinks>().inner().clone(),
        raygun_net::http_client()?,
    );
    app.manage(songs.clone());
    // Song edits and loudness (M46.12), loaded before anything plays.
    let sound = crate::sound::SoundService::new(app.handle().clone());
    app.manage(sound.clone());
    tauri::async_runtime::spawn(async move { sound.start().await });
    let mcp = tauri::async_runtime::block_on(crate::mcp::McpService::start(app.handle().clone()))?;
    app.manage(mcp);
    // Connect (M46.15) first: the remote answers other devices for it.
    let connect =
        tauri::async_runtime::block_on(crate::connect::ConnectService::start(app.handle().clone()));
    app.manage(connect);
    let remote =
        tauri::async_runtime::block_on(crate::remote::RemoteService::start(app.handle().clone()))?;
    app.manage(remote);
    let sync = tauri::async_runtime::block_on(crate::device_sync::SyncService::start(
        app.handle().clone(),
    ))?;
    app.manage(sync);
    // Linked devices keep each other in sync (M46.47).
    crate::device_link::start_auto_sync(app.handle().clone());

    // The phone's media session and notifications, and the permissions they need.
    app.manage(crate::permissions::PermissionPrompts::new(
        app.handle().clone(),
    ));
    crate::permissions::PermissionPrompts::start(app.handle());
    crate::background::BackgroundPlayback::start(app.handle());
    crate::notifications::DownloadNotices::start(app.handle());

    create_main_window(app, &paths)?;
    // The miniplayer window (M46.13) shares the main window's webview profile.
    app.manage(crate::miniplayer::MiniplayerDir(paths.webview()));

    app.manage(LogGuard(guard));
    app.manage(AppPaths(paths));

    // Pick up files added or removed while the app was closed (cheap when nothing changed).
    app.state::<crate::library::LibraryService>().start_scan();
    downloads.start();
    discovery.start_tagger();
    songs.start();
    video_downloads.start();
    podcasts.start();
    crate::device_state::install(app.handle());
    crate::tts::install(app.handle());
    crate::video::install(app.handle());
    crate::websites::install(app.handle());
    app.state::<crate::manga::MangaService>().start_scheduler();
    crate::system_ui::apply_saved_theme(app.handle().clone());
    Ok(())
}

/// Android plays streams and files through the native Media3 engine (M06: the webview gets frozen
/// in the background and has no media session). Other platforms have no native engine.
#[cfg(target_os = "android")]
fn native_engine(app: &App) -> Option<crate::playback::NativeEngineSink> {
    use raygun_model::EngineReport;
    use tauri_plugin_raygun_audio::NativeAudio;

    enum Native {
        Report(String),
        Control(String),
    }

    // Reports and controls arrive on the Android main thread. A single worker keeps them in order
    // and makes sure the main thread never waits on the playback service (which may call back
    // into Kotlin).
    let (sender, inbox) = std::sync::mpsc::channel::<Native>();
    let reports = sender.clone();
    tauri_plugin_raygun_audio::set_report_handler(move |json| {
        let _ = reports.send(Native::Report(json));
    });
    tauri_plugin_raygun_audio::set_control_handler(move |control| {
        let _ = sender.send(Native::Control(control));
    });
    let worker_handle = app.handle().clone();
    std::thread::spawn(move || {
        for message in inbox {
            // Looked up per message: this thread starts before the playback service is managed,
            // and the first reports only arrive once something plays.
            let Some(playback) = worker_handle.try_state::<crate::playback::PlaybackService>()
            else {
                tracing::warn!("native engine message before playback was ready");
                continue;
            };
            // One bad message must not end every later report; the panic hook logs it.
            let handled =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match message {
                    Native::Report(json) => match serde_json::from_str::<EngineReport>(&json) {
                        Ok(report) => playback.report(report),
                        Err(error) => {
                            tracing::warn!(%error, %json, "unreadable native engine report")
                        }
                    },
                    Native::Control(control) => {
                        match crate::system_ui::control_from_system(&control) {
                            Some(control) => playback.control(control),
                            None => tracing::warn!(%control, "unknown system media control"),
                        }
                    }
                }));
            if handled.is_err() {
                tracing::error!("a native engine message failed; continuing with the next");
            }
        }
    });

    let command_handle = app.handle().clone();
    Some(Box::new(move |command| {
        command_handle
            .state::<NativeAudio<tauri::Wry>>()
            .send(command)
    }))
}

#[cfg(not(target_os = "android"))]
fn native_engine(_app: &App) -> Option<crate::playback::NativeEngineSink> {
    None
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

    // lofty warns about every MP3 without a duration header, which floods the log during scans.
    let filter =
        || EnvFilter::try_from_env("RAYGUN_LOG").unwrap_or_else(|_| "info,lofty=error".into());
    // Incognito (M47): the files don't name what's played, watched, read or searched.
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(writer)
        .with_filter(filter().and(crate::privacy::log_filter()));
    let stderr_layer = cfg!(debug_assertions).then(|| {
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_filter(filter())
    });

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer)
        .try_init()?;

    // Panics on background threads otherwise vanish (Android drops stderr), leaving only symptoms.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(thread = ?std::thread::current().name(), "panic: {info}");
        default_hook(info);
    }));
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
