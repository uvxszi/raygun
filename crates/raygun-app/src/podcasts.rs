//! Podcasts (M37): subscriptions to RSS/Atom feeds, episodes with listening progress, directory
//! search through `podcast.directory` extensions, playback that resumes where listening stopped,
//! and episode downloads under `downloads/podcasts/`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use raygun_db::podcasts::{self as store, EpisodeRow, PodcastRow};
use raygun_ext_host::audiobook::AUDIOBOOK_DIRECTORY;
use raygun_ext_host::podcast::PODCAST_DIRECTORY;
use raygun_model::{
    AudiobookPage, AudiobookResult, AudiobookSummary, AudiobookView, EpisodeDownload,
    EpisodeDownloadState, MediaKind, MediaSource, PlaybackState, PlaybackStatus, PodcastEpisode,
    PodcastSearchResult, PodcastSummary, PodcastView, QueueItem, QueueItemInput, SpokenItem,
    SpokenKind,
};
use raygun_net::ImageLinks;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::io::AsyncWriteExt;

use crate::catalog::now_ms;
use crate::events::PodcastsChangedEvent;
use crate::extensions::ExtensionService;
use crate::playback::PlaybackService;
use crate::podcast_feed::{self, MAX_FEED_BYTES};
use crate::startup::AppDb;

/// Folder under the downloads root holding episode files.
const FOLDER: &str = "podcasts";
/// `local_meta` key of the last refresh of every subscription (unix ms).
const LAST_REFRESH: &str = "podcasts.lastRefresh";
/// Subscriptions refresh on start when the last refresh is older than this.
const REFRESH_EVERY_MS: i64 = 6 * 60 * 60 * 1000;
/// A feed opened again within this long isn't fetched again.
const FRESH_MS: i64 = 60 * 60 * 1000;
/// Listening progress is saved at most this often while playing.
const SAVE_EVERY_MS: f64 = 10_000.0;
/// Past this share of an episode it counts as played.
const PLAYED_SHARE: f64 = 0.95;
/// Episodes larger than this aren't downloaded.
const MAX_EPISODE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Download progress is written (and announced) every this many bytes.
const PROGRESS_STEP: u64 = 1024 * 1024;
/// Most directory results asked for.
const SEARCH_LIMIT: u32 = 30;

/// `remote_kind` of an audiobook (M46.22).
const AUDIOBOOK: &str = "audiobook";

pub struct PodcastService {
    app: AppHandle,
    client: reqwest::Client,
    images: ImageLinks,
    downloads_dir: PathBuf,
    follow: Arc<Mutex<Follow>>,
    downloading: Arc<Mutex<HashSet<String>>>,
}

/// What playback is doing to an episode.
#[derive(Debug, Default)]
struct Follow {
    item_id: Option<String>,
    episode_id: Option<String>,
    saved_ms: f64,
    finished: bool,
}

impl PodcastService {
    pub fn new(
        app: AppHandle,
        client: reqwest::Client,
        images: ImageLinks,
        downloads_dir: PathBuf,
    ) -> Self {
        Self {
            app,
            client,
            images,
            downloads_dir,
            follow: Arc::default(),
            downloading: Arc::default(),
        }
    }

