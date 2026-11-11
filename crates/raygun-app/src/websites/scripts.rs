//! Site scripts in the websites browser (`raygun_ext_host::site`): which installed extensions'
//! scripts a site gets and which repositories' would fit it, the per-site switches
//! (`WebsitePrefs::scripts_off`), and what pages offer to Raygun.
//!
//! The browser only ever hands Raygun data: a page action names an offer (a video, a song, a
//! playlist: an address, a title, a few words) and what to do with it. It's checked here against
//! the installed extension that offered it before Raygun's own flows take it (the player, the
//! queue, the library, song downloads, yt-dlp, Open with).

use std::sync::Arc;

use raygun_ext_host::site::{Platform, SiteExtension};
use raygun_model::{MediaKind, MediaSource, QueueItemInput, SiteScriptsInfo, SiteScriptsWebsite};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::catalog::to_message;
use crate::extensions::ExtensionService;
use crate::startup::AppDb;

/// Raygun's own script for every page: media, the browser's state, and the offers' way out.
pub const BRIDGE: &str = include_str!("page_bridge.js");
/// Where the composed scripts are written for Android's browser, under the websites folder.
const SCRIPTS_DIR: &str = "scripts";
const BRIDGE_FILE: &str = "page-bridge.js";
/// The longest address, title or name an offer may carry.
const MAX_URL: usize = 2048;
const MAX_TEXT: usize = 300;
/// How long opening a site waits for the repositories' suggestions.
const SUGGESTIONS_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

/// One extension's scripts as the browser gets them.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserScripts {
    pub id: String,
    pub name: String,
    /// Android: the composed script's file.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub file: String,
    /// Desktop: the composed script.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub code: String,
    /// The origins it's injected into (androidx.webkit's `allowedOriginRules`).
    pub origins: Vec<String>,
    /// Off on this site: listed for the menu's switch, never injected.
    pub on: bool,
}

/// A repository's site-scripts extension that fits the site and isn't installed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub id: String,
    pub name: String,
    /// The repository's id, for installing it.
    pub repository: String,
    pub origins: Vec<String>,
}

/// What a site's browser gets: its scripts and the ones it could install.
#[derive(Debug, Default)]
pub struct Plan {
    pub scripts: Vec<BrowserScripts>,
    pub suggestions: Vec<Suggestion>,
}

fn platform() -> Platform {
    if cfg!(target_os = "android") {
        Platform::Android
    } else {
        Platform::Desktop
    }
}

/// The site-scripts extensions whose scripts run on one of `pages`.
fn fitting<'a>(
    installed: &'a [SiteExtension],
    pages: &'a [url::Url],
) -> impl Iterator<Item = &'a SiteExtension> {
    installed
        .iter()
        .filter(|site| pages.iter().any(|page| site.scripts.matches(page)))
}

/// The scripts for the site `site_id` whose pages are `pages` (where it opens and its own
/// address): the installed extensions that run there, each composed for this platform (`also`
/// joins even when it doesn't fit those pages: one just installed for the page shown), and the
/// installed repositories' extensions that would.
pub async fn plan(app: &AppHandle, site_id: &str, pages: &[url::Url], also: Option<&str>) -> Plan {
    let Some(extensions) = app.try_state::<Arc<ExtensionService>>() else {
        return Plan::default();
    };
    let installed = extensions.site_extensions().await;
    let off = scripts_off(app, site_id).await;
    let platform = platform();
    let mut scripts = Vec::new();
    for site in &installed {
        let fits = pages.iter().any(|page| site.scripts.matches(page));
        if !fits && also != Some(site.id.as_str()) {
            continue;
        }
        let code = match site.compose(platform) {
            Ok(code) => code,
            Err(error) => {
                tracing::warn!(extension = %site.id, %error, "site scripts left out");
                continue;
            }
        };
        scripts.push(BrowserScripts {
            id: site.id.clone(),
            name: site.name.clone(),
            file: String::new(),
            code,
            origins: site.scripts.origin_rules(),
            on: !off.contains(&site.id),
        });
    }
    // Waiting long on a repository's index isn't worth a slow open: none are suggested then.
    let suggestions = tokio::time::timeout(SUGGESTIONS_WAIT, extensions.site_suggestions(pages))
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(repository, entry)| Suggestion {
            origins: raygun_ext_host::site::origin_rules(
                entry
                    .sites
                    .iter()
                    .filter_map(|pattern| raygun_ext_host::site::MatchPattern::parse(pattern))
                    .collect::<Vec<_>>()
                    .iter(),
            ),
            id: entry.id,
            name: entry.name,
            repository: repository.id,
        })
        .collect();
    Plan {
        scripts,
        suggestions,
    }
}

