//! Stremio addons in the Watch tab (M41): catalogs, metadata, streams and subtitles from addons
//! speaking Stremio's HTTP protocol (`protocol`), played in Raygun's own player. Titles saved to
//! the video library are stored with their episodes and refreshed by library updates (M43).
//!
//! Who is asked for what follows stremio-core's `AggrRequest`: the board lists every catalog that
//! needs nothing, a search goes to every catalog that searches, details come from the first addon
//! in the addon order that serves the type and id (the next when it fails), and streams and
//! subtitles from every addon that serves them, each asked by an id it knows
//! (`Meta::stream_ids_for_video`). Addons that didn't answer lately are asked last.

pub mod protocol;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::future::join_all;
use futures_util::stream::FuturesUnordered;
use raygun_db::stremio::{self as store, AddonRow};
use raygun_db::titles::{self, TitleFields, TitleRow, UnitFields};
use raygun_db::video::{self as progress_store, VideoEntry, VideoProgress};
use raygun_model::{
    StremioAddon, StremioCatalogInfo, StremioCatalogRow, StremioExtra, StremioExtraValue,
    StremioItem, StremioMeta, StremioPlayRequest, StremioProvides, StremioStream,
    StremioStreamGroup, StremioStreamKind, StremioSuggestion, StremioVideo, TitleRating,
    VideoRequest, VideoSubtitle, YouTubeVideo,
};
use raygun_resolver::video_match;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::{Semaphore, watch};

use self::protocol::{Catalog, Manifest, MetaPreview, StreamSource};
use crate::catalog::now_ms;
use crate::events::YouTubeVideoEvent;
use crate::extensions::ExtensionService;
use crate::local_network::{self, LocalNetwork};
use crate::startup::AppDb;
use crate::video_library::{self, STREMIO_SOURCE};
use crate::video_session;

/// Manifests older than this are fetched again, in the background.
const MANIFEST_MAX_AGE_MS: i64 = 24 * 60 * 60 * 1000;
/// Longest one addon request may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// Longest a search waits for one catalog.
const SEARCH_TIMEOUT: Duration = Duration::from_secs(12);
/// Biggest answer read from an addon (thousands of subtitles run to a few MB).
const MAX_RESPONSE_BYTES: usize = 12 * 1024 * 1024;
/// Most subtitle tracks handed to the player; more makes a menu of near-identical entries.
const MAX_SUBTITLES: usize = 20;
/// How long a title's details are reused (the title page, its streams and the player ask in turn).
const META_FRESH_MS: i64 = 10 * 60 * 1000;
/// How long details from an addon later in the order wait for the ones before it.
const META_GRACE: Duration = Duration::from_millis(1500);
/// How long an addon that failed since it last worked is "didn't answer lately".
const FAILING_FOR_MS: i64 = 3 * 24 * 60 * 60 * 1000;
const HEALTH_KEY: &str = "stremio.addonHealth";
/// How long an addon whose manifest couldn't be fetched isn't asked again (pages would wait).
const MANIFEST_RETRY_MS: i64 = 5 * 60 * 1000;

/// Addons offered with one tap: Stremio's own, which serve catalogs, metadata and subtitles.
const SUGGESTED: [(&str, &str, &str); 2] = [
    (
        "https://v3-cinemeta.strem.io/manifest.json",
        "Cinemeta",
        "Popular movies and series, with details, seasons and episodes.",
    ),
    (
        "https://opensubtitles-v3.strem.io/manifest.json",
        "OpenSubtitles v3",
        "Subtitles in many languages for movies and series.",
    ),
];

/// An addon with its manifest (or why it has none).
#[derive(Clone)]
struct Loaded {
    row: AddonRow,
    manifest: Result<Manifest, String>,
}

impl Loaded {
    fn manifest(&self) -> Option<&Manifest> {
        self.manifest.as_ref().ok()
    }

    fn name(&self) -> String {
        self.manifest().map_or_else(
            || self.row.transport_url.clone(),
            |manifest| manifest.name.clone(),
        )
    }
}

/// How an addon did lately: when it last answered and last failed (unreachable, too slow, broken
/// answers; not "nothing for this id").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Health {
    ok_at: i64,
    failed_at: i64,
}

impl Health {
    fn failing(&self, now: i64) -> bool {
        self.failed_at > self.ok_at && now - self.failed_at < FAILING_FOR_MS
    }
}

/// A failed request: what to tell, and whether it says the addon is down.
struct AddonError {
    message: String,
    down: bool,
}

/// A title's details as an addon gave them.
#[derive(Clone)]
struct FoundMeta {
    meta: protocol::Meta,
    addon_id: String,
    addon_name: String,
    at: i64,
}

/// Addons asked at the same time (M46.7): rows, searches, streams and subtitles go a few addons at
/// a time instead of all at once.
const ADDON_CONCURRENCY: usize = 6;

/// Runs `tasks` with at most `limit` at once, keeping their order. (A semaphore rather than a
/// buffered stream: the futures borrow `self`, and spawned sessions need them `Send`.)
pub(crate) async fn at_most<T, F: Future<Output = T>>(
    limit: usize,
    tasks: impl IntoIterator<Item = F>,
) -> Vec<T> {
    let permits = tokio::sync::Semaphore::new(limit);
    let permits = &permits;
    join_all(tasks.into_iter().map(|task| async move {
        let _permit = permits.acquire().await;
        task.await
    }))
    .await
}

/// A manifest read from storage: the addon's address, when it was fetched, and what it says.
type StoredManifest = (String, Option<i64>, Manifest);

pub struct StremioService {
    app: AppHandle,
    client: reqwest::Client,
    /// Catalog rows and searches: many at once from the board, so a few at a time.
    browsing: Semaphore,
    /// Details, streams and subtitles: someone waits on these, so they don't queue behind the
    /// board.
    asking: Semaphore,
    health: Mutex<Option<HashMap<String, Health>>>,
    metas: Mutex<HashMap<(String, String), FoundMeta>>,
    /// The query being searched; searches for an older one stop.
    search: watch::Sender<String>,
    /// Addons whose manifest is being fetched again.
    refreshing: Mutex<HashSet<String>>,
    /// Manifests read from storage, by addon: its address and when it was fetched, and what it
    /// says (each request would otherwise read every manifest again).
    manifests: Mutex<HashMap<String, StoredManifest>>,
    /// Addons whose manifest couldn't be fetched lately: when, and why.
    unreachable: Mutex<HashMap<String, (i64, String)>>,
}

impl StremioService {
    pub fn new(app: AppHandle, client: reqwest::Client) -> Self {
        Self {
            app,
            client,
            browsing: Semaphore::new(ADDON_CONCURRENCY),
            asking: Semaphore::new(8),
            health: Mutex::new(None),
            metas: Mutex::new(HashMap::new()),
            search: watch::Sender::new(String::new()),
            refreshing: Mutex::new(HashSet::new()),
            manifests: Mutex::new(HashMap::new()),
            unreachable: Mutex::new(HashMap::new()),
        }
    }

    // ---- Addons ----

    /// Installed addons with their manifests: the cached ones at once (stale ones are fetched
    /// again in the background), the others now. Adult addons are left out while NSFW content is
    /// hidden.
    async fn loaded(&self) -> Result<Vec<Loaded>, String> {
        let db = self.db();
        let rows = store::addons(&db).await.map_err(to_message)?;
        let now = now_ms();
        let loads = rows.into_iter().map(|row| async move {
            let cached = self.cached_manifest(&row);
            let stale = row
                .fetched_at
                .is_none_or(|at| now - at >= MANIFEST_MAX_AGE_MS);
            match cached {
                Some(manifest) => {
                    if stale {
                        self.refresh_manifest(&row);
                    }
                    Loaded {
                        manifest: Ok(manifest),
                        row,
                    }
                }
                None => {
                    // An addon that couldn't be reached a moment ago isn't waited for again.
                    let recent = self
                        .unreachable
                        .lock()
                        .expect("unreachable poisoned")
                        .get(&row.id)
                        .filter(|(at, _)| now - at < MANIFEST_RETRY_MS)
                        .map(|(_, error)| error.clone());
                    if let Some(error) = recent {
                        return Loaded {
                            manifest: Err(error),
                            row,
                        };
                    }
                    match self.fetch_manifest(&row.transport_url).await {
                        Ok((manifest, text)) => {
                            let _ = store::set_manifest(&self.db(), &row.id, &text, now).await;
                            Loaded {
                                manifest: Ok(manifest),
                                row,
                            }
                        }
                        Err(error) => {
                            self.unreachable
                                .lock()
                                .expect("unreachable poisoned")
                                .insert(row.id.clone(), (now, error.clone()));
                            Loaded {
                                manifest: Err(error),
                                row,
                            }
                        }
                    }
                }
            }
        });
        let show_nsfw = self.app.state::<Arc<ExtensionService>>().show_nsfw();
        Ok(at_most(ADDON_CONCURRENCY, loads)
            .await
            .into_iter()
            .filter(|loaded| show_nsfw || !loaded.manifest().is_some_and(|manifest| manifest.adult))
            .collect())
    }

    /// A stored manifest, read once per version of it.
    fn cached_manifest(&self, row: &AddonRow) -> Option<Manifest> {
        if let Some((url, fetched_at, manifest)) = self
            .manifests
            .lock()
            .expect("manifests poisoned")
            .get(&row.id)
            && *url == row.transport_url
            && *fetched_at == row.fetched_at
        {
            return Some(manifest.clone());
        }
        let manifest = row
            .manifest
            .as_deref()
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .and_then(|value| protocol::parse_manifest(&value).ok())?;
        self.manifests.lock().expect("manifests poisoned").insert(
            row.id.clone(),
            (row.transport_url.clone(), row.fetched_at, manifest.clone()),
        );
        Some(manifest)
    }

