//! Raygun's own updates (M46.47), from the GitHub releases of `uvxszi/raygun`.
//!
//! A check reads the latest release and compares its tag with this copy's version, once a day in
//! the background and whenever asked. How a newer one installs depends on how this copy got here:
//!
//! - the setup exe (NSIS; `uninstall.exe` beside Raygun): the new setup is downloaded and run
//!   passively, and Raygun quits so its files can be replaced; the setup starts it again;
//! - the MSI (Program Files, no uninstaller beside it): the new MSI runs with `msiexec /passive`;
//! - Android: the APK is downloaded to the cache and handed to Android's installer, after asking
//!   for "Install unknown apps" when Raygun doesn't have it; the release is signed with the same
//!   key, so it installs as an update;
//! - the portable exe and everything else: the release page opens, nothing is replaced.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use raygun_model::{AppUpdate, InstallKind, ReleaseNotes, UpdateState};
use tauri::AppHandle;
use tauri_specta::Event;

use crate::events::AppUpdateEvent;

pub const REPO_URL: &str = "https://github.com/uvxszi/raygun";
const RELEASES_URL: &str = "https://github.com/uvxszi/raygun/releases/latest";
const LATEST_API: &str = "https://api.github.com/repos/uvxszi/raygun/releases/latest";
/// Background checks at most this often.
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// The first background check waits for the app to settle.
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(20);
const MAX_INSTALLER_BYTES: u64 = 600 << 20;
/// A failed background check tries again after this long.
const RETRY_EVERY: Duration = Duration::from_secs(60 * 60);
const LAST_CHECK_KEY: &str = "updates.lastCheck";
/// The latest version a notification was posted for, so each is announced once.
const ANNOUNCED_KEY: &str = "updates.announced";
/// The last version whose "What's new" was shown (`local_meta`).
const NOTES_SHOWN_KEY: &str = "updates.notesShown";
const TAG_API: &str = "https://api.github.com/repos/uvxszi/raygun/releases/tags/";
/// `(version, notes)` for every `docs/releases/v<version>.md`, oldest first (`build.rs`).
const BUNDLED_NOTES: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/release_notes.rs"));

#[derive(serde::Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(serde::Deserialize, Clone)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
}

pub struct UpdateService {
    app: AppHandle,
    client: reqwest::Client,
    downloads: PathBuf,
    kind: InstallKind,
    state: Mutex<(AppUpdate, Option<Asset>)>,
}

impl UpdateService {
    pub fn new(app: AppHandle, cache: PathBuf, client: reqwest::Client) -> std::sync::Arc<Self> {
        let kind = install_kind();
        std::sync::Arc::new(Self {
            app,
            client,
            downloads: cache.join("updates"),
            kind,
            state: Mutex::new((
                AppUpdate {
                    current: env!("CARGO_PKG_VERSION").into(),
                    latest: None,
                    available: false,
                    notes: None,
                    page_url: RELEASES_URL.into(),
                    repo_url: REPO_URL.into(),
                    install_kind: kind,
                    asset_name: None,
                    asset_size: None,
                    checked_at: None,
                    state: UpdateState::Idle,
                    progress: None,
                    message: None,
                },
                None,
            )),
        })
    }

