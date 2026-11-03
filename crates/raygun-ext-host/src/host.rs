//! The extension host: loads extensions and routes provider calls to them.
//!
//! All extensions live on one dedicated thread running a current-thread tokio runtime and a
//! `LocalSet`. Requests arrive over a channel and each runs as a local task, so one extension
//! waiting on the network doesn't block another. Calls into the *same* extension are serialized
//! by its runtime lock (a provider can still run its own requests in parallel with `Promise.all`).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use raygun_model::{
    ExtensionInfo, ExtensionOrigin, ExtensionSetting, ExtensionStatus, LyricLine, Lyrics,
    MediaSource, MusicAlbum, MusicAlbumDetail, MusicArtist, MusicChart, MusicPlaylist,
    MusicPlaylistDetail, OnlineTrack, ProviderInfo, ProviderSearchResult, SettingKind,
    SettingValue, TrackQuery,
};
use raygun_resolver::Candidate;
use serde::Deserialize;
use tokio::sync::{mpsc, oneshot};

use crate::content::ContentFilter;
use crate::cookies::CookieJar;
use crate::js::{JsExtension, Limits};
use crate::manifest::{self, MANIFEST_FILE, Manifest, Runtime};
use crate::shosetsu::ShosetsuExtension;
use crate::storage::KvStore;

pub const MUSIC_METADATA: &str = "music.metadata";
pub const MUSIC_STREAM: &str = "music.stream";
pub const LYRICS: &str = "lyrics";
pub const MUSIC_CHARTS: &str = "music.charts";
pub const MUSIC_TAGS: &str = "music.tags";
/// Artist facts and song credits (M46.14).
pub const MUSIC_INFO: &str = "music.info";
/// Similar artists, top songs, new releases and radios (M46.20).
pub const MUSIC_RECOMMEND: &str = "music.recommend";
/// How long a search or a stream lookup waits for one source (M46.17); background work (charts,
/// tags, downloads) keeps the longer call limit.
pub const INTERACTIVE_TIMEOUT: Duration = Duration::from_secs(12);

/// Longest tag name kept from a provider.
const MAX_TAG_CHARS: usize = 60;

/// One `music.stream` provider's answer: its candidates, or why it failed.
#[derive(Debug, Clone)]
pub struct ProviderCandidates {
    pub provider: ProviderInfo,
    pub result: Result<Vec<Candidate>, String>,
}

/// A stream a provider resolved a candidate to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStream {
    pub source: MediaSource,
    /// Unix milliseconds after which the URL stops working (signed URLs), if the provider knows.
    pub expires_at: Option<i64>,
}

/// The user's setting values, per extension id, then per setting key.
pub type UserSettings = std::collections::HashMap<String, BTreeMap<String, serde_json::Value>>;

#[derive(Debug, Clone)]
pub struct HostConfig {
    /// Where per-extension storage lives (`extensions/.data`); `None` keeps storage in memory.
    pub storage_dir: Option<PathBuf>,
    pub limits: Limits,
    /// Longest a provider call may take, waiting included.
    pub call_timeout: Duration,
    /// The NSFW switch (M35), shared with the app.
    pub content: ContentFilter,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            storage_dir: None,
            limits: Limits::default(),
            call_timeout: Duration::from_secs(30),
            content: ContentFilter::default(),
        }
    }
}

/// An extension's files: compiled in, or a folder under `extensions/`.
#[derive(Debug, Clone)]
pub enum ExtensionFiles {
    Memory(BTreeMap<String, String>),
    Dir(PathBuf),
}

#[derive(Debug, Clone)]
pub struct ExtensionSource {
    pub origin: ExtensionOrigin,
    pub files: ExtensionFiles,
}

impl ExtensionSource {
    pub fn built_in(files: &[(&str, &str)]) -> Self {
        Self {
            origin: ExtensionOrigin::BuiltIn,
            files: ExtensionFiles::Memory(
                files
                    .iter()
                    .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
                    .collect(),
            ),
        }
    }

    pub fn installed(dir: PathBuf) -> Self {
        Self {
            origin: ExtensionOrigin::Installed,
            files: ExtensionFiles::Dir(dir),
        }
    }

    pub(crate) fn read(&self, path: &str) -> Result<String, String> {
        match &self.files {
            ExtensionFiles::Memory(files) => files
                .get(path)
                .cloned()
                .ok_or_else(|| format!("{path} is missing")),
            ExtensionFiles::Dir(dir) => std::fs::read_to_string(dir.join(path))
                .map_err(|error| format!("could not read {path}: {error}")),
        }
    }

    /// A name for errors when the manifest can't be read.
    fn label(&self) -> String {
        match &self.files {
            ExtensionFiles::Memory(_) => "built-in extension".to_owned(),
            ExtensionFiles::Dir(dir) => dir
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| dir.display().to_string()),
        }
    }
}

/// Every folder in `dir` holding a manifest. Folders starting with `.` (such as `.data`) are not
/// extensions.
pub fn discover_installed(dir: &Path) -> Vec<ExtensionSource> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .map(|entry| entry.path())
        .filter(|path| path.join(MANIFEST_FILE).is_file())
        .collect();
    dirs.sort();
    dirs.into_iter().map(ExtensionSource::installed).collect()
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum CallError {
    #[error("no provider {0}")]
    UnknownProvider(String),
    #[error("{0} did not answer in time")]
    Timeout(String),
    #[error("{0}")]
    Script(String),
    #[error("the extension host has stopped")]
    HostStopped,
}

enum Request {
    Load {
        sources: Vec<ExtensionSource>,
        settings: UserSettings,
        reply: oneshot::Sender<Vec<ExtensionInfo>>,
    },
    List {
        reply: oneshot::Sender<Vec<ExtensionInfo>>,
    },
    Call {
        provider: String,
        method: String,
        args: serde_json::Value,
        reply: oneshot::Sender<Result<serde_json::Value, CallError>>,
    },
}

/// Handle to the host thread; cheap to clone and usable from any thread or runtime.
#[derive(Clone)]
pub struct ExtensionHost {
    requests: mpsc::UnboundedSender<Request>,
    content: ContentFilter,
}