    /// The addons that answer: turned on, with a manifest; the ones that didn't answer lately
    /// after the others (a stable sort, so the addon order holds within each).
    async fn active(&self) -> Result<Vec<Loaded>, String> {
        let health = self.health_now().await;
        let now = now_ms();
        let mut active: Vec<Loaded> = self
            .loaded()
            .await?
            .into_iter()
            .filter(|loaded| loaded.row.enabled && loaded.manifest().is_some())
            .collect();
        active.sort_by_key(|loaded| {
            health
                .get(&loaded.row.id)
                .is_some_and(|health| health.failing(now))
        });
        Ok(active)
    }

    /// Fetches a stale manifest again without making anyone wait for it.
    fn refresh_manifest(&self, row: &AddonRow) {
        if !self
            .refreshing
            .lock()
            .expect("refreshing poisoned")
            .insert(row.id.clone())
        {
            return;
        }
        let app = self.app.clone();
        let (id, url) = (row.id.clone(), row.transport_url.clone());
        tauri::async_runtime::spawn(async move {
            let service = app.state::<Arc<StremioService>>();
            match service.fetch_manifest(&url).await {
                Ok((_, text)) => {
                    let _ = store::set_manifest(&service.db(), &id, &text, now_ms()).await;
                    service
                        .unreachable
                        .lock()
                        .expect("unreachable poisoned")
                        .remove(&id);
                }
                Err(error) => {
                    tracing::info!(%error, url, "an addon's manifest couldn't be fetched again")
                }
            }
            service
                .refreshing
                .lock()
                .expect("refreshing poisoned")
                .remove(&id);
        });
    }

    pub async fn addons(&self) -> Result<Vec<StremioAddon>, String> {
        let health = self.health_now().await;
        let now = now_ms();
        Ok(self
            .loaded()
            .await?
            .into_iter()
            .map(|loaded| {
                let failing = health
                    .get(&loaded.row.id)
                    .is_some_and(|health| health.failing(now));
                self.addon_view(loaded, failing)
            })
            .collect())
    }

    fn addon_view(&self, loaded: Loaded, failing: bool) -> StremioAddon {
        let Loaded { row, manifest } = loaded;
        match manifest {
            Ok(manifest) => StremioAddon {
                id: row.id,
                provides: provides(&manifest),
                configure_url: (manifest.configurable || manifest.configuration_required)
                    .then(|| protocol::configure_url(&row.transport_url)),
                url: row.transport_url,
                name: manifest.name,
                description: manifest.description,
                logo: manifest.logo.map(|logo| self.kept_image(logo)),
                version: manifest.version,
                resources: manifest
                    .resources
                    .iter()
                    .map(|resource| resource.name.clone())
                    .collect(),
                types: manifest.types,
                catalogs: manifest.catalogs.len() as u32,
                adult: manifest.adult,
                p2p: manifest.p2p,
                needs_setup: manifest.configuration_required,
                enabled: row.enabled,
                failing,
                error: None,
            },
            Err(error) => StremioAddon {
                name: row.transport_url.clone(),
                id: row.id,
                url: row.transport_url,
                description: None,
                logo: None,
                version: String::new(),
                resources: Vec::new(),
                types: Vec::new(),
                catalogs: 0,
                adult: false,
                p2p: false,
                provides: Vec::new(),
                configure_url: None,
                needs_setup: false,
                enabled: row.enabled,
                failing: true,
                error: Some(error),
            },
        }
    }

    pub async fn suggestions(&self) -> Result<Vec<StremioSuggestion>, String> {
        let rows = store::addons(&self.db()).await.map_err(to_message)?;
        Ok(SUGGESTED
            .iter()
            .map(|(url, name, description)| StremioSuggestion {
                added: rows.iter().any(|row| row.transport_url == *url),
                url: (*url).to_owned(),
                name: (*name).to_owned(),
                description: (*description).to_owned(),
            })
            .collect())
    }

    /// What the addon at `url` is before it's added: its name, what it provides, whether it must
    /// be set up first. Its `id` is the added addon's, or empty.
    pub async fn preview(&self, url: &str) -> Result<StremioAddon, String> {
        let url = protocol::normalize_addon_url(url)?;
        let (manifest, _) = self.fetch_manifest(&url).await?;
        let rows = store::addons(&self.db()).await.map_err(to_message)?;
        let added = rows.into_iter().find(|row| row.transport_url == url);
        let row = AddonRow {
            id: added.as_ref().map(|row| row.id.clone()).unwrap_or_default(),
            enabled: added.is_none_or(|row| row.enabled),
            transport_url: url,
            manifest: None,
            fetched_at: None,
        };
        Ok(self.addon_view(
            Loaded {
                row,
                manifest: Ok(manifest),
            },
            false,
        ))
    }

    /// Adds an addon by its address, after reading its manifest. An addon already added under
    /// another address (set up again on its website) gets the new address, in its place.
    pub async fn add(&self, url: &str) -> Result<StremioAddon, String> {
        let url = protocol::normalize_addon_url(url)?;
        let (manifest, text) = self.fetch_manifest(&url).await?;
        if manifest.configuration_required {
            return Err(format!(
                "{} needs to be set up on its website first: choose Set up, then add the address it gives you",
                manifest.name
            ));
        }
        let db = self.db();
        let same = self.loaded().await?.into_iter().find(|loaded| {
            loaded.row.transport_url != url
                && loaded
                    .manifest()
                    .is_some_and(|known| known.id == manifest.id)
        });
        let id = match same {
            Some(loaded) => {
                store::replace_url(&db, &loaded.row.id, &url, &text, now_ms())
                    .await
                    .map_err(to_message)?;
                loaded.row.id
            }
            None => store::add(&db, &url, &text, now_ms())
                .await
                .map_err(to_message)?,
        };
        self.forget_metas();
        self.unreachable
            .lock()
            .expect("unreachable poisoned")
            .remove(&id);
        self.addons()
            .await?
            .into_iter()
            .find(|addon| addon.id == id)
            .ok_or_else(|| "The addon couldn't be added".to_owned())
    }

    pub async fn remove(&self, id: &str) -> Result<(), String> {
        self.forget_metas();
        store::remove(&self.db(), id).await.map_err(to_message)
    }