/// Writes the bridge and each script for Android's browser, which reads them in its own process;
/// each script's `code` makes way for its `file`. Returns the bridge's path.
pub fn write_files(dir: &std::path::Path, plan: &mut Plan) -> Result<String, String> {
    let folder = dir.join(SCRIPTS_DIR);
    std::fs::create_dir_all(&folder).map_err(to_message)?;
    let bridge = folder.join(BRIDGE_FILE);
    write_if_changed(&bridge, BRIDGE)?;
    for script in &mut plan.scripts {
        let path = folder.join(format!("{}.js", script.id));
        write_if_changed(&path, &script.code)?;
        script.file = path.to_string_lossy().into_owned();
        script.code.clear();
    }
    Ok(bridge.to_string_lossy().into_owned())
}

/// Writes a file through a temporary one, unless it already holds `text`.
fn write_if_changed(path: &std::path::Path, text: &str) -> Result<(), String> {
    if std::fs::read_to_string(path).is_ok_and(|kept| kept == text) {
        return Ok(());
    }
    let partial = path.with_extension("part");
    std::fs::write(&partial, text).map_err(to_message)?;
    std::fs::rename(&partial, path).map_err(to_message)
}

/// The extensions turned off on a site.
async fn scripts_off(app: &AppHandle, site_id: &str) -> Vec<String> {
    let db = app.state::<AppDb>().inner().0.clone();
    crate::settings::load(&db)
        .await
        .map(|settings| {
            settings
                .websites
                .scripts_off
                .get(site_id)
                .cloned()
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

/// Turns an extension's scripts on or off for one of the user's sites.
pub async fn set_on(
    app: &AppHandle,
    site_id: &str,
    extension_id: &str,
    on: bool,
) -> Result<(), String> {
    if site_id.starts_with(super::PAGE_PREFIX) {
        // A source's page isn't one of the user's sites: nothing is kept for it.
        return Ok(());
    }
    let db = app.state::<AppDb>().inner().0.clone();
    let settings = crate::settings::load(&db).await.map_err(to_message)?;
    let mut prefs = settings.websites;
    let changed = switch(&mut prefs.scripts_off, site_id, extension_id, on);
    if !changed {
        return Ok(());
    }
    crate::settings::update(&db, raygun_model::SettingChange::Websites(prefs))
        .await
        .map_err(to_message)?;
    use tauri_specta::Event;
    let _ = crate::events::SettingsChangedEvent("websites".into()).emit(app);
    Ok(())
}

/// Records a switch in `off` (extension ids by site); false when it was so already.
fn switch(
    off: &mut std::collections::BTreeMap<String, Vec<String>>,
    site_id: &str,
    extension_id: &str,
    on: bool,
) -> bool {
    let list = off.entry(site_id.to_owned()).or_default();
    let before = list.len();
    if on {
        list.retain(|id| id != extension_id);
    } else if !list.iter().any(|id| id == extension_id) {
        list.push(extension_id.to_owned());
        list.sort();
    }
    let changed = list.len() != before;
    if list.is_empty() {
        off.remove(site_id);
    }
    changed
}

/// The addresses a site's scripts are matched against: where it opens and its own.
pub fn pages_of(url: &str, last_url: Option<&str>) -> Vec<url::Url> {
    [Some(url), last_url]
        .into_iter()
        .flatten()
        .filter_map(|address| url::Url::parse(address).ok())
        .collect()
}

/// The names of the installed extensions whose scripts run on a site (and aren't off there).
pub fn names_for(
    installed: &[SiteExtension],
    off: &std::collections::BTreeMap<String, Vec<String>>,
    site_id: &str,
    pages: &[url::Url],
) -> Vec<String> {
    let site_off = off.get(site_id);
    fitting(installed, pages)
        .filter(|site| site_off.is_none_or(|list| !list.contains(&site.id)))
        .map(|site| site.name.clone())
        .collect()
}

/// Every installed site-scripts extension with the user's sites it runs on, for Settings.
pub async fn overview(app: &AppHandle) -> Result<Vec<SiteScriptsInfo>, String> {
    let extensions = app.state::<Arc<ExtensionService>>();
    let installed = extensions.site_extensions().await;
    let db = app.state::<AppDb>().inner().0.clone();
    let sites = raygun_db::websites::list(&db).await.map_err(to_message)?;
    let off = crate::settings::load(&db)
        .await
        .map(|settings| settings.websites.scripts_off)
        .unwrap_or_default();
    Ok(installed
        .iter()
        .map(|extension| SiteScriptsInfo {
            extension_id: extension.id.clone(),
            name: extension.name.clone(),
            sites: site_labels(extension),
            websites: sites
                .iter()
                .filter(|site| {
                    pages_of(&site.url, site.last_url.as_deref())
                        .iter()
                        .any(|page| extension.scripts.matches(page))
                })
                .map(|site| SiteScriptsWebsite {
                    id: site.id.clone(),
                    name: site.name.clone(),
                    on: off
                        .get(&site.id)
                        .is_none_or(|list| !list.contains(&extension.id)),
                })
                .collect(),
        })
        .collect())
}

/// The sites an extension's scripts name, for people: `youtube.com`, `*` for every site.
fn site_labels(extension: &SiteExtension) -> Vec<String> {
    let mut labels: Vec<String> = extension
        .scripts
        .groups
        .iter()
        .flat_map(|group| &group.matches)
        .map(raygun_ext_host::site::MatchPattern::site_label)
        .collect();
    labels.sort();
    labels.dedup();
    labels
}

/// The installed site scripts and the user's websites they run on (Settings → Browse →
/// Websites).
#[tauri::command]
#[specta::specta]
pub async fn website_site_scripts(app: AppHandle) -> Result<Vec<SiteScriptsInfo>, String> {
    overview(&app).await
}

/// Turns an extension's site scripts on or off for one of the user's websites; it applies the
/// next time the site opens.
#[tauri::command]
#[specta::specta]
pub async fn website_set_site_scripts(
    app: AppHandle,
    site_id: String,
    extension_id: String,
    on: bool,
) -> Result<(), String> {
    set_on(&app, &site_id, &extension_id, on).await
}

// ---- Page actions ----

/// What a page offered, as the browser hands it over.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub kind: String,
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<f64>,
}

/// An action picked in the browser for an offer.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageAction {
    /// The site's id (or a source page's key).
    pub site: String,
    /// The page that made the offer.
    pub page: String,
    pub extension: String,
    pub action: String,
    pub offer: Offer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferKind {
    Video,
    Song,
    Playlist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Play,
    Queue,
    Save,
    Download,
}

/// An offer that passed the checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    pub kind: OfferKind,
    pub action: Action,
    pub url: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub artwork_url: Option<String>,
    pub duration_ms: Option<u32>,
    /// The YouTube video it is, when it's one.
    pub video_id: Option<String>,
}

/// A web address of at most [`MAX_URL`] characters, without a user name or password.
fn web_address(text: &str) -> Option<url::Url> {
    if text.len() > MAX_URL {
        return None;
    }
    let url = url::Url::parse(text.trim()).ok()?;
    (matches!(url.scheme(), "https" | "http")
        && url.host_str().is_some_and(|host| !host.is_empty())
        && url.username().is_empty()
        && url.password().is_none())
    .then_some(url)
}

/// Text as people see it: trimmed, on one line, at most [`MAX_TEXT`] characters.
fn shown_text(text: Option<&str>) -> Option<String> {
    let text: String = text?
        .chars()
        .map(|char| if char.is_control() { ' ' } else { char })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!text.is_empty()).then(|| text.chars().take(MAX_TEXT).collect())
}