impl ExtensionHost {
    pub fn start(config: HostConfig) -> std::io::Result<Self> {
        let (requests, receiver) = mpsc::unbounded_channel();
        let content = config.content.clone();
        std::thread::Builder::new()
            .name("raygun-ext".to_owned())
            .spawn(move || run(config, receiver))?;
        Ok(Self { requests, content })
    }

    /// The NSFW switch: off hides providers of `nsfw`-rated extensions and tells extensions to
    /// filter their results.
    pub fn content(&self) -> &ContentFilter {
        &self.content
    }

    /// Replaces every loaded extension with `sources`. An installed extension overrides a
    /// built-in one with the same id. Returns what loaded (or failed), sorted by name.
    pub async fn load(&self, sources: Vec<ExtensionSource>) -> Vec<ExtensionInfo> {
        self.load_with_settings(sources, UserSettings::new()).await
    }

    /// Like [`Self::load`], with the user's setting values (invalid ones are ignored).
    pub async fn load_with_settings(
        &self,
        sources: Vec<ExtensionSource>,
        settings: UserSettings,
    ) -> Vec<ExtensionInfo> {
        self.ask(|reply| Request::Load {
            sources,
            settings,
            reply,
        })
        .await
        .unwrap_or_default()
    }

    pub async fn list(&self) -> Vec<ExtensionInfo> {
        self.ask(|reply| Request::List { reply })
            .await
            .unwrap_or_default()
    }

    /// Providers of `kind` that searches and lookups use: not the disabled ones, and those of
    /// NSFW-rated extensions only while NSFW content is shown.
    pub async fn providers(&self, kind: &str) -> Vec<ProviderInfo> {
        self.listed_providers(kind)
            .await
            .into_iter()
            .filter(|provider| !self.content.is_disabled(&provider.id))
            .collect()
    }

    /// Providers of `kind` to list, disabled ones included (the user turns them back on there,
    /// M46.8); NSFW-rated extensions' only while NSFW content is shown (music: while explicit
    /// music is, M46.36).
    pub async fn listed_providers(&self, kind: &str) -> Vec<ProviderInfo> {
        let allowed = self.content.allows("nsfw", kind);
        self.list()
            .await
            .into_iter()
            .filter(|extension| allowed || extension.content_rating != "nsfw")
            .flat_map(|extension| extension.providers)
            .filter(|provider| provider.kind == kind)
            .collect()
    }

    /// A provider's songs, without explicit ones while Explicit music is off (M46.36).
    fn tracks(&self, value: serde_json::Value) -> Vec<OnlineTrack> {
        let mut tracks = parse_tracks(value);
        if !self.content.show_explicit_music() {
            tracks.retain(|track| !track.explicit);
        }
        tracks
    }