    pub async fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        self.forget_metas();
        store::set_enabled(&self.db(), id, enabled)
            .await
            .map_err(to_message)
    }

    /// Puts the addons in this order: details come from the first that has them.
    pub async fn set_order(&self, ids: &[String]) -> Result<(), String> {
        self.forget_metas();
        store::set_order(&self.db(), ids).await.map_err(to_message)
    }

    async fn fetch_manifest(&self, url: &str) -> Result<(Manifest, String), String> {
        let value = self
            .get(None, &protocol::manifest_url(url), REQUEST_TIMEOUT)
            .await
            .map_err(|error| error.message)?;
        let value = if protocol::is_legacy(url) {
            protocol::legacy_answer("manifest", value)?
        } else {
            value
        };
        let manifest = protocol::parse_manifest(&value)?;
        Ok((manifest, value.to_string()))
    }

    // ---- Catalogs ----

    /// Every catalog of the addons that answer, the ones that didn't answer lately last: rows for
    /// the board (the ones that need nothing), the ones that search, and what Discover browses.
    pub async fn catalogs(&self) -> Result<Vec<StremioCatalogInfo>, String> {
        let health = self.health_now().await;
        let now = now_ms();
        Ok(self
            .active()
            .await?
            .iter()
            .filter_map(|addon| Some((addon, addon.manifest()?)))
            .flat_map(|(addon, manifest)| {
                let failing = health
                    .get(&addon.row.id)
                    .is_some_and(|health| health.failing(now));
                manifest
                    .catalogs
                    .iter()
                    .map(move |catalog| catalog_info(&addon.row.id, manifest, catalog, failing))
            })
            .collect())
    }

    /// The first page of every catalog that needs nothing to list, a few at a time.
    pub async fn home(&self) -> Result<Vec<StremioCatalogRow>, String> {
        let active = self.active().await?;
        let rows: Vec<_> = active
            .iter()
            .filter_map(|addon| Some((addon, addon.manifest()?)))
            .flat_map(|(addon, manifest)| {
                manifest
                    .catalogs
                    .iter()
                    .filter(|catalog| catalog.browsable())
                    .map(move |catalog| (addon, manifest, catalog))
            })
            .collect();
        let asks = rows
            .into_iter()
            .map(|(addon, manifest, catalog)| {
                self.row(addon, manifest, catalog, Vec::new(), REQUEST_TIMEOUT)
            })
            .collect::<Vec<_>>();
        Ok(join_all(asks).await)
    }

    /// One catalog's page with the extras chosen (a genre, a year...), starting `skip` items in.
    pub async fn catalog(
        &self,
        addon_id: &str,
        kind: &str,
        catalog_id: &str,
        extra: &[StremioExtraValue],
        skip: u32,
    ) -> Result<StremioCatalogRow, String> {
        let loaded = self.loaded().await?;
        let addon = loaded
            .iter()
            .find(|addon| addon.row.id == addon_id)
            .ok_or_else(|| "This addon was removed".to_owned())?;
        if !addon.row.enabled {
            return Err("This addon is turned off".into());
        }
        let manifest = addon.manifest.as_ref().map_err(Clone::clone)?;
        let catalog = manifest
            .catalogs
            .iter()
            .find(|catalog| catalog.kind == kind && catalog.id == catalog_id)
            .ok_or_else(|| "The addon no longer has this catalog".to_owned())?;
        let values: Vec<(String, String)> = extra
            .iter()
            .filter(|value| value.name != "search" && value.name != "skip")
            .map(|value| (value.name.clone(), value.value.clone()))
            .collect();
        let extras = catalog.request_extras(&values, skip);
        let row = self
            .row(addon, manifest, catalog, extras, REQUEST_TIMEOUT)
            .await;
        match row.error {
            Some(error) => Err(error),
            None => Ok(row),
        }
    }

    /// One catalog's results for `query`. A search for another query stops this one.
    pub async fn search_catalog(
        &self,
        addon_id: &str,
        kind: &str,
        catalog_id: &str,
        query: &str,
    ) -> Result<StremioCatalogRow, String> {
        let query = query.trim().to_owned();
        let active = self.active().await?;
        let addon = active
            .iter()
            .find(|addon| addon.row.id == addon_id)
            .ok_or_else(|| "This addon was removed or turned off".to_owned())?;
        let manifest = addon.manifest().ok_or("This addon has no manifest")?;
        let catalog = manifest
            .catalogs
            .iter()
            .find(|catalog| {
                catalog.kind == kind && catalog.id == catalog_id && catalog.searchable()
            })
            .ok_or_else(|| "The addon no longer searches this catalog".to_owned())?;
        self.searching(&query, self.search_row(addon, manifest, catalog, &query))
            .await
            .ok_or_else(|| "A newer search replaced this one".to_owned())
    }

    /// Every catalog that searches, asked for `query` a few at a time; rows without results are
    /// left out. A newer search stops this one.
    pub async fn search(&self, query: &str) -> Result<Vec<StremioCatalogRow>, String> {
        let query = query.trim().to_owned();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let active = self.active().await?;
        let rows = active
            .iter()
            .filter_map(|addon| Some((addon, addon.manifest()?)))
            .flat_map(|(addon, manifest)| {
                manifest
                    .catalogs
                    .iter()
                    .filter(|catalog| catalog.searchable())
                    .map(move |catalog| (addon, manifest, catalog))
            })
            // Collected first: a lazy iterator of closures inside the future trips rustc's
            // higher-ranked lifetime check for the command's `Send`.
            .collect::<Vec<_>>();
        let asks = rows
            .into_iter()
            .map(|(addon, manifest, catalog)| self.search_row(addon, manifest, catalog, &query))
            .collect::<Vec<_>>();
        let found = self
            .searching(&query, join_all(asks))
            .await
            .ok_or_else(|| "A newer search replaced this one".to_owned())?;
        Ok(found
            .into_iter()
            .filter(|row| !row.items.is_empty())
            .collect())
    }

    async fn search_row(
        &self,
        addon: &Loaded,
        manifest: &Manifest,
        catalog: &Catalog,
        query: &str,
    ) -> StremioCatalogRow {
        let extras = vec![("search".to_owned(), query.to_owned())];
        self.row(addon, manifest, catalog, extras, SEARCH_TIMEOUT)
            .await
    }

    /// Runs `work` for a search of `query`; `None` once a search for another query starts.
    async fn searching<T>(&self, query: &str, work: impl Future<Output = T>) -> Option<T> {
        self.search.send_if_modified(|current| {
            if current == query {
                false
            } else {
                query.clone_into(current);
                true
            }
        });
        let mut changes = self.search.subscribe();
        let replaced = async move {
            loop {
                if changes.changed().await.is_err() {
                    std::future::pending::<()>().await;
                }
                if *changes.borrow() != query {
                    return;
                }
            }
        };
        tokio::select! {
            done = work => Some(done),
            () = replaced => None,
        }
    }

    async fn row(
        &self,
        addon: &Loaded,
        manifest: &Manifest,
        catalog: &Catalog,
        extras: Vec<(String, String)>,
        timeout: Duration,
    ) -> StremioCatalogRow {
        let url = protocol::resource_url(
            &addon.row.transport_url,
            "catalog",
            &catalog.kind,
            &catalog.id,
            &extras,
        );
        let permit = self.browsing.acquire().await;
        let answer = self.resource(addon, "catalog", &url, timeout).await;
        drop(permit);
        let (items, error) = match answer {
            Ok(value) => (protocol::parse_metas(&value, &catalog.kind), None),
            Err(error) => (Vec::new(), Some(error.message)),
        };
        StremioCatalogRow {
            addon_id: addon.row.id.clone(),
            addon_name: manifest.name.clone(),
            kind: catalog.kind.clone(),
            catalog_id: catalog.id.clone(),
            name: catalog.name.clone(),
            has_more: catalog.pages() && !items.is_empty(),
            genres: catalog
                .extra("genre")
                .map(|extra| extra.options.clone())
                .unwrap_or_default(),
            items: items
                .into_iter()
                .map(|preview| self.listed(item(preview)))
                .collect(),
            error,
        }
    }

    // ---- Titles ----

    /// A title's details from the first addon that has them, with watching progress. `id` (the
    /// catalog's id for it) stays the title's id whatever id the details carry; a title in the
    /// library gets its stored episodes brought up to date on the way, and opens from what's
    /// stored when no addon answers (offline, or the addon is down).
    pub async fn meta(&self, kind: &str, id: &str) -> Result<StremioMeta, String> {
        let db = self.db();
        let stored = titles::find_title(&db, STREMIO_SOURCE, &library_id(kind, id))
            .await
            .map_err(to_message)?;
        let found = match self.find_meta(kind, id, false).await {
            Ok(found) => found,
            Err(error) => match stored.filter(|row| row.in_library_at.is_some()) {
                Some(row) => {
                    tracing::info!(%error, "showing a saved title as stored");
                    let meta = stored_meta(&db, &row).await?;
                    return self.meta_view(kind, id, meta, "", "", Some(&row)).await;
                }
                None => return Err(error),
            },
        };
        let stored = match stored {
            Some(row) if row.in_library_at.is_some() => {
                match self.store(kind, id, &found.meta).await {
                    Ok((row, _)) => Some(row),
                    Err(error) => {
                        tracing::warn!(%error, "could not store a library title's episodes");
                        Some(row)
                    }
                }
            }
            other => other,
        };
        self.meta_view(
            kind,
            id,
            found.meta,
            &found.addon_id,
            &found.addon_name,
            stored.as_ref(),
        )
        .await
    }

    /// Every addon's ratings for a title (M46.45: the stars on its page open them), asked at
    /// once; the same site's rating is listed once.
    pub async fn ratings(&self, kind: &str, id: &str) -> Result<Vec<TitleRating>, String> {
        let active = self.active().await?;
        let asks = active
            .iter()
            .filter(|addon| {
                addon
                    .manifest()
                    .is_some_and(|manifest| manifest.serves("meta", kind, id))
            })
            .map(|addon| {
                let url = protocol::resource_url(&addon.row.transport_url, "meta", kind, id, &[]);
                async move {
                    let _permit = self.asking.acquire().await;
                    let value = self
                        .resource(addon, "meta", &url, REQUEST_TIMEOUT)
                        .await
                        .ok()?;
                    protocol::parse_meta(&value, kind).map(|meta| meta.ratings)
                }
            })
            .collect::<Vec<_>>();
        let mut ratings: Vec<TitleRating> = Vec::new();
        for found in join_all(asks).await.into_iter().flatten() {
            for rating in found {
                if !ratings.iter().any(|have| have.source == rating.source) {
                    ratings.push(rating);
                }
            }
        }
        Ok(ratings)
    }

    /// A title's details as the first addon serving them (in the addon order) has them: every
    /// such addon is asked at once, and the first in order that answers wins (stremio-core's
    /// `MetaDetails`). Kept for a while, unless `fresh` ones are wanted.
    async fn find_meta(&self, kind: &str, id: &str, fresh: bool) -> Result<FoundMeta, String> {
        let key = (kind.to_owned(), id.to_owned());
        if !fresh
            && let Some(found) = self
                .metas
                .lock()
                .expect("metas poisoned")
                .get(&key)
                .filter(|found| now_ms() - found.at < META_FRESH_MS)
                .cloned()
        {
            return Ok(found);
        }
        let active = self.active().await?;
        let serving: Vec<&Loaded> = active
            .iter()
            .filter(|addon| {
                addon
                    .manifest()
                    .is_some_and(|manifest| manifest.serves("meta", kind, id))
            })
            .collect();
        if serving.is_empty() {
            return Err(
                "None of your addons has details for this title. Addons that provide details can be added in Addons"
                    .to_owned(),
            );
        }
        let count = serving.len();
        let mut asks: FuturesUnordered<_> = serving
            .iter()
            .enumerate()
            .map(|(index, addon)| async move {
                let url = protocol::resource_url(&addon.row.transport_url, "meta", kind, id, &[]);
                let _permit = self.asking.acquire().await;
                let answer = match self.resource(addon, "meta", &url, REQUEST_TIMEOUT).await {
                    Ok(value) => protocol::parse_meta(&value, kind)
                        .ok_or_else(|| "no details for this title".to_owned()),
                    Err(error) => Err(error.message),
                };
                (index, answer)
            })
            .collect();
        // Each addon's answer, in the addon order: the first with details wins once every addon
        // before it has failed, or after a short wait for them (stremio-web shows the first
        // that's ready; a slow addon first in the list doesn't hold the page up).
        let mut answers: Vec<Option<Result<protocol::Meta, String>>> =
            std::iter::repeat_with(|| None).take(count).collect();
        let mut wait_until: Option<tokio::time::Instant> = None;
        loop {
            if let Some(best) = answers
                .iter()
                .position(|answer| matches!(answer, Some(Ok(_))))
            {
                if answers[..best].iter().all(Option::is_some) {
                    break;
                }
                wait_until.get_or_insert_with(|| tokio::time::Instant::now() + META_GRACE);
            }
            let next = match wait_until {
                Some(at) => tokio::time::timeout_at(at, asks.next())
                    .await
                    .ok()
                    .flatten(),
                None => asks.next().await,
            };
            let Some((index, answer)) = next else {
                break;
            };
            answers[index] = Some(answer);
        }
        drop(asks);
        let mut errors = Vec::new();
        for (addon, answer) in serving.iter().zip(answers) {
            match answer {
                Some(Ok(meta)) => {
                    let found = FoundMeta {
                        meta,
                        addon_id: addon.row.id.clone(),
                        addon_name: addon.name(),
                        at: now_ms(),
                    };
                    self.metas
                        .lock()
                        .expect("metas poisoned")
                        .insert(key, found.clone());
                    return Ok(found);
                }
                Some(Err(error)) => errors.push(format!("{}: {error}", addon.name())),
                None => errors.push(format!("{}: didn't answer in time", addon.name())),
            }
        }
        Err(errors.join("\n"))
    }

    /// The numberings a stream's name may give a video of a title found lately (see
    /// [`numberings`]), to tell streams of another episode; none for a video not known.
    pub fn episode_numbers(&self, kind: &str, video_id: &str) -> Vec<video_match::EpisodeNumber> {
        let Some(meta) = self.meta_of_video(kind, video_id) else {
            return Vec::new();
        };
        meta.videos
            .iter()
            .position(|video| video.id == video_id)
            .and_then(|index| numberings(&meta).into_iter().nth(index))
            .unwrap_or_default()
    }

    /// The details found lately for the title of a movie or episode, by its video id (the player
    /// asks by the video alone).
    fn meta_of_video(&self, kind: &str, video_id: &str) -> Option<protocol::Meta> {
        self.metas
            .lock()
            .expect("metas poisoned")
            .iter()
            .find(|((known_kind, id), found)| {
                known_kind == kind
                    && (id == video_id
                        || found.meta.own_video_id() == video_id
                        || found.meta.video(video_id).is_some())
            })
            .map(|(_, found)| found.meta.clone())
    }

    fn forget_metas(&self) {
        self.metas.lock().expect("metas poisoned").clear();
    }

    async fn meta_view(
        &self,
        kind: &str,
        id: &str,
        meta: protocol::Meta,
        addon_id: &str,
        addon_name: &str,
        stored: Option<&TitleRow>,
    ) -> Result<StremioMeta, String> {
        let db = self.db();
        let video_id = meta.own_video_id();
        let own_key = content_key(kind, &video_id);
        let keys: Vec<String> = meta
            .videos
            .iter()
            .map(|video| content_key(kind, &video.id))
            .chain([own_key.clone()])
            .collect();
        let progress = progress_for_keys(&db, &keys).await?;
        // Episodes marked in the library (a whole title marked watched) are marked on its
        // stored episodes only.
        let units: HashMap<String, titles::UnitRow> = match stored {
            Some(row) => titles::units(&db, &row.id)
                .await
                .map_err(to_message)?
                .into_iter()
                .map(|unit| (unit.fields.remote_id.clone(), unit))
                .collect(),
            None => HashMap::new(),
        };
        let continue_video_id =
            continue_video(kind, &meta.videos, &progress, &units, now_ms()).map(str::to_owned);
        let downloaded = video_library::downloaded_keys(&db, &keys).await?;
        let own = progress.get(&own_key);
        let in_library = stored.is_some_and(|row| row.in_library_at.is_some());
        Ok(StremioMeta {
            continue_video_id,
            videos: meta
                .videos
                .into_iter()
                .map(|video| {
                    let key = content_key(kind, &video.id);
                    let saved = progress.get(&key);
                    let marked = units
                        .get(&video.id)
                        .is_some_and(|unit| unit.completed_at.is_some());
                    StremioVideo {
                        position_ms: saved.map_or(0.0, |row| row.position_ms as f64),
                        duration_ms: saved.and_then(|row| row.duration_ms).map(|ms| ms as f64),
                        watched: saved.is_some_and(|row| row.watched_at.is_some()) || marked,
                        downloaded: downloaded.contains(&key),
                        id: video.id,
                        title: video.title,
                        season: video.season,
                        episode: video.episode,
                        released: video.released,
                        thumbnail: video.thumbnail.map(|url| self.image(url)),
                        overview: video.overview,
                    }
                })
                .collect(),
            position_ms: own.map_or(0.0, |row| row.position_ms as f64),
            duration_ms: own.and_then(|row| row.duration_ms).map(|ms| ms as f64),
            watched: own.is_some_and(|row| row.watched_at.is_some()),
            downloaded: downloaded.contains(&own_key),
            in_library,
            title_id: stored.map(|row| row.id.clone()),
            kind: kind.to_owned(),
            id: id.to_owned(),
            video_id,
            live: meta.live,
            name: meta.preview.name,
            // A saved title's art stays cached whatever the size limit (M46.2).
            poster: meta.preview.poster.map(|url| {
                if in_library {
                    self.kept_image(url)
                } else {
                    self.image(url)
                }
            }),
            background: meta.background.map(|url| self.image(url)),
            logo: meta.logo.map(|url| self.image(url)),
            description: meta.description,
            genres: meta.genres,
            release_info: meta.preview.release_info,
            runtime: meta.runtime,
            rating: meta.preview.rating,
            ratings: meta.ratings,
            cast: meta.cast,
            director: meta.director,
            trailer_yt_ids: meta.trailer_yt_ids,
            addon_id: addon_id.to_owned(),
            addon_name: addon_name.to_owned(),
        })
    }

    /// Streams for a movie or episode of the title `meta_id`: the details' own (a channel's
    /// videos, YouTube's) first, then every addon that serves streams, each asked by an id it
    /// knows, a few at a time; addons that didn't answer lately come last.
    pub async fn streams(
        &self,
        kind: &str,
        meta_id: Option<&str>,
        video_id: &str,
    ) -> Result<Vec<StremioStreamGroup>, String> {
        let found = match meta_id {
            Some(meta_id) => match self.find_meta(kind, meta_id, false).await {
                Ok(found) => Some(found),
                Err(error) => {
                    tracing::info!(%error, "streams asked for without the title's details");
                    None
                }
            },
            None => None,
        };
        let ids = match &found {
            Some(found) => found.meta.stream_ids_for_video(video_id),
            None => self.meta_of_video(kind, video_id).map_or_else(
                || vec![video_id.to_owned()],
                |meta| meta.stream_ids_for_video(video_id),
            ),
        };
        let active = self.active().await?;
        let health = self.health_now().await;
        let health = &health;
        let now = now_ms();
        let asks = active
            .iter()
            .filter_map(|addon| {
                let manifest = addon.manifest()?;
                let id = ids.iter().find(|id| manifest.serves("stream", kind, id))?;
                Some((addon, manifest, id.clone()))
            })
            .map(|(addon, manifest, id)| async move {
                let url =
                    protocol::resource_url(&addon.row.transport_url, "stream", kind, &id, &[]);
                let permit = self.asking.acquire().await;
                let answer = self.resource(addon, "stream", &url, REQUEST_TIMEOUT).await;
                drop(permit);
                let (streams, error) = match answer {
                    Ok(value) => (protocol::parse_streams(&value), None),
                    Err(error) => (Vec::new(), Some(error.message)),
                };
                StremioStreamGroup {
                    failing: health
                        .get(&addon.row.id)
                        .is_some_and(|health| health.failing(now)),
                    addon_id: addon.row.id.clone(),
                    addon_name: manifest.name.clone(),
                    streams: streams.into_iter().map(stream_view).collect(),
                    error,
                }
            })
            .collect::<Vec<_>>();
        let mut groups = Vec::new();
        if let Some(found) = &found
            && let Some(video) = found.meta.video(video_id)
            && !video.streams.is_empty()
        {
            groups.push(StremioStreamGroup {
                addon_id: found.addon_id.clone(),
                addon_name: found.addon_name.clone(),
                streams: video.streams.iter().cloned().map(stream_view).collect(),
                error: None,
                failing: false,
            });
        }
        groups.extend(join_all(asks).await);
        Ok(groups)
    }

    /// Subtitles from every subtitle addon, each asked by an id it knows, the preferred language
    /// first. A chosen stream's file name, size and hash go along, as Stremio sends them.
    async fn subtitles(
        &self,
        kind: &str,
        meta: Option<&protocol::Meta>,
        video_id: &str,
        language: &str,
        stream: Option<&StremioStream>,
    ) -> Vec<protocol::Subtitle> {
        let Ok(active) = self.active().await else {
            return Vec::new();
        };
        let ids = match meta {
            Some(meta) => meta.stream_ids_for_video(video_id),
            None => self.meta_of_video(kind, video_id).map_or_else(
                || vec![video_id.to_owned()],
                |meta| meta.stream_ids_for_video(video_id),
            ),
        };
        // stremio-core's order: each extra goes in front of the ones before it.
        let mut extras = Vec::new();
        if let Some(stream) = stream {
            extras.extend(
                stream
                    .filename
                    .clone()
                    .map(|name| ("filename".to_owned(), name)),
            );
            extras.extend(
                stream
                    .size
                    .filter(|size| *size > 0.0)
                    .map(|size| ("videoSize".to_owned(), (size as u64).to_string())),
            );
            extras.extend(
                stream
                    .video_hash
                    .clone()
                    .map(|hash| ("videoHash".to_owned(), hash)),
            );
        }
        let extras = &extras;
        let asks = active
            .iter()
            .filter_map(|addon| {
                let manifest = addon.manifest()?;
                let id = ids
                    .iter()
                    .find(|id| manifest.serves("subtitles", kind, id))?;
                Some((addon, id.clone()))
            })
            .map(|(addon, id)| async move {
                let url = protocol::resource_url(
                    &addon.row.transport_url,
                    "subtitles",
                    kind,
                    &id,
                    extras,
                );
                let _permit = self.asking.acquire().await;
                self.resource(addon, "subtitles", &url, REQUEST_TIMEOUT)
                    .await
                    .map(|value| protocol::parse_subtitles(&value))
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        let all = join_all(asks).await.into_iter().flatten().collect();
        protocol::order_subtitles(all, language, MAX_SUBTITLES)
    }

    /// Subtitles for a movie or episode from every added addon that has them, in `language`
    /// first (M46.33: Find subtitles in the player); and whether any added addon serves
    /// subtitles at all.
    pub async fn find_subtitles(
        &self,
        kind: &str,
        video_id: &str,
        language: &str,
    ) -> (Vec<VideoSubtitle>, bool) {
        let meta = self.meta_of_video(kind, video_id);
        let ids = meta.as_ref().map_or_else(
            || vec![video_id.to_owned()],
            |meta| meta.stream_ids_for_video(video_id),
        );
        let serves = self.active().await.is_ok_and(|active| {
            active
                .iter()
                .filter_map(|addon| addon.manifest())
                .any(|manifest| ids.iter().any(|id| manifest.serves("subtitles", kind, id)))
        });
        let found = self
            .subtitles(kind, meta.as_ref(), video_id, language, None)
            .await;
        (addon_subtitles(found), serves)
    }

    /// The suggested subtitles addon (OpenSubtitles v3) when it isn't added (M46.33).
    pub async fn subtitles_suggestion(&self) -> Option<StremioSuggestion> {
        self.suggestions()
            .await
            .ok()?
            .into_iter()
            .find(|suggestion| suggestion.url.contains("opensubtitles") && !suggestion.added)
    }

    /// Plays a stream: direct ones in Raygun's player (with subtitles), YouTube ones in the
    /// embedded player, outside links in the browser.
    pub async fn play(&self, request: StremioPlayRequest, language: &str) -> Result<(), String> {
        let stream = &request.stream;
        match stream.kind {
            StremioStreamKind::Direct => {
                let video = self.request(&request, language).await?;
                if request.external {
                    crate::video::play_elsewhere(&self.app, video).await
                } else {
                    crate::video::play(&self.app, video).await
                }
            }
            // Trailers and YouTube streams get their own full-window player (M46.4); the music
            // queue is left alone.
            StremioStreamKind::YouTube => {
                let embed_url = self
                    .app
                    .try_state::<raygun_net::MediaLinks>()
                    .ok_or("The YouTube player isn't available")?
                    .youtube_embed_url();
                crate::playback::other_media_started(&self.app).await;
                YouTubeVideoEvent(YouTubeVideo {
                    video_id: stream.target.clone(),
                    title: request.title.clone(),
                    subtitle: request.subtitle.clone(),
                    embed_url,
                })
                .emit(&self.app)
                .map_err(|error| error.to_string())
            }
            StremioStreamKind::External => self.open_link(&stream.target),
            StremioStreamKind::Torrent => {
                Err("This is a torrent: a torrent app plays it, from its magnet link".into())
            }
            StremioStreamKind::Server => {
                Err("Only the Stremio app plays this kind of stream (archives, Usenet)".into())
            }
            StremioStreamKind::Title => Err("This opens another title".into()),
        }
    }

    /// Opens a web page (an addon's setup page, an outside stream) or, on desktop, a torrent's
    /// magnet link with the system.
    pub fn open_link(&self, url: &str) -> Result<(), String> {
        let web = url.starts_with("https://") || url.starts_with("http://");
        if !web && !url.starts_with("magnet:") {
            return Err("Only web pages and magnet links open from here".into());
        }
        #[cfg(target_os = "android")]
        {
            if !url.starts_with("https://") {
                return Err("Only https pages open from here on Android".into());
            }
            self.app
                .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .open_url(url)
        }
        #[cfg(not(target_os = "android"))]
        {
            #[cfg(windows)]
            let mut command = std::process::Command::new("explorer");
            #[cfg(target_os = "macos")]
            let mut command = std::process::Command::new("open");
            #[cfg(not(any(windows, target_os = "macos")))]
            let mut command = std::process::Command::new("xdg-open");
            command
                .arg(url)
                .spawn()
                .map(|_| ())
                .map_err(|error| format!("Nothing opened it ({error})"))
        }
    }

    /// Opens the setup page (`configure`) of the addon at `url` in the browser.
    pub fn configure(&self, url: &str) -> Result<(), String> {
        let url = protocol::normalize_addon_url(url)?;
        self.open_link(&protocol::configure_url(&url))
    }

    /// What the player gets for a direct stream, with the subtitle addons' tracks (`language`
    /// first). Also what a download saves.
    pub async fn request(
        &self,
        request: &StremioPlayRequest,
        language: &str,
    ) -> Result<VideoRequest, String> {
        let stream = &request.stream;
        if stream.kind != StremioStreamKind::Direct {
            return Err("Only direct streams can be played or saved here".into());
        }
        self.app
            .state::<LocalNetwork>()
            .check(&stream.target, local_network::VIDEOS_DENIED)
            .await?;
        let meta = match &request.meta_id {
            Some(meta_id) => self
                .find_meta(&request.kind, meta_id, false)
                .await
                .ok()
                .map(|found| found.meta),
            None => None,
        };
        let found = self
            .subtitles(
                &request.kind,
                meta.as_ref(),
                &request.video_id,
                language,
                Some(stream),
            )
            .await;
        Ok(direct_request(request, &addon_subtitles(found)))
    }

    /// Every stream a session can play of a movie or episode, as player requests grouped by
    /// addon, each with the subtitle addons' tracks (asked once): direct ones in Raygun's player,
    /// YouTube's in its embed. Playback sessions pick from these (M44).
    pub async fn playable(
        &self,
        target: &StremioPlayTarget,
        language: &str,
    ) -> Result<Vec<video_session::Group>, String> {
        let meta = match &target.meta_id {
            Some(meta_id) => self
                .find_meta(&target.kind, meta_id, false)
                .await
                .ok()
                .map(|found| found.meta),
            None => None,
        };
        let (groups, found) = futures_util::future::join(
            self.streams(&target.kind, target.meta_id.as_deref(), &target.video_id),
            self.subtitles(
                &target.kind,
                meta.as_ref(),
                &target.video_id,
                language,
                None,
            ),
        )
        .await;
        let extra = addon_subtitles(found);
        let network = self.app.state::<LocalNetwork>();
        let mut playable = Vec::new();
        for group in groups? {
            let mut links = Vec::new();
            for stream in group.streams {
                let youtube = match stream.kind {
                    StremioStreamKind::Direct => false,
                    StremioStreamKind::YouTube => true,
                    _ => continue,
                };
                let label = [stream.name.clone(), stream.description.clone()]
                    .into_iter()
                    .flatten()
                    .map(|text| text.replace('\n', " "))
                    .collect::<Vec<_>>()
                    .join(" · ");
                let binge_group = stream.binge_group.clone();
                let blocked = if youtube {
                    None
                } else {
                    network
                        .check(&stream.target, local_network::VIDEOS_DENIED)
                        .await
                        .err()
                };
                let mut request = direct_request(
                    &StremioPlayRequest {
                        kind: target.kind.clone(),
                        video_id: target.video_id.clone(),
                        title: target.title.clone(),
                        subtitle: target.subtitle.clone(),
                        poster: target.poster.clone(),
                        stream,
                        // The session gives every link the series' memory.
                        meta_id: None,
                        external: false,
                    },
                    &extra,
                );
                if youtube {
                    // Plays in YouTube's embed (`video::YOUTUBE_LINK`).
                    request.url = format!("{}{}", crate::video::YOUTUBE_LINK, request.url);
                    request.subtitles.clear();
                    request.format = None;
                }
                links.push(video_session::Link {
                    label: if label.is_empty() {
                        group.addon_name.clone()
                    } else {
                        label
                    },
                    tag: binge_group,
                    failed: blocked,
                    mismatch: false,
                    request,
                });
            }
            playable.push(video_session::Group::finished(
                group.addon_id,
                group.addon_name,
                links,
                group.error,
            ));
        }
        Ok(playable)
    }

    /// Marks a movie or episode watched or not.
    pub async fn set_watched(
        &self,
        kind: &str,
        video_id: &str,
        title: &str,
        subtitle: Option<String>,
        poster: Option<String>,
        watched: bool,
    ) -> Result<(), String> {
        let entry = VideoEntry {
            key: content_key(kind, video_id),
            title: title.to_owned(),
            subtitle,
            poster,
            // Nothing was played, so there's no stream to replay.
            request: "{}".into(),
        };
        crate::video::mark(&self.db(), &entry, watched).await
    }

    // ---- Library (M43) ----

    /// Saves a movie or series to the video library, with its episodes, or takes it out.
    /// Episodes watched before it was saved count as watched.
    pub async fn set_in_library(
        &self,
        kind: &str,
        id: &str,
        in_library: bool,
    ) -> Result<(), String> {
        let db = self.db();
        if !in_library {
            let stored = titles::find_title(&db, STREMIO_SOURCE, &library_id(kind, id))
                .await
                .map_err(to_message)?;
            if let Some(row) = stored {
                titles::set_in_library(&db, &row.id, None)
                    .await
                    .map_err(to_message)?;
            }
            return Ok(());
        }
        let found = self.find_meta(kind, id, false).await?;
        let (row, _) = self.store(kind, id, &found.meta).await?;
        titles::set_in_library(&db, &row.id, Some(now_ms()))
            .await
            .map_err(to_message)?;
        backfill(&db, &row).await
    }

    /// Fetches a library title's details again and stores its episodes; returns how many are new.
    pub async fn refresh(&self, title: &TitleRow) -> Result<usize, String> {
        let (kind, id) = video_library::stremio_id(title)
            .ok_or_else(|| "This isn't a title from an addon".to_owned())?;
        let found = self.find_meta(kind, id, true).await?;
        Ok(self.store(kind, id, &found.meta).await?.1)
    }

    /// Stores a title under the id it's known by here, with its episodes (a movie is one
    /// episode: the id it plays by); returns it and how many episodes were new.
    async fn store(
        &self,
        kind: &str,
        id: &str,
        meta: &protocol::Meta,
    ) -> Result<(TitleRow, usize), String> {
        let db = self.db();
        let preview = &meta.preview;
        let release = preview.release_info.as_deref();
        let title_id = titles::upsert_title(
            &db,
            &TitleFields {
                kind: video_library::KIND.into(),
                source_id: STREMIO_SOURCE.into(),
                remote_id: library_id(kind, id),
                title: preview.name.clone(),
                cover: preview.poster.clone(),
                cover_headers: Default::default(),
                authors: meta.cast.clone(),
                artists: meta.director.clone(),
                description: meta.description.clone(),
                genres: meta.genres.clone(),
                status: release_status(kind, release).into(),
                url: None,
                media_type: Some(media_type(kind).into()),
                year: release.and_then(release_year),
            },
            now_ms(),
        )
        .await
        .map_err(to_message)?;
        let units: Vec<UnitFields> = if meta.videos.is_empty() {
            vec![UnitFields {
                remote_id: meta.own_video_id(),
                name: preview.name.clone(),
                number: None,
                volume: None,
                scanlator: None,
                date: None,
            }]
        } else {
            meta.videos
                .iter()
                .map(|video| UnitFields {
                    remote_id: video.id.clone(),
                    name: video.title.clone(),
                    number: video.episode.map(f64::from),
                    volume: video.season.map(|season| season.to_string()),
                    scanlator: None,
                    date: video.released.as_deref().and_then(release_ms),
                })
                .collect()
        };
        let added = titles::sync_units(&db, &title_id, &units, "episode", now_ms())
            .await
            .map_err(to_message)?;
        let row = titles::title(&db, &title_id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "The title went missing".to_owned())?;
        Ok((row, added))
    }

    // ---- HTTP ----

    /// An addon's answer to a resource request, in today's shape (legacy addons' too), noting how
    /// the addon did.
    async fn resource(
        &self,
        addon: &Loaded,
        resource: &str,
        url: &str,
        timeout: Duration,
    ) -> Result<Value, AddonError> {
        let value = self.get(Some(&addon.row.id), url, timeout).await?;
        if protocol::is_legacy(&addon.row.transport_url) {
            protocol::legacy_answer(resource, value).map_err(|message| AddonError {
                message,
                down: false,
            })
        } else {
            Ok(value)
        }
    }

    async fn get(
        &self,
        addon_id: Option<&str>,
        url: &str,
        timeout: Duration,
    ) -> Result<Value, AddonError> {
        let result = self.fetch_json(url, timeout).await;
        if let Some(addon_id) = addon_id {
            match &result {
                Ok(_) => self.note_health(addon_id, true).await,
                Err(error) if error.down => self.note_health(addon_id, false).await,
                Err(_) => {}
            }
        }
        result
    }

    async fn fetch_json(&self, url: &str, timeout: Duration) -> Result<Value, AddonError> {
        let quiet = |message: String| AddonError {
            message,
            down: false,
        };
        let down = |message: String| AddonError {
            message,
            down: true,
        };
        // Without Android 17's local network permission, an addon on the LAN would time out.
        self.app
            .state::<LocalNetwork>()
            .check(url, local_network::ADDONS_DENIED)
            .await
            .map_err(quiet)?;
        let response = self
            .client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .timeout(timeout)
            .send()
            .await
            .map_err(|error| {
                down(if error.is_timeout() {
                    "The addon didn't answer in time".to_owned()
                } else {
                    format!("Couldn't reach the addon ({})", root_cause(&error))
                })
            })?;
        let status = response.status();
        if !status.is_success() {
            let message = match status.as_u16() {
                404 => "The addon has nothing here (404)".to_owned(),
                code @ 400..=499 => format!("The addon refused the request ({code})"),
                code => format!("The addon had a problem ({code})"),
            };
            return Err(if status.is_server_error() {
                down(message)
            } else {
                quiet(message)
            });
        }
        let mut bytes = Vec::new();
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|error| {
                down(if error.is_timeout() {
                    "The addon didn't answer in time".to_owned()
                } else {
                    format!("The addon's answer broke off ({})", root_cause(&error))
                })
            })?;
            if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err(quiet("The addon's answer is too large".into()));
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| down("The addon's answer couldn't be read".to_owned()))
    }

    /// How addons did lately (read from storage the first time it's needed).
    async fn health_now(&self) -> HashMap<String, Health> {
        if let Some(health) = self.health.lock().expect("health poisoned").clone() {
            return health;
        }
        let saved: HashMap<String, Health> = raygun_db::local_value(&self.db(), HEALTH_KEY)
            .await
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let mut health = self.health.lock().expect("health poisoned");
        health.get_or_insert(saved).clone()
    }

    /// Notes that an addon answered or failed; saved when that changes whether it's failing.
    async fn note_health(&self, addon_id: &str, ok: bool) {
        let _ = self.health_now().await;
        let now = now_ms();
        let save = {
            let mut guard = self.health.lock().expect("health poisoned");
            let all = guard.get_or_insert_with(HashMap::new);
            let entry = all.entry(addon_id.to_owned()).or_default();
            let was = entry.failing(now);
            if ok {
                entry.ok_at = now;
            } else {
                entry.failed_at = now;
            }
            (was != entry.failing(now)).then(|| serde_json::to_string(all).ok())
        };
        if let Some(Some(text)) = save
            && let Err(error) = raygun_db::set_local_value(&self.db(), HEALTH_KEY, &text).await
        {
            tracing::warn!(%error, "could not keep how addons did");
        }
    }

    /// An addon's image through the lasting cover cache (M46.2); addons send plain web addresses.
    fn image(&self, url: String) -> String {
        let images = self.app.state::<raygun_net::ImageLinks>();
        if images.is_remote(&url) {
            images.cover(&url, &Default::default())
        } else {
            url
        }
    }

    /// Like [`Self::image`], kept whatever the cache's size limit: saved titles, installed addons.
    fn kept_image(&self, url: String) -> String {
        let images = self.app.state::<raygun_net::ImageLinks>();
        if images.is_remote(&url) {
            images.kept_cover(&url, &Default::default())
        } else {
            url
        }
    }

    fn listed(&self, item: StremioItem) -> StremioItem {
        StremioItem {
            poster: item.poster.map(|url| self.image(url)),
            ..item
        }
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().0.clone()
    }
}

