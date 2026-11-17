//! Made for You (M46.20), as QuietPlay and Spotify have it: mixes made from the user's listening
//! (`taste`), filled with songs from `music.recommend` sources (similar artists' best-known songs,
//! new releases, artist radios) and the genre sources (M46.19). Made once a day in the background
//! and kept in `local_value` `music.madeForYou`; a refresh makes them again. Hidden artists are
//! never asked about, and hidden songs leave the kept mixes as they're read, so hiding (and
//! showing again) takes effect at once.

pub mod taste;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use raygun_db::collections::{self, TrackFields};
use raygun_model::{
    ListeningStats, MadeForYou, MediaSource, Mix, MixKind, MixSummary, MusicTag, OnlineTrack,
    TagKind, TrackInput, TrackQuery,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use self::taste::{Known, MIN_MIX, MIX_SIZE, Taste, made, main_artist, shuffle};
use crate::collections::CollectionsService;
use crate::discovery::DiscoveryService;
use crate::events::MadeForYouEvent;
use crate::extensions::ExtensionService;
use crate::hidden::Hidden;
use crate::startup::AppDb;

const CACHE_KEY: &str = "music.madeForYou";
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// Plays read back, at most, and how far.
const PLAYS_READ: i64 = 5_000;
const PLAYS_SINCE_DAYS: i64 = 365;
/// Artists the online mixes start from.
const SEED_ARTISTS: usize = 6;
/// Songs per artist taken into a mix of new songs.
const PER_ARTIST: usize = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    /// The user's local day number when they were made.
    day: i64,
    made_at: i64,
    mixes: Vec<Mix>,
    stats: ListeningStats,
}

pub struct MadeForYouService {
    app: AppHandle,
    making: AtomicBool,
    /// When the last making ended with nothing made (ms; 0 = never). Visits don't start
    /// another one before `RETRY_AFTER_MS`, or the event they answer would start it again.
    came_up_empty_at: AtomicI64,
}

/// How long visits wait after a making that made nothing; a refresh doesn't wait.
const RETRY_AFTER_MS: i64 = 30 * 60 * 1000;

/// Whether a visit (not a refresh) may start a making, given when the last one came up empty.
fn may_retry(came_up_empty_at: i64, now: i64) -> bool {
    came_up_empty_at == 0 || now - came_up_empty_at >= RETRY_AFTER_MS
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}

fn local_day(at: i64, utc_offset_minutes: i32) -> i64 {
    (at / 60_000 + i64::from(utc_offset_minutes)).div_euclid(24 * 60)
}

fn song_key(title: &str, artist: Option<&str>) -> String {
    raygun_resolver::track_key(&TrackQuery {
        title: title.to_owned(),
        artist: artist.map(str::to_owned),
        album: None,
        duration_ms: None,
    })
}

fn summary(mix: &Mix) -> MixSummary {
    MixSummary {
        id: mix.id.clone(),
        kind: mix.kind,
        seed: mix.seed.clone(),
        number: mix.number,
        artists: mix.artists.clone(),
        artwork_urls: mix.artwork_urls.clone(),
        track_count: mix.tracks.len() as u32,
    }
}

fn online(track: TrackFields) -> OnlineTrack {
    OnlineTrack {
        external_id: None,
        title: track.title,
        artist: track.artist,
        album: track.album,
        duration_ms: track
            .duration_ms
            .and_then(|duration| u32::try_from(duration).ok()),
        artwork_url: track.artwork_url,
        source: track
            .source
            .and_then(|json| serde_json::from_str::<MediaSource>(&json).ok())
            .and_then(crate::collections::portable),
        explicit: false,
    }
}