    /// Refreshes subscriptions in the background when they haven't been for a while.
    pub fn start(self: &Arc<Self>) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            let db = service.db();
            let last = raygun_db::local_value(&db, LAST_REFRESH)
                .await
                .ok()
                .flatten()
                .and_then(|text| text.parse::<i64>().ok())
                .unwrap_or(0);
            if now_ms() - last >= REFRESH_EVERY_MS {
                match service.refresh_all().await {
                    Ok(added) => tracing::info!(added, "podcasts refreshed"),
                    Err(error) => tracing::warn!(%error, "podcast refresh failed"),
                }
            }
        });
    }

    // ---- Subscriptions and feeds ----

    pub async fn subscriptions(&self) -> Result<Vec<PodcastSummary>, String> {
        let rows = store::subscriptions(&self.db()).await.map_err(to_message)?;
        Ok(rows
            .into_iter()
            .map(|row| PodcastSummary {
                id: row.podcast.id,
                feed_url: row.podcast.fields.feed_url,
                title: row.podcast.fields.title,
                author: row.podcast.fields.author,
                artwork_url: self.kept_artwork(row.podcast.fields.artwork.as_deref()),
                episodes: row.episodes as u32,
                unplayed: row.unplayed as u32,
                latest_published_at: row.latest_published_at.map(|ms| ms as f64),
            })
            .collect())
    }

    /// The newest episodes across subscriptions.
    pub async fn latest(&self, limit: u32) -> Result<Vec<PodcastEpisode>, String> {
        let db = self.db();
        let rows = store::latest_episodes(&db, limit.clamp(1, 200))
            .await
            .map_err(to_message)?;
        self.episodes_view(rows).await
    }

    /// A podcast by its feed address: stored (and refreshed) on first sight, not subscribed.
    pub async fn open_feed(&self, feed_url: &str) -> Result<PodcastView, String> {
        let id = self.open_as(feed_url, None).await?;
        self.view(&id).await
    }

    /// Stores a feed on first sight (as `kind`, when given) and returns its id.
    async fn open_as(&self, feed_url: &str, kind: Option<&str>) -> Result<String, String> {
        let feed_url = normalize_feed_url(feed_url)?;
        let db = self.db();
        let known = store::podcast_by_feed(&db, &feed_url)
            .await
            .map_err(to_message)?;
        Ok(match known {
            Some(row)
                if row.refreshed_at.is_some_and(|at| now_ms() - at < FRESH_MS)
                    && kind.is_none_or(|kind| kind == row.fields.kind) =>
            {
                row.id
            }
            _ => self.refresh_feed_as(&feed_url, kind).await?.0,
        })
    }

    // ---- Audiobooks (M46.22) ----

    /// The books in the library, the last listened first.
    pub async fn audiobooks(&self) -> Result<Vec<AudiobookSummary>, String> {
        let rows = store::audiobooks(&self.db()).await.map_err(to_message)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                // Books stored before titles were tidied on refresh read the same.
                let (title, author) = book_title(
                    &row.podcast.fields.title,
                    row.podcast.fields.author.as_deref(),
                );
                AudiobookSummary {
                    artwork_url: self.kept_artwork(row.podcast.fields.artwork.as_deref()),
                    id: row.podcast.id,
                    title,
                    author,
                    chapters: row.chapters as u32,
                    finished: row.finished as u32,
                }
            })
            .collect())
    }

    /// Opens a book by its chapter feed (stored as an audiobook on first sight); returns its id.
    pub async fn open_audiobook(&self, feed_url: &str) -> Result<String, String> {
        self.open_as(feed_url, Some(AUDIOBOOK)).await
    }

    /// A book with its chapters in order.
    pub async fn audiobook(&self, id: &str) -> Result<AudiobookView, String> {
        let view = self.view(id).await?;
        let (title, author) = book_title(&view.title, view.author.as_deref());
        Ok(AudiobookView {
            id: view.id,
            feed_url: view.feed_url,
            title,
            author,
            description: view.description,
            artwork_url: view.artwork_url,
            link: view.link,
            in_library: view.subscribed,
            chapters: view.episodes,
        })
    }

    /// Plays a book from `chapter` (else the first chapter not finished, where it stopped), with
    /// the rest of the book queued after it.
    pub async fn play_audiobook(&self, id: &str, chapter: Option<&str>) -> Result<(), String> {
        let chapters = store::episodes(&self.db(), id).await.map_err(to_message)?;
        let start = match chapter {
            Some(chapter) => chapters
                .iter()
                .position(|row| row.id == chapter)
                .ok_or_else(|| "This chapter is no longer in the book".to_owned())?,
            None => chapters
                .iter()
                .position(|row| row.played_at.is_none())
                .unwrap_or(0),
        };
        let mut items = Vec::new();
        let mut start_ms = 0.0;
        for (index, row) in chapters.iter().enumerate().skip(start) {
            let (item, resume) = self.queue_item(&row.id).await?;
            if index == start {
                start_ms = resume;
            }
            items.push(item);
        }
        if items.is_empty() {
            return Err("This book has no chapters".into());
        }
        self.app
            .state::<PlaybackService>()
            .replace_from(items, start_ms);
        Ok(())
    }

    /// A page of books from the audiobook directories: `popular`, `latest`, `search` or
    /// `subject` (`query` for the last two).
    pub async fn audiobook_directory(
        &self,
        list: &str,
        query: &str,
        page: u32,
    ) -> Result<AudiobookPage, String> {
        let extensions = self.app.state::<Arc<ExtensionService>>();
        let (_ready, host) = extensions.ready().await;
        let providers = host.providers(AUDIOBOOK_DIRECTORY).await;
        if providers.is_empty() {
            return Err("No audiobook source is installed".into());
        }
        let mut found = AudiobookPage {
            books: Vec::new(),
            has_more: false,
        };
        let mut errors = Vec::new();
        // Every source at once (M46.47); a book two sources share shows once, from the first.
        let answers = futures_util::future::join_all(
            providers
                .iter()
                .map(|provider| host.audiobooks(&provider.id, list, query.trim(), page.max(1))),
        )
        .await;
        for (provider, answer) in providers.into_iter().zip(answers) {
            match answer {
                Ok(books) => {
                    found.has_more |= books.has_more;
                    for book in books.books {
                        if found.books.iter().any(|known| {
                            known.feed_url == book.feed_url
                                || same_book(
                                    (&known.title, known.author.as_deref()),
                                    (&book.title, book.author.as_deref()),
                                )
                        }) {
                            continue;
                        }
                        found.books.push(AudiobookResult {
                            artwork_url: self.artwork(book.artwork_url.as_deref()),
                            title: book.title,
                            author: book.author,
                            feed_url: book.feed_url,
                            runtime: book.runtime,
                            language: book.language,
                            description: book.description,
                            source: provider.name.clone(),
                        });
                    }
                }
                Err(error) => errors.push(raygun_net::issue::named(&provider.name, &error)),
            }
        }
        if found.books.is_empty() && !errors.is_empty() {
            return Err(raygun_net::issue::joined(&errors));
        }
        Ok(found)
    }

    /// A stored podcast; `refresh` reads its feed again first.
    pub async fn podcast(&self, id: &str, refresh: bool) -> Result<PodcastView, String> {
        if refresh {
            let row = self.row(id).await?;
            self.refresh_feed(&row.fields.feed_url).await?;
        }
        self.view(id).await
    }

    pub async fn set_subscribed(&self, id: &str, subscribed: bool) -> Result<(), String> {
        store::set_subscribed(&self.db(), id, subscribed.then(now_ms))
            .await
            .map_err(to_message)?;
        // Quick access follows (M46.41): a followed show or a book kept in the library.
        if let Ok(row) = self.row(id).await {
            let (kind, to) = if row.fields.kind == "audiobook" {
                ("audiobook", "/audiobook")
            } else {
                ("podcast", "/podcast")
            };
            if subscribed {
                let pin = crate::quick_access::page_pin(
                    kind,
                    to,
                    id,
                    &row.fields.title,
                    row.fields.artwork.clone(),
                );
                crate::quick_access::add(&self.app, pin).await;
            } else {
                let search = [("id".to_owned(), id.to_owned())].into_iter().collect();
                crate::quick_access::remove(&self.app, to, &search).await;
            }
        }
        self.changed(id);
        Ok(())
    }

    /// Reads every subscription's feed again; returns how many episodes were new.
    pub async fn refresh_all(&self) -> Result<u32, String> {
        let db = self.db();
        let subscriptions = store::subscriptions(&db).await.map_err(to_message)?;
        let mut added = 0;
        for subscription in subscriptions {
            match self
                .refresh_feed(&subscription.podcast.fields.feed_url)
                .await
            {
                Ok((_, new)) => added += new,
                Err(error) => tracing::info!(
                    podcast = %subscription.podcast.fields.title,
                    %error,
                    "podcast feed refresh failed"
                ),
            }
        }
        let _ = raygun_db::set_local_value(&db, LAST_REFRESH, &now_ms().to_string()).await;
        self.changed("");
        Ok(added)
    }

    /// Fetches and stores a feed; returns the podcast's id and how many episodes were new.
    async fn refresh_feed(&self, feed_url: &str) -> Result<(String, u32), String> {
        self.refresh_feed_as(feed_url, None).await
    }

    /// Reads a feed and stores it: as `kind` when given, else as it was (a podcast when new).
    async fn refresh_feed_as(
        &self,
        feed_url: &str,
        kind: Option<&str>,
    ) -> Result<(String, u32), String> {
        let bytes = self.fetch_feed(feed_url).await?;
        let url = feed_url.to_owned();
        let (mut podcast, episodes) =
            tauri::async_runtime::spawn_blocking(move || podcast_feed::parse(&bytes, &url))
                .await
                .map_err(to_message)??;
        let db = self.db();
        podcast.kind = match kind {
            Some(kind) => kind.to_owned(),
            None => store::podcast_by_feed(&db, feed_url)
                .await
                .map_err(to_message)?
                .map_or(podcast.kind, |row| row.fields.kind),
        };
        if podcast.kind == AUDIOBOOK {
            let (title, author) = book_title(&podcast.title, podcast.author.as_deref());
            podcast.title = title;
            podcast.author = author;
        }
        let now = now_ms();
        let id = store::upsert_podcast(&db, &podcast, now)
            .await
            .map_err(to_message)?;
        let added = store::sync_episodes(&db, &id, &episodes, now)
            .await
            .map_err(to_message)?;
        Ok((id, added as u32))
    }

    async fn fetch_feed(&self, feed_url: &str) -> Result<Vec<u8>, String> {
        let response = self
            .client
            .get(feed_url)
            .send()
            .await
            .map_err(|error| format!("Couldn't reach the feed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("The feed answered {}", response.status()));
        }
        let mut bytes = Vec::new();
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|error| format!("The feed broke off: {error}"))?;
            if bytes.len() + chunk.len() > MAX_FEED_BYTES {
                return Err("This feed is too large".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }

    async fn row(&self, id: &str) -> Result<PodcastRow, String> {
        store::podcast(&self.db(), id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "This podcast was removed".to_owned())
    }

    async fn view(&self, id: &str) -> Result<PodcastView, String> {
        let db = self.db();
        let row = self.row(id).await?;
        let episodes = store::episodes(&db, id).await.map_err(to_message)?;
        Ok(PodcastView {
            id: row.id,
            feed_url: row.fields.feed_url,
            title: row.fields.title,
            author: row.fields.author,
            description: row.fields.description,
            artwork_url: if row.subscribed_at.is_some() {
                self.kept_artwork(row.fields.artwork.as_deref())
            } else {
                self.artwork(row.fields.artwork.as_deref())
            },
            link: row.fields.link,
            subscribed: row.subscribed_at.is_some(),
            episodes: self.episodes_view(episodes).await?,
        })
    }

    async fn episodes_view(&self, rows: Vec<EpisodeRow>) -> Result<Vec<PodcastEpisode>, String> {
        let downloads = store::downloads(&self.db()).await.map_err(to_message)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let download = downloads
                    .iter()
                    .find(|download| download.episode_id == row.id)
                    .map(download_view);
                PodcastEpisode {
                    video: row
                        .fields
                        .audio_type
                        .as_deref()
                        .is_some_and(|kind| kind.starts_with("video/")),
                    artwork_url: self.artwork(
                        row.fields
                            .artwork
                            .as_deref()
                            .or(row.podcast_artwork.as_deref()),
                    ),
                    id: row.id,
                    podcast_id: row.podcast_id,
                    podcast_title: row.podcast_title,
                    title: row.fields.title,
                    description: row.fields.description,
                    published_at: row.fields.published_at.map(|ms| ms as f64),
                    duration_ms: row.fields.duration_ms.map(|ms| ms as f64),
                    position_ms: row.position_ms as f64,
                    played: row.played_at.is_some(),
                    download,
                }
            })
            .collect())
    }

    // ---- Directory ----

    /// Asks every podcast directory; results merge by feed, marked when already subscribed.
    pub async fn search(&self, query: &str) -> Result<Vec<PodcastSearchResult>, String> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        self.directory(Ask::Search(query)).await
    }

    /// Popular podcasts from the directories that list them.
    pub async fn top(&self) -> Result<Vec<PodcastSearchResult>, String> {
        self.directory(Ask::Top).await
    }

    async fn directory(&self, ask: Ask<'_>) -> Result<Vec<PodcastSearchResult>, String> {
        let extensions = self.app.state::<Arc<ExtensionService>>();
        let (_ready, host) = extensions.ready().await;
        let providers = host.providers(PODCAST_DIRECTORY).await;
        if providers.is_empty() {
            return Err("No podcast directory is installed".into());
        }
        let mut results: Vec<PodcastSearchResult> = Vec::new();
        let mut errors = Vec::new();
        for provider in providers {
            let found = match ask {
                Ask::Search(query) => host.podcast_search(&provider.id, query, SEARCH_LIMIT).await,
                Ask::Top => host.podcast_top(&provider.id, SEARCH_LIMIT).await,
            };
            match found {
                Ok(found) => {
                    for result in found {
                        if results
                            .iter()
                            .any(|known| known.feed_url == result.feed_url)
                        {
                            continue;
                        }
                        results.push(PodcastSearchResult {
                            artwork_url: self.artwork(result.artwork_url.as_deref()),
                            title: result.title,
                            author: result.author,
                            feed_url: result.feed_url,
                            description: result.description,
                            subscribed: false,
                        });
                    }
                }
                Err(error) => errors.push(raygun_net::issue::named(&provider.name, &error)),
            }
        }
        if results.is_empty() && !errors.is_empty() {
            return Err(raygun_net::issue::joined(&errors));
        }
        let db = self.db();
        for result in &mut results {
            result.subscribed = store::podcast_by_feed(&db, &result.feed_url)
                .await
                .ok()
                .flatten()
                .is_some_and(|row| row.subscribed_at.is_some());
        }
        Ok(results)
    }

    // ---- Playback ----

    /// Plays an episode, resuming where listening stopped (from the start once played).
    pub async fn play(&self, episode_id: &str) -> Result<(), String> {
        let (item, start_ms) = self.queue_item(episode_id).await?;
        self.app
            .state::<PlaybackService>()
            .replace_from(vec![item], start_ms);
        Ok(())
    }

    /// Queues an episode after the current item (`next`, Play next) or at the end (M46.16).
    pub async fn queue(&self, episode_id: &str, next: bool) -> Result<(), String> {
        let (item, _) = self.queue_item(episode_id).await?;
        let playback = self.app.state::<PlaybackService>();
        if next {
            playback.enqueue_next(vec![item]);
        } else {
            playback.enqueue(vec![item], false);
        }
        Ok(())
    }

    /// An episode as a queue item (its download when there is one), and where it resumes.
    async fn queue_item(&self, episode_id: &str) -> Result<(QueueItemInput, f64), String> {
        let db = self.db();
        let episode = store::episode(&db, episode_id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "This episode was removed".to_owned())?;
        let download = store::download(&db, episode_id).await.map_err(to_message)?;
        let local = download
            .filter(|download| download.state == "done")
            .and_then(|download| download.path)
            .map(|path| self.downloads_dir.join(path))
            .filter(|path| path.exists());
        let source = match local {
            Some(path) => MediaSource::LocalFile {
                path: path.to_string_lossy().into_owned(),
            },
            None => MediaSource::Url {
                url: episode.fields.audio_url.clone(),
                headers: Default::default(),
            },
        };
        let duration = episode.fields.duration_ms.unwrap_or(0) as f64;
        let position = episode.position_ms as f64;
        let start_ms = if episode.played_at.is_some()
            || (duration > 0.0 && position >= duration * PLAYED_SHARE)
        {
            0.0
        } else {
            position
        };
        let video = episode
            .fields
            .audio_type
            .as_deref()
            .is_some_and(|kind| kind.starts_with("video/"));
        // A book's chapters play as its author's, on an album named after the book.
        let (artist, album) = if episode.podcast_kind == AUDIOBOOK {
            let (book, author) =
                book_title(&episode.podcast_title, episode.podcast_author.as_deref());
            (author.or_else(|| Some(book.clone())), Some(book))
        } else {
            (Some(episode.podcast_title.clone()), None)
        };
        let item = QueueItemInput {
            title: episode.fields.title.clone(),
            artist,
            album,
            duration_ms: episode.fields.duration_ms.map(|ms| ms as u32),
            artwork_url: self.artwork(
                episode
                    .fields
                    .artwork
                    .as_deref()
                    .or(episode.podcast_artwork.as_deref()),
            ),
            media_kind: if video {
                MediaKind::Video
            } else {
                MediaKind::Audio
            },
            source,
        };
        Ok((item, start_ms))
    }

    /// Follows playback: saves the position of a playing episode and marks it played near the
    /// end. Called with every state change, so it does nothing slow itself.
    pub fn on_playback(&self, state: &PlaybackState, item: Option<&QueueItem>) {
        let Some(item) = item else {
            return;
        };
        let mut follow = self.follow.lock().unwrap_or_else(|p| p.into_inner());
        if follow.item_id.as_deref() != Some(item.id.as_str()) {
            *follow = Follow {
                item_id: Some(item.id.clone()),
                saved_ms: state.position_ms,
                ..Follow::default()
            };
            self.look_up(item);
            return;
        }
        let Some(episode_id) = follow.episode_id.clone() else {
            return;
        };
        // Incognito (M47): where it was and what was finished aren't saved.
        if crate::privacy::incognito(&self.app) {
            return;
        }
        let position = state.position_ms;
        let duration = state.duration_ms.unwrap_or(0.0);
        let near_end = duration > 0.0 && position >= duration * PLAYED_SHARE;
        if (state.status == PlaybackStatus::Ended || near_end) && !follow.finished {
            follow.finished = true;
            let db = self.db();
            let app = self.app.clone();
            tauri::async_runtime::spawn(async move {
                if store::set_played(&db, &episode_id, Some(now_ms()))
                    .await
                    .is_ok()
                {
                    announce(&app, "");
                }
            });
            return;
        }
        let paused = matches!(
            state.status,
            PlaybackStatus::Paused | PlaybackStatus::Idle | PlaybackStatus::Error
        );
        let moved = (position - follow.saved_ms).abs();
        if follow.finished || (moved < SAVE_EVERY_MS && !(paused && moved >= 1000.0)) {
            return;
        }
        follow.saved_ms = position;
        let db = self.db();
        tauri::async_runtime::spawn(async move {
            let _ = store::set_position(&db, &episode_id, position as i64).await;
        });
    }

    /// How an episode is looked up from what a queue item plays.
    fn key_of(&self, source: &MediaSource) -> Option<Key> {
        match source {
            MediaSource::Url { url, .. } => Some(Key::Audio(url.clone())),
            MediaSource::LocalFile { path } => Path::new(path)
                .strip_prefix(&self.downloads_dir)
                .ok()
                .map(|relative| Key::Download(relative.to_string_lossy().replace('\\', "/"))),
            _ => None,
        }
    }

    /// What a queue item is when it's a podcast episode or a book's chapter (M46.32).
    pub async fn spoken_item(&self, source: &MediaSource) -> Result<Option<SpokenItem>, String> {
        let Some(key) = self.key_of(source) else {
            return Ok(None);
        };
        let db = self.db();
        let found = match key {
            Key::Audio(url) => store::episode_by_audio(&db, &url).await,
            Key::Download(path) => store::episode_by_download(&db, &path).await,
        }
        .map_err(to_message)?;
        let Some(episode_id) = found else {
            return Ok(None);
        };
        let Some(episode) = store::episode(&db, &episode_id).await.map_err(to_message)? else {
            return Ok(None);
        };
        Ok(Some(SpokenItem {
            kind: if episode.podcast_kind == "audiobook" {
                SpokenKind::Audiobook
            } else {
                SpokenKind::Podcast
            },
            podcast_id: episode.podcast_id,
            episode_id,
            show: episode.podcast_title,
            author: episode.podcast_author,
        }))
    }

    /// The episode playing now, if any (M46.32).
    pub fn playing_episode(&self) -> Option<String> {
        self.follow
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .episode_id
            .clone()
    }

    /// Finds which episode (if any) a new queue item plays.
    fn look_up(&self, item: &QueueItem) {
        let Some(key) = self.key_of(&item.source) else {
            return;
        };
        let db = self.db();
        let follow = self.follow.clone();
        let item_id = item.id.clone();
        tauri::async_runtime::spawn(async move {
            let found = match key {
                Key::Audio(url) => store::episode_by_audio(&db, &url).await,
                Key::Download(path) => store::episode_by_download(&db, &path).await,
            };
            if let Ok(Some(episode_id)) = found {
                let mut follow = follow.lock().unwrap_or_else(|p| p.into_inner());
                if follow.item_id.as_deref() == Some(item_id.as_str()) {
                    follow.episode_id = Some(episode_id);
                }
            }
        });
    }

    pub async fn set_played(&self, episode_id: &str, played: bool) -> Result<(), String> {
        store::set_played(&self.db(), episode_id, played.then(now_ms))
            .await
            .map_err(to_message)?;
        let mut follow = self.follow.lock().unwrap_or_else(|p| p.into_inner());
        if follow.episode_id.as_deref() == Some(episode_id) {
            follow.finished = played;
        }
        Ok(())
    }

    // ---- Downloads ----

    /// Downloads an episode in the background; progress arrives as `PodcastsChangedEvent`s.
    pub async fn download(&self, episode_id: &str) -> Result<(), String> {
        let db = self.db();
        let episode = store::episode(&db, episode_id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "This episode was removed".to_owned())?;
        if !self
            .downloading
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(episode_id.to_owned())
        {
            return Ok(());
        }
        let job = DownloadJob {
            app: self.app.clone(),
            client: self.client.clone(),
            db,
            downloads_dir: self.downloads_dir.clone(),
            episode,
        };
        let downloading = self.downloading.clone();
        let id = episode_id.to_owned();
        tauri::async_runtime::spawn(async move {
            job.run().await;
            downloading
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&id);
        });
        Ok(())
    }

    pub async fn remove_download(&self, episode_id: &str) -> Result<(), String> {
        let db = self.db();
        if let Some(path) = store::download(&db, episode_id)
            .await
            .map_err(to_message)?
            .and_then(|download| download.path)
        {
            let _ = tokio::fs::remove_file(self.downloads_dir.join(path)).await;
        }
        store::remove_download(&db, episode_id)
            .await
            .map_err(to_message)?;
        if let Ok(Some(episode)) = store::episode(&db, episode_id).await {
            self.changed(&episode.podcast_id);
        }
        Ok(())
    }

    fn artwork(&self, url: Option<&str>) -> Option<String> {
        url.map(|url| self.images.cover(url, &Default::default()))
    }

    /// A followed show's art: cached for good (M46.2).
    fn kept_artwork(&self, url: Option<&str>) -> Option<String> {
        url.map(|url| self.images.kept_cover(url, &Default::default()))
    }

    fn changed(&self, podcast_id: &str) {
        announce(&self.app, podcast_id);
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().0.clone()
    }
}