/// What an addon provides, as the Addons page lists it: catalogs by type with how many, details
/// by type, streams, subtitles.
fn provides(manifest: &Manifest) -> Vec<StremioProvides> {
    let mut list: Vec<StremioProvides> = Vec::new();
    for catalog in &manifest.catalogs {
        match list.iter_mut().find(|entry| {
            entry.resource == "catalog" && entry.kind.as_deref() == Some(catalog.kind.as_str())
        }) {
            Some(entry) => entry.count += 1,
            None => list.push(StremioProvides {
                resource: "catalog".into(),
                kind: Some(catalog.kind.clone()),
                count: 1,
            }),
        }
    }
    for kind in manifest.resource_types("meta") {
        list.push(StremioProvides {
            resource: "meta".into(),
            kind: Some(kind),
            count: 1,
        });
    }
    for resource in ["stream", "subtitles"] {
        if manifest.has_resource(resource) {
            list.push(StremioProvides {
                resource: resource.into(),
                kind: None,
                count: 1,
            });
        }
    }
    list
}

fn catalog_info(
    addon_id: &str,
    manifest: &Manifest,
    catalog: &Catalog,
    failing: bool,
) -> StremioCatalogInfo {
    StremioCatalogInfo {
        addon_id: addon_id.to_owned(),
        addon_name: manifest.name.clone(),
        kind: catalog.kind.clone(),
        catalog_id: catalog.id.clone(),
        name: catalog.name.clone(),
        extras: catalog
            .extras
            .iter()
            .filter(|extra| extra.name != "skip" && extra.name != "search")
            .filter(|extra| !extra.options.is_empty())
            .map(|extra| StremioExtra {
                name: extra.name.clone(),
                required: extra.required,
                options: extra.options.clone(),
            })
            .collect(),
        board: catalog.browsable(),
        searchable: catalog.searchable(),
        discoverable: catalog.default_required().is_some(),
        pages: catalog.pages(),
        failing,
    }
}

