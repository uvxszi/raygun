//! Covers from other sources (M46.40): when a song's, an album's or an artist's own cover is
//! missing or fails to load, the music sources are asked for theirs (`ExtensionService::
//! cover_options`) and the nearest is used, remembered on this device with the source that gave
//! it (`raygun_db::song_memory`), so it's asked once. Lookups the UI waits on run two at a time,
//! so a list full of missing covers doesn't flood the sources; the background search for library
//! songs without covers (`cover_search`) also fetches what it finds into the cover cache.

use std::collections::HashMap;
use std::sync::Arc;

use raygun_db::song_memory::{self, FoundCover};
use raygun_model::DetailTarget;
use tauri::{AppHandle, Manager};
use tokio::sync::OnceCell;

use crate::extensions::ExtensionService;
use crate::startup::AppDb;

/// The `local_meta` key the remembered covers were kept under before they had a table.
const LEGACY_STORE: &str = "covers.fallback";
/// A lookup the UI asked for that found nothing is tried again after a day.
const RETRY_MS: i64 = 24 * 60 * 60 * 1000;
/// Lookups the UI waits on at once.
const AT_ONCE: usize = 2;
/// Of what a background lookup found, this many pictures are tried until one downloads.
const TRIED: usize = 3;

/// Turns for lookups, newest ask first: a list scrolled past keeps asking for the rows it left,
/// and the rows on screen now (asked last) go before them. A semaphore's queue is first come,
/// first served, so the rows on screen waited for every row scrolled past.
pub struct NewestFirst {
    turns: std::sync::Mutex<Turns>,
}

struct Turns {
    free: usize,
    waiting: Vec<tokio::sync::oneshot::Sender<()>>,
}

/// A lookup's turn; the next waiting (newest) ask gets it when it's dropped.
pub struct Turn<'a> {
    gate: &'a NewestFirst,
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        self.gate.release();
    }
}

/// An ask waiting for its turn; one given a turn just as it went away passes the turn on.
struct Waiting<'a> {
    gate: &'a NewestFirst,
    turn: tokio::sync::oneshot::Receiver<()>,
    given: bool,
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        if self.given {
            return;
        }
        self.turn.close();
        if self.turn.try_recv().is_ok() {
            self.gate.release();
        }
    }
}

impl NewestFirst {
    pub fn new(at_once: usize) -> Self {
        Self {
            turns: std::sync::Mutex::new(Turns {
                free: at_once,
                waiting: Vec::new(),
            }),
        }
    }

    pub async fn turn(&self) -> Turn<'_> {
        let turn = {
            let mut turns = self.turns.lock().unwrap_or_else(|p| p.into_inner());
            if turns.free > 0 {
                turns.free -= 1;
                return Turn { gate: self };
            }
            let (give, turn) = tokio::sync::oneshot::channel();
            turns.waiting.push(give);
            turn
        };
        let mut waiting = Waiting {
            gate: self,
            turn,
            given: false,
        };
        // The sender only goes away with a turn given: a closed channel is a turn too.
        let _ = (&mut waiting.turn).await;
        waiting.given = true;
        Turn { gate: self }
    }

    fn release(&self) {
        let mut turns = self.turns.lock().unwrap_or_else(|p| p.into_inner());
        while let Some(next) = turns.waiting.pop() {
            if next.send(()).is_ok() {
                return;
            }
        }
        turns.free += 1;
    }
}

pub struct CoverFallbacks {
    app: AppHandle,
    lookups: NewestFirst,
    /// The old value moved into the table, once a run.
    moved: OnceCell<()>,
}

/// How a background lookup went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Looked {
    /// A source had a cover; it's remembered and cached.
    Found,
    /// The sources answered without one; remembered as a miss.
    Missing,
    /// No source answered (offline, every source failing): nothing remembered.
    Unanswered,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

/// What a cover is remembered by: the kind and its normalized names.
pub fn key(target: &DetailTarget) -> String {
    match target {
        DetailTarget::Song { query } => song_key(&raygun_resolver::track_key(query)),
        DetailTarget::Album { artist, album } => {
            format!(
                "album:{}",
                crate::details::album_key(artist.as_deref(), album)
            )
        }
        DetailTarget::Artist { name } => format!("artist:{}", raygun_resolver::normalize(name)),
        DetailTarget::Playlist { id } => format!("playlist:{id}"),
    }
}