/// `mix` without hidden songs: `None` when it's about a hidden artist, or too little is left.
fn visible(mix: Mix, hidden: &Hidden) -> Option<Mix> {
    if hidden.is_empty() {
        return Some(mix);
    }
    let about_artist = matches!(
        mix.kind,
        MixKind::BecauseYouListened | MixKind::ArtistRadio | MixKind::Daily
    );
    if about_artist
        && mix
            .seed
            .as_deref()
            .is_some_and(|seed| hidden.hides_artist(seed))
    {
        return None;
    }
    if !mix.tracks.iter().any(|track| hidden.hides_track(track)) {
        return Some(mix);
    }
    let tracks: Vec<OnlineTrack> = mix
        .tracks
        .into_iter()
        .filter(|track| !hidden.hides_track(track))
        .collect();
    (tracks.len() >= MIN_MIX).then(|| made(mix.id, mix.kind, mix.seed, mix.number, tracks))
}

/// Takes from each list in turn (`step` at a time), skipping songs already taken or `excluded`.
fn interleave(
    lists: Vec<Vec<OnlineTrack>>,
    step: usize,
    limit: usize,
    excluded: &HashSet<String>,
) -> Vec<OnlineTrack> {
    let mut taken = Vec::new();
    let mut seen: HashSet<String> = excluded.clone();
    let mut cursors = vec![0usize; lists.len()];
    loop {
        let mut moved = false;
        for (index, list) in lists.iter().enumerate() {
            let mut picked = 0;
            while picked < step && cursors[index] < list.len() {
                let track = &list[cursors[index]];
                cursors[index] += 1;
                moved = true;
                if seen.insert(song_key(&track.title, track.artist.as_deref())) {
                    taken.push(track.clone());
                    picked += 1;
                    if taken.len() >= limit {
                        return taken;
                    }
                }
            }
        }
        if !moved {
            return taken;
        }
    }
}

/// What the sources were asked during one making, so an artist is asked about once; hidden
/// artists are never asked about or taken in.
#[derive(Default)]
struct Asked {
    similar: HashMap<String, Vec<String>>,
    top: HashMap<String, Vec<OnlineTrack>>,
    hidden: Hidden,
}

