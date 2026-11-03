//! What the user's listening says (M46.20), as pure functions over plays, favorites and song tags:
//! the songs and artists that matter to them now, the songs they've had on repeat or forgotten,
//! what they play at this time of day, and mixes of their own songs by genre and mood. The service
//! adds songs from sources to these; nothing here touches the network.

use std::collections::{HashMap, HashSet};

use raygun_model::{ListeningStats, Mix, MixKind, OnlineTrack};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// Plays older than this count for half as much.
const HALF_LIFE_DAYS: f64 = 30.0;
/// A favorite counts like this many recent plays.
const FAVORITE_WEIGHT: f64 = 3.0;
/// Songs in a mix of the user's own songs.
pub const MIX_SIZE: usize = 40;
/// Below this many songs a mix isn't worth showing.
pub const MIN_MIX: usize = 6;
/// Daily mixes, at most.
const DAILY_MIXES: usize = 6;

/// A song the user played or saved.
#[derive(Debug, Clone, PartialEq)]
pub struct Known {
    pub key: String,
    pub track: OnlineTrack,
    /// Genres first, then moods, as the song's tags say.
    pub genres: Vec<String>,
    pub moods: Vec<String>,
}

/// Everything the mixes are made from.
#[derive(Debug, Clone, Default)]
pub struct Taste {
    pub songs: HashMap<String, Known>,
    /// `(song key, unix ms)`, any order.
    pub plays: Vec<(String, i64)>,
    /// Song key → when it was saved.
    pub favorites: HashMap<String, i64>,
    pub now: i64,
    /// Minutes to add to UTC for the user's clock.
    pub utc_offset_minutes: i32,
}

/// The first artist of a credit: "A & B feat. C" → "A".
pub fn main_artist(credit: &str) -> String {
    let lower = credit.to_lowercase();
    let mut cut = credit.len();
    for separator in [",", " & ", " feat", " ft.", " x ", " with ", " / ", ";"] {
        if let Some(at) = lower.find(separator) {
            cut = cut.min(at);
        }
    }
    credit[..cut].trim().to_owned()
}

/// A steady shuffle: the same `seed` gives the same order (a mix changes daily, not per visit).
pub fn shuffle<T>(items: &mut [T], seed: &str) {
    let mut state = seed.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    }) | 1;
    for index in (1..items.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let other = (state % (index as u64 + 1)) as usize;
        items.swap(index, other);
    }
}

impl Taste {
    fn local_hour(&self, at: i64) -> u8 {
        let local = at / 60_000 + i64::from(self.utc_offset_minutes);
        (local.rem_euclid(24 * 60) / 60) as u8
    }

    /// How much each song matters now: recent plays count most, favorites add a little.
    pub fn weights(&self) -> HashMap<String, f64> {
        let mut weights: HashMap<String, f64> = HashMap::new();
        for (key, at) in &self.plays {
            let age_days = (self.now - at).max(0) as f64 / DAY_MS as f64;
            *weights.entry(key.clone()).or_default() += 0.5_f64.powf(age_days / HALF_LIFE_DAYS);
        }
        for key in self.favorites.keys() {
            *weights.entry(key.clone()).or_default() += FAVORITE_WEIGHT;
        }
        weights.retain(|key, _| self.songs.contains_key(key));
        weights
    }

    /// Songs by how much they matter, the heaviest first (ties by title, so it's steady).
    fn ranked(&self) -> Vec<(String, f64)> {
        let mut ranked: Vec<(String, f64)> = self.weights().into_iter().collect();
        ranked.sort_by(|a, b| {
            b.1.total_cmp(&a.1).then_with(|| {
                self.songs[&a.0]
                    .track
                    .title
                    .cmp(&self.songs[&b.0].track.title)
            })
        });
        ranked
    }