enum Key {
    Audio(String),
    Download(String),
}

/// What a directory is asked.
#[derive(Clone, Copy)]
enum Ask<'a> {
    Search(&'a str),
    Top,
}

struct DownloadJob {
    app: AppHandle,
    client: reqwest::Client,
    db: Arc<raygun_db::Db>,
    downloads_dir: PathBuf,
    episode: EpisodeRow,
}

impl DownloadJob {
    async fn run(self) {
        let relative = format!(
            "{FOLDER}/{}/{}.{}",
            self.episode.podcast_id,
            self.episode.id,
            extension(
                self.episode.fields.audio_type.as_deref(),
                &self.episode.fields.audio_url
            )
        );
        let mut record = store::EpisodeDownload {
            episode_id: self.episode.id.clone(),
            state: "downloading".into(),
            path: None,
            received_bytes: 0,
            total_bytes: None,
            error: None,
        };
        let _ = store::put_download(&self.db, &record, now_ms()).await;
        announce(&self.app, &self.episode.podcast_id);
        match self.fetch(&relative, &mut record).await {
            Ok(()) => {
                record.state = "done".into();
                record.path = Some(relative);
                tracing::info!(episode = %self.episode.fields.title, "episode downloaded");
            }
            Err(error) => {
                tracing::warn!(episode = %self.episode.fields.title, %error, "episode download failed");
                record.state = "failed".into();
                record.error = Some(error);
            }
        }
        let _ = store::put_download(&self.db, &record, now_ms()).await;
        announce(&self.app, &self.episode.podcast_id);
    }