/// Checks an action against the installed site-scripts extensions: the one named made the
/// offer on a page its scripts run on, and the offer is one Raygun can take.
pub fn check(action: &PageAction, installed: &[SiteExtension]) -> Result<Checked, String> {
    let extension = installed
        .iter()
        .find(|site| site.id == action.extension)
        .ok_or("The site scripts that offered this aren't installed or are off")?;
    let page = web_address(&action.page).ok_or("The page's address isn't a web address")?;
    if !extension.scripts.matches(&page) {
        return Err(format!("{} doesn't run on this page", extension.name));
    }
    let offer = &action.offer;
    let kind = match offer.kind.as_str() {
        "video" => OfferKind::Video,
        "song" => OfferKind::Song,
        "playlist" => OfferKind::Playlist,
        _ => return Err("Raygun takes videos, songs and playlists".into()),
    };
    let what = match action.action.as_str() {
        "play" => Action::Play,
        "queue" => Action::Queue,
        "save" => Action::Save,
        "download" => Action::Download,
        _ => return Err("That isn't something Raygun does with an offer".into()),
    };
    let url = web_address(&offer.url).ok_or("The offer's address isn't a web address")?;
    let video_id = crate::ytdlp::parse_link(url.as_str())
        .filter(|link| link.kind == raygun_model::YouTubeLinkKind::Video)
        .map(|link| link.id);
    match (kind, what) {
        (OfferKind::Playlist, Action::Play) => {}
        (OfferKind::Playlist, _) => return Err("A playlist opens in Raygun".into()),
        (OfferKind::Video, Action::Save) => {
            return Err("Only songs are saved to the library".into());
        }
        (OfferKind::Video, Action::Queue | Action::Download) if video_id.is_none() => {
            return Err("Raygun can queue and download only YouTube videos from pages".into());
        }
        _ => {}
    }
    let duration_ms = offer
        .duration_ms
        .filter(|duration| duration.is_finite() && *duration > 0.0 && *duration < 86_400_000.0)
        .map(|duration| duration.round() as u32);
    Ok(Checked {
        kind,
        action: what,
        url: url.to_string(),
        title: shown_text(offer.title.as_deref()),
        artist: shown_text(offer.artist.as_deref()),
        album: shown_text(offer.album.as_deref()),
        artwork_url: offer
            .artwork_url
            .as_deref()
            .and_then(web_address)
            .map(|url| url.to_string()),
        duration_ms,
        video_id,
    })
}

