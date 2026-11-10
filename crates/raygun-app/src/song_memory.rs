//! What Raygun keeps about songs, kept in tiers by one recency: when a song was last played or
//! seen (`raygun_db::song_memory::recency`: its play stamp, its plays, and when its kept facts
//! were made or used). Small text outlives pictures:
//!
//! 1. Cover pictures go first: the cover cache's least recently used once it's full
//!    (`CachePrefs::cover_cache_mb`), never the playing and queued songs' ([`in_use`]) nor the
//!    library's (`keep/`, `caches::sweep_later`).
//! 2. Where covers came from (the covers found at other sources, with the source that gave each)
//!    is kept for the `CachePrefs::cover_source_songs` most recently played or seen songs; songs
//!    in the library keep theirs, as do saved songs' albums and artists and followed ones.
//! 3. Everything else (the streams that played or failed, lengths, lyrics the lyrics settings
//!    don't keep) goes `CachePrefs::song_facts_days` after a song was last played or seen.
//!
//! A sweep runs a while after start and then once a day, in the background.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use raygun_db::song_memory as store;
use raygun_model::{CachePrefs, SongMemoryUsage, TrackQuery};
use tauri::{AppHandle, Manager};

use crate::startup::AppDb;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// The first sweep waits this long after start, out of the way.
const FIRST_SWEEP: Duration = Duration::from_secs(10 * 60);
const SWEEP_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Stamps a song as played on this device (`CollectionsService::record_play`, which leaves
/// incognito plays out).
pub async fn played(app: &AppHandle, track_key: &str) {
    let db = app.state::<AppDb>().0.clone();
    if let Err(error) = store::stamp(&db, track_key, now_ms()).await {
        tracing::debug!(%error, "couldn't stamp a song as played");
    }
}

/// The resolver's key of a song as named.
fn key_of(title: &str, artist: Option<&str>) -> String {
    raygun_resolver::track_key(&TrackQuery {
        title: title.to_owned(),
        artist: artist.map(str::to_owned),
        album: None,
        duration_ms: None,
    })
}

/// The cover cache's names of the playing and queued songs' covers: trimming the cache leaves
/// them (`CoverCache::spare_with`).
pub fn in_use(app: &AppHandle) -> HashSet<String> {
    let Some(playback) = app.try_state::<crate::playback::PlaybackService>() else {
        return HashSet::new();
    };
    let Some(images) = app.try_state::<raygun_net::ImageLinks>() else {
        return HashSet::new();
    };
    playback
        .queue()
        .items
        .iter()
        .filter_map(|item| item.artwork_url.as_deref())
        .filter_map(|url| {
            let relinked = images.relink(url);
            if images.is_remote(&relinked) {
                return Some(raygun_net::proxy_image_token(
                    &relinked,
                    &Default::default(),
                ));
            }
            relinked
                .split_once("/cover/")
                .map(|(_, token)| token.to_owned())
        })
        .collect()
}

/// What a sweep never takes, however long ago a song was played.
#[derive(Debug, Default)]
pub(crate) struct Protected {
    /// Songs in Favorites, playlists, downloads or the device's files: their covers' sources stay.
    pub library: HashSet<String>,
    /// Songs in the queue: nothing of theirs goes.
    pub queued: HashSet<String>,
    /// Covers of albums and artists kept by name (`album:…`, `artist:…`): saved songs' and
    /// followed or pinned artists'.
    pub names: HashSet<String>,
}

/// What a sweep forgets.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Plan {
    /// Songs whose kept facts all go (tier 3).
    pub forget: Vec<String>,
    /// Covers found elsewhere that go, by their keys (tier 2, and tier 3's).
    pub covers: Vec<String>,
}

/// The artist part of a song's key or a cover's names (`artist|title`, `artist|album`).
fn artist_part(names: &str) -> &str {
    names.split_once('|').map_or(names, |(artist, _)| artist)
}