    /// Calls `method` on a provider (its full `<extension>/<provider>` id), JSON in and out.
    /// A call someone is waiting on (M46.17): a search or a stream lookup gives up after
    /// [`INTERACTIVE_TIMEOUT`], so one source that never answers doesn't hold up the others.
    pub async fn call_interactive(
        &self,
        provider: &str,
        method: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, CallError> {
        tokio::time::timeout(INTERACTIVE_TIMEOUT, self.call(provider, method, args))
            .await
            .unwrap_or_else(|_| Err(CallError::Timeout(provider.to_owned())))
    }

    pub async fn call(
        &self,
        provider: &str,
        method: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, CallError> {
        let (provider, method) = (provider.to_owned(), method.to_owned());
        self.ask(|reply| Request::Call {
            provider,
            method,
            args,
            reply,
        })
        .await
        .unwrap_or(Err(CallError::HostStopped))
    }

    /// Asks every `music.metadata` provider at once; each answers with tracks or an error.
    pub async fn search_music(&self, query: &str, limit: u32) -> Vec<ProviderSearchResult> {
        let providers = self.providers(MUSIC_METADATA).await;
        let args = serde_json::json!({ "query": query, "limit": limit });
        let searches = providers.into_iter().map(|provider| {
            let args = args.clone();
            async move {
                let started = std::time::Instant::now();
                match self.call_interactive(&provider.id, "search", args).await {
                    Ok(value) => {
                        let tracks = self.tracks(value);
                        tracing::info!(
                            provider = %provider.id,
                            tracks = tracks.len(),
                            elapsed_ms = started.elapsed().as_millis(),
                            "search answered"
                        );
                        ProviderSearchResult {
                            tracks,
                            provider,
                            error: None,
                        }
                    }
                    Err(error) => {
                        tracing::warn!(provider = %provider.id, %error, "search failed");
                        ProviderSearchResult {
                            provider,
                            tracks: Vec::new(),
                            error: Some(error.to_string()),
                        }
                    }
                }
            }
        });
        futures_util::future::join_all(searches).await
    }

    /// Asks every `music.stream` provider at once for candidates for `query`.
    pub async fn stream_candidates(
        &self,
        query: &TrackQuery,
        limit: u32,
        quality: raygun_resolver::QualityTarget,
    ) -> Vec<ProviderCandidates> {
        let providers = self.providers(MUSIC_STREAM).await;
        let mut args = serde_json::json!({
            "title": query.title,
            "artist": query.artist,
            "album": query.album,
            "durationMs": query.duration_ms,
            "limit": limit,
        });
        // M46.18: providers with several streams per song pick the one nearest this; `null`
        // asks for the best there is.
        if quality != raygun_resolver::QualityTarget::Any {
            args["targetKbps"] = serde_json::json!(quality.kbps());
        }
        let requests = providers.into_iter().map(|provider| {
            let args = args.clone();
            async move {
                let result = self
                    .call_interactive(&provider.id, "candidates", args)
                    .await
                    .map(|value| parse_candidates(&provider, value))
                    .map_err(|error| error.to_string());
                ProviderCandidates { provider, result }
            }
        });
        futures_util::future::join_all(requests).await
    }

    /// Lyrics for `query` from every `lyrics` provider at once: synced lyrics beat plain ones,
    /// then providers keep their order. `Ok(None)` when no provider has any; an error only when
    /// every provider failed.
    pub async fn lyrics(&self, query: &TrackQuery) -> Result<Option<Lyrics>, String> {
        Ok(self.all_lyrics(query).await?.into_iter().next())
    }

    /// Every lyrics provider's lyrics for a song (M46.36: the user picks one), synced ones first.
    pub async fn all_lyrics(&self, query: &TrackQuery) -> Result<Vec<Lyrics>, String> {
        let providers = self.providers(LYRICS).await;
        if providers.is_empty() {
            return Ok(Vec::new());
        }
        let args = serde_json::json!({
            "title": query.title,
            "artist": query.artist,
            "album": query.album,
            "durationMs": query.duration_ms,
        });
        let requests = providers.into_iter().map(|provider| {
            let args = args.clone();
            async move {
                let answer = self.call(&provider.id, "lyrics", args).await;
                (provider, answer)
            }
        });
        let answers = futures_util::future::join_all(requests).await;
        let mut found: Vec<Lyrics> = Vec::new();
        let mut errors = Vec::new();
        for (provider, answer) in answers {
            match answer {
                Ok(value) => found.extend(parse_lyrics(&provider, &value)),
                Err(error) => {
                    tracing::warn!(provider = %provider.id, %error, "lyrics provider failed");
                    errors.push(error.to_string());
                }
            }
        }
        if found.is_empty() && !errors.is_empty() {
            return Err(errors.remove(0));
        }
        // Stable sort keeps provider order among equals.
        found.sort_by_key(|lyrics| lyrics.synced.is_empty());
        Ok(found)
    }

    /// Every `music.charts` provider's charts at once (M46.9); each answers with charts or an
    /// error. Charts' `tag`s are as the provider wrote them.
    pub async fn music_charts(
        &self,
        region: Option<&str>,
    ) -> Vec<(ProviderInfo, Result<Vec<MusicChart>, String>)> {
        let providers = self.providers(MUSIC_CHARTS).await;
        let args = serde_json::json!({ "region": region });
        let requests = providers.into_iter().map(|provider| {
            let args = args.clone();
            async move {
                let answer = self
                    .call(&provider.id, "charts", args)
                    .await
                    .map(|value| parse_charts(&provider, &value))
                    .map_err(|error| error.to_string());
                (provider, answer)
            }
        });
        futures_util::future::join_all(requests).await
    }

    /// Artists named like `query` from every `music.metadata` provider that searches artists
    /// (M46.9); providers without the call are left out.
    pub async fn search_artists(&self, query: &str, limit: u32) -> Vec<MusicArtist> {
        let answers = self.ask_metadata("searchArtists", query, limit).await;
        answers
            .into_iter()
            .flat_map(|(provider, value)| {
                value
                    .get("artists")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(move |artist| {
                        let name = artist.get("name")?.as_str()?.trim().to_owned();
                        let id = json_id(artist.get("id"))?;
                        (!name.is_empty()).then(|| MusicArtist {
                            provider: provider.clone(),
                            id,
                            name,
                            image_url: web_url(artist.get("imageUrl")),
                        })
                    })
            })
            .collect()
    }

    /// Albums for `query` from every `music.metadata` provider that searches albums (M46.9).
    pub async fn search_albums(&self, query: &str, limit: u32) -> Vec<MusicAlbum> {
        let answers = self.ask_metadata("searchAlbums", query, limit).await;
        answers
            .into_iter()
            .flat_map(|(provider, value)| {
                value
                    .get("albums")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(move |album| parse_album(&provider, &album))
            })
            .collect()
    }

    /// An artist's releases (M46.36: albums, singles, EPs...) from every `music.metadata` provider
    /// that lists them (`artistReleases`, asked with the artist's name as `query`).
    pub async fn artist_releases(&self, name: &str, limit: u32) -> Vec<MusicAlbum> {
        let answers = self.ask_metadata("artistReleases", name, limit).await;
        answers
            .into_iter()
            .flat_map(|(provider, value)| {
                value
                    .get("releases")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(move |album| parse_album(&provider, &album))
            })
            .collect()
    }

    /// Playlists for `query` from every `music.metadata` provider that searches them (M46.39);
    /// `featured` asks for the services' own playlists only.
    pub async fn search_playlists(
        &self,
        query: &str,
        featured: bool,
        limit: u32,
    ) -> Vec<MusicPlaylist> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let providers = self.providers(MUSIC_METADATA).await;
        let args = serde_json::json!({ "query": query, "limit": limit, "featured": featured });
        let requests = providers.into_iter().map(|provider| {
            let args = args.clone();
            async move {
                match self
                    .call_interactive(&provider.id, "searchPlaylists", args)
                    .await
                {
                    Ok(value) => Some((provider, value)),
                    Err(error) => {
                        log_unless_missing(&provider, "searchPlaylists", &error);
                        None
                    }
                }
            }
        });
        futures_util::future::join_all(requests)
            .await
            .into_iter()
            .flatten()
            .flat_map(|(provider, value)| {
                value
                    .get("playlists")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(move |playlist| parse_playlist(&provider, &playlist))
            })
            .filter(|playlist| !featured || playlist.featured)
            .collect()
    }

    /// A playlist's songs from the provider that has it (M46.39).
    pub async fn music_playlist(
        &self,
        provider_id: &str,
        playlist_id: &str,
    ) -> Result<MusicPlaylistDetail, CallError> {
        let provider = self
            .providers(MUSIC_METADATA)
            .await
            .into_iter()
            .find(|provider| provider.id == provider_id)
            .ok_or_else(|| CallError::Script("That source isn't available".to_owned()))?;
        let value = self
            .call(
                provider_id,
                "playlist",
                serde_json::json!({ "id": playlist_id }),
            )
            .await?;
        let playlist = value
            .get("playlist")
            .and_then(|playlist| parse_playlist(&provider, playlist))
            .ok_or_else(|| CallError::Script("The source gave no playlist".to_owned()))?;
        Ok(MusicPlaylistDetail {
            playlist,
            tracks: self.tracks(value),
        })
    }

    /// An album's songs from the provider that knows it (M46.9).
    pub async fn music_album(
        &self,
        provider_id: &str,
        album_id: &str,
    ) -> Result<MusicAlbumDetail, CallError> {
        let provider = self
            .providers(MUSIC_METADATA)
            .await
            .into_iter()
            .find(|provider| provider.id == provider_id)
            .ok_or_else(|| CallError::Script("That source isn't available".to_owned()))?;
        let value = self
            .call(provider_id, "album", serde_json::json!({ "id": album_id }))
            .await?;
        let album = value
            .get("album")
            .and_then(|album| parse_album(&provider, album))
            .ok_or_else(|| CallError::Script("The source gave no album".to_owned()))?;
        Ok(MusicAlbumDetail {
            album,
            tracks: self.tracks(value),
        })
    }

    /// Asks every `music.metadata` provider that has `method` at once; answers from providers
    /// without it, or that failed, are dropped (failures are logged).
    async fn ask_metadata(
        &self,
        method: &str,
        query: &str,
        limit: u32,
    ) -> Vec<(ProviderInfo, serde_json::Value)> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let providers = self.providers(MUSIC_METADATA).await;
        let args = serde_json::json!({ "query": query, "limit": limit });
        let requests = providers.into_iter().map(|provider| {
            let args = args.clone();
            async move {
                match self.call_interactive(&provider.id, method, args).await {
                    Ok(value) => Some((provider, value)),
                    Err(error) => {
                        let message = error.to_string();
                        if !message.contains("has no method") {
                            tracing::warn!(provider = %provider.id, %message, "{method} failed");
                        }
                        None
                    }
                }
            }
        });
        futures_util::future::join_all(requests)
            .await
            .into_iter()
            .flatten()
            .collect()
    }