impl Checked {
    fn title_or_address(&self) -> String {
        self.title.clone().unwrap_or_else(|| self.url.clone())
    }

    /// Where it plays from: YouTube's player for a YouTube video, else the songs' sources
    /// (looked up by its title and artist).
    fn source(&self) -> MediaSource {
        match &self.video_id {
            Some(video_id) => MediaSource::YouTube {
                video_id: video_id.clone(),
            },
            None => MediaSource::Lookup {
                title: self.title_or_address(),
                artist: self.artist.clone(),
                album: self.album.clone(),
                duration_ms: self.duration_ms,
            },
        }
    }

    pub fn queue_item(&self) -> QueueItemInput {
        QueueItemInput {
            title: self.title_or_address(),
            artist: self.artist.clone(),
            album: self.album.clone(),
            duration_ms: self.duration_ms,
            artwork_url: self.artwork_url.clone(),
            media_kind: if self.video_id.is_some() {
                MediaKind::Video
            } else {
                MediaKind::Audio
            },
            source: self.source(),
            explicit: false,
        }
    }

    pub fn track(&self) -> raygun_model::TrackInput {
        raygun_model::TrackInput {
            title: self.title_or_address(),
            artist: self.artist.clone(),
            album: self.album.clone(),
            duration_ms: self.duration_ms,
            artwork_url: self.artwork_url.clone(),
            // Lookups aren't stored: the song is found again by its key.
            source: self.video_id.as_ref().map(|_| self.source()),
            explicit: false,
        }
    }
}