/// Plans a sweep over the recency list (`recency`, the most recent first) and the covers found
/// elsewhere (`found`: their keys and when they were found): songs not played or seen for
/// `song_facts_days` lose everything kept of them; past the `cover_source_songs` most recent,
/// songs lose the covers found for them elsewhere. Albums' and artists' covers go with the last
/// of their artist's songs. What `protected` names stays.
pub(crate) fn plan(
    recency: &[(String, i64)],
    found: &[(String, i64)],
    protected: &Protected,
    prefs: &CachePrefs,
    now: i64,
) -> Plan {
    let cutoff =
        (prefs.song_facts_days > 0).then(|| now - i64::from(prefs.song_facts_days) * DAY_MS);
    let kept_places = prefs.cover_source_songs as usize;
    let mut forget = Vec::new();
    let mut with_covers: HashSet<&str> = HashSet::new();
    // The latest a song by each artist was played or seen, among those kept.
    let mut artists: HashMap<&str, i64> = HashMap::new();
    for (place, (key, at)) in recency.iter().enumerate() {
        let old = cutoff.is_some_and(|cutoff| *at < cutoff);
        if old && !protected.queued.contains(key) {
            forget.push(key.clone());
            continue;
        }
        let latest = artists.entry(artist_part(key)).or_insert(*at);
        *latest = (*latest).max(*at);
        if place < kept_places {
            with_covers.insert(key.as_str());
        }
    }
    let covers = found
        .iter()
        .filter(|(key, found_at)| {
            if let Some(song) = key.strip_prefix(store::SONG) {
                let kept = protected.library.contains(song)
                    || protected.queued.contains(song)
                    || with_covers.contains(song);
                return !kept;
            }
            let names = key
                .strip_prefix("album:")
                .or_else(|| key.strip_prefix("artist:"));
            let Some(names) = names else {
                return false;
            };
            if protected.names.contains(key.as_str()) {
                return false;
            }
            let seen = artists
                .get(artist_part(names))
                .copied()
                .unwrap_or(0)
                .max(*found_at);
            cutoff.is_some_and(|cutoff| seen < cutoff)
        })
        .map(|(key, _)| key.clone())
        .collect();
    Plan { forget, covers }
}

pub struct SongMemory {
    app: AppHandle,
}