    /// Checks on every start (after the app settles), then once a day, in release builds; a
    /// development build checks only when asked. What a check found isn't kept between runs, so
    /// the start's check is what lets Home say a release is out. Failed background checks are
    /// quiet and try again an hour later; battery saver and the background stretch every wait.
    pub fn start(self: &std::sync::Arc<Self>) {
        if cfg!(debug_assertions) {
            return;
        }
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(crate::device_state::pace(FIRST_CHECK_AFTER)).await;
            let mut last_ok: Option<i64> = None;
            loop {
                raygun_net::network_mode::wait_online().await;
                let now = crate::catalog::now_ms();
                let due = last_ok.is_none_or(|at| now - at >= CHECK_EVERY.as_millis() as i64);
                if due {
                    match service.check_with(true).await {
                        Ok(_) => last_ok = Some(now),
                        Err(error) => tracing::info!(%error, "update check failed"),
                    }
                }
                crate::device_state::rest(RETRY_EVERY).await;
            }
        });
    }

    pub fn current(&self) -> AppUpdate {
        self.state.lock().expect("update state poisoned").0.clone()
    }

    /// Reads the latest release.
    pub async fn check(&self) -> Result<AppUpdate, String> {
        self.check_with(false).await
    }

    /// Reads the latest release; a `quiet` (background) check that fails leaves the card as it
    /// was. A download or an install under way is never interrupted.
    async fn check_with(&self, quiet: bool) -> Result<AppUpdate, String> {
        let before = self.current();
        if matches!(
            before.state,
            UpdateState::Checking | UpdateState::Downloading | UpdateState::Installing
        ) {
            return Ok(before);
        }
        self.change(|update| {
            update.state = UpdateState::Checking;
            update.message = None;
        });
        let result = self.fetch_latest().await;
        match result {
            // Nothing released yet: a note, not a failure.
            Ok(None) => {
                self.remember_check().await;
                self.change_with_asset(None, |update| {
                    update.available = false;
                    update.checked_at = Some(crate::catalog::now_ms() as f64);
                    update.state = UpdateState::Idle;
                    update.message = Some("No release has been published yet".into());
                });
                Ok(self.current())
            }
            Ok(Some(release)) => {
                self.remember_check().await;
                let latest = release.tag_name.trim_start_matches('v').to_owned();
                let asset = pick_asset(self.kind, &release.assets);
                self.change_with_asset(asset.clone(), |update| {
                    update.available = newer(&latest, &update.current);
                    update.latest = Some(latest);
                    update.notes = release.body.clone();
                    if release.html_url.starts_with(REPO_URL) {
                        update.page_url = release.html_url.clone();
                    }
                    update.asset_name = asset.as_ref().map(|asset| asset.name.clone());
                    update.asset_size = asset.as_ref().map(|asset| asset.size as f64);
                    update.checked_at = Some(crate::catalog::now_ms() as f64);
                    update.state = UpdateState::Idle;
                });
                let found = self.current();
                if quiet && found.available {
                    self.announce(&found).await;
                }
                Ok(found)
            }
            Err(error) if quiet => {
                self.change(|update| {
                    update.state = before.state;
                    update.message = before.message;
                });
                Err(error)
            }
            Err(error) => {
                self.change(|update| {
                    update.state = UpdateState::Failed;
                    update.message = Some(error.clone());
                });
                Err(error)
            }
        }
    }

    /// When a check last answered; also how a later start knows an earlier version ran here.
    /// Posts a notification for a newer version a background check found, once per version
    /// (unless switched off in Settings → Notifications).
    async fn announce(&self, found: &AppUpdate) {
        let Some(latest) = found.latest.as_deref() else {
            return;
        };
        let db = self.db();
        let told = raygun_db::local_value(&db, ANNOUNCED_KEY)
            .await
            .ok()
            .flatten();
        if told.as_deref() == Some(latest) {
            return;
        }
        let wanted = crate::settings::load(&db)
            .await
            .map(|settings| settings.notifications.app_updates)
            .unwrap_or(true);
        if wanted {
            crate::notifications::show_notice(
                &self.app,
                crate::notifications::news::app_update_notice(latest),
            )
            .await;
        }
        let _ = raygun_db::set_local_value(&db, ANNOUNCED_KEY, latest).await;
    }

    async fn remember_check(&self) {
        let now = crate::catalog::now_ms();
        let _ = raygun_db::set_local_value(&self.db(), LAST_CHECK_KEY, &now.to_string()).await;
    }

    /// This version's notes when this is the first start since an update and they haven't been
    /// shown: bundled ones, else the GitHub release's (`None` offline; asked again next start).
    pub async fn whats_new(&self) -> Option<ReleaseNotes> {
        let db = self.db();
        let current = env!("CARGO_PKG_VERSION");
        let shown = raygun_db::local_value(&db, NOTES_SHOWN_KEY)
            .await
            .ok()
            .flatten();
        let ran_before = raygun_db::local_value(&db, LAST_CHECK_KEY)
            .await
            .ok()
            .flatten()
            .is_some();
        if !notes_due(shown.as_deref(), ran_before, current) {
            if shown.as_deref() != Some(current) {
                let _ = raygun_db::set_local_value(&db, NOTES_SHOWN_KEY, current).await;
            }
            return None;
        }
        let notes = match bundled_for(current) {
            Some(notes) => notes.to_owned(),
            None => self.fetch_tag_notes(current).await?,
        };
        Some(ReleaseNotes {
            version: current.into(),
            notes,
        })
    }

    /// The "What's new" sheet was shown for this version.
    pub async fn whats_new_seen(&self) {
        let _ = raygun_db::set_local_value(&self.db(), NOTES_SHOWN_KEY, env!("CARGO_PKG_VERSION"))
            .await;
    }

    /// A release's notes from GitHub, for a version whose notes weren't bundled.
    async fn fetch_tag_notes(&self, version: &str) -> Option<String> {
        let response = self
            .client
            .get(format!("{TAG_API}v{version}"))
            .header("Accept", "application/vnd.github+json")
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let release: Release = response.json().await.ok()?;
        release.body.filter(|body| !body.trim().is_empty())
    }

    async fn fetch_latest(&self) -> Result<Option<Release>, String> {
        let response = self
            .client
            .get(LATEST_API)
            .header("Accept", "application/vnd.github+json")
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(|error| format!("GitHub didn't answer: {error}"))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(format!("GitHub answered {}", response.status().as_u16()));
        }
        let release: Release = response
            .json()
            .await
            .map_err(|error| format!("GitHub's answer couldn't be read: {error}"))?;
        if release.draft || release.prerelease {
            return Ok(None);
        }
        Ok(Some(release))
    }

    /// Downloads the update and starts its installer; the release page opens where this copy
    /// can't replace itself.
    pub async fn install(&self) -> Result<AppUpdate, String> {
        let (update, asset) = {
            let state = self.state.lock().expect("update state poisoned");
            (state.0.clone(), state.1.clone())
        };
        if !update.available {
            return Err("This is the newest Raygun".into());
        }
        let Some(asset) = asset else {
            self.open_page(&update.page_url)?;
            return Ok(self.current());
        };
        let result = self.download_and_run(&asset).await;
        if let Err(error) = &result {
            self.change(|update| {
                update.state = UpdateState::Failed;
                update.progress = None;
                update.message = Some(error.clone());
            });
        }
        result.map(|()| self.current())
    }

    async fn download_and_run(&self, asset: &Asset) -> Result<(), String> {
        if !asset
            .browser_download_url
            .starts_with(&format!("{REPO_URL}/releases/download/"))
        {
            return Err("The release file isn't on Raygun's GitHub".into());
        }
        if asset.size > MAX_INSTALLER_BYTES {
            return Err("The release file is too big".into());
        }
        // Android asks first, so the download isn't wasted when it can't install.
        #[cfg(target_os = "android")]
        {
            use tauri::Manager;
            let native = self
                .app
                .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>();
            if !native.can_install_packages().unwrap_or(true) {
                // Without the permission the plugin opens its settings page, whatever the path.
                let _ = native.install_apk("");
                self.change(|update| update.state = UpdateState::NeedsPermission);
                return Ok(());
            }
        }
        let file = self.download(asset).await?;
        self.change(|update| {
            update.state = UpdateState::Installing;
            update.progress = None;
        });
        self.run_installer(&file)
    }

    async fn download(&self, asset: &Asset) -> Result<PathBuf, String> {
        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        tokio::fs::create_dir_all(&self.downloads)
            .await
            .map_err(|error| error.to_string())?;
        let name = Path::new(&asset.name)
            .file_name()
            .ok_or("The release file has no name")?;
        let path = self.downloads.join(name);
        let partial = path.with_extension("part");
        self.change(|update| {
            update.state = UpdateState::Downloading;
            update.progress = Some(0.0);
            update.message = None;
        });
        let response = self
            .client
            .get(&asset.browser_download_url)
            .timeout(Duration::from_secs(30 * 60))
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            return Err(format!("GitHub answered {}", response.status().as_u16()));
        }
        let total = response.content_length().unwrap_or(asset.size).max(1);
        let mut out = tokio::fs::File::create(&partial)
            .await
            .map_err(|error| error.to_string())?;
        let mut stream = response.bytes_stream();
        let mut done = 0_u64;
        let mut shown = 0.0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| error.to_string())?;
            done += chunk.len() as u64;
            if done > MAX_INSTALLER_BYTES {
                return Err("The release file is too big".into());
            }
            out.write_all(&chunk)
                .await
                .map_err(|error| error.to_string())?;
            let share = done as f64 / total as f64;
            if share - shown >= 0.02 {
                shown = share;
                self.change(|update| update.progress = Some(share.min(1.0)));
            }
        }
        out.flush().await.map_err(|error| error.to_string())?;
        drop(out);
        if done != asset.size {
            let _ = tokio::fs::remove_file(&partial).await;
            return Err("The download was cut short".into());
        }
        tokio::fs::rename(&partial, &path)
            .await
            .map_err(|error| error.to_string())?;
        Ok(path)
    }

    fn run_installer(&self, file: &Path) -> Result<(), String> {
        match self.kind {
            #[cfg(windows)]
            InstallKind::Setup | InstallKind::Msi => {
                use std::os::windows::process::CommandExt;
                let spawned = if self.kind == InstallKind::Setup {
                    // NSIS: passive (a progress bar, no questions), and Raygun starts afterwards.
                    std::process::Command::new(file)
                        .args(["/P", "/R", "/UPDATE"])
                        .spawn()
                } else {
                    std::process::Command::new("msiexec")
                        .arg("/i")
                        .arg(file)
                        .args(["/passive", "/norestart"])
                        .creation_flags(0x0800_0000)
                        .spawn()
                };
                spawned.map_err(|error| format!("The installer didn't start: {error}"))?;
                // The installer replaces Raygun's files: let go of them.
                let app = self.app.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    app.exit(0);
                });
                Ok(())
            }
            #[cfg(target_os = "android")]
            InstallKind::Android => {
                use tauri::Manager;
                let native = self
                    .app
                    .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>();
                let started = native.install_apk(&file.to_string_lossy())?;
                if !started {
                    self.change(|update| update.state = UpdateState::NeedsPermission);
                }
                Ok(())
            }
            _ => {
                let _ = file;
                self.open_page(RELEASES_URL)
            }
        }
    }

    /// Opens the release page or the repository in the system's browser.
    pub fn open_page(&self, url: &str) -> Result<(), String> {
        if !is_own_page(url) {
            return Err("Only Raygun's own pages open from here".into());
        }
        self.open_in_browser(url)
    }

    /// Opens a web address from release notes in the system's browser.
    pub fn open_link(&self, url: &str) -> Result<(), String> {
        if !is_web_link(url) {
            return Err("Only https links open from here".into());
        }
        self.open_in_browser(url)
    }

    fn open_in_browser(&self, url: &str) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            use tauri::Manager;
            self.app
                .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .open_url(url)
        }
        #[cfg(windows)]
        {
            std::process::Command::new("explorer")
                .arg(url)
                .spawn()
                .map(|_| ())
                .map_err(|error| format!("The browser didn't open: {error}"))
        }
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(url)
                .spawn()
                .map(|_| ())
                .map_err(|error| format!("The browser didn't open: {error}"))
        }
        #[cfg(not(any(target_os = "android", windows, target_os = "macos")))]
        {
            std::process::Command::new("xdg-open")
                .arg(url)
                .spawn()
                .map(|_| ())
                .map_err(|error| format!("The browser didn't open: {error}"))
        }
    }

    fn change(&self, edit: impl FnOnce(&mut AppUpdate)) {
        {
            let mut state = self.state.lock().expect("update state poisoned");
            edit(&mut state.0);
        }
        let _ = AppUpdateEvent(self.current()).emit(&self.app);
    }

    fn change_with_asset(&self, asset: Option<Asset>, edit: impl FnOnce(&mut AppUpdate)) {
        {
            let mut state = self.state.lock().expect("update state poisoned");
            state.1 = asset;
        }
        self.change(edit);
    }

    fn db(&self) -> std::sync::Arc<raygun_db::Db> {
        use tauri::Manager;
        self.app.state::<crate::startup::AppDb>().0.clone()
    }
}