/// The key watching progress is kept under for a movie or episode.
pub fn content_key(kind: &str, video_id: &str) -> String {
    format!("stremio:{kind}:{video_id}")
}

/// The episode a series continues from (M46.48): the one watched last (its progress here, or a
/// mark on the stored episode), or the next one that is out once that was finished, in watching
/// order; see [`crate::catalog::continue_index`].
pub(crate) fn continue_video<'a>(
    kind: &str,
    videos: &'a [protocol::Video],
    progress: &HashMap<String, VideoProgress>,
    units: &HashMap<String, titles::UnitRow>,
    now: i64,
) -> Option<&'a str> {
    let saved = |video: &protocol::Video| progress.get(&content_key(kind, &video.id));
    let seasons = videos
        .iter()
        .any(|video| video.season.is_some_and(|season| season >= 1));
    let index = crate::catalog::continue_index(
        videos,
        |video| {
            let watched = saved(video)
                .filter(|row| row.position_ms > 0 || row.watched_at.is_some())
                .map(|row| row.updated_at);
            let marked = units
                .get(&video.id)
                .filter(|unit| unit.completed_at.is_some() || unit.position.unwrap_or(0) > 0)
                .and_then(|unit| unit.progress_at);
            watched.max(marked)
        },
        |video| {
            saved(video).is_some_and(|row| row.watched_at.is_some())
                || units
                    .get(&video.id)
                    .is_some_and(|unit| unit.completed_at.is_some())
        },
        |video| {
            video
                .released
                .as_deref()
                .and_then(release_ms)
                .is_none_or(|at| at <= now)
        },
        |video| video_match::is_special(video.season.map(f64::from), seasons),
    )?;
    videos.get(index).map(|video| video.id.as_str())
}