    /// The user's artists by how much they matter now, the most first.
    pub fn top_artists(&self, count: usize) -> Vec<String> {
        let mut artists: HashMap<String, (f64, String)> = HashMap::new();
        for (key, weight) in self.weights() {
            let Some(credit) = self.songs[&key].track.artist.as_deref() else {
                continue;
            };
            let name = main_artist(credit);
            if name.is_empty() {
                continue;
            }
            let entry = artists
                .entry(name.to_lowercase())
                .or_insert((0.0, name.clone()));
            entry.0 += weight;
        }
        let mut list: Vec<(f64, String)> = artists.into_values().collect();
        list.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        list.into_iter().take(count).map(|(_, name)| name).collect()
    }

    /// Every song key the user has played.
    pub fn played(&self) -> HashSet<String> {
        self.plays.iter().map(|(key, _)| key.clone()).collect()
    }

    fn mix(&self, id: String, kind: MixKind, seed: Option<String>, keys: Vec<String>) -> Mix {
        let tracks: Vec<OnlineTrack> = keys
            .iter()
            .filter_map(|key| self.songs.get(key))
            .map(|song| song.track.clone())
            .collect();
        made(id, kind, seed, None, tracks)
    }

    fn plays_by_key(&self, since: i64) -> HashMap<String, u32> {
        let mut counts = HashMap::new();
        for (key, at) in &self.plays {
            if *at >= since {
                *counts.entry(key.clone()).or_default() += 1;
            }
        }
        counts
    }

    /// On Repeat: what the user played most in the last 30 days (twice or more).
    pub fn on_repeat(&self) -> Option<Mix> {
        let counts = self.plays_by_key(self.now - 30 * DAY_MS);
        let mut keys: Vec<(String, u32)> = counts
            .into_iter()
            .filter(|(key, count)| *count >= 2 && self.songs.contains_key(key))
            .collect();
        keys.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let keys: Vec<String> = keys.into_iter().take(30).map(|(key, _)| key).collect();
        (keys.len() >= MIN_MIX).then(|| self.mix("on-repeat".into(), MixKind::OnRepeat, None, keys))
    }

    /// Repeat Rewind: songs played three times or more that haven't been played in 30 days.
    pub fn repeat_rewind(&self) -> Option<Mix> {
        let recent = self.plays_by_key(self.now - 30 * DAY_MS);
        let all = self.plays_by_key(i64::MIN);
        let mut keys: Vec<(String, u32)> = all
            .into_iter()
            .filter(|(key, count)| {
                *count >= 3 && !recent.contains_key(key) && self.songs.contains_key(key)
            })
            .collect();
        keys.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let mut keys: Vec<String> = keys.into_iter().take(30).map(|(key, _)| key).collect();
        shuffle(&mut keys, &format!("rewind{}", self.now / DAY_MS));
        (keys.len() >= MIN_MIX)
            .then(|| self.mix("repeat-rewind".into(), MixKind::RepeatRewind, None, keys))
    }

