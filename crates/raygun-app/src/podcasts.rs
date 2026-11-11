//! Podcasts (M37): subscriptions to RSS/Atom feeds, episodes with listening progress, directory
//! search through `podcast.directory` extensions, playback that resumes where listening stopped,
//! and episode downloads under `downloads/podcasts/`. Audiobooks (M46.22) are feeds of chapters
//! kept beside them; M46.48 adds each show's own order and speed, Continue listening, and books
//! from sources that list their chapters without a feed.

mod bulk;
mod directory;
mod order;
mod titles;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use raygun_db::podcasts::{self as store, EpisodeFields, EpisodeRow, PodcastFields, PodcastRow};
use raygun_ext_host::audiobook::{AUDIOBOOK_DIRECTORY, BookPage, parse_listed_key};
use raygun_ext_host::podcast::PODCAST_DIRECTORY;
use raygun_model::{
    AudiobookPage, AudiobookResult, AudiobookSummary, AudiobookView, BookResume, EpisodeDownload,
    EpisodeDownloadState, EpisodeOrder, ListeningItem, MediaKind, MediaSource, PlaybackControl,
    PlaybackState, PlaybackStatus, PodcastEpisode, PodcastPicks, PodcastSearchResult,
    PodcastSummary, PodcastView, QueueItem, QueueItemInput, SpokenItem, SpokenKind,
};
use raygun_net::ImageLinks;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::io::AsyncWriteExt;

use crate::catalog::now_ms;
use crate::events::{AudiobooksFoundEvent, PodcastsChangedEvent};
use crate::extensions::ExtensionService;
use crate::notifications::news::{self, NewsKind};
use crate::playback::PlaybackService;
use crate::podcast_feed::{self, MAX_FEED_BYTES};
use crate::startup::AppDb;
use titles::BookName;

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
/// Episodes downloading at once (M46.48); the rest wait their turn.
const DOWNLOADS_AT_ONCE: usize = 2;
/// Most directory results asked for.
const SEARCH_LIMIT: u32 = 30;
/// Once one audiobook source has answered with books, the others get this long before the list
/// shows without them (they're kept, and the list asks again when they come).
const GRACE: Duration = Duration::from_secs(3);
/// The same once a source has answered without books.
const EMPTY_GRACE: Duration = Duration::from_secs(6);
/// Most Continue listening entries.
const MAX_CONTINUE: u32 = 40;
/// Speeds closer than this are the same.
const SAME_RATE: f64 = 0.01;

/// `remote_kind` of an audiobook (M46.22).
const AUDIOBOOK: &str = "audiobook";

pub struct PodcastService {
    app: AppHandle,
    client: reqwest::Client,
    images: ImageLinks,
    downloads_dir: PathBuf,
    follow: Arc<Mutex<Follow>>,
    downloading: Arc<Mutex<HashSet<String>>>,
    /// Downloads to stop (M46.48).
    cancelled: Arc<Mutex<HashSet<String>>>,
    download_turns: Arc<tokio::sync::Semaphore>,
    /// Audiobook sources' pages, kept a while (M46.48).
    pages: Arc<directory::Pages>,
}