    /// Songs for a genre or a mood from every `music.charts` provider that has `tagSongs`
    /// (M46.19), at once. Providers without the call are left out; each other answers with
    /// `(row name, songs)` or an error. `aliases` are the vocabulary's other spellings.
    pub async fn music_tag_songs(
        &self,
        tag: &str,
        aliases: &[String],
        kind: &str,
        region: Option<&str>,
        limit: u32,
    ) -> Vec<(
        ProviderInfo,
        Result<(Option<String>, Vec<OnlineTrack>), String>,
    )> {
        let providers = self.providers(MUSIC_CHARTS).await;
        let args = serde_json::json!({
            "tag": tag,
            "aliases": aliases,
            "kind": kind,
            "region": region,
            "limit": limit,
        });
        let requests = providers.into_iter().map(|provider| {
            let args = args.clone();
            async move {
                match self.call_interactive(&provider.id, "tagSongs", args).await {
                    Ok(value) => {
                        let name = value
                            .get("name")
                            .and_then(serde_json::Value::as_str)
                            .map(|name| name.trim().to_owned())
                            .filter(|name| !name.is_empty());
                        Some((provider, Ok((name, self.tracks(value)))))
                    }
                    Err(error) => {
                        let message = error.to_string();
                        (!message.contains("has no method")).then_some((provider, Err(message)))
                    }
                }
            }
        });
        futures_util::future::join_all(requests)
            .await
            .into_iter()
            .flatten()
            .collect()
    }

    /// Artists like `artist` from the first `music.recommend` provider that knows some (M46.20),
    /// the closest first.
    pub async fn similar_artists(&self, artist: &str, limit: u32) -> Vec<String> {
        let args = serde_json::json!({ "artist": artist, "limit": limit });
        for provider in self.providers(MUSIC_RECOMMEND).await {
            match self
                .call(&provider.id, "similarArtists", args.clone())
                .await
            {
                Ok(value) => {
                    let names: Vec<String> = value
                        .get("artists")
                        .and_then(serde_json::Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|artist| artist.get("name")?.as_str())
                        .map(|name| name.trim().to_owned())
                        .filter(|name| !name.is_empty())
                        .collect();
                    if !names.is_empty() {
                        return names;
                    }
                }
                Err(error) => log_unless_missing(&provider, "similarArtists", &error),
            }
        }
        Vec::new()
    }

    /// Songs from the first `music.recommend` provider whose `method` (`topTracks`,
    /// `newReleases` or `radio`) has some for `args` (M46.20).
    pub async fn recommended_tracks(
        &self,
        method: &str,
        args: serde_json::Value,
    ) -> Vec<OnlineTrack> {
        for provider in self.providers(MUSIC_RECOMMEND).await {
            match self.call(&provider.id, method, args.clone()).await {
                Ok(value) => {
                    let tracks = self.tracks(value);
                    if !tracks.is_empty() {
                        return tracks;
                    }
                }
                Err(error) => log_unless_missing(&provider, method, &error),
            }
        }
        Vec::new()
    }

    /// A chart's songs from the provider that keeps it (M46.9).
    pub async fn music_chart(
        &self,
        provider_id: &str,
        chart_id: &str,
        region: Option<&str>,
        limit: u32,
    ) -> Result<Vec<OnlineTrack>, CallError> {
        let args = serde_json::json!({ "id": chart_id, "region": region, "limit": limit });
        self.call(provider_id, "chart", args)
            .await
            .map(|value| self.tracks(value))
    }

    /// Every `music.tags` provider's genres for a song at once (M46.9): `(name, weight)` pairs as
    /// each wrote them, weights between 0 and 1 (1 when a provider gives none).
    pub async fn music_tags(
        &self,
        query: &TrackQuery,
    ) -> Vec<(ProviderInfo, Result<Vec<(String, f64)>, String>)> {
        let providers = self.providers(MUSIC_TAGS).await;
        let args = serde_json::json!({
            "title": query.title,
            "artist": query.artist,
            "album": query.album,
            "durationMs": query.duration_ms,
        });
        let requests = providers.into_iter().map(|provider| {
            let args = args.clone();
            async move {
                let answer = self
                    .call(&provider.id, "tags", args)
                    .await
                    .map(|value| parse_tags(&value))
                    .map_err(|error| error.to_string());
                (provider, answer)
            }
        });
        futures_util::future::join_all(requests).await
    }