    async fn fetch(
        &self,
        relative: &str,
        record: &mut store::EpisodeDownload,
    ) -> Result<(), String> {
        let target = self.downloads_dir.join(relative);
        let partial = target.with_extension("part");
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(to_message)?;
        }
        let response = self
            .client
            .get(&self.episode.fields.audio_url)
            .send()
            .await
            .map_err(|error| format!("Couldn't reach the episode: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("The server answered {}", response.status()));
        }
        record.total_bytes = response.content_length().map(|length| length as i64);
        if response
            .content_length()
            .is_some_and(|length| length > MAX_EPISODE_BYTES)
        {
            return Err("This episode is too large to download".into());
        }
        let mut file = tokio::fs::File::create(&partial)
            .await
            .map_err(to_message)?;
        let mut received: u64 = 0;
        let mut reported: u64 = 0;
        let mut body = response.bytes_stream();
        let result = async {
            while let Some(chunk) = body.next().await {
                let chunk = chunk.map_err(|error| format!("The download broke off: {error}"))?;
                received += chunk.len() as u64;
                if received > MAX_EPISODE_BYTES {
                    return Err("This episode is too large to download".to_owned());
                }
                file.write_all(&chunk).await.map_err(to_message)?;
                if received - reported >= PROGRESS_STEP {
                    reported = received;
                    record.received_bytes = received as i64;
                    let _ = store::put_download(&self.db, record, now_ms()).await;
                    announce(&self.app, &self.episode.podcast_id);
                }
            }
            file.flush().await.map_err(to_message)?;
            Ok(())
        }
        .await;
        if let Err(error) = result {
            let _ = tokio::fs::remove_file(&partial).await;
            return Err(error);
        }
        record.received_bytes = received as i64;
        tokio::fs::rename(&partial, &target)
            .await
            .map_err(to_message)
    }
}