/// Progress saved for each of `keys`, by key (the exact keys: an addon's video ids needn't start
/// with the title's id).
async fn progress_for_keys(
    db: &raygun_db::Db,
    keys: &[String],
) -> Result<HashMap<String, VideoProgress>, String> {
    Ok(progress_store::progress_for_keys(db, keys)
        .await
        .map_err(to_message)?
        .into_iter()
        .map(|row| (row.entry.key.clone(), row))
        .collect())
}

/// Copies progress saved before a title was stored onto its episodes that have none (a series
/// saved after watching some of it), by each episode's own key.
async fn backfill(db: &raygun_db::Db, title: &TitleRow) -> Result<(), String> {
    let units = titles::units(db, &title.id).await.map_err(to_message)?;
    let keys: Vec<String> = units
        .iter()
        .map(|unit| video_library::unit_key(title, &unit.fields.remote_id))
        .collect();
    let saved = progress_for_keys(db, &keys).await?;
    for (unit, key) in units.iter().zip(&keys) {
        if unit.position.is_some() || unit.completed_at.is_some() {
            continue;
        }
        let Some(row) = saved.get(key) else {
            continue;
        };
        titles::set_watch_progress(
            db,
            &unit.id,
            row.position_ms,
            row.duration_ms,
            row.watched_at.is_some(),
            row.watched_at.unwrap_or(row.updated_at),
        )
        .await
        .map_err(to_message)?;
    }
    Ok(())
}

