//! Stremio addons in the Watch tab (M41): catalogs, metadata, streams and subtitles from addons
//! speaking Stremio's HTTP protocol (`protocol`), played in Raygun's own player.

pub mod protocol;

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::future::join_all;
use raygun_db::stremio::{self as store, AddonRow};
use raygun_db::video::{self as progress_store, VideoEntry};
use raygun_model::{
    MediaKind, MediaSource, QueueItemInput, StremioAddon, StremioCatalogRow, StremioItem,
    StremioMeta, StremioPlayRequest, StremioStream, StremioStreamGroup, StremioStreamKind,
    StremioSuggestion, StremioVideo, VideoRequest, VideoSubtitle,
};
use serde_json::Value;
use tauri::{AppHandle, Manager};

use self::protocol::{Catalog, Manifest, MetaPreview, StreamSource};
use crate::catalog::now_ms;
use crate::extensions::ExtensionService;
use crate::local_network::{self, LocalNetwork};
use crate::playback::PlaybackService;
use crate::startup::AppDb;

/// Manifests older than this are fetched again.
const MANIFEST_MAX_AGE_MS: i64 = 24 * 60 * 60 * 1000;
/// Longest one addon request may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// Biggest answer read from an addon.
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
/// Most catalogs shown on the Watch home.
const MAX_HOME_ROWS: usize = 24;
/// Most subtitle tracks handed to the player; more makes a menu of near-identical entries.
const MAX_SUBTITLES: usize = 20;

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
struct Loaded {
    row: AddonRow,
    manifest: Result<Manifest, String>,
}

impl Loaded {
    fn manifest(&self) -> Option<&Manifest> {
        self.manifest.as_ref().ok()
    }
}

pub struct StremioService {
    app: AppHandle,
    client: reqwest::Client,
}

impl StremioService {
    pub fn new(app: AppHandle, client: reqwest::Client) -> Self {
        Self { app, client }
    }

    // ---- Addons ----