/// How this copy was installed: Android; on Windows, the NSIS uninstaller beside the exe, or the
/// exe under Program Files (the MSI's place); otherwise portable.
fn install_kind() -> InstallKind {
    if cfg!(target_os = "android") {
        return InstallKind::Android;
    }
    if !cfg!(windows) || cfg!(debug_assertions) {
        return if cfg!(windows) {
            InstallKind::Portable
        } else {
            InstallKind::Other
        };
    }
    let Ok(exe) = std::env::current_exe() else {
        return InstallKind::Portable;
    };
    let Some(dir) = exe.parent() else {
        return InstallKind::Portable;
    };
    if dir.join("uninstall.exe").is_file() {
        return InstallKind::Setup;
    }
    let program_files = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from);
    for base in program_files {
        if dir.starts_with(&base) {
            return InstallKind::Msi;
        }
    }
    InstallKind::Portable
}

/// Whether `url` is the repository or one of its pages (not another repository whose name starts
/// the same).
fn is_own_page(url: &str) -> bool {
    url.strip_prefix(REPO_URL)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '#', '?']))
}

/// Whether `url` is an https address with a host (no `javascript:`, files or app links).
fn is_web_link(url: &str) -> bool {
    url.strip_prefix("https://").is_some_and(|rest| {
        !rest.is_empty() && !rest.starts_with(['/', '@', ':']) && !url.contains(char::is_whitespace)
    })
}