    /// What the first `music.info` provider that knows the artist says about them (M46.14).
    pub async fn artist_info(&self, name: &str) -> Option<raygun_model::ArtistInfo> {
        for provider in self.providers(MUSIC_INFO).await {
            let answer = self
                .call(&provider.id, "artist", serde_json::json!({ "name": name }))
                .await;
            match answer {
                Ok(value) => {
                    if let Some(info) = parse_artist_info(&value, &provider.name) {
                        return Some(info);
                    }
                }
                Err(error) => {
                    tracing::debug!(provider = %provider.id, %error, "no artist info");
                }
            }
        }
        None
    }

    /// Who made a song, from the first `music.info` provider that knows it (M46.14).
    pub async fn song_credits(&self, query: &TrackQuery) -> Option<raygun_model::SongCredits> {
        let args = serde_json::json!({
            "title": query.title,
            "artist": query.artist,
            "album": query.album,
            "durationMs": query.duration_ms,
        });
        for provider in self.providers(MUSIC_INFO).await {
            match self.call(&provider.id, "credits", args.clone()).await {
                Ok(value) => {
                    if let Some(credits) = parse_credits(&value, &provider.name) {
                        return Some(credits);
                    }
                }
                Err(error) => tracing::debug!(provider = %provider.id, %error, "no credits"),
            }
        }
        None
    }

    /// Asks a provider for the stream behind one of its candidates.
    pub async fn resolve_stream(
        &self,
        provider_id: &str,
        candidate_id: &str,
    ) -> Result<ResolvedStream, CallError> {
        let value = self
            .call(
                provider_id,
                "resolve",
                serde_json::json!({ "id": candidate_id }),
            )
            .await?;
        let source = value
            .get("source")
            .cloned()
            .and_then(|source| serde_json::from_value::<MediaSource>(source).ok())
            .filter(allowed_source)
            .ok_or_else(|| CallError::Script("resolve returned no playable source".to_owned()))?;
        let expires_at = value.get("expiresAt").and_then(serde_json::Value::as_f64);
        Ok(ResolvedStream {
            source,
            expires_at: expires_at.filter(|ms| ms.is_finite()).map(|ms| ms as i64),
        })
    }

    async fn ask<T>(&self, request: impl FnOnce(oneshot::Sender<T>) -> Request) -> Option<T> {
        let (reply, answer) = oneshot::channel();
        self.requests.send(request(reply)).ok()?;
        answer.await.ok()
    }
}

struct Loaded {
    extension: Rc<Extension>,
}

/// A loaded extension: JavaScript, or a Shosetsu script (M32).
enum Extension {
    Js(JsExtension),
    Shosetsu(Box<ShosetsuExtension>),
}

impl Extension {
    fn manifest(&self) -> &Manifest {
        match self {
            Self::Js(extension) => extension.manifest(),
            Self::Shosetsu(extension) => extension.manifest(),
        }
    }

    fn user_settings(&self) -> &BTreeMap<String, serde_json::Value> {
        match self {
            Self::Js(extension) => extension.user_settings(),
            Self::Shosetsu(extension) => extension.user_settings(),
        }
    }

    fn providers(&self) -> Vec<ProviderInfo> {
        match self {
            Self::Js(extension) => extension.providers(),
            Self::Shosetsu(extension) => extension.providers(),
        }
    }

    /// The manifest's website, else a Shosetsu script's `baseURL`.
    fn website(&self) -> Option<String> {
        let declared = self.manifest().website.clone();
        match self {
            Self::Js(_) => declared,
            Self::Shosetsu(extension) => {
                declared.or_else(|| extension.website().map(str::to_owned))
            }
        }
    }

    async fn call(
        &self,
        local_id: &str,
        method: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match self {
            Self::Js(extension) => extension.call(local_id, method, args).await,
            Self::Shosetsu(extension) => extension.call(local_id, method, args).await,
        }
    }
}

struct HostState {
    config: HostConfig,
    loaded: RefCell<BTreeMap<String, Loaded>>,
    infos: RefCell<Vec<ExtensionInfo>>,
}

fn run(config: HostConfig, mut receiver: mpsc::UnboundedReceiver<Request>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!(%error, "extension host could not start");
            return;
        }
    };
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async move {
        let state = Rc::new(HostState {
            config,
            loaded: RefCell::new(BTreeMap::new()),
            infos: RefCell::new(Vec::new()),
        });
        while let Some(request) = receiver.recv().await {
            let state = state.clone();
            tokio::task::spawn_local(async move { state.handle(request).await });
        }
    });
}

impl HostState {
    async fn handle(&self, request: Request) {
        match request {
            Request::Load {
                sources,
                settings,
                reply,
            } => {
                let infos = self.load(sources, settings).await;
                let _ = reply.send(infos);
            }
            Request::List { reply } => {
                let _ = reply.send(self.infos.borrow().clone());
            }
            Request::Call {
                provider,
                method,
                args,
                reply,
            } => {
                let _ = reply.send(self.call(&provider, &method, &args).await);
            }
        }
    }

    async fn load(
        &self,
        sources: Vec<ExtensionSource>,
        mut settings: UserSettings,
    ) -> Vec<ExtensionInfo> {
        // Parse every manifest first, so installed copies can override built-in ones.
        let mut chosen: BTreeMap<String, (ExtensionSource, Manifest)> = BTreeMap::new();
        let mut infos = Vec::new();
        for source in sources {
            let manifest = source
                .read(MANIFEST_FILE)
                .and_then(|text| Manifest::parse(&text).map_err(|error| error.to_string()));
            match manifest {
                Ok(manifest) => {
                    let replace = chosen.get(&manifest.id).is_none_or(|(existing, _)| {
                        source.origin == ExtensionOrigin::Installed
                            || existing.origin == ExtensionOrigin::BuiltIn
                    });
                    if replace {
                        chosen.insert(manifest.id.clone(), (source, manifest));
                    }
                }
                Err(error) => {
                    let label = source.label();
                    tracing::warn!(extension = %label, %error, "extension manifest rejected");
                    infos.push(failed_info(&label, &label, "", source.origin, error));
                }
            }
        }

        self.loaded.borrow_mut().clear();
        for (id, (source, manifest)) in chosen {
            let origin = source.origin;
            let (name, version) = (manifest.name.clone(), manifest.version.to_string());
            // A conversion that fails still says which converter made it, so a newer one is
            // offered (M46.9.1).
            let compatibility = manifest.nuclear.as_ref().and_then(compat_report);
            let user_settings = settings.remove(&id).unwrap_or_default();
            match self.load_one(source, manifest, user_settings).await {
                Ok(extension) => {
                    let info = loaded_info(&extension, origin);
                    tracing::info!(extension = %id, providers = info.providers.len(), "extension loaded");
                    infos.push(info);
                    self.loaded.borrow_mut().insert(
                        id,
                        Loaded {
                            extension: Rc::new(extension),
                        },
                    );
                }
                Err(error) => {
                    tracing::warn!(extension = %id, %error, "extension failed to load");
                    infos.push(ExtensionInfo {
                        compatibility,
                        ..failed_info(&id, &name, &version, origin, error)
                    });
                }
            }
        }
        infos.sort_by_key(|info| info.name.to_lowercase());
        *self.infos.borrow_mut() = infos.clone();
        infos
    }