/// A saved title as an addon would describe it, from the title store: its details and episodes
/// (a movie's one unit is the id it plays by).
async fn stored_meta(db: &raygun_db::Db, row: &TitleRow) -> Result<protocol::Meta, String> {
    let (kind, id) = video_library::stremio_id(row)
        .ok_or_else(|| "This isn't a title from an addon".to_owned())?;
    let units = titles::units(db, &row.id).await.map_err(to_message)?;
    let movie = units.len() == 1
        && (units[0].fields.remote_id == id
            || (units[0].fields.number.is_none() && units[0].fields.volume.is_none()));
    let fields = &row.fields;
    Ok(protocol::Meta {
        ratings: Vec::new(),
        preview: MetaPreview {
            kind: kind.to_owned(),
            id: id.to_owned(),
            name: fields.title.clone(),
            poster: fields.cover.clone(),
            poster_shape: "poster".into(),
            release_info: fields.year.map(|year| year.to_string()),
            rating: None,
        },
        background: None,
        logo: None,
        description: fields.description.clone(),
        genres: fields.genres.clone(),
        runtime: None,
        cast: fields.authors.clone(),
        director: fields.artists.clone(),
        trailer_yt_ids: Vec::new(),
        default_video_id: movie.then(|| units[0].fields.remote_id.clone()),
        imdb_id: None,
        tmdb_id: None,
        live: false,
        videos: if movie {
            Vec::new()
        } else {
            units
                .into_iter()
                .map(|unit| protocol::Video {
                    season: unit
                        .fields
                        .volume
                        .as_deref()
                        .and_then(|season| season.parse().ok()),
                    episode: unit.fields.number.map(|number| number as u32),
                    released: unit
                        .fields
                        .date
                        .and_then(chrono::DateTime::from_timestamp_millis)
                        .map(|date| date.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
                    id: unit.fields.remote_id,
                    title: unit.fields.name,
                    thumbnail: None,
                    overview: None,
                    streams: Vec::new(),
                    imdb: None,
                })
                .collect()
        },
    })
}

/// The numberings a stream's name may give each of a title's videos by: its own season and
/// episode, the IMDb season and episode an anime addon maps it to (`Video::imdb`), and its
/// number across seasons. A Kitsu entry numbers every episode "season 1" while its name says
/// which season it is; its own number then goes with that season, or with none when its name
/// doesn't say.
pub(crate) fn numberings(meta: &protocol::Meta) -> Vec<Vec<video_match::EpisodeNumber>> {
    let title = video_match::parse_title(&meta.preview.name);
    let one_season = meta
        .videos
        .iter()
        .all(|video| video.season.is_none_or(|season| season == 1));
    let mut across = 0;
    meta.videos
        .iter()
        .map(|video| {
            let regular = video.season.is_some_and(|season| season >= 1);
            if regular {
                across += 1;
            }
            let Some(episode) = video.episode else {
                return Vec::new();
            };
            let (imdb_season, imdb_episode) = video
                .imdb
                .as_ref()
                .map_or((None, None), |(_, season, episode)| (*season, *episode));
            let season = if one_season && title.names_season() {
                title.season
            } else if imdb_season.is_some() {
                // An entry numbered as its IMDb season is (Attack on Titan's first) is that
                // season; one numbered across seasons (One Piece) isn't any one.
                imdb_season.filter(|_| imdb_episode == Some(episode))
            } else {
                video.season
            };
            let mut numbers = vec![video_match::EpisodeNumber { season, episode }];
            if let (Some(season), Some(episode)) = (imdb_season, imdb_episode) {
                numbers.push(video_match::EpisodeNumber {
                    season: Some(season),
                    episode,
                });
            }
            // Counted over the title's seasons (Cinemeta's); only when it differs: "episode 2 of
            // any season" would let other seasons' files pass.
            let absolute = (regular && imdb_season.is_none())
                .then_some(across)
                .filter(|absolute| *absolute != episode);
            if let Some(episode) = absolute {
                numbers.push(video_match::EpisodeNumber {
                    season: None,
                    episode,
                });
            }
            let mut seen = std::collections::HashSet::new();
            numbers.retain(|number| seen.insert(*number));
            numbers
        })
        .collect()
}

/// What a session plays for a Stremio movie or episode: the ids and what the player shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StremioPlayTarget {
    pub kind: String,
    /// The movie's or series' id, whose details say which ids its streams go by.
    pub meta_id: Option<String>,
    pub video_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub poster: Option<String>,
}

/// The subtitle addons' tracks as player subtitles.
fn addon_subtitles(found: Vec<protocol::Subtitle>) -> Vec<VideoSubtitle> {
    found
        .into_iter()
        .map(|subtitle| VideoSubtitle {
            url: subtitle.url,
            label: subtitle.label.or_else(|| subtitle.lang.clone()),
            lang: subtitle.lang,
            format: None,
        })
        .collect()
}

/// A direct stream as a player request: its own subtitles first, then `extra`.
fn direct_request(request: &StremioPlayRequest, extra: &[VideoSubtitle]) -> VideoRequest {
    let stream = &request.stream;
    let mut subtitles: Vec<VideoSubtitle> = stream.subtitles.clone();
    subtitles.extend(extra.iter().cloned());
    subtitles.truncate(MAX_SUBTITLES);
    let lower = stream.target.to_lowercase();
    let path = lower.split(['?', '#']).next().unwrap_or(&lower);
    let format = if path.ends_with(".m3u8") || (!path.ends_with(".mpd") && lower.contains(".m3u8"))
    {
        Some("hls".to_owned())
    } else if path.ends_with(".mpd") || lower.contains(".mpd") {
        Some("dash".to_owned())
    } else {
        None
    };
    VideoRequest {
        key: content_key(&request.kind, &request.video_id),
        title: request.title.clone(),
        subtitle: request.subtitle.clone(),
        poster: request.poster.clone(),
        url: stream.target.clone(),
        headers: stream.headers.clone(),
        format,
        subtitles,
        group: request
            .meta_id
            .as_ref()
            .map(|id| format!("stremio:{}:{id}", request.kind)),
        chapters: Vec::new(),
        anime: None,
    }
}

/// A Stremio title's `remote_id` in the title store.
fn library_id(kind: &str, id: &str) -> String {
    format!("{kind}:{id}")
}

/// A title's status from its type and release range: "2008–2013" ended, "2019–" still
/// running.
fn release_status(kind: &str, release_info: Option<&str>) -> &'static str {
    if kind == "movie" {
        return "completed";
    }
    let Some(range) = release_info.map(str::trim) else {
        return "unknown";
    };
    match range.split_once(['–', '-']) {
        Some((_, end)) if end.trim().is_empty() => "ongoing",
        Some((_, end)) if release_year(end).is_some() => "completed",
        // A single year: a miniseries, or a show that started this year.
        _ => "unknown",
    }
}

/// The first four-digit year in a release range.
fn release_year(text: &str) -> Option<i32> {
    let characters: Vec<char> = text.chars().collect();
    characters
        .windows(4)
        .find(|window| window.iter().all(char::is_ascii_digit))
        .and_then(|window| window.iter().collect::<String>().parse().ok())
}

/// A release date (`2008-01-20T12:00:00.000Z` or `2008-01-20`) as Unix ms.
pub(crate) fn release_ms(text: &str) -> Option<i64> {
    protocol::release_ms(text)
}

/// The title store's media type for a Stremio type.
fn media_type(kind: &str) -> &'static str {
    match kind {
        "movie" => "movie",
        "series" => "series",
        _ => "other",
    }
}

fn item(preview: MetaPreview) -> StremioItem {
    StremioItem {
        kind: preview.kind,
        id: preview.id,
        name: preview.name,
        poster: preview.poster,
        poster_shape: preview.poster_shape,
        release_info: preview.release_info,
        rating: preview.rating,
    }
}

fn stream_view(stream: protocol::Stream) -> StremioStream {
    StremioStream {
        kind: match stream.source {
            StreamSource::Direct => StremioStreamKind::Direct,
            StreamSource::YouTube => StremioStreamKind::YouTube,
            StreamSource::External => StremioStreamKind::External,
            StreamSource::Torrent => StremioStreamKind::Torrent,
            StreamSource::Title => StremioStreamKind::Title,
            StreamSource::Server => StremioStreamKind::Server,
        },
        name: stream.name,
        description: stream.description,
        target: stream.target,
        headers: stream.headers,
        binge_group: stream.binge_group,
        subtitles: stream
            .subtitles
            .into_iter()
            .map(|subtitle| VideoSubtitle {
                url: subtitle.url,
                label: subtitle.label.or_else(|| subtitle.lang.clone()),
                lang: subtitle.lang,
                format: None,
            })
            .collect(),
        filename: stream.filename,
        size: stream.size.map(|size| size as f64),
        video_hash: stream.video_hash,
        not_web_ready: stream.not_web_ready,
    }
}