/// The bundled notes of this version and the ones before it, newest first.
pub fn bundled_notes() -> Vec<ReleaseNotes> {
    let current = env!("CARGO_PKG_VERSION");
    let mut notes: Vec<ReleaseNotes> = BUNDLED_NOTES
        .iter()
        .filter(|(version, _)| !newer(version, current))
        .map(|(version, notes)| ReleaseNotes {
            version: (*version).into(),
            notes: (*notes).into(),
        })
        .collect();
    notes.sort_by(|a, b| {
        if newer(&a.version, &b.version) {
            std::cmp::Ordering::Less
        } else if newer(&b.version, &a.version) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });
    notes
}

fn bundled_for(version: &str) -> Option<&'static str> {
    BUNDLED_NOTES
        .iter()
        .find(|(bundled, _)| *bundled == version)
        .map(|(_, notes)| *notes)
}

/// Whether this start shows "What's new": the first start of a version newer than the last one
/// shown. A first install has nothing to tell; an earlier version that ran here without
/// remembering (1.0.0) left its update checks behind.
fn notes_due(shown: Option<&str>, ran_before: bool, current: &str) -> bool {
    match shown {
        Some(shown) => newer(current, shown),
        None => ran_before,
    }
}

/// The release file this kind of copy updates from (named by the release workflow).
fn pick_asset(kind: InstallKind, assets: &[Asset]) -> Option<Asset> {
    let lower = |asset: &&Asset| asset.name.to_ascii_lowercase();
    let find =
        |test: &dyn Fn(&str) -> bool| assets.iter().find(|asset| test(&lower(asset))).cloned();
    match kind {
        InstallKind::Setup => find(&|name| name.ends_with("-setup.exe")),
        InstallKind::Msi => find(&|name| name.ends_with(".msi")),
        InstallKind::Android => find(&|name| name.ends_with(".apk") && name.contains("universal"))
            .or_else(|| find(&|name| name.ends_with(".apk"))),
        InstallKind::Portable | InstallKind::Other => None,
    }
}