/// What playback is doing to an episode.
#[derive(Debug, Default)]
struct Follow {
    item_id: Option<String>,
    episode_id: Option<String>,
    podcast_id: Option<String>,
    saved_ms: f64,
    finished: bool,
    /// The player's speed at the last state change.
    rate: f64,
    /// The show's own speed, while one of its episodes plays (M46.48).
    show_speed: Option<f64>,
    /// A speed asked of the player that it hasn't reported yet.
    awaiting_rate: Option<f64>,
    /// The player's speed before a show's own took over, to go back to after it.
    rate_before_show: Option<f64>,
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
            cancelled: Arc::default(),
            download_turns: Arc::new(tokio::sync::Semaphore::new(DOWNLOADS_AT_ONCE)),
            pages: Arc::default(),
        }
    }

    /// Refreshes subscriptions in the background when they haven't been for a while, and takes
    /// up downloads a previous run left unfinished.
    pub fn start(self: &Arc<Self>) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            let db = service.db();
            if let Ok(downloads) = store::downloads(&db).await {
                for download in downloads
                    .iter()
                    .filter(|download| matches!(download.state.as_str(), "downloading" | "queued"))
                {
                    if let Err(error) = service.download(&download.episode_id).await {
                        tracing::info!(%error, "an unfinished episode download was dropped");
                        let _ = store::remove_download(&db, &download.episode_id).await;
                    }
                }
            }
            // Feeds are read again once Offline Mode is off.
            raygun_net::network_mode::wait_online().await;
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
        self.episodes_view(rows, None).await
    }

    /// A podcast by its feed address: stored (and refreshed) on first sight, not subscribed.
    pub async fn open_feed(&self, feed_url: &str) -> Result<PodcastView, String> {
        let id = self.open_as(feed_url, None).await?;
        self.view(&id).await
    }

    /// Stores a feed on first sight (as `kind`, when given) and returns its id. A known feed that
    /// can't be read now (offline, the source gone) opens as it was last read.
    async fn open_as(&self, feed_url: &str, kind: Option<&str>) -> Result<String, String> {
        let feed_url = if parse_listed_key(feed_url).is_some() {
            feed_url.trim().to_owned()
        } else {
            normalize_feed_url(feed_url)?
        };
        let db = self.db();
        let known = store::podcast_by_feed(&db, &feed_url)
            .await
            .map_err(to_message)?;
        match known {
            Some(row)
                if row.refreshed_at.is_some_and(|at| now_ms() - at < FRESH_MS)
                    && kind.is_none_or(|kind| kind == row.fields.kind) =>
            {
                Ok(row.id)
            }
            Some(row) => match self.refresh_feed_as(&feed_url, kind).await {
                Ok((id, _)) => Ok(id),
                Err(error) => {
                    tracing::info!(%error, feed = %feed_url, "opened as last read");
                    Ok(row.id)
                }
            },
            None => Ok(self.refresh_feed_as(&feed_url, kind).await?.0),
        }
    }

    // ---- Audiobooks (M46.22) ----

    /// The books in the library, the last listened first.
    pub async fn audiobooks(&self) -> Result<Vec<AudiobookSummary>, String> {
        let rows = store::audiobooks(&self.db()).await.map_err(to_message)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let name = book_name(&row.podcast.fields);
                AudiobookSummary {
                    artwork_url: self.kept_artwork(row.podcast.fields.artwork.as_deref()),
                    id: row.podcast.id,
                    title: name.title,
                    author: name.author,
                    edition: name.edition,
                    chapters: row.chapters as u32,
                    finished: row.finished as u32,
                    total_ms: row.total_ms as f64,
                    listened_ms: row.listened_ms as f64,
                }
            })
            .collect())
    }

    /// Opens a book by its chapter feed (stored as an audiobook on first sight); returns its id.
    pub async fn open_audiobook(&self, feed_url: &str) -> Result<String, String> {
        self.open_as(feed_url, Some(AUDIOBOOK)).await
    }

    /// A book with its chapters, as its order lists them, and where it goes on from.
    pub async fn audiobook(&self, id: &str) -> Result<AudiobookView, String> {
        let db = self.db();
        let row = self.row(id).await?;
        let rows = store::episodes(&db, id).await.map_err(to_message)?;
        let name = book_name(&row.fields);
        let resume = resume_point(&rows).map(|index| BookResume {
            chapter_id: rows[index].id.clone(),
            number: index as u32 + 1,
            title: titles::chapter(&rows[index].fields.title, &name.title),
            position_ms: resume_ms(&rows[index]),
        });
        let chapters = self.episodes_view(rows, Some(&name)).await?;
        let progress = Progress::of(&chapters);
        let order = order::stored(row.episode_order.as_deref(), true);
        Ok(AudiobookView {
            id: row.id,
            feed_url: row.fields.feed_url,
            title: name.title,
            author: name.author,
            edition: name.edition,
            description: row.fields.description,
            artwork_url: if row.subscribed_at.is_some() {
                self.kept_artwork(row.fields.artwork.as_deref())
            } else {
                self.artwork(row.fields.artwork.as_deref())
            },
            link: row.fields.link,
            in_library: row.subscribed_at.is_some(),
            chapter_count: chapters.len() as u32,
            finished: progress.finished,
            downloaded: progress.downloaded,
            downloading: progress.downloading,
            total_ms: progress.total_ms,
            listened_ms: progress.listened_ms,
            resume,
            speed: row.speed,
            custom_order: order != order::default_for(true),
            chapters: order::arrange(chapters, &order),
            order,
        })
    }

    /// Plays a book from `chapter` (else where listening goes on: the chapter being listened, or
    /// the one after the last finished), with the rest of the book queued after it.
    pub async fn play_audiobook(&self, id: &str, chapter: Option<&str>) -> Result<(), String> {
        let db = self.db();
        let row = self.row(id).await?;
        let chapters = store::episodes(&db, id).await.map_err(to_message)?;
        if chapters.is_empty() {
            return Err("This book has no chapters".into());
        }
        let start = match chapter {
            Some(chapter) => chapters
                .iter()
                .position(|row| row.id == chapter)
                .ok_or_else(|| "This chapter is no longer in the book".to_owned())?,
            None => resume_point(&chapters).unwrap_or(0),
        };
        let downloads = self.downloads_by_episode().await?;
        let name = book_name(&row.fields);
        let items = chapters[start..]
            .iter()
            .map(|chapter| self.item_for(chapter, downloads.get(&chapter.id), Some(&name)))
            .collect();
        self.app
            .state::<PlaybackService>()
            .replace_from(items, resume_ms(&chapters[start]));
        Ok(())
    }

    /// A page of books from every audiobook source at once: `popular`, `latest`, `search` or
    /// `subject` (`query` for the last two). A search ranks books by how well they answer it; a
    /// book two sources share shows once. Sources that fail stay quiet while another answers.
    pub async fn audiobook_directory(
        &self,
        list: &str,
        query: &str,
        page: u32,
    ) -> Result<AudiobookPage, String> {
        let query = query.trim().to_owned();
        let page = page.max(1);
        let (providers, waiting) = {
            let extensions = self.app.state::<Arc<ExtensionService>>();
            let (_ready, host) = extensions.ready().await;
            let waiting = host
                .sources_needing_setup()
                .iter()
                .any(|source| source.kinds.iter().any(|kind| kind == AUDIOBOOK_DIRECTORY));
            (host.providers(AUDIOBOOK_DIRECTORY).await, waiting)
        };
        if providers.is_empty() {
            // Sources waiting for their key aren't asked; the page counts them in a line.
            if waiting {
                return Ok(AudiobookPage {
                    books: Vec::new(),
                    has_more: false,
                });
            }
            return Err("No audiobook source is installed".into());
        }
        let mut answers: Vec<Option<Result<BookPage, String>>> = vec![None; providers.len()];
        let mut waiting = FuturesUnordered::new();
        for (index, provider) in providers.iter().enumerate() {
            let ask = directory::Ask {
                provider: provider.id.clone(),
                list: list.to_owned(),
                query: query.to_lowercase(),
                page,
            };
            if let Some(found) = self.pages.get(&ask) {
                answers[index] = Some(Ok(found));
                continue;
            }
            let (app, pages, list, query) = (
                self.app.clone(),
                self.pages.clone(),
                list.to_owned(),
                query.clone(),
            );
            // Its own task, so an answer that comes after the list showed is still kept.
            let task = tauri::async_runtime::spawn(async move {
                let extensions = app.state::<Arc<ExtensionService>>().inner().clone();
                let (_ready, host) = extensions.ready().await;
                let answer = host.audiobooks(&ask.provider, &list, &query, page).await;
                if let Ok(found) = &answer {
                    pages.put(ask, found.clone());
                }
                answer
            });
            waiting.push(async move {
                let answer = task.await.map_err(to_message).and_then(|answer| answer);
                (index, answer)
            });
        }
        // Once a source has answered, the others get a while: less once there are books.
        let grace = |answer: &Result<BookPage, String>| {
            let books = answer.as_ref().is_ok_and(|found| !found.books.is_empty());
            tokio::time::Instant::now() + if books { GRACE } else { EMPTY_GRACE }
        };
        let mut deadline = answers.iter().flatten().map(grace).min();
        while !waiting.is_empty() {
            let next = match deadline {
                Some(at) => match tokio::time::timeout_at(at, waiting.next()).await {
                    Ok(next) => next,
                    Err(_) => break,
                },
                None => waiting.next().await,
            };
            let Some((index, answer)) = next else {
                break;
            };
            let until = grace(&answer);
            deadline = Some(deadline.map_or(until, |at| at.min(until)));
            answers[index] = Some(answer);
        }
        if !waiting.is_empty() {
            // Sources still answering: their books are kept, and the list asks again for them.
            let (app, list, query) = (self.app.clone(), list.to_owned(), query.clone());
            tauri::async_runtime::spawn(async move {
                let mut found = false;
                while let Some((_, answer)) = waiting.next().await {
                    found |= answer.is_ok_and(|page| !page.books.is_empty());
                }
                if found && let Err(error) = AudiobooksFoundEvent(list, query).emit(&app) {
                    tracing::debug!(%error, "could not announce late audiobooks");
                }
            });
        }
        let search = (list == "search" && !query.is_empty()).then_some(query.as_str());
        // What earlier pages listed (they're kept): a later page doesn't list it again.
        let earlier: Vec<directory::Found> = (1..page)
            .flat_map(|before| {
                let pages = providers
                    .iter()
                    .filter_map(|provider| {
                        let ask = directory::Ask {
                            provider: provider.id.clone(),
                            list: list.to_owned(),
                            query: query.to_lowercase(),
                            page: before,
                        };
                        self.pages
                            .get(&ask)
                            .map(|found| (provider.name.clone(), found))
                    })
                    .collect();
                directory::merge(pages, search).0
            })
            .collect();
        let mut pages = Vec::new();
        let mut errors = Vec::new();
        for (provider, answer) in providers.into_iter().zip(answers) {
            match answer {
                Some(Ok(found)) => pages.push((provider.name, found)),
                Some(Err(error)) => errors.push(raygun_net::issue::named(&provider.name, &error)),
                None => {}
            }
        }
        if pages.is_empty() && !errors.is_empty() {
            // The first source's sentence leads; every source's error follows for More info.
            return Err(raygun_net::issue::joined(&errors));
        }
        let (books, has_more) = directory::merge(pages, search);
        let books = directory::after(books, &earlier);
        Ok(AudiobookPage {
            books: books
                .into_iter()
                .map(|found| AudiobookResult {
                    artwork_url: self.artwork(found.book.artwork_url.as_deref()),
                    title: found.name.title,
                    author: found.name.author,
                    edition: found.name.edition,
                    feed_url: found.book.feed_url,
                    runtime: found.book.runtime,
                    language: found.book.language,
                    description: found.book.description,
                    source: found.source,
                })
                .collect(),
            has_more,
        })
    }

    /// The same book at the audiobook sources (Find in other sources): each source searched for
    /// `title`, and the books whose title answers it by the same author (when both say), closest
    /// first. Every source's copy shows, even of a recording two sources share; `own_feed` (the
    /// book itself) doesn't. A source that fails is left out unless all do.
    pub async fn audiobooks_elsewhere(
        &self,
        title: &str,
        author: Option<&str>,
        own_feed: &str,
    ) -> Result<Vec<AudiobookResult>, String> {
        let title = title.trim();
        if title.is_empty() {
            return Ok(Vec::new());
        }
        let extensions = self.app.state::<Arc<ExtensionService>>().inner().clone();
        let (_ready, host) = extensions.ready().await;
        let providers = host.providers(AUDIOBOOK_DIRECTORY).await;
        if providers.is_empty() {
            return Err("No audiobook source is installed".into());
        }
        let answers = futures_util::future::join_all(providers.iter().map(|provider| async move {
            // A search shown in the directory moments ago is the same answer.
            let ask = directory::Ask {
                provider: provider.id.clone(),
                list: "search".into(),
                query: title.to_lowercase(),
                page: 1,
            };
            if let Some(found) = self.pages.get(&ask) {
                return Ok(found);
            }
            let answer = host.audiobooks(&provider.id, "search", title, 1).await;
            if let Ok(found) = &answer {
                self.pages.put(ask, found.clone());
            }
            answer
        }))
        .await;
        let mut pages = Vec::new();
        let mut errors = Vec::new();
        for (provider, answer) in providers.into_iter().zip(answers) {
            match answer {
                Ok(found) => pages.push((provider.name, found)),
                Err(error) => errors.push(raygun_net::issue::named(&provider.name, &error)),
            }
        }
        if pages.is_empty() && !errors.is_empty() {
            return Err(raygun_net::issue::joined(&errors));
        }
        Ok(directory::elsewhere(pages, title, author, own_feed)
            .into_iter()
            .map(|found| AudiobookResult {
                artwork_url: self.artwork(found.book.artwork_url.as_deref()),
                title: found.name.title,
                author: found.name.author,
                edition: found.name.edition,
                feed_url: found.book.feed_url,
                runtime: found.book.runtime,
                language: found.book.language,
                description: found.book.description,
                source: found.source,
            })
            .collect())
    }

    /// Downloads every chapter of a book that isn't yet; returns how many were queued.
    pub async fn download_all(&self, id: &str) -> Result<u32, String> {
        let db = self.db();
        let chapters = store::episodes(&db, id).await.map_err(to_message)?;
        let downloads = self.downloads_by_episode().await?;
        let mut queued = 0;
        for chapter in chapters {
            let done = downloads
                .get(&chapter.id)
                .is_some_and(|download| download.state == "done");
            if !done && !self.is_downloading(&chapter.id) {
                self.download(&chapter.id).await?;
                queued += 1;
            }
        }
        Ok(queued)
    }

    // ---- Continue listening (M46.48) ----

    /// Podcast episodes and books started and not finished, the last listened first.
    pub async fn continue_listening(&self, limit: u32) -> Result<Vec<ListeningItem>, String> {
        let db = self.db();
        let limit = limit.clamp(1, MAX_CONTINUE);
        let mut items: Vec<ListeningItem> = store::episodes_in_progress(&db, limit)
            .await
            .map_err(to_message)?
            .into_iter()
            .map(|row| ListeningItem {
                kind: SpokenKind::Podcast,
                artwork_url: self.artwork(
                    row.fields
                        .artwork
                        .as_deref()
                        .or(row.podcast_artwork.as_deref()),
                ),
                podcast_id: row.podcast_id,
                episode_id: row.id,
                title: row.fields.title,
                show: row.podcast_title,
                author: row.podcast_author,
                position_ms: row.position_ms as f64,
                duration_ms: row.fields.duration_ms.map(|ms| ms as f64),
                chapter: None,
                chapters: None,
                listened_at: row.listened_at.map(|ms| ms as f64),
            })
            .collect();
        for book in store::books_in_progress(&db, limit)
            .await
            .map_err(to_message)?
        {
            let chapters = store::episodes(&db, &book.podcast.id)
                .await
                .map_err(to_message)?;
            let Some(index) = resume_point(&chapters) else {
                continue;
            };
            let name = book_name(&book.podcast.fields);
            let chapter = &chapters[index];
            items.push(ListeningItem {
                kind: SpokenKind::Audiobook,
                artwork_url: self.artwork(book.podcast.fields.artwork.as_deref()),
                podcast_id: book.podcast.id.clone(),
                episode_id: chapter.id.clone(),
                title: titles::chapter(&chapter.fields.title, &name.title),
                show: name.title,
                author: name.author,
                position_ms: resume_ms(chapter),
                duration_ms: chapter.fields.duration_ms.map(|ms| ms as f64),
                chapter: Some(index as u32 + 1),
                chapters: Some(chapters.len() as u32),
                listened_at: book.listened_at.map(|ms| ms as f64),
            });
        }
        items.sort_by(|a, b| {
            b.listened_at
                .unwrap_or(0.0)
                .total_cmp(&a.listened_at.unwrap_or(0.0))
        });
        items.truncate(limit as usize);
        Ok(items)
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
            let pin = page_pin(&row);
            if subscribed {
                crate::quick_access::add(&self.app, pin).await;
            } else {
                crate::quick_access::remove(&self.app, &pin.to, &pin.search).await;
            }
        }
        self.changed(id);
        Ok(())
    }

    // ---- Many at once: a selection's actions ----

    /// Follows or unfollows shows (keeps books in the library or takes them out) together;
    /// returns how many changed.
    pub async fn set_subscribed_many(
        &self,
        ids: &[String],
        subscribed: bool,
    ) -> Result<u32, String> {
        let db = self.db();
        let mut changed = 0;
        let mut seen = HashSet::new();
        for id in ids.iter().filter(|id| seen.insert(id.as_str())) {
            let Some(row) = store::podcast(&db, id).await.map_err(to_message)? else {
                continue;
            };
            if row.subscribed_at.is_some() != subscribed {
                self.set_subscribed(id, subscribed).await?;
                changed += 1;
            }
        }
        Ok(changed)
    }

    /// Marks every episode or chapter of the picks played (finished) or unplayed; returns how
    /// many changed.
    pub async fn set_played_picks(
        &self,
        picks: &PodcastPicks,
        played: bool,
    ) -> Result<u32, String> {
        let db = self.db();
        let gathered = bulk::gather(&db, picks).await.map_err(to_message)?;
        let ids = bulk::to_mark(gathered.all(), played);
        store::set_played_many(&db, &ids, played.then(now_ms))
            .await
            .map_err(to_message)?;
        self.follow_marked(&ids, played);
        for id in gathered.podcast_ids() {
            self.changed(&id);
        }
        Ok(ids.len() as u32)
    }

    /// Downloads the picks: books whole, shows' new episodes and the episodes picked, two at a
    /// time; returns how many were queued.
    pub async fn download_picks(&self, picks: &PodcastPicks) -> Result<u32, String> {
        let gathered = bulk::gather(&self.db(), picks).await.map_err(to_message)?;
        let downloads = self.downloads_by_episode().await?;
        let ids = bulk::to_download(&gathered, &downloads, |id| self.is_downloading(id));
        for id in &ids {
            self.download(id).await?;
        }
        Ok(ids.len() as u32)
    }

    /// Deletes every download of the picks and stops those under way; returns how many went.
    pub async fn remove_downloads_picks(&self, picks: &PodcastPicks) -> Result<u32, String> {
        let gathered = bulk::gather(&self.db(), picks).await.map_err(to_message)?;
        let downloads = self.downloads_by_episode().await?;
        let ids = bulk::with_downloads(&gathered, &downloads, |id| self.is_downloading(id));
        for id in &ids {
            self.remove_download(id).await?;
        }
        Ok(ids.len() as u32)
    }

    /// Queues the picks after what plays (`next`) or at the end: books from where they go on,
    /// shows' newest unplayed episodes and the episodes picked; returns how many were queued.
    pub async fn queue_picks(&self, picks: &PodcastPicks, next: bool) -> Result<u32, String> {
        let gathered = bulk::gather(&self.db(), picks).await.map_err(to_message)?;
        let mut items = Vec::new();
        for id in bulk::to_queue(&gathered) {
            items.push(self.queue_item(&id).await?.0);
        }
        let count = items.len() as u32;
        if count > 0 {
            let playback = self.app.state::<PlaybackService>();
            if next {
                playback.enqueue_next(items);
            } else {
                playback.enqueue(items, false);
            }
        }
        Ok(count)
    }

    /// Pins shows and books to quick access by hand, or unpins them; returns how many changed.
    pub async fn pin_many(&self, ids: &[String], pinned: bool) -> Result<u32, String> {
        let db = self.db();
        let mut pins = Vec::new();
        let mut seen = HashSet::new();
        for id in ids.iter().filter(|id| seen.insert(id.as_str())) {
            if let Some(row) = store::podcast(&db, id).await.map_err(to_message)? {
                pins.push(page_pin(&row));
            }
        }
        Ok(crate::quick_access::set_pinned(&self.app, pins, pinned).await)
    }

    /// How a show's or book's episodes list; `None` (or its kind's own) forgets the change.
    pub async fn set_episode_order(
        &self,
        id: &str,
        order: Option<EpisodeOrder>,
    ) -> Result<(), String> {
        let row = self.row(id).await?;
        let book = row.fields.kind == AUDIOBOOK;
        let order = order
            .map(|order| order::stored(serde_json::to_string(&order).ok().as_deref(), book))
            .filter(|order| *order != order::default_for(book));
        let json = order
            .map(|order| serde_json::to_string(&order))
            .transpose()
            .map_err(to_message)?;
        store::set_episode_order(&self.db(), id, json.as_deref())
            .await
            .map_err(to_message)?;
        self.changed(id);
        Ok(())
    }

    /// The speed a show's or book's episodes play at (`None`: the player's own). It applies at
    /// once when one of them is playing.
    pub async fn set_speed(&self, id: &str, speed: Option<f64>) -> Result<(), String> {
        let speed = speed
            .filter(|speed| speed.is_finite())
            .map(|speed| (speed.clamp(0.5, 2.0) * 100.0).round() / 100.0);
        store::set_speed(&self.db(), id, speed)
            .await
            .map_err(to_message)?;
        let rate = {
            let mut follow = self.follow.lock().unwrap_or_else(|p| p.into_inner());
            if follow.podcast_id.as_deref() != Some(id) {
                None
            } else {
                match speed {
                    Some(speed) => {
                        let current = follow.rate;
                        follow.rate_before_show.get_or_insert(current);
                        follow.show_speed = Some(speed);
                        follow.awaiting_rate = Some(speed);
                        Some(speed)
                    }
                    None => {
                        follow.show_speed = None;
                        let before = follow.rate_before_show.take();
                        follow.awaiting_rate = before;
                        before
                    }
                }
            }
        };
        if let Some(rate) = rate {
            self.set_rate(rate);
        }
        self.changed(id);
        Ok(())
    }

    /// Reads every subscription's feed again; returns how many episodes were new.
    pub async fn refresh_all(&self) -> Result<u32, String> {
        let db = self.db();
        let subscriptions = store::subscriptions(&db).await.map_err(to_message)?;
        let mut added = 0;
        let mut news = Vec::new();
        for subscription in subscriptions {
            match self
                .refresh_feed(&subscription.podcast.fields.feed_url)
                .await
            {
                Ok((id, new)) => {
                    added += new;
                    if new > 0 {
                        let fields = &subscription.podcast.fields;
                        let (kind, title) = if fields.kind == AUDIOBOOK {
                            (NewsKind::Audiobooks, book_name(fields).title)
                        } else {
                            (NewsKind::Podcasts, fields.title.trim().to_owned())
                        };
                        news.push((
                            kind,
                            news::library_notice(
                                kind,
                                &id,
                                &title,
                                new as usize,
                                news::title_route(kind, "", &id),
                            ),
                        ));
                    }
                }
                Err(error) => tracing::info!(
                    podcast = %subscription.podcast.fields.title,
                    %error,
                    "podcast feed refresh failed"
                ),
            }
        }
        let _ = raygun_db::set_local_value(&db, LAST_REFRESH, &now_ms().to_string()).await;
        self.changed("");
        if !news.is_empty() {
            let prefs = crate::settings::load(&db)
                .await
                .map(|settings| settings.notifications)
                .unwrap_or_default();
            news::announce(&self.app, &prefs, news).await;
        }
        Ok(added)
    }

    /// Fetches and stores a feed; returns the podcast's id and how many episodes were new.
    async fn refresh_feed(&self, feed_url: &str) -> Result<(String, u32), String> {
        self.refresh_feed_as(feed_url, None).await
    }

    /// Reads a feed (or a listed book's chapters) and stores it: as `kind` when given, else as it
    /// was (a podcast when new). Titles are stored as the feed has them and tidied when shown.
    async fn refresh_feed_as(
        &self,
        feed_url: &str,
        kind: Option<&str>,
    ) -> Result<(String, u32), String> {
        let (mut podcast, mut episodes) = match parse_listed_key(feed_url) {
            Some((provider, book)) => self.listed_book(feed_url, provider, book).await?,
            None => {
                let bytes = self.fetch_feed(feed_url).await?;
                let url = feed_url.to_owned();
                tauri::async_runtime::spawn_blocking(move || podcast_feed::parse(&bytes, &url))
                    .await
                    .map_err(to_message)??
            }
        };
        let db = self.db();
        podcast.kind = match kind {
            Some(kind) => kind.to_owned(),
            None => store::podcast_by_feed(&db, feed_url)
                .await
                .map_err(to_message)?
                .map_or(podcast.kind, |row| row.fields.kind),
        };
        if podcast.kind == AUDIOBOOK {
            number_chapters(&mut episodes);
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

    /// A book from a source that lists its chapters itself (M46.48), stored under `key`.
    async fn listed_book(
        &self,
        key: &str,
        provider: &str,
        book: &str,
    ) -> Result<(PodcastFields, Vec<EpisodeFields>), String> {
        let details = {
            let extensions = self.app.state::<Arc<ExtensionService>>();
            let (_ready, host) = extensions.ready().await;
            let name = host
                .providers(AUDIOBOOK_DIRECTORY)
                .await
                .into_iter()
                .find(|known| known.id == provider)
                .map(|known| known.name)
                .ok_or_else(|| "This book's source isn't installed or is turned off".to_owned())?;
            host.audiobook_details(provider, book)
                .await
                .map_err(|error| format!("{name} didn't send the book's chapters: {error}"))?
        };
        let podcast = PodcastFields {
            feed_url: key.to_owned(),
            title: details.title,
            author: details.author,
            description: details.description.as_deref().map(podcast_feed::clean_text),
            artwork: details.artwork_url,
            link: details.link,
            kind: AUDIOBOOK.into(),
        };
        let episodes = details
            .chapters
            .into_iter()
            .enumerate()
            .map(|(index, chapter)| EpisodeFields {
                guid: chapter.id,
                title: if chapter.title.is_empty() {
                    format!("Chapter {}", index + 1)
                } else {
                    chapter.title
                },
                description: chapter.description.as_deref().map(podcast_feed::clean_text),
                audio_url: chapter.audio_url,
                audio_type: chapter.audio_type,
                duration_ms: chapter.duration_ms,
                published_at: None,
                artwork: None,
                number: Some(index as i64 + 1),
                transcript: None,
            })
            .collect();
        Ok((podcast, episodes))
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
        let rows = store::episodes(&db, id).await.map_err(to_message)?;
        let count = rows.len() as u32;
        let episodes = self.episodes_view(rows, None).await?;
        let downloaded = episodes
            .iter()
            .filter(|episode| episode.download.is_some())
            .count() as u32;
        let order = order::stored(row.episode_order.as_deref(), false);
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
            custom_order: order != order::default_for(false),
            episodes: order::arrange(episodes, &order),
            order,
            episode_count: count,
            downloaded,
            speed: row.speed,
        })
    }

    /// Episodes as lists show them; a book's chapters (`book`) get tidied titles and their place.
    async fn episodes_view(
        &self,
        rows: Vec<EpisodeRow>,
        book: Option<&BookName>,
    ) -> Result<Vec<PodcastEpisode>, String> {
        let downloads = self.downloads_by_episode().await?;
        Ok(rows
            .into_iter()
            .enumerate()
            .map(|(index, row)| {
                let download = downloads.get(&row.id).map(download_view);
                let (title, podcast_title, number) = match book {
                    Some(book) => (
                        titles::chapter(&row.fields.title, &book.title),
                        book.title.clone(),
                        Some(index as u32 + 1),
                    ),
                    None => (
                        row.fields.title,
                        row.podcast_title,
                        row.fields
                            .number
                            .and_then(|number| u32::try_from(number).ok()),
                    ),
                };
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
                    podcast_title,
                    title,
                    description: row.fields.description,
                    published_at: row.fields.published_at.map(|ms| ms as f64),
                    duration_ms: row.fields.duration_ms.map(|ms| ms as f64),
                    position_ms: row.position_ms as f64,
                    played: row.played_at.is_some(),
                    download,
                    number,
                }
            })
            .collect())
    }

    async fn downloads_by_episode(
        &self,
    ) -> Result<HashMap<String, store::EpisodeDownload>, String> {
        Ok(store::downloads(&self.db())
            .await
            .map_err(to_message)?
            .into_iter()
            .map(|download| (download.episode_id.clone(), download))
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
            // Directories waiting for their key aren't asked, and nothing failed: the page
            // counts them in a line of its own.
            let waiting = host
                .sources_needing_setup()
                .iter()
                .any(|source| source.kinds.iter().any(|kind| kind == PODCAST_DIRECTORY));
            if waiting {
                return Ok(Vec::new());
            }
            return Err("No podcast directory is installed".into());
        }
        // Every directory at once (M46.48), each in the order it ranked its shows.
        let answers = futures_util::future::join_all(providers.iter().map(|provider| async {
            match ask {
                Ask::Search(query) => host.podcast_search(&provider.id, query, SEARCH_LIMIT).await,
                Ask::Top => host.podcast_top(&provider.id, SEARCH_LIMIT).await,
            }
        }))
        .await;
        let mut results: Vec<PodcastSearchResult> = Vec::new();
        let mut errors = Vec::new();
        for (provider, found) in providers.into_iter().zip(answers) {
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
                            title: podcast_feed::plain_line(&result.title),
                            author: result.author.as_deref().map(podcast_feed::plain_line),
                            feed_url: result.feed_url,
                            description: result.description,
                            subscribed: false,
                        });
                    }
                }
                // It just said it needs a key: counted with the sources waiting for one.
                Err(error) if raygun_ext_host::needs_setup_in(&error.to_string()).is_some() => {}
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
        let book = (episode.podcast_kind == AUDIOBOOK)
            .then(|| titles::book(&episode.podcast_title, episode.podcast_author.as_deref()));
        let item = self.item_for(&episode, download.as_ref(), book.as_ref());
        Ok((item, resume_ms(&episode)))
    }

    /// An episode as a queue item: its download when there is one; a book's chapter (`book`)
    /// plays as its author's, on an album named after the book.
    fn item_for(
        &self,
        episode: &EpisodeRow,
        download: Option<&store::EpisodeDownload>,
        book: Option<&BookName>,
    ) -> QueueItemInput {
        let local = download
            .filter(|download| download.state == "done")
            .and_then(|download| download.path.as_deref())
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
        let video = episode
            .fields
            .audio_type
            .as_deref()
            .is_some_and(|kind| kind.starts_with("video/"));
        let (title, artist, album) = match book {
            Some(book) => (
                titles::chapter(&episode.fields.title, &book.title),
                book.author.clone().or_else(|| Some(book.title.clone())),
                Some(book.title.clone()),
            ),
            None => (
                episode.fields.title.clone(),
                Some(episode.podcast_title.clone()),
                None,
            ),
        };
        QueueItemInput {
            explicit: false,
            title,
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
        }
    }

    /// Follows playback: saves the position of a playing episode, marks it played near the end,
    /// and plays each show at its own speed (M46.48). Called with every state change, so it does
    /// nothing slow itself.
    pub fn on_playback(&self, state: &PlaybackState, item: Option<&QueueItem>) {
        let Some(item) = item else {
            return;
        };
        let mut follow = self.follow.lock().unwrap_or_else(|p| p.into_inner());
        if follow.item_id.as_deref() != Some(item.id.as_str()) {
            *follow = Follow {
                item_id: Some(item.id.clone()),
                saved_ms: state.position_ms,
                rate: state.rate,
                rate_before_show: follow.rate_before_show,
                ..Follow::default()
            };
            drop(follow);
            self.look_up(item);
            return;
        }
        follow.rate = state.rate;
        match follow.awaiting_rate {
            Some(wanted) if (state.rate - wanted).abs() < SAME_RATE => follow.awaiting_rate = None,
            Some(_) => {}
            None => {
                // The listener changed the speed while a show with its own plays: it keeps it.
                if let (Some(speed), Some(podcast_id)) =
                    (follow.show_speed, follow.podcast_id.clone())
                    && (state.rate - speed).abs() >= SAME_RATE
                {
                    follow.show_speed = Some(state.rate);
                    let (db, rate) = (self.db(), state.rate);
                    tauri::async_runtime::spawn(async move {
                        let _ = store::set_speed(&db, &podcast_id, Some(rate)).await;
                    });
                }
            }
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
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            // Incognito (M47) may have come on since: nothing is saved or announced then.
            if crate::privacy::incognito(&app) {
                return;
            }
            // Shown progress follows a pause at once; while it plays, views read it when asked.
            if store::set_position(&db, &episode_id, position as i64, now_ms())
                .await
                .is_ok()
                && paused
            {
                announce(&app, "");
            }
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
        let book = episode.podcast_kind == AUDIOBOOK;
        let (show, author) = if book {
            let name = titles::book(&episode.podcast_title, episode.podcast_author.as_deref());
            (name.title, name.author)
        } else {
            (episode.podcast_title, episode.podcast_author)
        };
        Ok(Some(SpokenItem {
            kind: if book {
                SpokenKind::Audiobook
            } else {
                SpokenKind::Podcast
            },
            podcast_id: episode.podcast_id,
            episode_id,
            show,
            author,
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

    /// Finds which episode (if any) a new queue item plays, and puts the player at its show's
    /// speed, or back at the speed it had before a show's own.
    fn look_up(&self, item: &QueueItem) {
        let key = self.key_of(&item.source);
        let db = self.db();
        let follow = self.follow.clone();
        let app = self.app.clone();
        let item_id = item.id.clone();
        tauri::async_runtime::spawn(async move {
            let found = match key {
                Some(Key::Audio(url)) => store::episode_by_audio(&db, &url).await.ok().flatten(),
                Some(Key::Download(path)) => {
                    store::episode_by_download(&db, &path).await.ok().flatten()
                }
                None => None,
            };
            let episode = match found {
                Some(id) => store::episode(&db, &id).await.ok().flatten(),
                None => None,
            };
            let rate = {
                let mut follow = follow.lock().unwrap_or_else(|p| p.into_inner());
                if follow.item_id.as_deref() != Some(item_id.as_str()) {
                    return;
                }
                let speed = episode.as_ref().and_then(|episode| episode.podcast_speed);
                if let Some(episode) = episode {
                    follow.episode_id = Some(episode.id);
                    follow.podcast_id = Some(episode.podcast_id);
                }
                match speed {
                    Some(speed) => {
                        let current = follow.rate;
                        follow.rate_before_show.get_or_insert(current);
                        follow.show_speed = Some(speed);
                        ((current - speed).abs() >= SAME_RATE).then(|| {
                            follow.awaiting_rate = Some(speed);
                            speed
                        })
                    }
                    None => follow.rate_before_show.take().filter(|before| {
                        let differs = (follow.rate - before).abs() >= SAME_RATE;
                        if differs {
                            follow.awaiting_rate = Some(*before);
                        }
                        differs
                    }),
                }
            };
            if let Some(rate) = rate {
                app.state::<PlaybackService>()
                    .control(PlaybackControl::SetRate { rate });
            }
        });
    }

    fn set_rate(&self, rate: f64) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            app.state::<PlaybackService>()
                .control(PlaybackControl::SetRate { rate });
        });
    }

    pub async fn set_played(&self, episode_id: &str, played: bool) -> Result<(), String> {
        store::set_played(&self.db(), episode_id, played.then(now_ms))
            .await
            .map_err(to_message)?;
        self.follow_marked(&[episode_id.to_owned()], played);
        Ok(())
    }

    /// Marks every episode of a show (or chapter of a book) played or unplayed (M46.48).
    pub async fn set_all_played(&self, id: &str, played: bool) -> Result<(), String> {
        let db = self.db();
        let ids: Vec<String> = store::episodes(&db, id)
            .await
            .map_err(to_message)?
            .into_iter()
            .filter(|row| row.played_at.is_some() != played)
            .map(|row| row.id)
            .collect();
        store::set_played_many(&db, &ids, played.then(now_ms))
            .await
            .map_err(to_message)?;
        self.follow_marked(&ids, played);
        self.changed(id);
        Ok(())
    }

    /// Marks the episodes before this one played: a book's earlier chapters, or a show's older
    /// episodes (M46.48). Returns how many changed.
    pub async fn set_earlier_played(&self, episode_id: &str) -> Result<u32, String> {
        let db = self.db();
        let episode = store::episode(&db, episode_id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "This episode was removed".to_owned())?;
        let rows = store::episodes(&db, &episode.podcast_id)
            .await
            .map_err(to_message)?;
        let at = rows
            .iter()
            .position(|row| row.id == episode_id)
            .unwrap_or_default();
        // A book's chapters are stored in order; a show's episodes newest first.
        let earlier: Vec<String> = if episode.podcast_kind == AUDIOBOOK {
            rows[..at].iter()
        } else {
            rows[at + 1..].iter()
        }
        .filter(|row| row.played_at.is_none())
        .map(|row| row.id.clone())
        .collect();
        store::set_played_many(&db, &earlier, Some(now_ms()))
            .await
            .map_err(to_message)?;
        self.follow_marked(&earlier, true);
        self.changed(&episode.podcast_id);
        Ok(earlier.len() as u32)
    }

    /// Forgets where listening stopped in an episode, without marking it played (M46.48).
    pub async fn forget_position(&self, episode_id: &str) -> Result<(), String> {
        store::clear_position(&self.db(), episode_id)
            .await
            .map_err(to_message)?;
        self.changed("");
        Ok(())
    }

    /// The playing episode was marked: it isn't marked again at its end, or is once unmarked.
    fn follow_marked(&self, ids: &[String], played: bool) {
        let mut follow = self.follow.lock().unwrap_or_else(|p| p.into_inner());
        if follow
            .episode_id
            .as_ref()
            .is_some_and(|playing| ids.contains(playing))
        {
            follow.finished = played;
        }
    }

    // ---- Downloads ----

    /// Downloads an episode in the background, two at a time; progress arrives as
    /// `PodcastsChangedEvent`s.
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
        self.cancelled
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(episode_id);
        let waiting = store::EpisodeDownload {
            episode_id: episode.id.clone(),
            state: "queued".into(),
            path: None,
            received_bytes: 0,
            total_bytes: None,
            error: None,
        };
        let _ = store::put_download(&db, &waiting, now_ms()).await;
        announce(&self.app, &episode.podcast_id);
        let job = DownloadJob {
            app: self.app.clone(),
            client: self.client.clone(),
            db,
            downloads_dir: self.downloads_dir.clone(),
            cancelled: self.cancelled.clone(),
            episode,
        };
        let downloading = self.downloading.clone();
        let turns = self.download_turns.clone();
        let id = episode_id.to_owned();
        tauri::async_runtime::spawn(async move {
            if let Ok(_turn) = turns.acquire_owned().await {
                // Offline Mode pauses downloads: one it stopped goes on once it's off.
                loop {
                    raygun_net::network_mode::wait_online().await;
                    if !job.run().await {
                        break;
                    }
                }
            }
            downloading
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&id);
        });
        Ok(())
    }

    fn is_downloading(&self, episode_id: &str) -> bool {
        self.downloading
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(episode_id)
    }

    /// Whether an episode's download runs or waits its turn in this run, and wasn't cancelled:
    /// a record left "downloading" by a run that ended isn't.
    pub fn download_active(&self, episode_id: &str) -> bool {
        self.is_downloading(episode_id)
            && !self
                .cancelled
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .contains(episode_id)
    }

    /// Stops a download that's waiting or under way; what it fetched is deleted (M46.48). One
    /// nothing runs (a record left behind) is just forgotten.
    pub async fn cancel_download(&self, episode_id: &str) -> Result<(), String> {
        self.remove_download(episode_id).await
    }

    /// Deletes a downloaded episode, or stops its download (which then leaves nothing behind).
    pub async fn remove_download(&self, episode_id: &str) -> Result<(), String> {
        let db = self.db();
        if self.is_downloading(episode_id) {
            self.cancelled
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(episode_id.to_owned());
            // Said now, not when the job notices: its notification and row go at once.
            if let Ok(Some(episode)) = store::episode(&db, episode_id).await {
                self.changed(&episode.podcast_id);
            }
            return Ok(());
        }
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

    /// Deletes every downloaded episode of a show or chapter of a book, and stops those under
    /// way (M46.48).
    pub async fn remove_downloads(&self, id: &str) -> Result<(), String> {
        let db = self.db();
        let downloads = self.downloads_by_episode().await?;
        for episode in store::episodes(&db, id).await.map_err(to_message)? {
            if downloads.contains_key(&episode.id) || self.is_downloading(&episode.id) {
                self.remove_download(&episode.id).await?;
            }
        }
        self.changed(id);
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

/// How far a book was listened, and how much of it is downloaded, from its chapters.
struct Progress {
    finished: u32,
    total_ms: f64,
    listened_ms: f64,
    downloaded: u32,
    downloading: u32,
}

impl Progress {
    fn of(chapters: &[PodcastEpisode]) -> Self {
        let mut progress = Progress {
            finished: 0,
            total_ms: 0.0,
            listened_ms: 0.0,
            downloaded: 0,
            downloading: 0,
        };
        for chapter in chapters {
            match chapter.download.as_ref().map(|download| download.state) {
                Some(EpisodeDownloadState::Done) => progress.downloaded += 1,
                Some(EpisodeDownloadState::Downloading | EpisodeDownloadState::Queued) => {
                    progress.downloading += 1;
                }
                _ => {}
            }
            let length = chapter.duration_ms.unwrap_or(0.0);
            progress.total_ms += length;
            if chapter.played {
                progress.finished += 1;
                progress.listened_ms += length;
            } else {
                progress.listened_ms += chapter.position_ms.min(length.max(chapter.position_ms));
            }
        }
        progress
    }
}

/// A book's name as listeners read it (M46.48).
fn book_name(fields: &PodcastFields) -> BookName {
    titles::book(&fields.title, fields.author.as_deref())
}

/// A show's or book's page, for quick access.
fn page_pin(row: &PodcastRow) -> crate::quick_access::QuickPin {
    let (kind, to, name) = if row.fields.kind == AUDIOBOOK {
        ("audiobook", "/audiobook", book_name(&row.fields).title)
    } else {
        ("podcast", "/podcast", row.fields.title.clone())
    };
    crate::quick_access::page_pin(kind, to, &row.id, &name, row.fields.artwork.clone())
}

/// Where a book goes on from, as a place in its chapters: the chapter listened last if it isn't
/// finished, else the next one not finished after the chapter finished last; `None` when every
/// chapter is finished.
fn resume_point(chapters: &[EpisodeRow]) -> Option<usize> {
    let latest = |row: &EpisodeRow| row.listened_at.or(row.played_at).unwrap_or(0);
    let listening = chapters
        .iter()
        .enumerate()
        .filter(|(_, row)| row.played_at.is_none() && row.position_ms > 0)
        .max_by_key(|(index, row)| (latest(row), *index));
    let finished = chapters
        .iter()
        .enumerate()
        .filter(|(_, row)| row.played_at.is_some())
        .max_by_key(|(index, row)| (latest(row), *index));
    match (listening, finished) {
        (Some((index, row)), Some((_, done))) if latest(row) >= latest(done) => Some(index),
        (Some((index, _)), None) => Some(index),
        (_, Some((done, _))) => chapters[done + 1..]
            .iter()
            .position(|row| row.played_at.is_none())
            .map(|after| done + 1 + after)
            .or_else(|| chapters.iter().position(|row| row.played_at.is_none())),
        (None, None) => (!chapters.is_empty()).then_some(0),
    }
}

/// Where an episode resumes: where listening stopped, or its start once played (or nearly).
fn resume_ms(episode: &EpisodeRow) -> f64 {
    let duration = episode.fields.duration_ms.unwrap_or(0) as f64;
    let position = episode.position_ms as f64;
    if episode.played_at.is_some() || (duration > 0.0 && position >= duration * PLAYED_SHARE) {
        0.0
    } else {
        position
    }
}

/// A book feed without chapter numbers (Loyal Books') lists its chapters in order: they get
/// their places, which its made-up dates would get backwards.
fn number_chapters(episodes: &mut [EpisodeFields]) {
    if episodes.iter().all(|episode| episode.number.is_none()) {
        for (index, episode) in episodes.iter_mut().enumerate() {
            episode.number = Some(index as i64 + 1);
        }
    }
}

struct DownloadJob {
    app: AppHandle,
    client: reqwest::Client,
    db: Arc<raygun_db::Db>,
    downloads_dir: PathBuf,
    cancelled: Arc<Mutex<HashSet<String>>>,
    episode: EpisodeRow,
}

/// Why a download stopped.
enum Stop {
    Cancelled,
    Failed(String),
}

impl DownloadJob {
    /// Fetches the episode; true when Offline Mode stopped it, to go on once it's off.
    async fn run(&self) -> bool {
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
        if self.is_cancelled() {
            self.forget().await;
            return false;
        }
        let _ = store::put_download(&self.db, &record, now_ms()).await;
        announce(&self.app, &self.episode.podcast_id);
        match self.fetch(&relative, &mut record).await {
            Ok(()) => {
                record.state = "done".into();
                record.path = Some(relative);
                tracing::info!(episode = %self.episode.fields.title, "episode downloaded");
            }
            Err(Stop::Cancelled) => {
                self.forget().await;
                return false;
            }
            // Offline Mode pauses it: it waits in the queue for Offline Mode to end.
            Err(Stop::Failed(_)) if raygun_net::network_mode::offline() => {
                record.state = "queued".into();
                let _ = store::put_download(&self.db, &record, now_ms()).await;
                announce(&self.app, &self.episode.podcast_id);
                return true;
            }
            Err(Stop::Failed(error)) => {
                tracing::info!(episode = %self.episode.fields.title, %error, "episode file failed; capturing its stream");
                // The last resort (M46.49): the stream that plays it, captured.
                match self.capture(&mut record).await {
                    Ok(relative) => {
                        record.state = "done".into();
                        record.path = Some(relative);
                        tracing::info!(episode = %self.episode.fields.title, "episode captured");
                    }
                    Err(Stop::Cancelled) => {
                        self.forget().await;
                        return false;
                    }
                    Err(Stop::Failed(capture)) => {
                        tracing::warn!(episode = %self.episode.fields.title, %error, %capture, "episode download failed");
                        record.state = "failed".into();
                        record.error = Some(format!(
                            "{error}. Capturing it didn't work either: {capture}"
                        ));
                    }
                }
            }
        }
        let _ = store::put_download(&self.db, &record, now_ms()).await;
        announce(&self.app, &self.episode.podcast_id);
        false
    }

    /// Captures the episode's stream (M46.49): its bytes through the stream proxy by ranges, else
    /// its sound as it decodes. Returns the file's path under the downloads folder.
    async fn capture(&self, record: &mut store::EpisodeDownload) -> Result<String, Stop> {
        use crate::music_downloads::capture;

        let dir = self
            .downloads_dir
            .join(FOLDER)
            .join(&self.episode.podcast_id);
        let stop = Arc::new(tokio::sync::Notify::new());
        // Cancelling stops the capture too.
        let watcher = {
            let (cancelled, id, stop) = (
                self.cancelled.clone(),
                self.episode.id.clone(),
                stop.clone(),
            );
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    if cancelled
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .contains(&id)
                    {
                        stop.notify_one();
                        return;
                    }
                }
            })
        };
        let quality = crate::settings::load(&self.db)
            .await
            .unwrap_or_default()
            .audio_quality
            .download;
        let source = raygun_model::MediaSource::Url {
            url: self.episode.fields.audio_url.clone(),
            headers: Default::default(),
        };
        let (app, podcast_id) = (self.app.clone(), self.episode.podcast_id.clone());
        let last = std::sync::Mutex::new(std::time::Instant::now());
        let progress = |bytes: u64, _share: Option<f64>| {
            let mut told = last.lock().unwrap_or_else(|p| p.into_inner());
            if bytes > 0 && told.elapsed() >= Duration::from_secs(1) {
                *told = std::time::Instant::now();
                announce(&app, &podcast_id);
            }
        };
        let stem = format!("{}.captured", self.episode.id);
        let captured = capture::capture(
            &self.app,
            &self.client,
            capture::Request {
                source: &source,
                dir: &dir,
                stem: &stem,
                expected: capture::Expected {
                    duration_ms: self
                        .episode
                        .fields
                        .duration_ms
                        .and_then(|ms| u64::try_from(ms).ok()),
                    start_ms: 0,
                    end_ms: None,
                },
                quality,
                tags: Some(crate::music_downloads::transcode::SongTags {
                    title: self.episode.fields.title.clone(),
                    artist: None,
                    album: None,
                    cover: None,
                }),
            },
            &progress,
            &stop,
        )
        .await;
        watcher.abort();
        let captured = captured.map_err(|error| match error {
            capture::CaptureError::Stopped => Stop::Cancelled,
            capture::CaptureError::Device(error)
            | capture::CaptureError::Failed(error)
            | capture::CaptureError::Partial { why: error, .. } => Stop::Failed(error),
            // Only YouTube's player's captures are interrupted, and episodes never are those.
            capture::CaptureError::Interrupted => Stop::Failed("The capture stopped".into()),
        })?;
        record.received_bytes = tokio::fs::metadata(dir.join(&captured.file))
            .await
            .map(|meta| meta.len() as i64)
            .unwrap_or(0);
        record.total_bytes = Some(record.received_bytes);
        Ok(format!(
            "{FOLDER}/{}/{}",
            self.episode.podcast_id, captured.file
        ))
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(&self.episode.id)
    }

    /// A cancelled download leaves nothing behind.
    async fn forget(&self) {
        self.cancelled
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.episode.id);
        let _ = store::remove_download(&self.db, &self.episode.id).await;
        announce(&self.app, &self.episode.podcast_id);
    }

    async fn fetch(&self, relative: &str, record: &mut store::EpisodeDownload) -> Result<(), Stop> {
        let failed = Stop::Failed;
        let target = self.downloads_dir.join(relative);
        let partial = target.with_extension("part");
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| failed(error.to_string()))?;
        }
        // A connected server's chapters sign in (Audiobookshelf).
        let response = raygun_net::server_auth::get(
            &self.client,
            &self.episode.fields.audio_url,
            &Default::default(),
        )
        .send()
        .await
        .map_err(|error| failed(format!("Couldn't reach the episode: {error}")))?;
        if !response.status().is_success() {
            return Err(failed(format!("The server answered {}", response.status())));
        }
        record.total_bytes = response.content_length().map(|length| length as i64);
        if response
            .content_length()
            .is_some_and(|length| length > MAX_EPISODE_BYTES)
        {
            return Err(failed("This episode is too large to download".into()));
        }
        let mut file = tokio::fs::File::create(&partial)
            .await
            .map_err(|error| failed(error.to_string()))?;
        let mut received: u64 = 0;
        let mut reported: u64 = 0;
        let mut body = response.bytes_stream();
        let result = async {
            while let Some(chunk) = body.next().await {
                if self.is_cancelled() {
                    return Err(Stop::Cancelled);
                }
                let chunk =
                    chunk.map_err(|error| failed(format!("The download broke off: {error}")))?;
                received += chunk.len() as u64;
                if received > MAX_EPISODE_BYTES {
                    return Err(failed("This episode is too large to download".to_owned()));
                }
                file.write_all(&chunk)
                    .await
                    .map_err(|error| failed(error.to_string()))?;
                if received - reported >= PROGRESS_STEP {
                    reported = received;
                    record.received_bytes = received as i64;
                    let _ = store::put_download(&self.db, record, now_ms()).await;
                    announce(&self.app, &self.episode.podcast_id);
                }
            }
            file.flush()
                .await
                .map_err(|error| failed(error.to_string()))?;
            Ok(())
        }
        .await;
        if let Err(stop) = result {
            drop(file);
            let _ = tokio::fs::remove_file(&partial).await;
            return Err(stop);
        }
        record.received_bytes = received as i64;
        tokio::fs::rename(&partial, &target)
            .await
            .map_err(|error| failed(error.to_string()))
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
            "queued" => EpisodeDownloadState::Queued,
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