/// Does what was picked in the browser with a page's offer, after checking it.
pub async fn act(app: &AppHandle, action: PageAction) -> Result<(), String> {
    let extensions = app.state::<Arc<ExtensionService>>();
    let installed = extensions.site_extensions().await;
    let offer = check(&action, &installed)?;
    match (offer.kind, offer.action) {
        (OfferKind::Video, Action::Play) => {
            show_raygun(app);
            match &offer.video_id {
                Some(video_id) => {
                    crate::video::play_youtube(
                        app,
                        video_id,
                        &offer.title_or_address(),
                        offer.artist.clone(),
                    )
                    .await
                }
                // Anything else goes where Open with sends an address (a stream to the video
                // player, a page to Search).
                None => {
                    crate::open_with::hand_over(app, &offer.url, None);
                    Ok(())
                }
            }
        }
        (OfferKind::Playlist, _) => {
            show_raygun(app);
            crate::open_with::hand_over(app, &offer.url, offer.title.as_deref());
            Ok(())
        }
        (OfferKind::Song, Action::Play) => {
            show_raygun(app);
            app.state::<crate::playback::PlaybackService>().replace(
                vec![offer.queue_item()],
                Some(0),
                false,
            );
            Ok(())
        }
        (_, Action::Queue) => {
            app.state::<crate::playback::PlaybackService>()
                .enqueue(vec![offer.queue_item()], false);
            Ok(())
        }
        (_, Action::Save) => {
            app.state::<crate::collections::CollectionsService>()
                .set_favorites(vec![offer.track()], true)
                .await
        }
        (OfferKind::Video, Action::Download) if crate::ytdlp::supported() => app
            .state::<Arc<crate::ytdlp::YtDlpService>>()
            .download(
                &offer.url,
                &offer.title_or_address(),
                raygun_model::YtDlpKind::Video,
            )
            .map(|_| ()),
        // Phones download a video's sound as a song, as a YouTube link does in Search.
        (_, Action::Download) => {
            app.state::<Arc<crate::music_downloads::SongDownloadService>>()
                .enqueue(vec![offer.track()])
                .await
        }
    }
}

/// Raygun's own window comes forward (desktop; on Android the browser steps aside itself).
fn show_raygun(app: &AppHandle) {
    #[cfg(not(target_os = "android"))]
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    #[cfg(target_os = "android")]
    let _ = app;
}