/// Whether `latest` is a newer version than `current` ("1.2.0" > "1.1.9"; "1.0.0" > "1.0.0-rc1").
fn newer(latest: &str, current: &str) -> bool {
    let parse = |version: &str| {
        let (core, pre) = version
            .trim()
            .trim_start_matches('v')
            .split_once('-')
            .map_or(
                (version.trim().trim_start_matches('v'), None),
                |(core, pre)| (core, Some(pre.to_owned())),
            );
        let numbers: Vec<u64> = core
            .split('.')
            .map(|part| part.parse().unwrap_or(0))
            .chain(std::iter::repeat(0))
            .take(3)
            .collect();
        (numbers, pre)
    };
    let (latest_numbers, latest_pre) = parse(latest);
    let (current_numbers, current_pre) = parse(current);
    match latest_numbers.cmp(&current_numbers) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => match (latest_pre, current_pre) {
            (None, Some(_)) => true,
            (Some(latest), Some(current)) => latest > current,
            _ => false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_only_raygun_pages() {
        assert!(is_own_page(REPO_URL));
        assert!(is_own_page(RELEASES_URL));
        assert!(is_own_page(&format!("{REPO_URL}/blob/main/LICENSE")));
        assert!(!is_own_page("https://github.com/uvxszi/raygun-elsewhere"));
        assert!(!is_own_page(
            "https://example.com/?https://github.com/uvxszi/raygun"
        ));
    }

    #[test]
    fn opens_only_web_links() {
        assert!(is_web_link("https://github.com/uvxszi/raygun/pull/1"));
        assert!(!is_web_link("http://example.com"));
        assert!(!is_web_link("javascript:alert(1)"));
        assert!(!is_web_link("https://"));
        assert!(!is_web_link("https:///etc"));
        assert!(!is_web_link("https://a b"));
    }

    #[test]
    fn whats_new_shows_once_after_an_update() {
        assert!(!notes_due(None, false, "1.1.0"), "a first install");
        assert!(
            notes_due(None, true, "1.1.0"),
            "from 1.0.0, which didn't remember"
        );
        assert!(notes_due(Some("1.0.0"), true, "1.1.0"));
        assert!(!notes_due(Some("1.1.0"), true, "1.1.0"));
        assert!(!notes_due(Some("1.2.0"), true, "1.1.0"), "a downgrade");
    }

    #[test]
    fn release_notes_are_bundled() {
        assert!(
            bundled_for("1.1.0").is_some_and(|notes| notes.contains("## What's in this release"))
        );
        let all = bundled_notes();
        assert!(
            all.iter()
                .all(|notes| !newer(&notes.version, env!("CARGO_PKG_VERSION")))
        );
        assert!(
            all.windows(2)
                .all(|pair| newer(&pair[0].version, &pair[1].version))
        );
    }

    #[test]
    fn versions_compare_as_numbers() {
        assert!(newer("1.0.0", "0.1.0"));
        assert!(newer("v1.10.0", "1.9.3"));
        assert!(newer("1.0.0", "1.0.0-rc1"));
        assert!(!newer("1.0.0", "1.0.0"));
        assert!(!newer("0.9.9", "1.0.0"));
        assert!(!newer("1.0.0-rc2", "1.0.0"));
    }

    #[test]
    fn each_install_picks_its_own_file() {
        let assets: Vec<Asset> = [
            "Raygun_1.0.0_x64-setup.exe",
            "Raygun_1.0.0_x64_en-US.msi",
            "Raygun_1.0.0_x64_portable.exe",
            "Raygun_1.0.0_android_arm64.apk",
            "Raygun_1.0.0_android_universal.apk",
        ]
        .into_iter()
        .map(|name| Asset {
            name: name.into(),
            browser_download_url: format!("{REPO_URL}/releases/download/v1.0.0/{name}"),
            size: 1,
        })
        .collect();
        let name = |kind| pick_asset(kind, &assets).map(|asset| asset.name);
        assert_eq!(
            name(InstallKind::Setup).as_deref(),
            Some("Raygun_1.0.0_x64-setup.exe")
        );
        assert_eq!(
            name(InstallKind::Msi).as_deref(),
            Some("Raygun_1.0.0_x64_en-US.msi")
        );
        assert_eq!(
            name(InstallKind::Android).as_deref(),
            Some("Raygun_1.0.0_android_universal.apk")
        );
        assert_eq!(name(InstallKind::Portable), None);
    }
}