fn announce(app: &AppHandle, podcast_id: &str) {
    if let Err(error) = PodcastsChangedEvent(podcast_id.to_owned()).emit(app) {
        tracing::debug!(%error, "could not announce podcast changes");
    }
}

fn download_view(download: &store::EpisodeDownload) -> EpisodeDownload {
    EpisodeDownload {
        state: match download.state.as_str() {
            "done" => EpisodeDownloadState::Done,
            "failed" => EpisodeDownloadState::Failed,
            _ => EpisodeDownloadState::Downloading,
        },
        received_bytes: download.received_bytes as f64,
        total_bytes: download.total_bytes.map(|bytes| bytes as f64),
        error: download.error.clone(),
    }
}

/// A file extension for a downloaded episode: from its type, else its URL, else `mp3`.
fn extension(audio_type: Option<&str>, url: &str) -> &'static str {
    const KNOWN: [(&str, &str); 9] = [
        ("audio/mpeg", "mp3"),
        ("audio/mp4", "m4a"),
        ("audio/x-m4a", "m4a"),
        ("audio/aac", "aac"),
        ("audio/ogg", "ogg"),
        ("audio/opus", "opus"),
        ("audio/wav", "wav"),
        ("video/mp4", "mp4"),
        ("video/quicktime", "mov"),
    ];
    if let Some((_, extension)) = audio_type.and_then(|kind| {
        KNOWN
            .iter()
            .find(|(known, _)| kind.eq_ignore_ascii_case(known))
    }) {
        return extension;
    }
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_lowercase();
    ["mp3", "m4a", "aac", "ogg", "opus", "wav", "mp4", "mov"]
        .into_iter()
        .find(|extension| path.ends_with(&format!(".{extension}")))
        .unwrap_or("mp3")
}