    /// The part of the day `hour` is in: morning, afternoon, evening or night.
    pub fn daypart(hour: u8) -> &'static str {
        match hour {
            5..=11 => "morning",
            12..=16 => "afternoon",
            17..=21 => "evening",
            _ => "night",
        }
    }

    /// The songs the user plays at this time of day, on other days.
    pub fn time_of_day(&self) -> Option<Mix> {
        let part = Self::daypart(self.local_hour(self.now));
        let today = (self.now / 60_000 + i64::from(self.utc_offset_minutes)).div_euclid(24 * 60);
        let mut counts: HashMap<String, u32> = HashMap::new();
        for (key, at) in &self.plays {
            let day = (at / 60_000 + i64::from(self.utc_offset_minutes)).div_euclid(24 * 60);
            if day != today && Self::daypart(self.local_hour(*at)) == part {
                *counts.entry(key.clone()).or_default() += 1;
            }
        }
        let mut keys: Vec<(String, u32)> = counts
            .into_iter()
            .filter(|(key, _)| self.songs.contains_key(key))
            .collect();
        keys.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let mut keys: Vec<String> = keys
            .into_iter()
            .take(MIX_SIZE)
            .map(|(key, _)| key)
            .collect();
        shuffle(&mut keys, &format!("{part}{today}"));
        (keys.len() >= MIN_MIX).then(|| {
            self.mix(
                format!("time-{part}"),
                MixKind::TimeOfDay,
                Some(part.to_owned()),
                keys,
            )
        })
    }

    /// The user's songs grouped by `pick` (a song's genre or mood), the heaviest groups first, each
    /// with at least `MIN_MIX` songs: `(name, keys by weight)`.
    fn groups(&self, pick: impl Fn(&Known) -> Option<String>) -> Vec<(String, Vec<String>)> {
        let mut groups: HashMap<String, (f64, Vec<String>)> = HashMap::new();
        for (key, weight) in self.ranked() {
            if let Some(name) = pick(&self.songs[&key]) {
                let entry = groups.entry(name).or_default();
                entry.0 += weight;
                entry.1.push(key);
            }
        }
        let mut list: Vec<(String, (f64, Vec<String>))> = groups
            .into_iter()
            .filter(|(_, (_, keys))| keys.len() >= MIN_MIX)
            .collect();
        list.sort_by(|a, b| b.1.0.total_cmp(&a.1.0).then_with(|| a.0.cmp(&b.0)));
        list.into_iter()
            .map(|(name, (_, keys))| (name, keys))
            .collect()
    }

    /// Daily Mixes: the user's songs by their main genre (by main artist when songs aren't tagged
    /// yet), one mix per group, in a daily order. The service adds similar artists' songs.
    pub fn daily_mixes(&self) -> Vec<Mix> {
        let mut groups = self.groups(|song| song.genres.first().cloned());
        if groups.is_empty() {
            let artists: HashMap<String, String> = self
                .songs
                .iter()
                .filter_map(|(key, song)| {
                    Some((key.clone(), main_artist(song.track.artist.as_deref()?)))
                })
                .collect();
            groups = self.groups(|song| artists.get(&song.key).cloned());
        }
        let day = self.now / DAY_MS;
        groups
            .into_iter()
            .take(DAILY_MIXES)
            .enumerate()
            .map(|(index, (name, mut keys))| {
                keys.truncate(MIX_SIZE);
                shuffle(&mut keys, &format!("daily{name}{day}"));
                let mut mix = self.mix(
                    format!("daily-{}", index + 1),
                    MixKind::Daily,
                    Some(name),
                    keys,
                );
                mix.number = Some(index as u32 + 1);
                mix
            })
            .collect()
    }

    /// A mix for each of the user's two strongest moods.
    pub fn mood_mixes(&self) -> Vec<Mix> {
        let day = self.now / DAY_MS;
        self.groups(|song| song.moods.first().cloned())
            .into_iter()
            .take(2)
            .map(|(name, mut keys)| {
                keys.truncate(MIX_SIZE);
                shuffle(&mut keys, &format!("mood{name}{day}"));
                self.mix(
                    format!("mood-{}", name.to_lowercase()),
                    MixKind::Mood,
                    Some(name),
                    keys,
                )
            })
            .collect()
    }

    /// The user's three strongest genres, for genre mixes the service fills from sources.
    pub fn top_genres(&self, count: usize) -> Vec<(String, Vec<String>)> {
        self.groups(|song| song.genres.first().cloned())
            .into_iter()
            .take(count)
            .collect()
    }

    /// The user's song `key`, as a track.
    pub fn track(&self, key: &str) -> Option<&OnlineTrack> {
        self.songs.get(key).map(|song| &song.track)
    }

    /// Numbers for the "Your listening" cards: this week's plays and minutes, this month's top
    /// artist and song, and the hour the user plays most.
    pub fn stats(&self) -> ListeningStats {
        let week = self.now - 7 * DAY_MS;
        let month = self.now - 30 * DAY_MS;
        let mut plays_this_week = 0;
        let mut ms_this_week: u64 = 0;
        let mut hours = [0u32; 24];
        let mut songs: HashMap<&str, u32> = HashMap::new();
        let mut artists: HashMap<String, (u32, String)> = HashMap::new();
        for (key, at) in &self.plays {
            hours[usize::from(self.local_hour(*at))] += 1;
            let song = self.songs.get(key);
            if *at >= week {
                plays_this_week += 1;
                ms_this_week += song
                    .and_then(|song| song.track.duration_ms)
                    .map_or(180_000, u64::from);
            }
            if *at >= month {
                *songs.entry(key.as_str()).or_default() += 1;
                if let Some(credit) = song.and_then(|song| song.track.artist.as_deref()) {
                    let name = main_artist(credit);
                    artists.entry(name.to_lowercase()).or_insert((0, name)).0 += 1;
                }
            }
        }
        let top_song = songs
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))
            .and_then(|(key, _)| self.track(key).cloned());
        let top_artist = artists
            .into_values()
            .max_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
            .map(|(_, name)| name);
        let peak = hours
            .iter()
            .enumerate()
            .max_by_key(|(hour, count)| (**count, std::cmp::Reverse(*hour)))
            .filter(|(_, count)| **count > 0)
            .map(|(hour, _)| hour as u8);
        ListeningStats {
            plays_this_week,
            minutes_this_week: (ms_this_week / 60_000) as u32,
            top_artist,
            top_song,
            peak_hour: peak,
            songs_known: self.songs.len() as u32,
        }
    }
}