#[cfg(test)]
mod tests {
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

    fn chapter(
        id: &str,
        played_at: Option<i64>,
        position_ms: i64,
        listened_at: Option<i64>,
    ) -> EpisodeRow {
        EpisodeRow {
            id: id.into(),
            podcast_id: "b".into(),
            fields: EpisodeFields {
                guid: id.into(),
                title: id.into(),
                description: None,
                audio_url: format!("https://x/{id}.mp3"),
                audio_type: None,
                duration_ms: Some(600_000),
                published_at: None,
                artwork: None,
                number: None,
                transcript: None,
            },
            position_ms,
            played_at,
            found_at: 0,
            listened_at,
            podcast_title: "Book".into(),
            podcast_author: None,
            podcast_artwork: None,
            podcast_kind: AUDIOBOOK.into(),
            podcast_speed: None,
        }
    }

    #[test]
    fn a_book_goes_on_where_it_was_listened() {
        let fresh = [chapter("1", None, 0, None), chapter("2", None, 0, None)];
        assert_eq!(resume_point(&fresh), Some(0), "nothing listened: the start");
        let in_order = [
            chapter("1", Some(10), 0, Some(10)),
            chapter("2", Some(20), 0, Some(20)),
            chapter("3", None, 0, None),
        ];
        assert_eq!(resume_point(&in_order), Some(2), "after the last finished");
        let jumped = [
            chapter("1", None, 0, None),
            chapter("2", None, 0, None),
            chapter("3", None, 90_000, Some(30)),
        ];
        assert_eq!(resume_point(&jumped), Some(2), "the chapter being listened");
        let went_back = [
            chapter("1", None, 5_000, Some(5)),
            chapter("2", Some(40), 0, Some(40)),
            chapter("3", None, 0, None),
        ];
        assert_eq!(
            resume_point(&went_back),
            Some(2),
            "a chapter finished since beats an old place"
        );
        let finished_last = [
            chapter("1", None, 0, None),
            chapter("2", Some(9), 0, Some(9)),
        ];
        assert_eq!(
            resume_point(&finished_last),
            Some(0),
            "wraps round to what's left"
        );
        assert_eq!(resume_point(&[chapter("1", Some(1), 0, Some(1))]), None);
        assert_eq!(resume_point(&[]), None);

        assert_eq!(resume_ms(&jumped[2]), 90_000.0);
        assert_eq!(
            resume_ms(&chapter("x", None, 590_000, None)),
            0.0,
            "nearly done starts over"
        );
    }