/// A song's remembered cover's key, by its track key.
pub fn song_key(track_key: &str) -> String {
    format!("{}{track_key}", song_memory::SONG)
}

impl CoverFallbacks {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app,
            lookups: NewestFirst::new(AT_ONCE),
            moved: OnceCell::new(),
        })
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().0.clone()
    }

    /// The table, with what was remembered the old way moved in.
    async fn ready(&self) -> Arc<raygun_db::Db> {
        let db = self.db();
        self.moved
            .get_or_init(|| async {
                match song_memory::import_legacy(&db, LEGACY_STORE).await {
                    Ok(0) => {}
                    Ok(moved) => tracing::info!(moved, "remembered covers moved to their table"),
                    Err(error) => tracing::warn!(%error, "couldn't move remembered covers"),
                }
            })
            .await;
        db
    }

    async fn remembered_many(&self, keys: &[String]) -> HashMap<String, FoundCover> {
        let db = self.ready().await;
        song_memory::found(&db, keys).await.unwrap_or_else(|error| {
            tracing::warn!(%error, "couldn't read remembered covers");
            HashMap::new()
        })
    }

    async fn remembered(&self, key: &str) -> Option<FoundCover> {
        self.remembered_many(&[key.to_owned()]).await.remove(key)
    }

    async fn remember(&self, key: &str, url: Option<&str>, source: Option<&str>) {
        let db = self.ready().await;
        if let Err(error) = song_memory::put_found(&db, key, url, source, now_ms()).await {
            tracing::warn!(%error, "could not keep a fallback cover");
        }
    }

    /// The covers found elsewhere for songs by track key (only those with one): what a list
    /// shows for songs without their own (a playlist's cover made of its songs').
    pub async fn remembered_songs(&self, track_keys: &[String]) -> HashMap<String, String> {
        let keys: Vec<String> = track_keys.iter().map(|key| song_key(key)).collect();
        self.remembered_many(&keys)
            .await
            .into_iter()
            .filter_map(|(key, found)| {
                Some((key.strip_prefix(song_memory::SONG)?.to_owned(), found.url?))
            })
            .collect()
    }

    /// The covers found for songs in Favorites, playlists or downloads, and for their albums and
    /// artists (M46.46): the cache sweep keeps them.
    pub async fn kept_addresses(
        &self,
        saved: &[(String, Option<String>, Option<String>)],
    ) -> Vec<String> {
        let mut keys: Vec<String> = Vec::new();
        for (track_key, artist, album) in saved {
            keys.push(song_key(track_key));
            if let Some(artist) = artist {
                keys.push(format!("artist:{}", raygun_resolver::normalize(artist)));
            }
            if let Some(album) = album {
                keys.push(format!(
                    "album:{}",
                    crate::details::album_key(artist.as_deref(), album)
                ));
            }
        }
        keys.sort_unstable();
        keys.dedup();
        self.remembered_many(&keys)
            .await
            .into_values()
            .filter_map(|found| found.url)
            .collect()
    }

    /// The pictures found elsewhere for these artists (quick access's): the cache sweep keeps
    /// them.
    pub async fn artist_addresses(&self, names: &[String]) -> Vec<String> {
        let keys: Vec<String> = names
            .iter()
            .map(|name| key(&DetailTarget::Artist { name: name.clone() }))
            .collect();
        self.remembered_many(&keys)
            .await
            .into_values()
            .filter_map(|found| found.url)
            .collect()
    }

    /// Another source's cover for `target`, or `None` when no source has one.
    pub async fn cover(&self, target: &DetailTarget) -> Option<String> {
        // A playlist's cover is its songs' (or the user's).
        if matches!(target, DetailTarget::Playlist { .. }) {
            return None;
        }
        let key = key(target);
        if let Some(found) = self.remembered(&key).await
            && (found.url.is_some() || now_ms() - found.found_at < RETRY_MS)
        {
            return found.url;
        }
        let _turn = self.lookups.turn().await;
        // Another lookup may have found it while this one waited.
        if let Some(FoundCover { url: Some(url), .. }) = self.remembered(&key).await {
            return Some(url);
        }
        let extensions = self.app.state::<Arc<ExtensionService>>().inner().clone();
        let (options, answered) = extensions.cover_options_answered(target).await;
        let first = options.into_iter().next();
        if first.is_some() || answered {
            self.remember(
                &key,
                first.as_ref().map(|option| option.url.as_str()),
                first.as_ref().map(|option| option.source.as_str()),
            )
            .await;
        }
        first.map(|option| option.url)
    }

    /// Looks for a library song's cover in the background, remembered under `key` (the song's
    /// stored key): the sources' first pictures are tried with `fetch` (into the cover cache)
    /// until one downloads, and the one that did is remembered with its source. When none
    /// downloads the first is remembered all the same (a picture that failed now may load
    /// later); none at all is remembered as a miss, unless no source answered.
    pub async fn search<F, Fut>(&self, key: &str, target: &DetailTarget, fetch: F) -> Looked
    where
        F: Fn(String) -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let extensions = self.app.state::<Arc<ExtensionService>>().inner().clone();
        let (options, answered) = extensions.cover_options_answered(target).await;
        if options.is_empty() {
            if !answered {
                return Looked::Unanswered;
            }
            self.remember(key, None, None).await;
            return Looked::Missing;
        }
        let mut chosen = &options[0];
        for option in options.iter().take(TRIED) {
            if fetch(option.url.clone()).await {
                chosen = option;
                break;
            }
        }
        self.remember(key, Some(&chosen.url), Some(&chosen.source))
            .await;
        Looked::Found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Twenty rows scrolled past ask for lookups, then the row on screen does; each lookup takes
    /// 20 ms, two at a time. Returns how long the row on screen waited for its cover.
    async fn on_screen_wait<G, F, Fut>(gate: Arc<G>, lookup: F) -> std::time::Duration
    where
        G: Send + Sync + 'static,
        F: Fn(Arc<G>) -> Fut + Copy + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        // Asked in row order: the test's runtime starts its tasks in the order they're spawned.
        let mut asks = Vec::new();
        for _ in 0..21 {
            let gate = gate.clone();
            asks.push(tokio::spawn(async move {
                let start = std::time::Instant::now();
                lookup(gate).await;
                start.elapsed()
            }));
        }
        let mut waits = Vec::new();
        for ask in asks {
            waits.push(ask.await.unwrap());
        }
        waits[20]
    }

    #[tokio::test]
    async fn the_row_on_screen_is_looked_up_before_rows_scrolled_past() {
        let lookup = std::time::Duration::from_millis(20);
        let fifo = on_screen_wait(
            Arc::new(tokio::sync::Semaphore::new(AT_ONCE)),
            move |gate: Arc<tokio::sync::Semaphore>| async move {
                let _turn = gate.acquire().await.unwrap();
                tokio::time::sleep(lookup).await;
            },
        )
        .await;
        let newest = on_screen_wait(
            Arc::new(NewestFirst::new(AT_ONCE)),
            move |gate: Arc<NewestFirst>| async move {
                let _turn = gate.turn().await;
                tokio::time::sleep(lookup).await;
            },
        )
        .await;
        eprintln!(
            "the row on screen waited {} ms first come first served, {} ms newest first",
            fifo.as_millis(),
            newest.as_millis()
        );
        assert!(newest * 3 < fifo, "{newest:?} vs {fifo:?}");
    }

    #[tokio::test]
    async fn an_ask_that_goes_away_passes_its_turn_on() {
        let gate = Arc::new(NewestFirst::new(1));
        let held = gate.turn().await;
        let gone = {
            let gate = gate.clone();
            tokio::spawn(async move {
                let _turn = gate.turn().await;
            })
        };
        tokio::task::yield_now().await;
        gone.abort();
        let _ = gone.await;
        drop(held);
        // The turn is free again, not lost with the ask that went away.
        tokio::time::timeout(std::time::Duration::from_secs(1), gate.turn())
            .await
            .expect("the turn came back");
    }

    #[test]
    fn keys_name_the_kind_and_its_names() {
        let song = DetailTarget::Song {
            query: raygun_model::TrackQuery {
                title: "Get Lucky (Radio Edit)".into(),
                artist: Some("Daft Punk".into()),
                album: None,
                duration_ms: None,
            },
        };
        assert_eq!(key(&song), "song:daft punk|get lucky");
        assert_eq!(song_key("daft punk|get lucky"), key(&song));
        let artist = DetailTarget::Artist {
            name: "Daft Punk".into(),
        };
        assert_eq!(key(&artist), "artist:daft punk");
    }
}