/// A mix of `tracks`: its artwork is the first four different covers, and its artists the first
/// three different main artists.
pub fn made(
    id: String,
    kind: MixKind,
    seed: Option<String>,
    number: Option<u32>,
    tracks: Vec<OnlineTrack>,
) -> Mix {
    let mut artwork_urls: Vec<String> = Vec::new();
    let mut artists: Vec<String> = Vec::new();
    for track in &tracks {
        if let Some(url) = &track.artwork_url
            && artwork_urls.len() < 4
            && !artwork_urls.contains(url)
        {
            artwork_urls.push(url.clone());
        }
        if let Some(credit) = &track.artist {
            let name = main_artist(credit);
            if artists.len() < 3
                && !name.is_empty()
                && !artists
                    .iter()
                    .any(|known| known.eq_ignore_ascii_case(&name))
            {
                artists.push(name);
            }
        }
    }
    Mix {
        id,
        kind,
        seed,
        number,
        artists,
        artwork_urls,
        tracks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000_000;

    fn song(title: &str, artist: &str, genre: Option<&str>, mood: Option<&str>) -> Known {
        Known {
            key: format!("{}|{}", artist.to_lowercase(), title.to_lowercase()),
            track: OnlineTrack {
                external_id: None,
                title: title.into(),
                artist: Some(artist.into()),
                album: None,
                duration_ms: Some(200_000),
                artwork_url: Some(format!("https://img.example/{title}.jpg")),
                source: None,
                explicit: false,
            },
            genres: genre.map(str::to_owned).into_iter().collect(),
            moods: mood.map(str::to_owned).into_iter().collect(),
        }
    }

    fn taste(songs: Vec<Known>) -> Taste {
        Taste {
            songs: songs
                .into_iter()
                .map(|song| (song.key.clone(), song))
                .collect(),
            now: NOW,
            ..Taste::default()
        }
    }

    fn play(taste: &mut Taste, title_artist: (&str, &str), days_ago: i64, times: usize) {
        let key = format!(
            "{}|{}",
            title_artist.1.to_lowercase(),
            title_artist.0.to_lowercase()
        );
        for time in 0..times {
            taste
                .plays
                .push((key.clone(), NOW - days_ago * DAY_MS - time as i64 * 1000));
        }
    }

    #[test]
    fn credits_have_a_main_artist() {
        assert_eq!(main_artist("Daft Punk feat. Pharrell"), "Daft Punk");
        assert_eq!(main_artist("Simon & Garfunkel"), "Simon");
        assert_eq!(main_artist("KAROL G, Judeline"), "KAROL G");
        assert_eq!(main_artist("Nina Simone"), "Nina Simone");
    }

    #[test]
    fn shuffles_are_steady_for_a_seed() {
        let mut one: Vec<u32> = (0..20).collect();
        let mut two = one.clone();
        shuffle(&mut one, "day 1");
        shuffle(&mut two, "day 1");
        assert_eq!(one, two);
        let mut three: Vec<u32> = (0..20).collect();
        shuffle(&mut three, "day 2");
        assert_ne!(one, three);
        let mut sorted = one.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn on_repeat_and_repeat_rewind_come_from_plays() {
        let titles: Vec<String> = (0..8).map(|n| format!("Song {n}")).collect();
        let mut taste = taste(
            titles
                .iter()
                .map(|title| song(title, "Band", None, None))
                .collect(),
        );
        for (index, title) in titles.iter().enumerate() {
            if index < 6 {
                play(&mut taste, (title, "Band"), 2, 2 + index);
            } else {
                play(&mut taste, (title, "Band"), 60, 4);
            }
        }
        let repeat = taste.on_repeat().unwrap();
        assert_eq!(repeat.tracks.len(), 6);
        assert_eq!(repeat.tracks[0].title, "Song 5", "the most played first");
        assert!(
            taste.repeat_rewind().is_none(),
            "two forgotten songs aren't a mix"
        );
        assert_eq!(repeat.artists, vec!["Band".to_owned()]);
        assert_eq!(repeat.artwork_urls.len(), 4);
    }

    #[test]
    fn daily_mixes_follow_genres_or_artists() {
        let mut songs = Vec::new();
        for n in 0..7 {
            songs.push(song(
                &format!("Rock {n}"),
                "Rocker",
                Some("Rock"),
                Some("Energetic"),
            ));
            songs.push(song(
                &format!("Jazz {n}"),
                "Jazzer",
                Some("Jazz"),
                Some("Chill"),
            ));
        }
        songs.push(song("Lone", "Solo", Some("Polka"), None));
        let mut taste = taste(songs);
        for n in 0..7 {
            play(&mut taste, (&format!("Rock {n}"), "Rocker"), 1, 3);
            play(&mut taste, (&format!("Jazz {n}"), "Jazzer"), 1, 1);
        }
        let mixes = taste.daily_mixes();
        assert_eq!(mixes.len(), 2, "Polka has too few songs");
        assert_eq!(mixes[0].seed.as_deref(), Some("Rock"));
        assert_eq!(mixes[0].number, Some(1));
        assert_eq!(mixes[1].seed.as_deref(), Some("Jazz"));
        assert_eq!(taste.mood_mixes().len(), 2);
        assert_eq!(taste.top_artists(1), vec!["Rocker".to_owned()]);

        // Untagged songs group by artist.
        for song in taste.songs.values_mut() {
            song.genres.clear();
        }
        let mixes = taste.daily_mixes();
        assert_eq!(mixes[0].seed.as_deref(), Some("Rocker"));
    }

    #[test]
    fn stats_count_this_week_and_the_peak_hour() {
        let mut taste = taste(vec![
            song("A", "One", None, None),
            song("B", "Two", None, None),
        ]);
        play(&mut taste, ("A", "One"), 1, 3);
        play(&mut taste, ("B", "Two"), 10, 1);
        let stats = taste.stats();
        assert_eq!(stats.plays_this_week, 3);
        assert_eq!(stats.minutes_this_week, 10);
        assert_eq!(stats.top_artist.as_deref(), Some("One"));
        assert_eq!(stats.top_song.map(|song| song.title).as_deref(), Some("A"));
        assert!(stats.peak_hour.is_some());
    }

    #[test]
    fn the_time_of_day_mix_uses_other_days_at_this_hour() {
        let titles: Vec<String> = (0..7).map(|n| format!("Night {n}")).collect();
        let mut taste = taste(
            titles
                .iter()
                .map(|title| song(title, "Owl", None, None))
                .collect(),
        );
        for title in &titles {
            // The same time of day, earlier days.
            play(&mut taste, (title, "Owl"), 3, 1);
        }
        let mix = taste.time_of_day().unwrap();
        assert_eq!(mix.tracks.len(), 7);
        assert_eq!(mix.kind, MixKind::TimeOfDay);
    }
}