    #[test]
    fn unnumbered_book_feeds_number_their_chapters_in_order() {
        let mut chapters: Vec<EpisodeFields> = ["a", "b"]
            .iter()
            .map(|id| chapter(id, None, 0, None).fields)
            .collect();
        number_chapters(&mut chapters);
        assert_eq!(
            chapters.iter().map(|c| c.number).collect::<Vec<_>>(),
            [Some(1), Some(2)]
        );
        chapters[0].number = Some(7);
        chapters[1].number = None;
        number_chapters(&mut chapters);
        assert_eq!(chapters[0].number, Some(7), "a feed's own numbers stay");
    }

    /// Loyal Books' feed for Alice (November 2026, trimmed): made-up dates newest first, its
    /// chapters numbered in their titles, the publisher as the author.
    const LOYAL_ALICE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" version="2.0"><channel>
<title>Alice's Adventures in Wonderland by Lewis Carroll</title>
<itunes:author>Loyal Books</itunes:author>
<item><title>00 - Dramatis Personae</title><pubDate>Fri, 02 Jan 2026 00:00:00 +0000</pubDate>
<enclosure url="http://www.archive.org/download/aliceinwonderland_1102_librivox/aliceinwonderland_00_carroll_64kb.mp3" type="audio/mpeg"/><itunes:duration>03:58</itunes:duration></item>
<item><title>01 - Down the Rabbit-Hole</title><pubDate>Thu, 01 Jan 2026 00:00:00 +0000</pubDate>
<enclosure url="http://www.archive.org/download/aliceinwonderland_1102_librivox/aliceinwonderland_01_carroll_64kb.mp3" type="audio/mpeg"/><itunes:duration>12:20</itunes:duration></item>
<item><title>12 - Alice's Evidence</title><pubDate>Wed, 31 Dec 2025 00:00:00 +0000</pubDate>
<enclosure url="http://www.archive.org/download/aliceinwonderland_1102_librivox/aliceinwonderland_12_carroll_64kb.mp3" type="audio/mpeg"/></item>
</channel></rss>"#;

