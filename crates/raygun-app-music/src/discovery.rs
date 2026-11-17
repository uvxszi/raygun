//! Music discovery (M46.9): charts from `music.charts` providers, cached a little while, and
//! genres and moods for songs from `music.tags` providers (plus a library file's own genre), kept
//! per song in `local_track_tags`. A background tagger works through the songs the user keeps
//! that have none yet, one at a time with a rest between them.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use raygun_model::{
    MediaSource, MusicCharts, MusicTag, MusicTagSources, OnlineTrack, SavedTrack, SourceFailure,
    TagCount, TagKind, TagSongsRow, TrackQuery, TrackTags,
};
use tauri::{AppHandle, Manager};
use tokio::sync::Notify;

use crate::extensions::ExtensionService;
use crate::genres;
use crate::startup::AppDb;

/// How long a region's chart list and a chart's songs are kept.
const CHARTS_FOR: Duration = Duration::from_secs(60 * 60);
const CHART_SONGS_FOR: Duration = Duration::from_secs(30 * 60);
/// Songs asked per chart, and per source for a genre or mood.
const CHART_LIMIT: u32 = 50;
/// How long a genre's or mood's songs from the sources are kept (M46.19).
const TAG_SOURCES_FOR: Duration = Duration::from_secs(30 * 60);
/// The tagger starts after the app has settled, rests between songs (MusicBrainz allows one
/// request a second, and a song can take two), and looks again for new songs now and then.
const TAGGER_DELAY: Duration = Duration::from_secs(20);
const TAGGER_REST: Duration = Duration::from_secs(3);
const TAGGER_IDLE: Duration = Duration::from_secs(10 * 60);
const TAGGER_BATCH: i64 = 25;
/// After this many songs in a row that no source could answer, the tagger waits a while.
const TAGGER_FAILURES: u32 = 5;

type ChartKey = (String, String, String);

pub struct DiscoveryService {
    app: AppHandle,
    charts: Mutex<HashMap<String, (Instant, MusicCharts)>>,
    songs: Mutex<HashMap<ChartKey, (Instant, Vec<OnlineTrack>)>>,
    tag_sources: Mutex<HashMap<(String, String), (Instant, MusicTagSources)>>,
    wake: Arc<Notify>,
}

/// A song's query and its key.
fn keyed(query: &TrackQuery) -> String {
    raygun_resolver::track_key(query)
}

/// A chart's songs with repeats left out, each at its first place: a source can list one song
/// twice (ListenBrainz counts some listens under a second entry for the same recording).
fn without_repeats(songs: Vec<OnlineTrack>) -> Vec<OnlineTrack> {
    let mut seen = HashSet::new();
    songs
        .into_iter()
        .filter(|song| {
            seen.insert(keyed(&TrackQuery {
                title: song.title.clone(),
                artist: song.artist.clone(),
                album: None,
                duration_ms: None,
            }))
        })
        .collect()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}

/// A file's genre field split into genres ("Rock; Pop", "Rock/Pop").
fn file_genres(genre: Option<&str>) -> Vec<(String, f64)> {
    genre
        .into_iter()
        .flat_map(|genre| genre.split([';', '/', ',']))
        .map(str::trim)
        .filter(|genre| !genre.is_empty())
        .map(|genre| (genre.to_owned(), 1.0))
        .collect()
}

fn parse_tags(json: &str) -> Vec<MusicTag> {
    serde_json::from_str(json).unwrap_or_default()
}

