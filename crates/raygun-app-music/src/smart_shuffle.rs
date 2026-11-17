//! Smart Shuffle (a Smart Chip, M46.57): the songs it mixes into a shuffled list, about one for
//! every three of the list's own (`raygun_playback::smart`). They're chosen from the list itself:
//! first what `music.recommend` sources say about its artists (similar artists' best-known songs,
//! an artist radio), then its artists' other popular songs, then Made for You's taste. Songs the
//! list holds, hidden artists and songs, explicit songs while those are off, songs no source has,
//! and songs turned down for this list ("Not for me") are left out. Offline, only songs downloaded
//! to this device are suggested.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use raygun_model::{
    MediaKind, MediaSource, OnlineTrack, PlayContext, QueueItem, QueueItemInput, SongDownloadState,
    TrackInput,
};
use tauri::{AppHandle, Manager};

use crate::hidden::{Hidden, song_key};

/// Songs turned down per list: `{ "<list key>": ["<track key>", ...] }` in `local_meta`.
const NOT_FOR_ME_KEY: &str = "music.smartShuffle.notForMe";
/// Lists' artists asked about, the most frequent first.
const LIST_ARTISTS: usize = 3;
/// Similar artists whose songs are asked for.
const SIMILAR_ARTISTS: usize = 6;
/// Songs of one artist among one refill's suggestions, at most.
const PER_ARTIST: usize = 2;
/// Turned-down songs kept per list, the latest.
const NOT_FOR_ME_KEPT: usize = 500;

/// A list's key for what's turned down in it: the page it plays from, or the queue itself.
pub fn list_key(context: Option<&PlayContext>) -> String {
    context.map_or_else(
        || "queue".to_owned(),
        |context| crate::hidden::page_key(&context.to, &context.search),
    )
}

/// What may not be suggested.
#[derive(Default)]
pub struct Exclusions {
    /// Keys of the list's own songs, and of songs turned down for it.
    pub keys: HashSet<String>,
    pub hidden: Hidden,
    /// Explicit songs are left out (Settings: explicit music off).
    pub no_explicit: bool,
    /// Keys of songs no source has.
    pub unavailable: HashSet<String>,
}

impl Exclusions {
    fn allows(&self, track: &OnlineTrack, key: &str) -> bool {
        !self.keys.contains(key)
            && !self.unavailable.contains(key)
            && !(self.no_explicit && track.explicit)
            && !self.hidden.hides_track(track)
    }
}

/// Up to `wanted` songs from `tiers`, the first tier's first: each song once, at most
/// [`PER_ARTIST`] by an artist, nothing `exclusions` leaves out. Each tier is shuffled by `seed`
/// first, so turning Smart Shuffle on again brings other songs.
pub fn pick(
    tiers: Vec<Vec<OnlineTrack>>,
    exclusions: &Exclusions,
    wanted: usize,
    seed: u64,
) -> Vec<OnlineTrack> {
    let mut picked = Vec::with_capacity(wanted);
    let mut seen = HashSet::new();
    let mut by_artist: HashMap<String, usize> = HashMap::new();
    for (number, mut tier) in tiers.into_iter().enumerate() {
        shuffle(&mut tier, seed.wrapping_add(number as u64));
        for track in tier {
            if picked.len() == wanted {
                return picked;
            }
            let key = song_key(&track.title, track.artist.as_deref());
            if !seen.insert(key.clone()) || !exclusions.allows(&track, &key) {
                continue;
            }
            let artist = crate::hidden::fold(&crate::made_for_you::taste::main_artist(
                track.artist.as_deref().unwrap_or(""),
            ));
            let count = by_artist.entry(artist).or_default();
            if *count >= PER_ARTIST {
                continue;
            }
            *count += 1;
            picked.push(track);
        }
    }
    picked
}

fn shuffle<T>(items: &mut [T], seed: u64) {
    let mut state = seed.max(1);
    for last in (1..items.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        items.swap(last, (state % (last as u64 + 1)) as usize);
    }
}

/// The list's artists, the most frequent first.
pub fn list_artists(own: &[QueueItem]) -> Vec<String> {
    let mut counts: HashMap<String, (usize, String)> = HashMap::new();
    for item in own {
        let Some(artist) = item.artist.as_deref() else {
            continue;
        };
        let main = crate::made_for_you::taste::main_artist(artist);
        if main.is_empty() {
            continue;
        }
        let entry = counts
            .entry(crate::hidden::fold(&main))
            .or_insert((0, main));
        entry.0 += 1;
    }
    let mut ranked: Vec<(usize, String)> = counts.into_values().collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    ranked.into_iter().map(|(_, name)| name).collect()
}