/// A feed address as typed: `feed://` and bare hosts become https; anything else is refused.
fn normalize_feed_url(text: &str) -> Result<String, String> {
    let text = text.trim();
    let text = text
        .strip_prefix("feed://")
        .map(|rest| format!("https://{rest}"))
        .unwrap_or_else(|| text.to_owned());
    let text = if text.contains("://") {
        text
    } else {
        format!("https://{text}")
    };
    match url::Url::parse(&text) {
        Ok(url) if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() => {
            Ok(url.to_string())
        }
        _ => Err("That isn't a feed address".into()),
    }
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// A book's own title and author from a feed that names the reader's project instead: LibriVox
/// titles its feeds "Pride and Prejudice by Jane Austen (1775 - 1817)" with the author "LibriVox".
fn book_title(title: &str, author: Option<&str>) -> (String, Option<String>) {
    let generic = author.is_none_or(|author| author.trim().eq_ignore_ascii_case("librivox"));
    if generic && let Some((book, writer)) = title.rsplit_once(" by ") {
        let writer = match writer.trim_end().strip_suffix(')') {
            Some(dated) => dated.rsplit_once('(').map_or(writer, |(name, _)| name),
            None => writer,
        }
        .trim();
        if !book.trim().is_empty() && !writer.is_empty() {
            return (book.trim().to_owned(), Some(writer.to_owned()));
        }
    }
    (title.to_owned(), author.map(str::to_owned))
}

/// Two sources' copies of one book (M46.47): the same title and author, ignoring case and marks.
fn same_book(a: (&str, Option<&str>), b: (&str, Option<&str>)) -> bool {
    let simple = |text: &str| {
        text.chars()
            .filter(|letter| letter.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let author = |name: Option<&str>| simple(name.unwrap_or_default());
    simple(a.0) == simple(b.0) && (author(a.1) == author(b.1) || a.1.is_none() || b.1.is_none())
}

#[cfg(test)]
mod tests {
    #[test]
    fn librivox_titles_give_the_book_and_its_author() {
        assert_eq!(
            super::book_title(
                "Pride and Prejudice by Jane Austen (1775 - 1817)",
                Some("LibriVox")
            ),
            (
                "Pride and Prejudice".to_owned(),
                Some("Jane Austen".to_owned())
            )
        );
        assert_eq!(
            super::book_title("Stand by Me", Some("Ben E. King")),
            ("Stand by Me".to_owned(), Some("Ben E. King".to_owned())),
            "a real author is kept"
        );
        assert_eq!(
            super::book_title("Short Poetry Collection 281", Some("LibriVox")),
            (
                "Short Poetry Collection 281".to_owned(),
                Some("LibriVox".to_owned())
            )
        );
    }

    use super::*;

    #[test]
    fn feed_addresses_and_file_types() {
        assert_eq!(
            normalize_feed_url(" feed://example.com/rss ").unwrap(),
            "https://example.com/rss"
        );
        assert_eq!(
            normalize_feed_url("example.com/podcast.xml").unwrap(),
            "https://example.com/podcast.xml"
        );
        assert!(normalize_feed_url("javascript:alert(1)").is_err());
        assert!(normalize_feed_url("file:///etc/passwd").is_err());

        assert_eq!(extension(Some("audio/mpeg"), "https://x/a"), "mp3");
        assert_eq!(extension(None, "https://x/a.M4A?token=1"), "m4a");
        assert_eq!(
            extension(Some("application/octet-stream"), "https://x/a"),
            "mp3"
        );
    }
}