impl DiscoveryService {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app,
            charts: Mutex::new(HashMap::new()),
            songs: Mutex::new(HashMap::new()),
            tag_sources: Mutex::new(HashMap::new()),
            wake: Arc::new(Notify::new()),
        })
    }

    fn db(&self) -> &raygun_db::Db {
        &self.app.state::<AppDb>().inner().0
    }

    fn extensions(&self) -> Arc<ExtensionService> {
        self.app.state::<Arc<ExtensionService>>().inner().clone()
    }

    /// Every source's charts for `region`, genre charts named in the app's vocabulary.
    pub async fn charts(&self, region: Option<String>) -> MusicCharts {
        let region = region.filter(|region| !region.is_empty());
        let key = region.clone().unwrap_or_default();
        if let Some((at, charts)) = self.charts.lock().unwrap().get(&key)
            && at.elapsed() < CHARTS_FOR
        {
            return charts.clone();
        }
        let answers = self.extensions().music_charts(region.as_deref()).await;
        let mut found = MusicCharts {
            charts: Vec::new(),
            errors: Vec::new(),
        };
        for (provider, answer) in answers {
            match answer {
                // A genre outside the vocabulary keeps the source's name, so the chart still
                // counts as a genre's rather than a main chart.
                Ok(charts) => found.charts.extend(charts.into_iter().map(|mut chart| {
                    chart.tag = chart
                        .tag
                        .map(|tag| genres::canonical(&tag).map_or(tag, |known| known.name));
                    chart
                })),
                Err(error) => found
                    .errors
                    .push(raygun_net::issue::named(&provider.name, &error)),
            }
        }
        // A failure is asked again next time; an answer is kept.
        if !found.charts.is_empty() {
            self.charts
                .lock()
                .unwrap()
                .insert(key, (Instant::now(), found.clone()));
        }
        found
    }

    /// A chart's songs.
    pub async fn chart(
        &self,
        provider_id: &str,
        chart_id: &str,
        region: Option<String>,
    ) -> Result<Vec<OnlineTrack>, String> {
        let region = region.filter(|region| !region.is_empty());
        let key = (
            provider_id.to_owned(),
            chart_id.to_owned(),
            region.clone().unwrap_or_default(),
        );
        if let Some((at, songs)) = self.songs.lock().unwrap().get(&key)
            && at.elapsed() < CHART_SONGS_FOR
        {
            return Ok(songs.clone());
        }
        let songs = without_repeats(
            self.extensions()
                .music_chart(provider_id, chart_id, region.as_deref(), CHART_LIMIT)
                .await?,
        );
        self.songs
            .lock()
            .unwrap()
            .insert(key, (Instant::now(), songs.clone()));
        Ok(songs)
    }

    /// Songs for a genre or a mood from every chart source that has some (M46.19), a row per
    /// source in the sources' order; empty rows are left out.
    pub async fn tag_sources(&self, name: String, region: Option<String>) -> MusicTagSources {
        let region = region.filter(|region| !region.is_empty());
        let key = (name.clone(), region.clone().unwrap_or_default());
        if let Some((at, found)) = self.tag_sources.lock().unwrap().get(&key)
            && at.elapsed() < TAG_SOURCES_FOR
        {
            return found.clone();
        }
        let kind = match genres::canonical(&name).map(|tag| tag.kind) {
            Some(TagKind::Mood) => "mood",
            _ => "genre",
        };
        let aliases = genres::aliases(&name);
        let answers = self
            .extensions()
            .music_tag_songs(&name, &aliases, kind, region.as_deref(), CHART_LIMIT)
            .await;
        let mut found = MusicTagSources {
            rows: Vec::new(),
            failures: Vec::new(),
        };
        for (provider, answer) in answers {
            match answer {
                Ok((_, tracks)) if tracks.is_empty() => {}
                Ok((row_name, tracks)) => found.rows.push(TagSongsRow {
                    name: row_name.unwrap_or_else(|| format!("{name} on {}", provider.name)),
                    provider,
                    tracks,
                }),
                Err(error) => found.failures.push(SourceFailure {
                    source: provider.name,
                    error,
                }),
            }
        }
        // Answers are kept; a page where every source failed asks again next time.
        if !found.rows.is_empty() {
            self.tag_sources
                .lock()
                .unwrap()
                .insert(key, (Instant::now(), found.clone()));
        }
        found
    }

    /// Asks the sources for a song's tags. `Err` when no source could answer, so the song is
    /// tried again later rather than remembered as having none.
    async fn fetch_tags(
        &self,
        query: &TrackQuery,
        file_genre: Option<&str>,
    ) -> Result<Vec<MusicTag>, String> {
        let answers = self.extensions().music_tags(query).await;
        let mut found: Vec<Vec<(String, f64)>> = vec![file_genres(file_genre)];
        let mut errors = Vec::new();
        let mut answered = !found[0].is_empty();
        for (provider, answer) in answers {
            match answer {
                Ok(tags) => {
                    answered = true;
                    found.push(tags);
                }
                Err(error) => errors.push(raygun_net::issue::named(&provider.name, &error)),
            }
        }
        if !answered {
            return Err(errors
                .into_iter()
                .next()
                .unwrap_or_else(|| "No source finds song genres".into()));
        }
        Ok(genres::merge(&found))
    }

    async fn store(&self, key: &str, tags: &[MusicTag]) -> Result<(), String> {
        let json = serde_json::to_string(tags).map_err(|error| error.to_string())?;
        raygun_db::track_tags::put(self.db(), key, &json, now_ms())
            .await
            .map_err(|error| error.to_string())
    }

    /// A song's tags: the stored ones, or the sources' (then stored).
    pub async fn track_tags(&self, query: TrackQuery) -> Result<Vec<MusicTag>, String> {
        let key = keyed(&query);
        let stored = raygun_db::track_tags::get_many(self.db(), std::slice::from_ref(&key))
            .await
            .map_err(|error| error.to_string())?;
        if let Some((_, json)) = stored.first() {
            return Ok(parse_tags(json));
        }
        let tags = self.fetch_tags(&query, None).await?;
        // Incognito (M47): a song looked up while it's on isn't remembered.
        if !crate::privacy::incognito(&self.app) {
            self.store(&key, &tags).await?;
        }
        Ok(tags)
    }

    /// Stored tags for many songs at once; songs without any are left out.
    pub async fn tags_for(&self, keys: Vec<String>) -> Result<Vec<TrackTags>, String> {
        let rows = raygun_db::track_tags::get_many(self.db(), &keys)
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows
            .into_iter()
            .map(|(track_key, json)| TrackTags {
                track_key,
                tags: parse_tags(&json),
            })
            .filter(|row| !row.tags.is_empty())
            .collect())
    }

    /// The tags across the songs the user keeps, the most common first.
    pub async fn summary(&self) -> Result<Vec<TagCount>, String> {
        let rows = raygun_db::track_tags::kept(self.db())
            .await
            .map_err(|error| error.to_string())?;
        let mut counts: HashMap<MusicTag, u32> = HashMap::new();
        for (_, json) in rows {
            for tag in parse_tags(&json) {
                *counts.entry(tag).or_default() += 1;
            }
        }
        let mut summary: Vec<TagCount> = counts
            .into_iter()
            .map(|(tag, count)| TagCount {
                name: tag.name,
                kind: tag.kind,
                count,
            })
            .collect();
        summary.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
        Ok(summary)
    }

    /// Favorites and playlist songs with the tag `name`.
    pub async fn songs_with_tag(&self, name: &str) -> Result<Vec<SavedTrack>, String> {
        let rows = raygun_db::track_tags::saved_with_tag(self.db(), name)
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows
            .into_iter()
            .map(|row| SavedTrack {
                explicit: false,
                track_key: row.track_key,
                title: row.title,
                artist: row.artist,
                album: row.album,
                duration_ms: row.duration_ms.and_then(|ms| u32::try_from(ms).ok()),
                artwork_url: row.artwork_url,
                source: row
                    .source
                    .as_deref()
                    .and_then(|json| serde_json::from_str::<MediaSource>(json).ok()),
                local: false,
                added_at: None,
                play_count: 0,
            })
            .collect())
    }

    /// Wakes the tagger (songs were saved, or tagging was turned on).
    pub fn nudge(&self) {
        self.wake.notify_one();
    }

    /// Starts the background tagger.
    pub fn start_tagger(self: &Arc<Self>) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(TAGGER_DELAY).await;
            service.run_tagger().await;
        });
    }

    async fn tagging_on(&self) -> bool {
        crate::settings::load(self.db())
            .await
            .is_ok_and(|settings| settings.discovery.tag_songs)
    }

    async fn run_tagger(&self) {
        // Songs no source could answer this session, so they don't hold up the rest.
        let mut skipped: HashSet<String> = HashSet::new();
        let mut failures = 0u32;
        loop {
            // Offline Mode, or Data Saver's part for background lookups: none until they end.
            crate::network_mode::wait_for_lookups().await;
            let waiting = if self.tagging_on().await {
                raygun_db::track_tags::untagged(self.db(), TAGGER_BATCH + skipped.len() as i64)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|song| !skipped.contains(&song.track_key))
                    .collect()
            } else {
                Vec::new()
            };
            if waiting.is_empty() {
                let _ = tokio::time::timeout(TAGGER_IDLE, self.wake.notified()).await;
                continue;
            }
            for song in waiting {
                if !self.tagging_on().await {
                    break;
                }
                let query = TrackQuery {
                    title: song.title.clone(),
                    artist: song.artist.clone(),
                    album: song.album.clone(),
                    duration_ms: song.duration_ms.and_then(|ms| u32::try_from(ms).ok()),
                };
                match self.fetch_tags(&query, song.file_genre.as_deref()).await {
                    Ok(tags) => {
                        failures = 0;
                        if let Err(error) = self.store(&song.track_key, &tags).await {
                            tracing::warn!(%error, "couldn't store a song's tags");
                        }
                    }
                    Err(error) => {
                        tracing::info!(song = %song.track_key, %error, "no tags for a song yet");
                        skipped.insert(song.track_key);
                        failures += 1;
                        if failures >= TAGGER_FAILURES {
                            // Offline, or every source is down: look again later.
                            failures = 0;
                            skipped.clear();
                            let _ = tokio::time::timeout(TAGGER_IDLE, self.wake.notified()).await;
                            break;
                        }
                    }
                }
                // Slower in battery saver or idle in the background.
                crate::device_state::rest(TAGGER_REST).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_genres_split_on_separators() {
        assert_eq!(
            file_genres(Some("Rock; Pop/Electronic, ")),
            [
                ("Rock".to_owned(), 1.0),
                ("Pop".to_owned(), 1.0),
                ("Electronic".to_owned(), 1.0)
            ]
        );
        assert!(file_genres(None).is_empty());
    }

    #[test]
    fn charts_list_each_song_once() {
        let song = |title: &str, artist: &str| OnlineTrack {
            external_id: None,
            title: title.to_owned(),
            artist: Some(artist.to_owned()),
            album: None,
            duration_ms: None,
            artwork_url: None,
            source: None,
            explicit: false,
        };
        let titles: Vec<String> = without_repeats(vec![
            song("SWIM", "BTS"),
            song("NORMAL", "BTS"),
            song("Swim", "BTS"),
            song("SWIM", "Another Band"),
        ])
        .into_iter()
        .map(|song| format!("{} / {}", song.title, song.artist.unwrap_or_default()))
        .collect();
        assert_eq!(
            titles,
            ["SWIM / BTS", "NORMAL / BTS", "SWIM / Another Band"]
        );
    }
}