    /// Installed addons with their manifests, refreshing stale ones. Adult addons are left out
    /// while NSFW content is hidden.
    async fn loaded(&self) -> Result<Vec<Loaded>, String> {
        let db = self.db();
        let rows = store::addons(&db).await.map_err(to_message)?;
        let now = now_ms();
        let loads = rows.into_iter().map(|row| async move {
            let cached = row
                .manifest
                .as_deref()
                .and_then(|text| serde_json::from_str::<Value>(text).ok())
                .and_then(|value| protocol::parse_manifest(&value).ok());
            let fresh = row
                .fetched_at
                .is_some_and(|at| now - at < MANIFEST_MAX_AGE_MS);
            if let (Some(manifest), true) = (&cached, fresh) {
                return Loaded {
                    manifest: Ok(manifest.clone()),
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
                // A cached manifest still works while the addon is unreachable.
                Err(error) => Loaded {
                    manifest: cached.ok_or(error),
                    row,
                },
            }
        });
        let show_nsfw = self.app.state::<Arc<ExtensionService>>().show_nsfw();
        Ok(join_all(loads)
            .await
            .into_iter()
            .filter(|loaded| show_nsfw || !loaded.manifest().is_some_and(|manifest| manifest.adult))
            .collect())
    }

    pub async fn addons(&self) -> Result<Vec<StremioAddon>, String> {
        Ok(self
            .loaded()
            .await?
            .into_iter()
            .map(|loaded| match loaded.manifest {
                Ok(manifest) => StremioAddon {
                    id: loaded.row.id,
                    url: loaded.row.transport_url,
                    name: manifest.name,
                    description: manifest.description,
                    logo: manifest.logo,
                    version: manifest.version,
                    resources: manifest
                        .resources
                        .iter()
                        .map(|resource| resource.name.clone())
                        .collect(),
                    types: manifest.types,
                    catalogs: manifest.catalogs.len() as u32,
                    adult: manifest.adult,
                    error: None,
                },
                Err(error) => StremioAddon {
                    name: loaded.row.transport_url.clone(),
                    id: loaded.row.id,
                    url: loaded.row.transport_url,
                    description: None,
                    logo: None,
                    version: String::new(),
                    resources: Vec::new(),
                    types: Vec::new(),
                    catalogs: 0,
                    adult: false,
                    error: Some(error),
                },
            })
            .collect())
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

    /// Adds an addon by its address, after reading its manifest.
    pub async fn add(&self, url: &str) -> Result<StremioAddon, String> {
        let url = protocol::normalize_addon_url(url)?;
        let (manifest, text) = self.fetch_manifest(&url).await?;
        if manifest.configuration_required {
            return Err(format!(
                "{} needs to be set up on its website first; add the address it gives you then",
                manifest.name
            ));
        }
        let id = store::add(&self.db(), &url, &text, now_ms())
            .await
            .map_err(to_message)?;
        self.addons()
            .await?
            .into_iter()
            .find(|addon| addon.id == id)
            .ok_or_else(|| "The addon couldn't be added".to_owned())
    }

    pub async fn remove(&self, id: &str) -> Result<(), String> {
        store::remove(&self.db(), id).await.map_err(to_message)
    }

    async fn fetch_manifest(&self, url: &str) -> Result<(Manifest, String), String> {
        let value = self.get_json(url).await?;
        let manifest = protocol::parse_manifest(&value)?;
        Ok((manifest, value.to_string()))
    }

    // ---- Catalogs ----

    /// The first page of every catalog that needs nothing to list, catalogs in parallel.
    pub async fn home(&self) -> Result<Vec<StremioCatalogRow>, String> {
        let loaded = self.loaded().await?;
        let catalogs: Vec<(&Loaded, &Manifest, &Catalog)> = loaded
            .iter()
            .filter_map(|addon| Some((addon, addon.manifest()?)))
            .flat_map(|(addon, manifest)| {
                manifest
                    .catalogs
                    .iter()
                    .filter(|catalog| catalog.browsable())
                    .map(move |catalog| (addon, manifest, catalog))
            })
            .take(MAX_HOME_ROWS)
            .collect();
        let rows = catalogs
            .into_iter()
            .map(|(addon, manifest, catalog)| self.row(addon, manifest, catalog, Vec::new()));
        Ok(join_all(rows).await)
    }

    /// One catalog's page: optionally a genre, starting `skip` items in.
    pub async fn catalog(
        &self,
        addon_id: &str,
        kind: &str,
        catalog_id: &str,
        genre: Option<&str>,
        skip: u32,
    ) -> Result<StremioCatalogRow, String> {
        let loaded = self.loaded().await?;
        let addon = loaded
            .iter()
            .find(|addon| addon.row.id == addon_id)
            .ok_or_else(|| "This addon was removed".to_owned())?;
        let manifest = addon.manifest.as_ref().map_err(Clone::clone)?;
        let catalog = manifest
            .catalogs
            .iter()
            .find(|catalog| catalog.kind == kind && catalog.id == catalog_id)
            .ok_or_else(|| "The addon no longer has this catalog".to_owned())?;
        let mut extras = Vec::new();
        if let Some(genre) = genre.filter(|genre| !genre.is_empty()) {
            extras.push(("genre", genre.to_owned()));
        }
        if skip > 0 {
            extras.push(("skip", skip.to_string()));
        }
        let row = self.row(addon, manifest, catalog, extras).await;
        match row.error {
            Some(error) => Err(error),
            None => Ok(row),
        }
    }

    /// Every catalog that searches, asked for `query` in parallel; rows without results are left
    /// out.
    pub async fn search(&self, query: &str) -> Result<Vec<StremioCatalogRow>, String> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let loaded = self.loaded().await?;
        let rows = loaded
            .iter()
            .filter_map(|addon| Some((addon, addon.manifest()?)))
            .flat_map(|(addon, manifest)| {
                manifest
                    .catalogs
                    .iter()
                    .filter(|catalog| catalog.searchable() && catalog.search_only_ok())
                    .map(move |catalog| {
                        self.row(addon, manifest, catalog, vec![("search", query.to_owned())])
                    })
            });
        Ok(join_all(rows)
            .await
            .into_iter()
            .filter(|row| !row.items.is_empty() || row.error.is_some())
            .collect())
    }

    async fn row(
        &self,
        addon: &Loaded,
        manifest: &Manifest,
        catalog: &Catalog,
        extras: Vec<(&str, String)>,
    ) -> StremioCatalogRow {
        let url = protocol::resource_url(
            &addon.row.transport_url,
            "catalog",
            &catalog.kind,
            &catalog.id,
            &extras,
        );
        let (items, error) = match self.get_json(&url).await {
            Ok(value) => (protocol::parse_metas(&value, &catalog.kind), None),
            Err(error) => (Vec::new(), Some(error)),
        };
        StremioCatalogRow {
            addon_id: addon.row.id.clone(),
            addon_name: manifest.name.clone(),
            kind: catalog.kind.clone(),
            catalog_id: catalog.id.clone(),
            name: catalog.name.clone(),
            has_more: catalog.extra("skip").is_some() && !items.is_empty(),
            genres: catalog
                .extra("genre")
                .map(|extra| extra.options.clone())
                .unwrap_or_default(),
            items: items.into_iter().map(item).collect(),
            error,
        }
    }

    // ---- Titles ----

    /// A title's details from the first addon that has them, with watching progress.
    pub async fn meta(&self, kind: &str, id: &str) -> Result<StremioMeta, String> {
        let loaded = self.loaded().await?;
        let mut last_error = None;
        for addon in &loaded {
            let Some(manifest) = addon.manifest() else {
                continue;
            };
            if !manifest.serves("meta", kind, id) {
                continue;
            }
            let url = protocol::resource_url(&addon.row.transport_url, "meta", kind, id, &[]);
            match self.get_json(&url).await {
                Ok(value) => {
                    if let Some(meta) = protocol::parse_meta(&value, kind) {
                        return self.meta_view(meta, &manifest.name).await;
                    }
                }
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| "No addon has details for this title".to_owned()))
    }

    async fn meta_view(
        &self,
        meta: protocol::Meta,
        addon_name: &str,
    ) -> Result<StremioMeta, String> {
        let db = self.db();
        let kind = meta.preview.kind.clone();
        let own_key = content_key(&kind, &meta.preview.id);
        let mut progress = progress_store::progress_with_prefix(&db, &format!("{own_key}:"))
            .await
            .map_err(to_message)?;
        if let Some(own) = progress_store::progress(&db, &own_key)
            .await
            .map_err(to_message)?
        {
            progress.push(own);
        }
        let find = |key: &str| progress.iter().find(|row| row.entry.key == key);
        let own = find(&own_key);
        Ok(StremioMeta {
            videos: meta
                .videos
                .into_iter()
                .map(|video| {
                    let saved = find(&content_key(&kind, &video.id));
                    StremioVideo {
                        position_ms: saved.map_or(0.0, |row| row.position_ms as f64),
                        duration_ms: saved.and_then(|row| row.duration_ms).map(|ms| ms as f64),
                        watched: saved.is_some_and(|row| row.watched_at.is_some()),
                        id: video.id,
                        title: video.title,
                        season: video.season,
                        episode: video.episode,
                        released: video.released,
                        thumbnail: video.thumbnail,
                        overview: video.overview,
                    }
                })
                .collect(),
            position_ms: own.map_or(0.0, |row| row.position_ms as f64),
            duration_ms: own.and_then(|row| row.duration_ms).map(|ms| ms as f64),
            watched: own.is_some_and(|row| row.watched_at.is_some()),
            kind,
            id: meta.preview.id,
            name: meta.preview.name,
            poster: meta.preview.poster,
            background: meta.background,
            logo: meta.logo,
            description: meta.description,
            genres: meta.genres,
            release_info: meta.preview.release_info,
            runtime: meta.runtime,
            rating: meta.preview.rating,
            cast: meta.cast,
            director: meta.director,
            trailer_yt_ids: meta.trailer_yt_ids,
            addon_name: addon_name.to_owned(),
        })
    }

    /// Streams for a movie or episode from every addon that has them, in parallel.
    pub async fn streams(
        &self,
        kind: &str,
        video_id: &str,
    ) -> Result<Vec<StremioStreamGroup>, String> {
        let loaded = self.loaded().await?;
        let asks = loaded
            .iter()
            .filter_map(|addon| Some((addon, addon.manifest()?)))
            .filter(|(_, manifest)| manifest.serves("stream", kind, video_id))
            .map(|(addon, manifest)| async move {
                let url =
                    protocol::resource_url(&addon.row.transport_url, "stream", kind, video_id, &[]);
                let (streams, error) = match self.get_json(&url).await {
                    Ok(value) => (protocol::parse_streams(&value), None),
                    Err(error) => (Vec::new(), Some(error)),
                };
                StremioStreamGroup {
                    addon_id: addon.row.id.clone(),
                    addon_name: manifest.name.clone(),
                    streams: streams.into_iter().map(stream_view).collect(),
                    error,
                }
            });
        Ok(join_all(asks).await)
    }

    /// Subtitles from every subtitle addon, the preferred language first.
    async fn subtitles(
        &self,
        kind: &str,
        video_id: &str,
        language: &str,
    ) -> Vec<protocol::Subtitle> {
        let Ok(loaded) = self.loaded().await else {
            return Vec::new();
        };
        let asks = loaded
            .iter()
            .filter_map(|addon| Some((addon, addon.manifest()?)))
            .filter(|(_, manifest)| manifest.serves("subtitles", kind, video_id))
            .map(|(addon, _)| async move {
                let url = protocol::resource_url(
                    &addon.row.transport_url,
                    "subtitles",
                    kind,
                    video_id,
                    &[],
                );
                self.get_json(&url)
                    .await
                    .map(|value| protocol::parse_subtitles(&value))
                    .unwrap_or_default()
            });
        let all = join_all(asks).await.into_iter().flatten().collect();
        protocol::order_subtitles(all, language, MAX_SUBTITLES)
    }

    /// Plays a stream: direct ones in Raygun's player (with subtitles), YouTube ones in the
    /// embedded player.
    pub async fn play(&self, request: StremioPlayRequest, language: &str) -> Result<(), String> {
        let stream = &request.stream;
        match stream.kind {
            StremioStreamKind::Direct => {
                self.app
                    .state::<LocalNetwork>()
                    .check(&stream.target, local_network::VIDEOS_DENIED)
                    .await?;
                let mut subtitles: Vec<VideoSubtitle> = stream.subtitles.clone();
                let found = self
                    .subtitles(&request.kind, &request.video_id, language)
                    .await;
                subtitles.extend(found.into_iter().map(|subtitle| VideoSubtitle {
                    url: subtitle.url,
                    label: subtitle.lang.clone(),
                    lang: subtitle.lang,
                    format: None,
                }));
                subtitles.truncate(MAX_SUBTITLES);
                let lower = stream.target.to_lowercase();
                let format = if lower.contains(".m3u8") {
                    Some("hls".to_owned())
                } else if lower.contains(".mpd") {
                    Some("dash".to_owned())
                } else {
                    None
                };
                let video = VideoRequest {
                    key: content_key(&request.kind, &request.video_id),
                    title: request.title.clone(),
                    subtitle: request.subtitle.clone(),
                    poster: request.poster.clone(),
                    url: stream.target.clone(),
                    headers: stream.headers.clone(),
                    format,
                    subtitles,
                };
                crate::video::play(&self.app, video).await
            }
            StremioStreamKind::YouTube => {
                self.app.state::<PlaybackService>().replace(
                    vec![QueueItemInput {
                        title: request.title.clone(),
                        artist: request.subtitle.clone(),
                        album: None,
                        duration_ms: None,
                        artwork_url: request.poster.clone(),
                        media_kind: MediaKind::Video,
                        source: MediaSource::YouTube {
                            video_id: stream.target.clone(),
                        },
                    }],
                    Some(0),
                    false,
                );
                Ok(())
            }
            StremioStreamKind::External => Err("This stream opens outside Raygun".into()),
            StremioStreamKind::Torrent => Err("Torrent streams aren't supported yet".into()),
        }
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
        progress_store::mark(&self.db(), &entry, watched, now_ms())
            .await
            .map_err(to_message)
    }

    // ---- HTTP ----

    async fn get_json(&self, url: &str) -> Result<Value, String> {
        // Without Android 17's local network permission, an addon on the LAN would time out.
        self.app
            .state::<LocalNetwork>()
            .check(url, local_network::ADDONS_DENIED)
            .await?;
        let response = self
            .client
            .get(url)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|error| format!("Couldn't reach the addon ({})", root_cause(&error)))?;
        if !response.status().is_success() {
            return Err(format!("The addon answered {}", response.status()));
        }
        let mut bytes = Vec::new();
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|error| format!("The addon's answer broke off: {error}"))?;
            if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err("The addon's answer is too large".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| "The addon's answer couldn't be read".to_owned())
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().0.clone()
    }
}

/// The key watching progress is kept under for a movie or episode.
pub fn content_key(kind: &str, video_id: &str) -> String {
    format!("stremio:{kind}:{video_id}")
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
        },
        name: stream.name,
        description: stream.description,
        target: stream.target,
        headers: stream.headers,
        subtitles: stream
            .subtitles
            .into_iter()
            .map(|subtitle| VideoSubtitle {
                url: subtitle.url,
                label: subtitle.lang.clone(),
                lang: subtitle.lang,
                format: None,
            })
            .collect(),
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