    /// LibriVox's feed for The Count of Monte Cristo (`librivox.org/rss/47`, trimmed).
    const LIBRIVOX_MONTE_CRISTO: &str = r#"<?xml version="1.0" ?>
<rss xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" version="2.0"><channel>
<title><![CDATA[Count of Monte Cristo, The by Alexandre Dumas (1802 - 1870)]]></title>
<itunes:type>serial</itunes:type><itunes:author>LibriVox</itunes:author>
<item><title><![CDATA[Marseilles–The Arrival ]]></title><itunes:episode><![CDATA[1]]></itunes:episode>
<enclosure url="https://www.archive.org/download/count_monte_cristo_0711_librivox/count_of_monte_cristo_001_dumas_64kb.mp3" length="0" type="audio/mpeg" /></item>
<item><title><![CDATA[The Chateau D’If]]></title><itunes:episode><![CDATA[8]]></itunes:episode>
<enclosure url="https://www.archive.org/download/count_monte_cristo_0711_librivox/count_of_monte_cristo_008_dumas_64kb.mp3" length="0" type="audio/mpeg" /></item>
</channel></rss>"#;

    /// Real book feeds read cleanly: `cargo test -p raygun-app -- --ignored book_titles_live
    /// --nocapture`.
    #[tokio::test]
    #[ignore = "needs network access to librivox.org and loyalbooks.com"]
    async fn book_titles_live_read_cleanly() {
        let client = raygun_net::http_client().unwrap();
        for feed in [
            "https://librivox.org/rss/47",
            "https://librivox.org/rss/9481",
            "https://librivox.org/rss/894",
            "https://librivox.org/rss/567",
            "https://librivox.org/rss/261",
            "https://www.loyalbooks.com/book/moby-dick-by-herman-melville/feed",
            "https://www.loyalbooks.com/book/pride-and-prejudice-by-jane-austen/feed",
            "https://www.loyalbooks.com/book/the-art-of-war-by-sun-tzu/feed",
        ] {
            let bytes = client
                .get(feed)
                .send()
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap();
            let (book, mut chapters) = podcast_feed::parse(&bytes, feed).unwrap();
            number_chapters(&mut chapters);
            let name = book_name(&book);
            println!(
                "{feed}: {:?} by {:?} ({:?})",
                name.title, name.author, name.edition
            );
            assert!(
                !name.title.contains(" by ") && !name.title.contains(" ("),
                "{feed}"
            );
            assert!(
                name.author.as_deref().is_some_and(|author| {
                    !author.contains('(') && author != "LibriVox" && author != "Loyal Books"
                }),
                "{feed}"
            );
            for chapter in &chapters {
                let title = titles::chapter(&chapter.title, &name.title);
                assert!(chapter.number.is_some(), "{feed}: {title}");
                assert!(
                    !title.contains("&amp;") && !title.is_empty(),
                    "{feed}: {title}"
                );
                assert!(
                    titles::chapter(&title, &name.title) == title,
                    "{feed}: {title}"
                );
            }
            println!(
                "  {:?}",
                chapters
                    .iter()
                    .take(4)
                    .map(|chapter| titles::chapter(&chapter.title, &name.title))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn book_feeds_read_as_their_books_with_tidy_chapters() {
        let (book, mut chapters) = podcast_feed::parse(
            LOYAL_ALICE.as_bytes(),
            "https://www.loyalbooks.com/book/alice/feed",
        )
        .unwrap();
        number_chapters(&mut chapters);
        let name = book_name(&book);
        assert_eq!(name.title, "Alice's Adventures in Wonderland");
        assert_eq!(name.author.as_deref(), Some("Lewis Carroll"));
        assert_eq!(
            chapters
                .iter()
                .map(|chapter| (chapter.number, titles::chapter(&chapter.title, &name.title)))
                .collect::<Vec<_>>(),
            [
                (Some(1), "Dramatis Personae".to_owned()),
                (Some(2), "Down the Rabbit-Hole".to_owned()),
                (Some(3), "Alice's Evidence".to_owned()),
            ],
            "in the feed's order, which its dates would turn round"
        );

        let (book, chapters) = podcast_feed::parse(
            LIBRIVOX_MONTE_CRISTO.as_bytes(),
            "https://librivox.org/rss/47",
        )
        .unwrap();
        let name = book_name(&book);
        assert_eq!(
            (name.title.as_str(), name.author.as_deref()),
            ("The Count of Monte Cristo", Some("Alexandre Dumas"))
        );
        assert_eq!(chapters[0].title, "Marseilles–The Arrival");
        assert_eq!(chapters[1].number, Some(8));
    }
}