/// A suggested song as the queue takes it: its own source, or found by its names when it plays.
pub fn queue_input(track: OnlineTrack) -> QueueItemInput {
    let source = track.source.clone().unwrap_or(MediaSource::Lookup {
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        duration_ms: track.duration_ms,
    });
    QueueItemInput {
        title: track.title,
        artist: track.artist,
        album: track.album,
        duration_ms: track.duration_ms,
        artwork_url: track.artwork_url,
        media_kind: MediaKind::Audio,
        source,
        explicit: track.explicit,
    }
}

fn db(app: &AppHandle) -> Arc<raygun_db::Db> {
    app.state::<crate::startup::AppDb>().inner().0.clone()
}

async fn not_for_me_lists(app: &AppHandle) -> HashMap<String, Vec<String>> {
    raygun_db::local_value(&db(app), NOT_FOR_ME_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

/// Songs for Smart Shuffle to add to the list `own` (the queue's own songs) played from
/// `context`: at most `wanted`.
pub async fn suggestions(
    app: &AppHandle,
    own: &[QueueItem],
    context: Option<&PlayContext>,
    wanted: usize,
) -> Vec<QueueItemInput> {
    if wanted == 0 {
        return Vec::new();
    }
    let db = db(app);
    let settings = crate::settings::load(&db).await.unwrap_or_default();
    let mut keys: HashSet<String> = own
        .iter()
        .map(|item| song_key(&item.title, item.artist.as_deref()))
        .collect();
    keys.extend(
        not_for_me_lists(app)
            .await
            .remove(&list_key(context))
            .unwrap_or_default(),
    );
    let artists = list_artists(own);
    let tiers = if raygun_net::network_mode::offline() {
        downloaded(app, &artists).await
    } else {
        online(app, &artists).await
    };
    let candidates: Vec<String> = tiers
        .iter()
        .flatten()
        .map(|track| song_key(&track.title, track.artist.as_deref()))
        .collect();
    let unavailable = raygun_db::collections::unavailable(&db, &candidates)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();
    let exclusions = Exclusions {
        keys,
        hidden: Hidden::new(&settings.hidden),
        no_explicit: !settings.explicit_music,
        unavailable,
    };
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |elapsed| elapsed.as_nanos() as u64);
    pick(tiers, &exclusions, wanted, seed)
        .into_iter()
        .map(queue_input)
        .collect()
}

/// The sources' songs, in Smart Shuffle's order of preference.
async fn online(app: &AppHandle, artists: &[String]) -> Vec<Vec<OnlineTrack>> {
    let extensions = app
        .state::<Arc<crate::extensions::ExtensionService>>()
        .inner()
        .clone();
    let hidden = Hidden::load(&db(app)).await;
    let mine: HashSet<String> = artists
        .iter()
        .map(|artist| crate::hidden::fold(artist))
        .collect();
    let seeds: Vec<&String> = artists.iter().take(LIST_ARTISTS).collect();

    // `music.recommend`: artists like the list's, their best-known songs, and a radio.
    let mut recommended = Vec::new();
    let mut similar: Vec<String> = Vec::new();
    for seed in &seeds {
        for name in extensions.similar_artists(seed, 8).await {
            let folded = crate::hidden::fold(&name);
            if !mine.contains(&folded)
                && !hidden.hides_artist(&name)
                && !similar
                    .iter()
                    .any(|known| crate::hidden::fold(known) == folded)
            {
                similar.push(name);
            }
        }
    }
    for name in similar.iter().take(SIMILAR_ARTISTS) {
        recommended.extend(
            extensions
                .recommended_tracks(
                    "topTracks",
                    serde_json::json!({ "artist": name, "limit": 5 }),
                )
                .await,
        );
    }
    if let Some(seed) = seeds.first() {
        recommended.extend(
            extensions
                .recommended_tracks(
                    "radio",
                    serde_json::json!({ "artist": seed, "title": null, "limit": 20 }),
                )
                .await,
        );
    }

    // The list's artists' other popular songs, as their pages list them.
    let mut popular = Vec::new();
    if let Some(profiles) = app.try_state::<Arc<crate::artist_profiles::ArtistProfiles>>() {
        for seed in &seeds {
            let (sources, asked) = (extensions.clone(), (*seed).clone());
            popular.extend(
                profiles
                    .get(
                        seed,
                        crate::artist_profiles::Part::Popular,
                        move || async move {
                            let providers = sources.search(&asked).await;
                            crate::search::artist_tracks(&providers, &asked, 20)
                        },
                    )
                    .await,
            );
        }
    }

    // Made for You's taste.
    let taste = match app.try_state::<Arc<crate::made_for_you::MadeForYouService>>() {
        Some(service) => service.taste_tracks().await,
        None => Vec::new(),
    };
    vec![recommended, popular, taste]
}

/// Offline: the songs downloaded to this device, the list's artists' first.
async fn downloaded(app: &AppHandle, artists: &[String]) -> Vec<Vec<OnlineTrack>> {
    let Some(songs) = app.try_state::<Arc<crate::music_downloads::SongDownloadService>>() else {
        return Vec::new();
    };
    let mine: HashSet<String> = artists
        .iter()
        .map(|artist| crate::hidden::fold(artist))
        .collect();
    let (mut theirs, mut others) = (Vec::new(), Vec::new());
    for song in songs.list().await.unwrap_or_default() {
        if song.state != SongDownloadState::Done {
            continue;
        }
        let artist = song
            .artist
            .as_deref()
            .map(|artist| crate::hidden::fold(&crate::made_for_you::taste::main_artist(artist)));
        let track = OnlineTrack {
            external_id: None,
            title: song.title,
            artist: song.artist,
            album: song.album,
            duration_ms: song.duration_ms,
            artwork_url: song.artwork_url,
            source: None,
            explicit: false,
        };
        if artist.is_some_and(|artist| mine.contains(&artist)) {
            theirs.push(track);
        } else {
            others.push(track);
        }
    }
    vec![theirs, others]
}

/// "Not for me": the suggested song leaves the queue and is never suggested for this list again.
pub async fn not_for_me(app: &AppHandle, item_id: &str) -> Result<(), String> {
    let playback = app.state::<crate::playback::PlaybackService>();
    let (item, context) = playback
        .suggested_item(item_id)
        .ok_or_else(|| "That song isn't one Smart Shuffle added".to_owned())?;
    let key = song_key(&item.title, item.artist.as_deref());
    let mut lists = not_for_me_lists(app).await;
    let list = lists.entry(list_key(context.as_ref())).or_default();
    list.retain(|known| *known != key);
    list.push(key);
    if list.len() > NOT_FOR_ME_KEPT {
        list.drain(..list.len() - NOT_FOR_ME_KEPT);
    }
    let json = serde_json::to_string(&lists).map_err(|error| error.to_string())?;
    raygun_db::set_local_value(&db(app), NOT_FOR_ME_KEY, &json)
        .await
        .map_err(|error| error.to_string())?;
    playback.control(raygun_model::PlaybackControl::Remove {
        item_id: item_id.to_owned(),
    });
    Ok(())
}

/// "Add to this playlist": the suggested song joins the playlist the queue plays from, and is the
/// list's own from now on.
pub async fn add_to_list(app: &AppHandle, item_id: &str) -> Result<(), String> {
    let playback = app.state::<crate::playback::PlaybackService>();
    let (item, context) = playback
        .suggested_item(item_id)
        .ok_or_else(|| "That song isn't one Smart Shuffle added".to_owned())?;
    let playlist = playlist_of(context.as_ref())
        .ok_or_else(|| "This queue doesn't play from a playlist".to_owned())?;
    let source = match &item.source {
        MediaSource::Lookup { .. } => None,
        source => crate::collections::portable(source.clone()),
    };
    let track = TrackInput {
        explicit: item.explicit,
        title: item.title.clone(),
        artist: item.artist.clone(),
        album: item.album.clone(),
        duration_ms: item.duration_ms,
        artwork_url: item
            .artwork_url
            .as_deref()
            .map(|url| app.state::<raygun_net::ImageLinks>().original(url)),
        source,
    };
    app.state::<crate::collections::CollectionsService>()
        .add_to_playlist(&playlist, vec![track], false)
        .await?;
    playback.keep_suggested(item_id);
    Ok(())
}

/// The saved playlist a context is, when it's one.
pub fn playlist_of(context: Option<&PlayContext>) -> Option<String> {
    let context = context?;
    (context.kind == "playlist" && context.to == "/library/playlist")
        .then(|| context.search.get("id").cloned())
        .flatten()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use raygun_model::{HiddenArtist, HiddenPrefs};

    use super::*;

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

    fn titles(tracks: &[OnlineTrack]) -> Vec<&str> {
        tracks.iter().map(|track| track.title.as_str()).collect()
    }

    #[test]
    fn earlier_sources_come_first_and_each_song_once() {
        let tiers = vec![
            vec![track("A1", "A"), track("B1", "B")],
            vec![track("A1", "A"), track("C1", "C")],
            vec![track("D1", "D")],
        ];
        let picked = pick(tiers, &Exclusions::default(), 3, 1);
        let mut first_two = titles(&picked[..2]).to_vec();
        first_two.sort_unstable();
        assert_eq!(first_two, ["A1", "B1"], "the recommendations first");
        assert_eq!(
            picked[2].title, "C1",
            "then the artists' songs; A1 only once"
        );
        assert_eq!(pick(vec![], &Exclusions::default(), 3, 1), vec![]);
    }

    #[test]
    fn the_lists_songs_hidden_explicit_unavailable_and_turned_down_ones_stay_out() {
        let mut explicit = track("Loud", "E");
        explicit.explicit = true;
        let tiers = vec![vec![
            track("In the list", "A"),
            track("Hidden", "Hidden One"),
            explicit,
            track("Gone", "G"),
            track("Turned down", "T"),
            track("Fine", "F"),
        ]];
        let mut keys: HashSet<String> = [song_key("In the list", Some("A"))].into();
        keys.insert(song_key("Turned down", Some("T")));
        let exclusions = Exclusions {
            keys,
            hidden: Hidden::new(&HiddenPrefs {
                artists: vec![HiddenArtist {
                    name: "hidden one".into(),
                    hidden_at: 1,
                }],
                ..HiddenPrefs::default()
            }),
            no_explicit: true,
            unavailable: [song_key("Gone", Some("G"))].into(),
        };
        assert_eq!(titles(&pick(tiers, &exclusions, 10, 7)), ["Fine"]);
    }

    #[test]
    fn one_artist_never_takes_over() {
        let tiers = vec![(0..6).map(|n| track(&format!("S{n}"), "Same")).collect()];
        assert_eq!(pick(tiers, &Exclusions::default(), 6, 3).len(), PER_ARTIST);
    }

    #[test]
    fn refreshes_bring_other_orders() {
        let tier: Vec<OnlineTrack> = (0..12)
            .map(|n| track(&format!("S{n}"), &format!("Artist {n}")))
            .collect();
        let one = pick(vec![tier.clone()], &Exclusions::default(), 3, 1);
        let other = pick(vec![tier], &Exclusions::default(), 3, 99);
        assert_ne!(titles(&one), titles(&other));
    }

    #[test]
    fn lists_are_known_by_their_page_and_artists_by_how_often_they_play() {
        let context = PlayContext {
            kind: "playlist".into(),
            name: "Mine".into(),
            subtitle: None,
            artwork_url: None,
            to: "/library/playlist".into(),
            search: BTreeMap::from([("id".to_owned(), "p1".to_owned())]),
            played_at: 0.0,
        };
        assert_eq!(list_key(Some(&context)), "/library/playlist?id=p1");
        assert_eq!(list_key(None), "queue");
        assert_eq!(playlist_of(Some(&context)).as_deref(), Some("p1"));
        let album = PlayContext {
            kind: "album".into(),
            to: "/music/album".into(),
            ..context
        };
        assert_eq!(playlist_of(Some(&album)), None);

        let item = |artist: &str| QueueItem {
            id: artist.into(),
            title: "t".into(),
            artist: Some(artist.into()),
            album: None,
            duration_ms: None,
            artwork_url: None,
            media_kind: MediaKind::Audio,
            source: MediaSource::Lookup {
                title: "t".into(),
                artist: None,
                album: None,
                duration_ms: None,
            },
            explicit: false,
        };
        let own = [
            item("B"),
            item("A feat. C"),
            item("A"),
            item("B"),
            item("A"),
        ];
        assert_eq!(list_artists(&own), ["A", "B"]);
    }
}

/// "Not for me" on a song Smart Shuffle added: it leaves and isn't suggested for this list again.
#[tauri::command]
#[specta::specta]
pub async fn smart_shuffle_not_for_me(app: AppHandle, item_id: String) -> Result<(), String> {
    not_for_me(&app, &item_id).await
}

/// "Add to this playlist" on a song Smart Shuffle added.
#[tauri::command]
#[specta::specta]
pub async fn smart_shuffle_add_to_list(app: AppHandle, item_id: String) -> Result<(), String> {
    add_to_list(&app, &item_id).await
}