impl MadeForYouService {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app,
            making: AtomicBool::new(false),
            came_up_empty_at: AtomicI64::new(0),
        })
    }

    fn db(&self) -> &raygun_db::Db {
        &self.app.state::<AppDb>().inner().0
    }

    fn extensions(&self) -> Arc<ExtensionService> {
        self.app.state::<Arc<ExtensionService>>().inner().clone()
    }

    async fn stored(&self) -> Option<Stored> {
        let json = raygun_db::local_value(self.db(), CACHE_KEY).await.ok()??;
        serde_json::from_str(&json).ok()
    }

    /// The mixes made for today (making them in the background when they're from another day,
    /// or when `refresh`), and the listening numbers.
    pub async fn overview(self: &Arc<Self>, refresh: bool, utc_offset_minutes: i32) -> MadeForYou {
        let stored = self.stored().await;
        let today = local_day(now_ms(), utc_offset_minutes);
        let stale = stored.as_ref().is_none_or(|stored| stored.day != today);
        let waiting = !may_retry(self.came_up_empty_at.load(Ordering::SeqCst), now_ms());
        if refresh || (stale && !waiting) {
            self.start(utc_offset_minutes);
        }
        let making = self.making.load(Ordering::SeqCst);
        let hidden = Hidden::load(self.db()).await;
        match stored {
            Some(mut stored) => MadeForYou {
                mixes: stored
                    .mixes
                    .into_iter()
                    .filter_map(|mix| visible(mix, &hidden))
                    .map(|mix| summary(&mix))
                    .collect(),
                stats: {
                    stored
                        .stats
                        .top_artists
                        .retain(|artist| !hidden.hides_artist(artist));
                    stored.stats
                },
                made_at: Some(stored.made_at as f64),
                making,
            },
            None => MadeForYou {
                mixes: Vec::new(),
                stats: self
                    .taste(utc_offset_minutes)
                    .await
                    .map(|taste| {
                        let mut stats = taste.stats();
                        stats
                            .top_artists
                            .retain(|artist| !hidden.hides_artist(artist));
                        stats
                    })
                    .unwrap_or_default(),
                made_at: None,
                making,
            },
        }
    }

    /// One mix with its songs.
    pub async fn mix(&self, id: &str) -> Result<Mix, String> {
        let hidden = Hidden::load(self.db()).await;
        self.stored()
            .await
            .and_then(|stored| stored.mixes.into_iter().find(|mix| mix.id == id))
            .and_then(|mix| visible(mix, &hidden))
            .ok_or_else(|| "This mix is no longer made for you; open Made for You again".to_owned())
    }

    /// Saves a mix as a playlist named `name`; returns the playlist's id.
    pub async fn save(&self, id: &str, name: &str) -> Result<String, String> {
        let mix = self.mix(id).await?;
        let collections = self.app.state::<CollectionsService>();
        let playlist = collections.create_playlist(name).await?;
        let tracks = mix
            .tracks
            .into_iter()
            .map(|track| TrackInput {
                explicit: track.explicit,
                title: track.title,
                artist: track.artist,
                album: track.album,
                duration_ms: track.duration_ms,
                artwork_url: track.artwork_url,
                source: track.source,
            })
            .collect();
        collections
            .add_to_playlist(&playlist.id, tracks, false)
            .await?;
        Ok(playlist.id)
    }

    fn start(self: &Arc<Self>, utc_offset_minutes: i32) {
        if self.making.swap(true, Ordering::SeqCst) {
            return;
        }
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            match service.make(utc_offset_minutes).await {
                // Nothing made although there is listening to go on: the sources didn't answer,
                // so the last mixes stay and the next visit tries again.
                Ok(stored) if stored.mixes.is_empty() && stored.stats.songs_known > 0 => {
                    tracing::warn!("no mixes made; the sources may not have answered");
                    service.came_up_empty_at.store(now_ms(), Ordering::SeqCst);
                }
                Ok(stored) => {
                    service.came_up_empty_at.store(0, Ordering::SeqCst);
                    tracing::info!(mixes = stored.mixes.len(), "made for you");
                    match serde_json::to_string(&stored) {
                        Ok(json) => {
                            if let Err(error) =
                                raygun_db::set_local_value(service.db(), CACHE_KEY, &json).await
                            {
                                tracing::warn!(%error, "couldn't keep the mixes");
                            }
                        }
                        Err(error) => tracing::warn!(%error, "couldn't keep the mixes"),
                    }
                }
                Err(error) => tracing::warn!(%error, "couldn't make the mixes"),
            }
            service.making.store(false, Ordering::SeqCst);
            if let Err(error) = MadeForYouEvent.emit(&service.app) {
                tracing::warn!(%error, "couldn't announce the mixes");
            }
        });
    }

    /// Everything the user's listening says: a year of plays, the favorites, and their tags.
    async fn taste(&self, utc_offset_minutes: i32) -> Result<Taste, String> {
        let db = self.db();
        let now = now_ms();
        let plays = collections::plays_since(db, now - PLAYS_SINCE_DAYS * DAY_MS, PLAYS_READ)
            .await
            .map_err(|error| error.to_string())?;
        let favorites = collections::favorites(db)
            .await
            .map_err(|error| error.to_string())?;
        let mut taste = Taste {
            now,
            utc_offset_minutes,
            ..Taste::default()
        };
        let mut tracks: HashMap<String, TrackFields> = HashMap::new();
        for play in plays {
            taste
                .plays
                .push((play.track.track_key.clone(), play.played_at));
            tracks
                .entry(play.track.track_key.clone())
                .or_insert(play.track);
        }
        for favorite in favorites {
            taste
                .favorites
                .insert(favorite.track.track_key.clone(), favorite.added_at);
            tracks
                .entry(favorite.track.track_key.clone())
                .or_insert(favorite.track);
        }
        let keys: Vec<String> = tracks.keys().cloned().collect();
        let mut tags: HashMap<String, Vec<MusicTag>> = HashMap::new();
        for chunk in keys.chunks(500) {
            for (key, json) in raygun_db::track_tags::get_many(db, chunk)
                .await
                .map_err(|error| error.to_string())?
            {
                tags.insert(key, serde_json::from_str(&json).unwrap_or_default());
            }
        }
        for (key, track) in tracks {
            let song_tags = tags.remove(&key).unwrap_or_default();
            let names = |kind: TagKind| {
                song_tags
                    .iter()
                    .filter(|tag| tag.kind == kind)
                    .map(|tag| tag.name.clone())
                    .collect()
            };
            taste.songs.insert(
                key.clone(),
                Known {
                    key,
                    genres: names(TagKind::Genre),
                    moods: names(TagKind::Mood),
                    track: online(track),
                },
            );
        }
        Ok(taste)
    }

    async fn similar(&self, asked: &mut Asked, artist: &str) -> Vec<String> {
        let wanted = artist.to_lowercase();
        if let Some(found) = asked.similar.get(&wanted) {
            return found.clone();
        }
        let found = self.extensions().similar_artists(artist, 10).await;
        asked.similar.insert(wanted, found.clone());
        found
    }

    async fn top(&self, asked: &mut Asked, artist: &str) -> Vec<OnlineTrack> {
        let wanted = artist.to_lowercase();
        if let Some(found) = asked.top.get(&wanted) {
            return found.clone();
        }
        let found = self
            .extensions()
            .recommended_tracks(
                "topTracks",
                serde_json::json!({ "artist": artist, "limit": 10 }),
            )
            .await;
        asked.top.insert(wanted, found.clone());
        found
    }

    /// Songs by artists like `seeds` that aren't the user's: at most `per_artist` each.
    async fn songs_of_similar(
        &self,
        asked: &mut Asked,
        seeds: &[String],
        mine: &HashSet<String>,
        artists_wanted: usize,
        per_artist: usize,
    ) -> Vec<Vec<OnlineTrack>> {
        let mut counts: HashMap<String, (usize, String)> = HashMap::new();
        for seed in seeds {
            for (rank, name) in self.similar(asked, seed).await.into_iter().enumerate() {
                if mine.contains(&name.to_lowercase()) || asked.hidden.hides_artist(&name) {
                    continue;
                }
                let entry = counts.entry(name.to_lowercase()).or_insert((0, name));
                // Named by several seeds, or near the top of one, comes first.
                entry.0 += 10 - rank.min(9);
            }
        }
        let mut ranked: Vec<(usize, String)> = counts.into_values().collect();
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        let mut lists = Vec::new();
        for (_, name) in ranked.into_iter().take(artists_wanted) {
            let mut songs = self.top(asked, &name).await;
            // A top song may credit a hidden artist alongside this one.
            asked.hidden.retain_tracks(&mut songs);
            songs.truncate(per_artist.max(1) * 2);
            if !songs.is_empty() {
                lists.push(songs);
            }
        }
        lists
    }

    /// Makes every mix: the user's own, then the ones filled from sources. A source that fails
    /// only leaves its mixes out.
    async fn make(&self, utc_offset_minutes: i32) -> Result<Stored, String> {
        let taste = self.taste(utc_offset_minutes).await?;
        let hidden = Hidden::load(self.db()).await;
        let now = taste.now;
        let day = local_day(now, utc_offset_minutes);
        let week = day.div_euclid(7);
        let played = taste.played();
        let known: HashSet<String> = taste.songs.keys().cloned().collect();
        let mine: HashSet<String> = taste
            .songs
            .values()
            .filter_map(|song| song.track.artist.as_deref())
            .map(|credit| main_artist(credit).to_lowercase())
            .collect();
        // The listener's artists that aren't hidden, most played first.
        let top_artists = |count: usize| -> Vec<String> {
            taste
                .top_artists(count + 20)
                .into_iter()
                .filter(|artist| !hidden.hides_artist(artist))
                .take(count)
                .collect()
        };
        let seeds = top_artists(SEED_ARTISTS);
        tracing::info!(
            songs = taste.songs.len(),
            plays = taste.plays.len(),
            ?seeds,
            "making mixes"
        );
        let mut asked = Asked {
            hidden: hidden.clone(),
            ..Asked::default()
        };
        let mut mixes: Vec<Mix> = Vec::new();

        // Daily Mixes: the user's songs of a genre (or artist), with similar artists' songs mixed
        // in, one in three.
        for mut mix in taste.daily_mixes() {
            let lead: Vec<String> = mix
                .artists
                .iter()
                .filter(|artist| !hidden.hides_artist(artist))
                .take(2)
                .cloned()
                .collect();
            let lists = self.songs_of_similar(&mut asked, &lead, &mine, 4, 3).await;
            let fresh = interleave(lists, 1, MIX_SIZE / 3, &known);
            let mut tracks = Vec::new();
            let mut fresh = fresh.into_iter();
            for (index, track) in mix.tracks.drain(..).enumerate() {
                tracks.push(track);
                if index % 2 == 1
                    && let Some(new) = fresh.next()
                {
                    tracks.push(new);
                }
            }
            tracks.extend(fresh);
            tracks.truncate(MIX_SIZE);
            mixes.push(made(mix.id, mix.kind, mix.seed, mix.number, tracks));
        }

        // Discover Weekly: songs the user hasn't heard by artists like theirs; new each week.
        let lists = self
            .songs_of_similar(&mut asked, &seeds, &mine, 15, PER_ARTIST)
            .await;
        let mut discover = interleave(lists, PER_ARTIST, 30, &played);
        shuffle(&mut discover, &format!("discover{week}"));
        if discover.len() >= MIN_MIX {
            mixes.push(made(
                "discover-weekly".into(),
                MixKind::DiscoverWeekly,
                None,
                None,
                discover,
            ));
        }

        // Release Radar: the last four weeks' releases by the user's artists.
        let mut releases = Vec::new();
        for artist in top_artists(10) {
            let mut songs = self
                .extensions()
                .recommended_tracks(
                    "newReleases",
                    serde_json::json!({ "artist": artist, "days": 28, "limit": 6 }),
                )
                .await;
            hidden.retain_tracks(&mut songs);
            if !songs.is_empty() {
                releases.push(songs);
            }
        }
        let radar = interleave(releases, 3, 30, &HashSet::new());
        if radar.len() >= MIN_MIX.min(4) {
            mixes.push(made(
                "release-radar".into(),
                MixKind::ReleaseRadar,
                None,
                None,
                radar,
            ));
        }

        mixes.extend(taste.on_repeat());
        mixes.extend(taste.time_of_day());
        mixes.extend(taste.repeat_rewind());

        // Because you listened to …: the top three artists' neighbours.
        for artist in seeds.iter().take(3) {
            let lists = self
                .songs_of_similar(&mut asked, std::slice::from_ref(artist), &mine, 8, 2)
                .await;
            let songs = interleave(lists, 2, 20, &played);
            if songs.len() >= MIN_MIX {
                mixes.push(made(
                    format!("because-{}", artist.to_lowercase()),
                    MixKind::BecauseYouListened,
                    Some(artist.clone()),
                    None,
                    songs,
                ));
            }
        }

        // Radios for the two top artists.
        for artist in seeds.iter().take(2) {
            let mut songs = self
                .extensions()
                .recommended_tracks(
                    "radio",
                    serde_json::json!({ "artist": artist, "title": null, "limit": 40 }),
                )
                .await;
            hidden.retain_tracks(&mut songs);
            if songs.len() >= MIN_MIX {
                mixes.push(made(
                    format!("radio-{}", artist.to_lowercase()),
                    MixKind::ArtistRadio,
                    Some(artist.clone()),
                    None,
                    songs,
                ));
            }
        }

        // Genre and mood mixes: the user's songs with the genre sources' (M46.19).
        let discovery = self.app.state::<Arc<DiscoveryService>>().inner().clone();
        let mut themed: Vec<(MixKind, String, Vec<OnlineTrack>)> = taste
            .top_genres(3)
            .into_iter()
            .map(|(name, keys)| {
                let own = keys
                    .iter()
                    .filter_map(|key| taste.track(key).cloned())
                    .collect();
                (MixKind::Genre, name, own)
            })
            .collect();
        themed.extend(
            taste
                .mood_mixes()
                .into_iter()
                .filter_map(|mix| Some((MixKind::Mood, mix.seed.clone()?, mix.tracks))),
        );
        for (kind, name, own) in themed {
            let rows = discovery.tag_sources(name.clone(), None).await.rows;
            let mut lists = vec![own];
            lists.extend(rows.into_iter().map(|mut row| {
                hidden.retain_tracks(&mut row.tracks);
                row.tracks
            }));
            let mut songs = interleave(lists, 2, MIX_SIZE, &HashSet::new());
            shuffle(&mut songs, &format!("{name}{day}"));
            if songs.len() >= MIN_MIX {
                let id = match kind {
                    MixKind::Mood => format!("mood-{}", name.to_lowercase()),
                    _ => format!("genre-{}", name.to_lowercase()),
                };
                mixes.push(made(id, kind, Some(name), None, songs));
            }
        }

        Ok(Stored {
            day,
            made_at: now,
            stats: taste.stats(),
            mixes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visits_wait_after_a_making_that_made_nothing() {
        assert!(may_retry(0, 1_000));
        assert!(!may_retry(1_000, 1_000 + RETRY_AFTER_MS - 1));
        assert!(may_retry(1_000, 1_000 + RETRY_AFTER_MS));
    }

    fn track(title: &str, artist: &str) -> OnlineTrack {
        OnlineTrack {
            external_id: None,
            title: title.into(),
            artist: Some(artist.into()),
            album: None,
            duration_ms: None,
            artwork_url: None,
            source: None,
            explicit: false,
        }
    }

    #[test]
    fn interleaving_takes_turns_and_skips_what_is_known() {
        let lists = vec![
            vec![track("A1", "A"), track("A2", "A"), track("A3", "A")],
            vec![track("B1", "B"), track("A1", "A")],
        ];
        let excluded: HashSet<String> = [song_key("A2", Some("A"))].into_iter().collect();
        let titles: Vec<String> = interleave(lists, 1, 10, &excluded)
            .into_iter()
            .map(|track| track.title)
            .collect();
        assert_eq!(titles, vec!["A1", "B1", "A3"]);
    }

    #[test]
    fn hidden_artists_and_songs_leave_the_kept_mixes() {
        use raygun_model::{HiddenArtist, HiddenItem, HiddenPrefs};
        let mut tracks: Vec<OnlineTrack> = (0..8).map(|n| track(&format!("A{n}"), "A")).collect();
        tracks.extend((0..3).map(|n| track(&format!("B{n}"), "B feat. Hidden One")));
        let mix = made(
            "daily-1".into(),
            MixKind::Daily,
            Some("Rock".into()),
            Some(1),
            tracks,
        );
        let hidden = Hidden::new(&HiddenPrefs {
            artists: vec![HiddenArtist {
                name: "hidden one".into(),
                hidden_at: 1,
            }],
            items: vec![HiddenItem {
                kind: "song".into(),
                key: song_key("A0", Some("A")),
                name: "A0".into(),
                ..HiddenItem::default()
            }],
        });
        let shown = visible(mix.clone(), &hidden).unwrap();
        assert_eq!(
            shown.tracks.len(),
            7,
            "B's songs credit a hidden artist, A0 isn't wanted"
        );
        assert_eq!(shown.artists, vec!["A".to_owned()]);
        assert_eq!(visible(mix.clone(), &Hidden::default()).unwrap(), mix);

        // A mix about a hidden artist goes, and so does one left too small.
        let about = made(
            "because-hidden one".into(),
            MixKind::BecauseYouListened,
            Some("Hidden One".into()),
            None,
            (0..8).map(|n| track(&format!("C{n}"), "C")).collect(),
        );
        assert!(visible(about, &hidden).is_none());
        let small = made(
            "on-repeat".into(),
            MixKind::OnRepeat,
            None,
            None,
            (0..7)
                .map(|n| track(&format!("D{n}"), if n < 3 { "Hidden One" } else { "D" }))
                .collect(),
        );
        assert!(visible(small, &hidden).is_none());
    }

    #[test]
    fn a_local_day_follows_the_offset() {
        let midnight_utc = 1_800_000_000_000 - 1_800_000_000_000 % DAY_MS;
        assert_eq!(local_day(midnight_utc, 0), local_day(midnight_utc + 1, 0));
        assert_eq!(local_day(midnight_utc, -60) + 1, local_day(midnight_utc, 0));
    }
}