    async fn load_one(
        &self,
        source: ExtensionSource,
        manifest: Manifest,
        mut user_settings: BTreeMap<String, serde_json::Value>,
    ) -> Result<Extension, String> {
        let data_dir = self
            .config
            .storage_dir
            .as_ref()
            .map(|dir| dir.join(&manifest.id));
        let cookies = CookieJar::open(data_dir.as_ref().map(|dir| dir.join("cookies.json")));
        if manifest.runtime == Runtime::Shosetsu {
            let load = ShosetsuExtension::load(manifest, source, cookies);
            return tokio::time::timeout(self.config.call_timeout, load)
                .await
                .map_err(|_| "loading timed out".to_owned())?
                .map(|extension| Extension::Shosetsu(Box::new(extension)));
        }
        user_settings.retain(|key, value| manifest.check_setting(key, value).is_ok());
        let code = source.read(&manifest.entry)?;
        let storage = KvStore::open(data_dir.map(|dir| dir.join("storage.json")));
        let load = JsExtension::load(
            manifest,
            code,
            storage,
            cookies,
            user_settings,
            &self.config.limits,
            self.config.content.clone(),
        );
        tokio::time::timeout(self.config.call_timeout, load)
            .await
            .map_err(|_| "loading timed out".to_owned())?
            .map(Extension::Js)
    }