impl SongMemory {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self { app })
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().0.clone()
    }

    /// Sweeps a while after start, then once a day.
    pub fn start(self: &Arc<Self>) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(FIRST_SWEEP).await;
            loop {
                match service.sweep().await {
                    Ok(plan) if plan.forget.is_empty() && plan.covers.is_empty() => {}
                    Ok(plan) => tracing::info!(
                        songs = plan.forget.len(),
                        covers = plan.covers.len(),
                        "forgot what was kept of songs not played lately"
                    ),
                    Err(error) => tracing::warn!(%error, "couldn't sweep what's kept of songs"),
                }
                tokio::time::sleep(SWEEP_EVERY).await;
            }
        });
    }

    /// What a sweep never takes now.
    async fn protected(&self) -> Result<Protected, String> {
        let db = self.db();
        let mut library: HashSet<String> = raygun_db::collections::saved_keys(&db)
            .await
            .map_err(to_message)?
            .into_iter()
            .collect();
        if let Some(songs) = self
            .app
            .try_state::<Arc<crate::music_downloads::SongDownloadService>>()
        {
            library.extend(songs.downloaded_keys());
        }
        library.extend(
            raygun_db::local_music::track_keys(&db)
                .await
                .map_err(to_message)?,
        );
        let queued = self
            .app
            .try_state::<crate::playback::PlaybackService>()
            .map(|playback| {
                playback
                    .queue()
                    .items
                    .iter()
                    .map(|item| key_of(&item.title, item.artist.as_deref()))
                    .collect()
            })
            .unwrap_or_default();
        let mut names: HashSet<String> = HashSet::new();
        for (_, artist, album, _) in raygun_db::collections::saved_songs(&db)
            .await
            .map_err(to_message)?
        {
            if let Some(album) = &album {
                names.insert(format!(
                    "album:{}",
                    crate::details::album_key(artist.as_deref(), album)
                ));
            }
            if let Some(artist) = &artist {
                names.insert(format!("artist:{}", raygun_resolver::normalize(artist)));
            }
        }
        if let Some(follows) = self.app.try_state::<Arc<crate::follows::ArtistFollows>>() {
            for artist in follows.list().await {
                names.insert(format!(
                    "artist:{}",
                    raygun_resolver::normalize(&artist.name)
                ));
            }
        }
        let pins = crate::settings::load(&db)
            .await
            .map(|settings| settings.rail.pins)
            .unwrap_or_default();
        for pin in pins.iter().filter(|pin| pin.kind == "artist") {
            names.insert(format!("artist:{}", raygun_resolver::normalize(&pin.name)));
        }
        Ok(Protected {
            library,
            queued,
            names,
        })
    }

    /// Forgets what the settings no longer keep of songs (tiers 2 and 3; the cover cache trims
    /// itself, tier 1).
    async fn sweep(&self) -> Result<Plan, String> {
        let db = self.db();
        let prefs = crate::settings::load(&db).await.map_err(to_message)?.cache;
        let recency = store::recency(&db).await.map_err(to_message)?;
        let found = store::found_times(&db).await.map_err(to_message)?;
        let protected = self.protected().await?;
        let plan = plan(&recency, &found, &protected, &prefs, now_ms());
        store::remove_found(&db, &plan.covers)
            .await
            .map_err(to_message)?;
        if !plan.forget.is_empty() {
            store::forget(&db, &plan.forget).await.map_err(to_message)?;
            // Lyrics as the lyrics settings say (some always kept, or none ever forgotten).
            self.app
                .state::<Arc<crate::lyrics_store::LyricsStore>>()
                .forget_unplayed(&plan.forget)
                .await?;
        }
        Ok(plan)
    }

    /// How much is kept, for Settings → Data → Storage.
    pub async fn usage(&self) -> Result<SongMemoryUsage, String> {
        let db = self.db();
        let songs = store::recency(&db).await.map_err(to_message)?.len();
        Ok(SongMemoryUsage {
            songs: u32::try_from(songs).unwrap_or(u32::MAX),
            cover_sources: store::found_count(&db).await.map_err(to_message)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1000 * DAY_MS;

    fn days_ago(days: i64) -> i64 {
        NOW - days * DAY_MS
    }

    fn prefs(cover_source_songs: u32, song_facts_days: u32) -> CachePrefs {
        CachePrefs {
            cover_source_songs,
            song_facts_days,
            ..CachePrefs::default()
        }
    }

    fn list(entries: &[(&str, i64)]) -> Vec<(String, i64)> {
        entries
            .iter()
            .map(|(key, at)| ((*key).to_owned(), *at))
            .collect()
    }

    #[test]
    fn cover_sources_go_before_the_rest_and_everything_goes_at_the_end_of_the_list() {
        // The recency list, the most recent first.
        let recency = list(&[
            ("a|new", days_ago(1)),
            ("b|recent", days_ago(10)),
            ("c|older", days_ago(100)),
            ("d|saved", days_ago(200)),
            ("e|ancient", days_ago(400)),
            ("f|queued", days_ago(500)),
        ]);
        let found = list(&[
            ("song:a|new", days_ago(1)),
            ("song:c|older", days_ago(100)),
            ("song:d|saved", days_ago(300)),
            ("song:e|ancient", days_ago(400)),
            ("song:f|queued", days_ago(500)),
            // b's songs are recent: its album stays; e's last song goes, and its artist with it.
            ("album:b|an album", days_ago(900)),
            ("artist:e", days_ago(400)),
            // Followed: stays.
            ("artist:g", days_ago(900)),
        ]);
        let protected = Protected {
            library: HashSet::from(["d|saved".to_owned()]),
            queued: HashSet::from(["f|queued".to_owned()]),
            names: HashSet::from(["artist:g".to_owned()]),
        };
        // Covers' sources for the 2 most recent songs; the rest after a year.
        let plan = plan(&recency, &found, &protected, &prefs(2, 365), NOW);
        assert_eq!(plan.forget, ["e|ancient"]);
        assert_eq!(plan.covers, ["song:c|older", "song:e|ancient", "artist:e"]);

        // With room for every song's cover source, only the end of the list goes.
        let roomy = super::plan(&recency, &found, &protected, &prefs(100_000, 365), NOW);
        assert_eq!(roomy.covers, ["song:e|ancient", "artist:e"]);

        // Kept for ever: nothing goes but what's past the number of songs.
        let forever = super::plan(&recency, &found, &protected, &prefs(2, 0), NOW);
        assert!(forever.forget.is_empty());
        assert_eq!(forever.covers, ["song:c|older", "song:e|ancient"]);
    }
}