// ---- Reports from the browser ----

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Switch {
    site: String,
    extension: String,
    on: bool,
    /// Desktop: the page shown, where the window opens again with the new set.
    #[serde(default)]
    page: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Install {
    site: String,
    extension: String,
    repository: String,
    /// The page shown, which the extension was suggested for.
    #[serde(default)]
    page: Option<String>,
}

/// Takes a browser report about site scripts (`pageAction`, `siteScripts`,
/// `installSiteScripts`); false when it's about something else.
pub fn take_report(
    app: &AppHandle,
    fields: &mut serde_json::Map<String, serde_json::Value>,
) -> bool {
    let app = app.clone();
    if let Some(action) = fields.remove("pageAction") {
        tauri::async_runtime::spawn(async move {
            match serde_json::from_value::<PageAction>(action) {
                Ok(action) => {
                    let site = action.site.clone();
                    if let Err(error) = act(&app, action).await {
                        tracing::info!(%error, %site, "a page's offer wasn't taken");
                    }
                }
                Err(error) => tracing::warn!(%error, "unreadable page action"),
            }
        });
        return true;
    }
    if let Some(switch) = fields.remove("siteScripts") {
        tauri::async_runtime::spawn(async move {
            let Ok(switch) = serde_json::from_value::<Switch>(switch) else {
                tracing::warn!("unreadable site scripts switch");
                return;
            };
            if let Err(error) = set_on(&app, &switch.site, &switch.extension, switch.on).await {
                tracing::warn!(%error, "a site's scripts weren't switched");
                return;
            }
            // Desktop windows take their scripts when they're made: the site opens again.
            #[cfg(not(target_os = "android"))]
            reopen(&app, &switch.site, switch.page.as_deref()).await;
            #[cfg(target_os = "android")]
            let _ = switch.page;
        });
        return true;
    }
    if let Some(install) = fields.remove("installSiteScripts") {
        tauri::async_runtime::spawn(async move {
            let Ok(install) = serde_json::from_value::<Install>(install) else {
                tracing::warn!("unreadable site scripts install");
                return;
            };
            install_suggested(&app, install).await;
        });
        return true;
    }
    false
}

/// Installs a repository's site scripts suggested in the browser, then hands the browser its new
/// set (Android) or opens the site again with it (desktop).
async fn install_suggested(app: &AppHandle, install: Install) {
    let extensions = app.state::<Arc<ExtensionService>>();
    let result = extensions
        .install(&install.repository, &install.extension)
        .await
        .and_then(|infos| {
            let info = infos
                .iter()
                .find(|info| info.id == install.extension)
                .ok_or("It wasn't installed")?;
            match &info.error {
                Some(error) if info.status != raygun_model::ExtensionStatus::Loaded => {
                    Err(error.lines().next().unwrap_or_default().to_owned())
                }
                _ => Ok(()),
            }
        });
    #[cfg(target_os = "android")]
    {
        let update = match result {
            Ok(()) => {
                let pages = site_pages(app, &install.site, install.page.as_deref()).await;
                let mut plan = plan(app, &install.site, &pages, Some(&install.extension)).await;
                let dir = app
                    .state::<Arc<super::WebsiteService>>()
                    .dir()
                    .to_path_buf();
                match write_files(&dir, &mut plan) {
                    Ok(_) => serde_json::json!({
                        "site": install.site,
                        "siteScripts": plan.scripts,
                        "installed": install.extension,
                    }),
                    Err(error) => serde_json::json!({
                        "site": install.site,
                        "extension": install.extension,
                        "error": error,
                    }),
                }
            }
            Err(error) => serde_json::json!({
                "site": install.site,
                "extension": install.extension,
                "error": error,
            }),
        };
        if let Err(error) = app
            .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
            .website_scripts(&update)
        {
            tracing::warn!(%error, "the browser wasn't told about its site scripts");
        }
    }
    #[cfg(not(target_os = "android"))]
    match result {
        Ok(()) => reopen(app, &install.site, install.page.as_deref()).await,
        Err(error) => tracing::info!(%error, "suggested site scripts weren't installed"),
    }
}

/// The pages a site's scripts are matched against: the page shown, where it opens, its address.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
async fn site_pages(app: &AppHandle, site_id: &str, shown: Option<&str>) -> Vec<url::Url> {
    let db = app.state::<AppDb>().inner().0.clone();
    let mut pages: Vec<url::Url> = shown
        .and_then(|page| url::Url::parse(page).ok())
        .into_iter()
        .collect();
    if let Ok(Some(site)) = raygun_db::websites::get(&db, site_id).await {
        pages.extend(pages_of(&site.url, site.last_url.as_deref()));
    }
    pages
}

/// Opens a desktop site window again at `page` (else where it was left), with its scripts as
/// they are now.
#[cfg(not(target_os = "android"))]
async fn reopen(app: &AppHandle, site_id: &str, page: Option<&str>) {
    let service = app.state::<Arc<super::WebsiteService>>().inner().clone();
    if let Err(error) = service.reopen(site_id, page).await {
        tracing::info!(%error, "the site didn't open again");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A site-scripts extension loaded from a folder that lasts as long as the first value.
    fn site(json: serde_json::Value) -> (tempfile::TempDir, Vec<SiteExtension>) {
        let dir = tempfile::TempDir::new().unwrap();
        let folder = dir.path().join("dev.example.tube");
        std::fs::create_dir_all(folder.join("dist")).unwrap();
        std::fs::write(folder.join("raygun-extension.json"), json.to_string()).unwrap();
        std::fs::write(folder.join("dist/tube.js"), "void 0;").unwrap();
        let host =
            raygun_ext_host::ExtensionHost::start(raygun_ext_host::HostConfig::default()).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(host.load(raygun_ext_host::discover_installed(dir.path())));
        (dir, host.site_extensions())
    }

    fn tube() -> (tempfile::TempDir, Vec<SiteExtension>) {
        site(serde_json::json!({
            "id": "dev.example.tube", "name": "Tube", "version": "1.0.0", "apiVersion": 1,
            "runtime": "site", "entry": "dist/tube.js", "provides": ["site.scripts"],
            "site": { "scripts": [{
                "matches": ["https://*.youtube.com/*", "https://music.example.com/*"],
                "js": ["dist/tube.js"]
            }] }
        }))
    }

    fn action(kind: &str, url: &str, what: &str) -> PageAction {
        PageAction {
            site: "s1".into(),
            page: "https://m.youtube.com/watch?v=dQw4w9WgXcQ".into(),
            extension: "dev.example.tube".into(),
            action: what.into(),
            offer: Offer {
                kind: kind.into(),
                url: url.into(),
                title: Some("  Never \n Gonna  ".into()),
                artist: Some("Rick".into()),
                album: None,
                artwork_url: Some("javascript:alert(1)".into()),
                duration_ms: Some(212_000.4),
            },
        }
    }

    #[test]
    fn offers_are_checked_before_raygun_takes_them() {
        let (_dir, installed) = tube();
        assert_eq!(installed.len(), 1);
        let video = check(
            &action(
                "video",
                "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                "play",
            ),
            &installed,
        )
        .unwrap();
        assert_eq!(video.kind, OfferKind::Video);
        assert_eq!(video.video_id.as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(video.title.as_deref(), Some("Never Gonna"));
        // Only web pictures.
        assert_eq!(video.artwork_url, None);
        assert_eq!(video.duration_ms, Some(212_000));
        assert_eq!(
            video.queue_item().source,
            MediaSource::YouTube {
                video_id: "dQw4w9WgXcQ".into()
            }
        );

        let song = check(
            &action("song", "https://music.example.com/track/1", "save"),
            &installed,
        )
        .unwrap();
        assert!(matches!(
            song.queue_item().source,
            MediaSource::Lookup { .. }
        ));
        assert_eq!(song.track().source, None);

        for (bad, why) in [
            (action("video", "javascript:alert(1)", "play"), "address"),
            (
                action(
                    "video",
                    "https://user:pw@www.youtube.com/watch?v=dQw4w9WgXcQ",
                    "play",
                ),
                "address",
            ),
            (action("file", "https://www.youtube.com/", "play"), "kind"),
            (
                action(
                    "video",
                    "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                    "delete",
                ),
                "action",
            ),
            (
                action(
                    "video",
                    "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                    "save",
                ),
                "songs only",
            ),
            (
                action("video", "https://video.example.com/1", "download"),
                "youtube only",
            ),
            (
                action(
                    "playlist",
                    "https://www.youtube.com/playlist?list=PL1234567890",
                    "queue",
                ),
                "opens",
            ),
        ] {
            assert!(check(&bad, &installed).is_err(), "{why}");
        }
        // An extension that isn't installed, or a page its scripts don't run on, offers nothing.
        let mut stranger = action(
            "video",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "play",
        );
        stranger.extension = "dev.example.other".into();
        assert!(check(&stranger, &installed).is_err());
        let mut elsewhere = action(
            "video",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "play",
        );
        elsewhere.page = "https://evil.example/".into();
        assert!(check(&elsewhere, &installed).is_err());
    }

    #[test]
    fn switches_are_kept_per_site() {
        let mut off = std::collections::BTreeMap::new();
        assert!(switch(&mut off, "s1", "dev.example.tube", false));
        assert!(!switch(&mut off, "s1", "dev.example.tube", false));
        assert_eq!(off["s1"], ["dev.example.tube"]);
        assert!(switch(&mut off, "s1", "dev.example.tube", true));
        assert!(off.is_empty());
        assert!(!switch(&mut off, "s2", "dev.example.tube", true));
        assert!(off.is_empty());
    }

    #[test]
    fn sites_say_which_scripts_run_on_them() {
        let (_dir, installed) = tube();
        let mut off = std::collections::BTreeMap::new();
        let youtube = pages_of("https://www.youtube.com/", None);
        assert_eq!(names_for(&installed, &off, "s1", &youtube), ["Tube"]);
        assert!(
            names_for(
                &installed,
                &off,
                "s1",
                &pages_of("https://example.org/", None)
            )
            .is_empty()
        );
        switch(&mut off, "s1", "dev.example.tube", false);
        assert!(names_for(&installed, &off, "s1", &youtube).is_empty());
        assert_eq!(names_for(&installed, &off, "s2", &youtube), ["Tube"]);
        assert_eq!(
            site_labels(&installed[0]),
            ["music.example.com", "youtube.com"]
        );
    }

    #[test]
    fn android_reads_the_scripts_from_files() {
        let (_folder, installed) = tube();
        let dir = tempfile::TempDir::new().unwrap();
        let mut plan = Plan {
            scripts: vec![BrowserScripts {
                id: installed[0].id.clone(),
                name: installed[0].name.clone(),
                file: String::new(),
                code: installed[0].compose(Platform::Android).unwrap(),
                origins: installed[0].scripts.origin_rules(),
                on: true,
            }],
            suggestions: Vec::new(),
        };
        let bridge = write_files(dir.path(), &mut plan).unwrap();
        assert_eq!(std::fs::read_to_string(&bridge).unwrap(), BRIDGE);
        let script = &plan.scripts[0];
        assert!(script.code.is_empty());
        assert!(
            std::fs::read_to_string(&script.file)
                .unwrap()
                .contains("void 0;")
        );
        let json = serde_json::to_value(&plan.scripts).unwrap();
        assert_eq!(json[0]["origins"][0], "https://*.youtube.com");
        assert!(json[0].get("code").is_none());
    }
}