    async fn call(
        &self,
        provider: &str,
        method: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, CallError> {
        let unknown = || CallError::UnknownProvider(provider.to_owned());
        let (extension_id, local_id) = provider.split_once('/').ok_or_else(unknown)?;
        let extension = self
            .loaded
            .borrow()
            .get(extension_id)
            .map(|loaded| loaded.extension.clone())
            .ok_or_else(unknown)?;
        if !extension
            .providers()
            .iter()
            .any(|registered| registered.id == provider)
        {
            return Err(unknown());
        }
        // Shosetsu scripts block on each request and may page through long chapter lists.
        let timeout = match extension.as_ref() {
            Extension::Js(_) => self.config.call_timeout,
            Extension::Shosetsu(_) => self
                .config
                .call_timeout
                .max(crate::shosetsu::runtime::CALL_BUDGET + Duration::from_secs(5)),
        };
        tokio::time::timeout(timeout, extension.call(local_id, method, args))
            .await
            .map_err(|_| CallError::Timeout(provider.to_owned()))?
            .map_err(CallError::Script)
    }
}

fn loaded_info(extension: &Extension, origin: ExtensionOrigin) -> ExtensionInfo {
    let manifest = extension.manifest();
    let values = extension.user_settings();
    ExtensionInfo {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.to_string(),
        description: manifest.description.clone(),
        content_rating: manifest.content_rating.as_str().to_owned(),
        origin,
        status: ExtensionStatus::Loaded,
        error: None,
        provides: manifest.provides.clone(),
        net: manifest
            .net
            .patterns()
            .iter()
            .map(ToString::to_string)
            .collect(),
        providers: extension.providers(),
        compatibility: manifest.nuclear.as_ref().and_then(compat_report),
        icon: None,
        website: extension.website(),
        settings: manifest
            .settings
            .iter()
            .map(|setting| {
                let default = setting.default.as_ref().and_then(SettingValue::from_json);
                ExtensionSetting {
                    key: setting.key.clone(),
                    label: setting
                        .label
                        .clone()
                        .unwrap_or_else(|| key_label(&setting.key)),
                    kind: match setting.kind {
                        manifest::SettingKind::Text => SettingKind::Text,
                        manifest::SettingKind::Number => SettingKind::Number,
                        manifest::SettingKind::Boolean => SettingKind::Boolean,
                        manifest::SettingKind::Select => SettingKind::Select,
                    },
                    options: setting.options.clone(),
                    value: values
                        .get(&setting.key)
                        .and_then(SettingValue::from_json)
                        .or_else(|| default.clone()),
                    default,
                }
            })
            .collect(),
    }
}

/// The report the Nuclear converter stored in the manifest (`nuclear: { package, report }`).
fn compat_report(nuclear: &serde_json::Value) -> Option<raygun_model::CompatReport> {
    let report = nuclear.get("report")?;
    let list = |key: &str| -> Vec<String> {
        report
            .get(key)
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    Some(raygun_model::CompatReport {
        package: nuclear.get("package")?.as_str()?.to_owned(),
        converter: nuclear
            .get("converter")
            .and_then(serde_json::Value::as_u64)
            .and_then(|version| u32::try_from(version).ok())
            .unwrap_or(1),
        supported: list("supported"),
        unsupported_categories: list("unsupportedCategories"),
        stubbed_apis: list("stubbedApis"),
        ui_imports: list("uiImports"),
    })
}

fn failed_info(
    id: &str,
    name: &str,
    version: &str,
    origin: ExtensionOrigin,
    error: String,
) -> ExtensionInfo {
    ExtensionInfo {
        id: id.to_owned(),
        name: name.to_owned(),
        version: version.to_owned(),
        description: None,
        content_rating: "safe".to_owned(),
        origin,
        status: ExtensionStatus::Failed,
        error: Some(error),
        provides: Vec::new(),
        net: Vec::new(),
        providers: Vec::new(),
        settings: Vec::new(),
        compatibility: None,
        icon: None,
        website: None,
    }
}

/// A short text field: trimmed, at most `limit` characters, none when empty.
fn text_field(value: &serde_json::Value, key: &str, limit: usize) -> Option<String> {
    let text = value.get(key)?.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    Some(text.chars().take(limit).collect())
}

/// An http(s) link field.
fn link_field(value: &serde_json::Value, key: &str) -> Option<String> {
    text_field(value, key, 2048).filter(|url| is_web_url(url))
}

fn parse_artist_info(
    value: &serde_json::Value,
    provider: &str,
) -> Option<raygun_model::ArtistInfo> {
    let info = value.get("artist").unwrap_or(value);
    let name = text_field(info, "name", 200)?;
    Some(raygun_model::ArtistInfo {
        name,
        kind: text_field(info, "kind", 40),
        bio: text_field(info, "bio", 4000),
        image_url: link_field(info, "imageUrl"),
        url: link_field(info, "url"),
        origin: text_field(info, "origin", 200),
        active: text_field(info, "active", 60),
        members: info
            .get("members")
            .and_then(serde_json::Value::as_array)
            .map(|members| {
                members
                    .iter()
                    .filter_map(|member| member.as_str().map(str::trim))
                    .filter(|member| !member.is_empty())
                    .take(40)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        provider_name: provider.to_owned(),
    })
}

fn parse_credits(value: &serde_json::Value, provider: &str) -> Option<raygun_model::SongCredits> {
    let groups = value.get("credits")?.as_array()?;
    let credits: Vec<raygun_model::CreditGroup> = groups
        .iter()
        .filter_map(|group| {
            let role = text_field(group, "role", 80)?;
            let names: Vec<String> = group
                .get("names")?
                .as_array()?
                .iter()
                .filter_map(|name| name.as_str().map(str::trim))
                .filter(|name| !name.is_empty())
                .take(40)
                .map(str::to_owned)
                .collect();
            (!names.is_empty()).then_some(raygun_model::CreditGroup { role, names })
        })
        .take(40)
        .collect();
    if credits.is_empty() {
        return None;
    }
    Some(raygun_model::SongCredits {
        credits,
        url: link_field(value, "url"),
        provider_name: provider.to_owned(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTrack {
    #[serde(default)]
    id: Option<serde_json::Value>,
    title: String,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    album: Option<String>,
    #[serde(default)]
    duration_ms: Option<f64>,
    #[serde(default)]
    artwork_url: Option<String>,
    #[serde(default)]
    source: Option<MediaSource>,
    #[serde(default)]
    explicit: Option<bool>,
}

fn is_web_url(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

/// Reads `{ tracks: [...] }` leniently: malformed entries are skipped, not fatal. Extensions may
/// only hand out web streams and YouTube ids, never local files.
fn parse_tracks(value: serde_json::Value) -> Vec<OnlineTrack> {
    let Some(tracks) = value.get("tracks").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    tracks
        .iter()
        .filter_map(|track| serde_json::from_value::<RawTrack>(track.clone()).ok())
        .filter(|track| !track.title.trim().is_empty())
        .map(|track| OnlineTrack {
            external_id: track.id.map(|id| match id {
                serde_json::Value::String(id) => id,
                other => other.to_string(),
            }),
            title: track.title.trim().to_owned(),
            artist: track.artist.filter(|artist| !artist.trim().is_empty()),
            album: track.album.filter(|album| !album.trim().is_empty()),
            duration_ms: track
                .duration_ms
                .filter(|ms| ms.is_finite() && *ms > 0.0 && *ms < f64::from(u32::MAX))
                .map(|ms| ms as u32),
            artwork_url: track.artwork_url.filter(|url| is_web_url(url)),
            source: track.source.filter(allowed_source),
            explicit: track.explicit.unwrap_or(false),
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawChart {
    id: serde_json::Value,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    artwork_url: Option<String>,
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    group: Option<String>,
}

/// Logs a failed optional call, unless the provider just doesn't have it.
fn log_unless_missing(provider: &ProviderInfo, method: &str, error: &CallError) {
    let message = error.to_string();
    if !message.contains("has no method") {
        tracing::warn!(provider = %provider.id, %message, "{method} failed");
    }
}

/// A provider's id for something: a string, or a number written out.
fn json_id(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(id) if !id.is_empty() => Some(id.clone()),
        serde_json::Value::Number(id) => Some(id.to_string()),
        _ => None,
    }
}

fn web_url(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(serde_json::Value::as_str)
        .filter(|url| is_web_url(url))
        .map(str::to_owned)
}

/// Reads `{ id, title, author?, artworkUrl?, trackCount?, featured? }` (M46.39).
fn parse_playlist(provider: &ProviderInfo, playlist: &serde_json::Value) -> Option<MusicPlaylist> {
    let title = playlist.get("title")?.as_str()?.trim().to_owned();
    if title.is_empty() {
        return None;
    }
    Some(MusicPlaylist {
        provider: provider.clone(),
        id: json_id(playlist.get("id"))?,
        title,
        author: playlist
            .get("author")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|author| !author.is_empty())
            .map(str::to_owned),
        artwork_url: web_url(playlist.get("artworkUrl")),
        track_count: playlist
            .get("trackCount")
            .and_then(serde_json::Value::as_u64)
            .and_then(|count| u32::try_from(count).ok()),
        featured: playlist
            .get("featured")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

/// Reads `{ id, title, artist?, year?, artworkUrl?, trackCount? }`.
fn parse_album(provider: &ProviderInfo, album: &serde_json::Value) -> Option<MusicAlbum> {
    let title = album.get("title")?.as_str()?.trim().to_owned();
    if title.is_empty() {
        return None;
    }
    let number = |key: &str| {
        album
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
    };
    Some(MusicAlbum {
        provider: provider.clone(),
        id: json_id(album.get("id"))?,
        title,
        artist: album
            .get("artist")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|artist| !artist.is_empty())
            .map(str::to_owned),
        year: number("year"),
        artwork_url: web_url(album.get("artworkUrl")),
        track_count: number("trackCount"),
        kind: album
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .map(raygun_model::ReleaseKind::parse),
    })
}

/// Reads `{ charts: [{ id, name, description?, artworkUrl?, tag? }] }`, dropping charts without an
/// id or a name.
fn parse_charts(provider: &ProviderInfo, value: &serde_json::Value) -> Vec<MusicChart> {
    let Some(charts) = value.get("charts").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let text = |value: Option<String>| {
        value
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
    };
    charts
        .iter()
        .filter_map(|chart| serde_json::from_value::<RawChart>(chart.clone()).ok())
        .filter_map(|chart| {
            let id = match chart.id {
                serde_json::Value::String(id) => id,
                serde_json::Value::Number(id) => id.to_string(),
                _ => return None,
            };
            let name = chart.name.trim().to_owned();
            let tag = text(chart.tag);
            // Without a group (older sources), a genre's chart is a genre chart, else a main one.
            let group = chart
                .group
                .as_deref()
                .and_then(raygun_model::ChartGroup::parse)
                .unwrap_or(if tag.is_some() {
                    raygun_model::ChartGroup::Genre
                } else {
                    raygun_model::ChartGroup::Top
                });
            (!id.is_empty() && !name.is_empty()).then(|| MusicChart {
                provider: provider.clone(),
                id,
                name,
                description: text(chart.description),
                artwork_url: chart.artwork_url.filter(|url| is_web_url(url)),
                tag,
                group,
            })
        })
        .collect()
}

/// Reads `{ tags: [{ name, weight? }] }`: names trimmed and at most `MAX_TAG_CHARS` long,
/// weights clamped to 0-1.
fn parse_tags(value: &serde_json::Value) -> Vec<(String, f64)> {
    let Some(tags) = value.get("tags").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    tags.iter()
        .filter_map(|tag| {
            let name = tag.get("name")?.as_str()?.trim();
            if name.is_empty() || name.chars().count() > MAX_TAG_CHARS {
                return None;
            }
            let weight = tag
                .get("weight")
                .and_then(serde_json::Value::as_f64)
                .filter(|weight| weight.is_finite())
                .map_or(1.0, |weight| weight.clamp(0.0, 1.0));
            Some((name.to_owned(), weight))
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLine {
    time_ms: f64,
    text: String,
}

/// Reads `{ synced?: [{ timeMs, text }], plain? }`. Lines with bad times are dropped and the rest
/// sorted; an answer with neither synced lines nor plain text counts as "no lyrics".
fn parse_lyrics(provider: &ProviderInfo, value: &serde_json::Value) -> Option<Lyrics> {
    if value.is_null() {
        return None;
    }
    let mut synced: Vec<LyricLine> = value
        .get("synced")
        .and_then(serde_json::Value::as_array)
        .map(|lines| {
            lines
                .iter()
                .filter_map(|line| serde_json::from_value::<RawLine>(line.clone()).ok())
                .filter(|line| line.time_ms.is_finite() && line.time_ms >= 0.0)
                .map(|line| LyricLine {
                    time_ms: line.time_ms.min(f64::from(u32::MAX)) as u32,
                    text: line.text.trim().to_owned(),
                })
                .collect()
        })
        .unwrap_or_default();
    synced.sort_by_key(|line| line.time_ms);
    let plain = value
        .get("plain")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);
    if synced.iter().all(|line| line.text.is_empty()) && plain.is_none() {
        return None;
    }
    Some(Lyrics {
        provider_name: provider.name.clone(),
        synced,
        plain,
        offset_ms: 0,
        edited: false,
    })
}

/// Sources an extension may hand out: web streams and YouTube ids, never local files or lookups.
fn allowed_source(source: &MediaSource) -> bool {
    match source {
        MediaSource::Url { url, .. } => is_web_url(url),
        MediaSource::YouTube { .. } => true,
        MediaSource::LocalFile { .. } | MediaSource::Lookup { .. } => false,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCandidate {
    id: serde_json::Value,
    title: String,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    duration_ms: Option<f64>,
    #[serde(default)]
    quality: Option<String>,
    #[serde(default)]
    bitrate_kbps: Option<f64>,
    #[serde(default)]
    source: Option<MediaSource>,
}

/// Reads `{ candidates: [...] }` like [`parse_tracks`]: malformed entries are skipped, and a
/// candidate whose inline source isn't allowed is dropped (it would never play).
fn parse_candidates(provider: &ProviderInfo, value: serde_json::Value) -> Vec<Candidate> {
    let Some(candidates) = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };
    candidates
        .iter()
        .filter_map(|candidate| serde_json::from_value::<RawCandidate>(candidate.clone()).ok())
        .filter(|candidate| candidate.source.as_ref().is_none_or(allowed_source))
        .filter(|candidate| !candidate.title.trim().is_empty())
        .map(|candidate| Candidate {
            provider_id: provider.id.clone(),
            provider_name: provider.name.clone(),
            id: match candidate.id {
                serde_json::Value::String(id) => id,
                other => other.to_string(),
            },
            title: candidate.title.trim().to_owned(),
            artist: candidate.artist.filter(|artist| !artist.trim().is_empty()),
            duration_ms: candidate
                .duration_ms
                .filter(|ms| ms.is_finite() && *ms > 0.0 && *ms < f64::from(u32::MAX))
                .map(|ms| ms as u32),
            quality: candidate.quality,
            bitrate_kbps: candidate
                .bitrate_kbps
                .filter(|kbps| kbps.is_finite() && *kbps >= 1.0 && *kbps < 100_000.0)
                .map(|kbps| kbps.round() as u32),
            source: candidate.source,
        })
        .collect()
}

/// A label for a setting that has none: `itemsPerSearch` and `items_per_search` read as
/// "Items per search".
pub(crate) fn key_label(key: &str) -> String {
    let mut words = String::new();
    let mut previous_lower = false;
    for character in key.chars() {
        if character == '_' || character == '-' {
            words.push(' ');
            previous_lower = false;
            continue;
        }
        if character.is_uppercase() && previous_lower {
            words.push(' ');
        }
        previous_lower = character.is_lowercase() || character.is_ascii_digit();
        words.extend(character.to_lowercase());
    }
    let mut chars = words.trim().chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}