/// The innermost cause of an error ("operation timed out", "Connection refused"), which says more
/// than reqwest's "error sending request for url".
fn root_cause(error: &(dyn std::error::Error + 'static)) -> String {
    let mut cause = error;
    while let Some(inner) = cause.source() {
        cause = inner;
    }
    cause.to_string()
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod live_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn saved_titles_open_from_storage() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = raygun_db::Db::open(&temp.path().join("raygun.db"))
            .await
            .unwrap();
        let fields = |remote_id: &str| TitleFields {
            kind: video_library::KIND.into(),
            source_id: STREMIO_SOURCE.into(),
            remote_id: remote_id.into(),
            title: "Saved".into(),
            cover: None,
            cover_headers: Default::default(),
            authors: vec!["Actor".into()],
            artists: vec![],
            description: None,
            genres: vec![],
            status: "ongoing".into(),
            url: None,
            media_type: None,
            year: Some(2008),
        };
        let unit = |remote_id: &str, number: Option<f64>, season: Option<&str>| UnitFields {
            remote_id: remote_id.into(),
            name: "Pilot".into(),
            number,
            volume: season.map(str::to_owned),
            scanlator: None,
            date: Some(86_400_000),
        };
        let stored = |remote_id: &'static str, units: Vec<UnitFields>| {
            let db = &db;
            async move {
                let id = titles::upsert_title(db, &fields(remote_id), 1)
                    .await
                    .unwrap();
                titles::sync_units(db, &id, &units, "episode", 1)
                    .await
                    .unwrap();
                let row = titles::title(db, &id).await.unwrap().unwrap();
                stored_meta(db, &row).await.unwrap()
            }
        };

        let series = stored("series:tt1", vec![unit("tt1:1:1", Some(1.0), Some("1"))]).await;
        assert_eq!(
            (series.preview.kind.as_str(), series.preview.id.as_str()),
            ("series", "tt1")
        );
        assert_eq!(series.preview.release_info.as_deref(), Some("2008"));
        assert_eq!(series.cast, ["Actor"]);
        assert_eq!(
            series.videos,
            [protocol::Video {
                id: "tt1:1:1".into(),
                title: "Pilot".into(),
                season: Some(1),
                episode: Some(1),
                released: Some("1970-01-02T00:00:00.000Z".into()),
                thumbnail: None,
                overview: None,
                streams: Vec::new(),
                imdb: None,
            }]
        );
        let movie = stored("movie:tt2", vec![unit("tt2", None, None)]).await;
        assert!(movie.videos.is_empty(), "a movie is its own one unit");
        // A movie found by TMDB's id plays by IMDb's: still a movie.
        let other = stored("movie:tmdb:3", vec![unit("tt3", None, None)]).await;
        assert!(other.videos.is_empty());
        assert_eq!(other.own_video_id(), "tt3");
    }

    #[tokio::test]
    async fn progress_is_found_whatever_the_episodes_are_called() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = raygun_db::Db::open(&temp.path().join("raygun.db"))
            .await
            .unwrap();
        let entry = |key: &str| VideoEntry {
            key: key.into(),
            title: "Show".into(),
            subtitle: None,
            poster: None,
            request: "{}".into(),
        };
        for key in [
            "stremio:series:tt0903747:1:1",
            "stremio:series:tt0903747:2:1",
            "stremio:series:tt09037470:1:1",
            "stremio:movie:tt22084616",
        ] {
            progress_store::mark(&db, &entry(key), true, 5)
                .await
                .unwrap();
        }
        // A TMDB series whose episodes go by IMDb's ids, and a movie by its IMDb id.
        let keys = [
            "stremio:series:tt0903747:1:1",
            "stremio:series:tt0903747:1:2",
            "stremio:series:tt0903747:2:1",
            "stremio:movie:tt22084616",
        ]
        .map(str::to_owned);
        let found = progress_for_keys(&db, &keys).await.unwrap();
        let mut names: Vec<&str> = found.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "stremio:movie:tt22084616",
                "stremio:series:tt0903747:1:1",
                "stremio:series:tt0903747:2:1"
            ],
            "only what was asked for"
        );
    }

    #[test]
    fn what_an_addon_provides() {
        let manifest = protocol::parse_manifest(&serde_json::json!({
            "id": "x", "name": "X", "types": ["movie", "series"],
            "resources": ["catalog", { "name": "meta", "types": ["series"] }, "stream", "subtitles"],
            "catalogs": [
                { "type": "movie", "id": "a" }, { "type": "movie", "id": "b" },
                { "type": "series", "id": "c" }
            ]
        }))
        .unwrap();
        let list: Vec<(String, Option<String>, u32)> = provides(&manifest)
            .into_iter()
            .map(|entry| (entry.resource, entry.kind, entry.count))
            .collect();
        assert_eq!(
            list,
            [
                ("catalog".into(), Some("movie".into()), 2),
                ("catalog".into(), Some("series".into()), 1),
                ("meta".into(), Some("series".into()), 1),
                ("stream".into(), None, 1),
                ("subtitles".into(), None, 1),
            ]
        );
    }

    #[test]
    fn direct_streams_know_their_format() {
        let request = |url: &str| StremioPlayRequest {
            kind: "movie".into(),
            video_id: "tt1".into(),
            title: "T".into(),
            subtitle: None,
            poster: None,
            stream: StremioStream {
                kind: StremioStreamKind::Direct,
                name: None,
                description: None,
                target: url.into(),
                headers: Default::default(),
                subtitles: Vec::new(),
                binge_group: None,
                filename: None,
                size: None,
                video_hash: None,
                not_web_ready: false,
            },
            meta_id: Some("tmdb:1".into()),
            external: false,
        };
        let hls = direct_request(&request("https://cdn.example/master.m3u8?token=1"), &[]);
        assert_eq!(hls.format.as_deref(), Some("hls"));
        assert_eq!(hls.group.as_deref(), Some("stremio:movie:tmdb:1"));
        assert_eq!(
            direct_request(&request("https://cdn.example/a.mpd"), &[])
                .format
                .as_deref(),
            Some("dash")
        );
        assert_eq!(
            direct_request(&request("https://cdn.example/a.mkv"), &[]).format,
            None
        );
    }

    #[test]
    fn addons_that_failed_since_they_worked_are_failing_for_a_while() {
        let failing = Health {
            ok_at: 1_000,
            failed_at: 2_000,
        };
        assert!(failing.failing(5_000));
        assert!(
            !failing.failing(2_000 + FAILING_FOR_MS),
            "forgotten after three days"
        );
        let fixed = Health {
            ok_at: 3_000,
            failed_at: 2_000,
        };
        assert!(!fixed.failing(5_000));
    }

    fn number(season: Option<u32>, episode: u32) -> video_match::EpisodeNumber {
        video_match::EpisodeNumber { season, episode }
    }

    #[test]
    fn episodes_are_numbered_as_streams_may_name_them() {
        let meta = |json: Value| protocol::parse_meta(&json, "series").unwrap();
        // Cinemeta: every season under one title; numbers across seasons are counted.
        let cinemeta = meta(serde_json::json!({ "meta": {
            "id": "tt2560140", "type": "series", "name": "Attack on Titan",
            "videos": [
                { "id": "tt2560140:1:1", "season": 1, "episode": 1 },
                { "id": "tt2560140:1:2", "season": 1, "episode": 2 },
                { "id": "tt2560140:2:1", "season": 2, "episode": 1 },
                { "id": "tt2560140:0:1", "season": 0, "episode": 1 }
            ]
        }}));
        assert_eq!(
            numberings(&cinemeta),
            vec![
                vec![number(Some(1), 1)],
                vec![number(Some(1), 2)],
                vec![number(Some(2), 1), number(None, 3)],
                vec![number(Some(0), 1)],
            ]
        );
        // anime-kitsu: one entry per season, numbered from 1, mapped onto IMDb's.
        let season_two = meta(serde_json::json!({ "meta": {
            "id": "kitsu:8671", "type": "series", "name": "Attack on Titan Season 2",
            "videos": [{ "id": "kitsu:8671:1", "season": 1, "episode": 1, "imdb_id": "tt2560140",
                "imdbSeason": 2, "imdbEpisode": 1, "absoluteEpisode": 26 }]
        }}));
        assert_eq!(
            numberings(&season_two),
            vec![vec![number(Some(2), 1)]],
            "its own numbering and IMDb's agree"
        );
        let part_two = meta(serde_json::json!({ "meta": {
            "id": "kitsu:41982", "type": "series", "name": "Attack on Titan Season 3 Part 2",
            "videos": [{ "id": "kitsu:41982:1", "season": 1, "episode": 1, "imdb_id": "tt2560140",
                "imdbSeason": 3, "imdbEpisode": 13, "absoluteEpisode": 50 }]
        }}));
        assert_eq!(
            numberings(&part_two),
            vec![vec![number(Some(3), 1), number(Some(3), 13)]]
        );
        // A long entry numbered across IMDb's seasons is no one season.
        let long = meta(serde_json::json!({ "meta": {
            "id": "kitsu:12", "type": "series", "name": "One Piece",
            "videos": [{ "id": "kitsu:12:1071", "season": 1, "episode": 1071, "imdb_id": "tt0388629",
                "imdbSeason": 21, "imdbEpisode": 179, "absoluteEpisode": 1071 }]
        }}));
        assert_eq!(
            numberings(&long),
            vec![vec![number(None, 1071), number(Some(21), 179)]]
        );
    }

    #[test]
    fn release_ranges_give_status_and_year() {
        assert_eq!(release_status("movie", Some("1994")), "completed");
        assert_eq!(release_status("series", Some("2008–2013")), "completed");
        assert_eq!(release_status("series", Some("2019–")), "ongoing");
        assert_eq!(release_status("series", Some("2019-")), "ongoing");
        assert_eq!(release_status("series", Some("2024")), "unknown");
        assert_eq!(release_status("series", None), "unknown");
        assert_eq!(release_year("2008–2013"), Some(2008));
        assert_eq!(release_year("n/a"), None);
        assert_eq!(release_ms("1970-01-02T00:00:00.000Z"), Some(86_400_000));
        assert_eq!(release_ms("1970-01-02"), Some(86_400_000));
        assert_eq!(release_ms("soon"), None);
    }
}
