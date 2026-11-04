//! One identity for saved songs (M46.48). A song is saved with the names it came with, and those
//! differ from screen to screen: a Spotify import credits every artist ("Daft Punk, Pharrell
//! Williams") and decorates titles ("Get Lucky - Radio Edit"), most sources credit one artist,
//! and the user may rename a song or an artist (Edit details, Find a source). Screens ask about a
//! song by the names they show, so a saved copy answers to several *song keys* — its stored key,
//! its names' keys, and the keys of the names it shows with the user's details — and two keys
//! are one song when their titles agree and one artist credit holds every word of the other.
//! `music/trackKey.ts` mirrors `plain_title`, `song_key` and `same_song` for the lists' marks.

use std::collections::{HashMap, HashSet};

use raygun_db::collections::SavedRef;
use raygun_model::{SavedSong, TrackQuery, TrackSaves};
use raygun_resolver::normalize;

/// Words that make what follows " - " in a title a decoration rather than part of the name, as
/// brackets are: "Song - Remastered 2011", "Song - Live at Wembley", "Song - From \"Film\"".
const DECORATIONS: &[&str] = &[
    "acoustic",
    "album",
    "anniversary",
    "bonus",
    "clean",
    "deluxe",
    "demo",
    "edit",
    "edition",
    "explicit",
    "extended",
    "feat",
    "featuring",
    "from",
    "ft",
    "instrumental",
    "live",
    "mix",
    "mono",
    "original",
    "radio",
    "recorded",
    "remaster",
    "remastered",
    "remix",
    "session",
    "single",
    "stereo",
    "unplugged",
    "version",
];

const SEPARATORS: [&str; 3] = [" - ", " – ", " — "];

fn is_year(word: &str) -> bool {
    word.len() == 4
        && (word.starts_with("19") || word.starts_with("20"))
        && word.chars().all(|c| c.is_ascii_digit())
}

fn is_decoration(rest: &str) -> bool {
    normalize(rest)
        .split(' ')
        .any(|word| DECORATIONS.contains(&word) || is_year(word))
}

/// A title without a decoration after " - " (see [`DECORATIONS`]); other titles as they are.
pub fn plain_title(title: &str) -> &str {
    let mut cut: Option<usize> = None;
    for separator in SEPARATORS {
        let mut from = 0;
        while let Some(found) = title[from..].find(separator) {
            let at = from + found;
            if is_decoration(&title[at + separator.len()..]) {
                cut = Some(cut.map_or(at, |earlier| earlier.min(at)));
                break;
            }
            from = at + separator.len();
        }
    }
    match cut {
        Some(at) if !title[..at].trim().is_empty() => title[..at].trim_end(),
        _ => title,
    }
}

fn track_key(title: &str, artist: Option<&str>) -> String {
    raygun_resolver::track_key(&TrackQuery {
        title: title.to_owned(),
        artist: artist.map(str::to_owned),
        album: None,
        duration_ms: None,
    })
}

/// The resolver's key of a song with its title's " - " decoration dropped.
pub fn song_key(title: &str, artist: Option<&str>) -> String {
    track_key(plain_title(title), artist)
}

/// The keys a song with these names answers to: its song key and the resolver's key.
pub fn keys_of(title: &str, artist: Option<&str>) -> Vec<String> {
    let song = song_key(title, artist);
    let track = track_key(title, artist);
    if song == track {
        vec![song]
    } else {
        vec![song, track]
    }
}

/// Whether two keys (`artist|title`, normalized) are one song: the same title, and artists where
/// one credit holds every word of the other ("daft punk" and "daft punk pharrell williams").
/// A key without an artist is only ever the same as itself.
pub fn same_song(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let (Some((artist_a, title_a)), Some((artist_b, title_b))) =
        (a.split_once('|'), b.split_once('|'))
    else {
        return false;
    };
    if title_a != title_b || title_a.is_empty() {
        return false;
    }
    let words = |artist: &str| -> HashSet<String> {
        artist
            .split(' ')
            .filter(|word| !word.is_empty())
            .map(str::to_owned)
            .collect()
    };
    let (words_a, words_b) = (words(artist_a), words(artist_b));
    !words_a.is_empty()
        && !words_b.is_empty()
        && (words_a.is_subset(&words_b) || words_b.is_subset(&words_a))
}

/// Whether any key of `a` is the same song as any key of `b`.
pub fn any_same(a: &[String], b: &[String]) -> bool {
    a.iter()
        .any(|left| b.iter().any(|right| same_song(left, right)))
}

/// A key's title part: only keys with the same title can be one song.
fn title_of(key: &str) -> &str {
    key.split_once('|').map_or(key, |(_, title)| title)
}

/// Songs by the keys they answer to, looked up by title (M46.48: merging and repairing).
#[derive(Default)]
pub struct SongSet {
    songs: Vec<Vec<String>>,
    by_title: HashMap<String, Vec<usize>>,
}

impl SongSet {
    pub fn contains(&self, keys: &[String]) -> bool {
        keys.iter().any(|key| {
            self.by_title.get(title_of(key)).is_some_and(|found| {
                found
                    .iter()
                    .any(|&at| any_same(&self.songs[at], std::slice::from_ref(key)))
            })
        })
    }

    /// Adds a song; false when one of its keys was there already.
    pub fn insert(&mut self, keys: Vec<String>) -> bool {
        if self.contains(&keys) {
            return false;
        }
        let at = self.songs.len();
        for key in &keys {
            let bucket = self.by_title.entry(title_of(key).to_owned()).or_default();
            if !bucket.contains(&at) {
                bucket.push(at);
            }
        }
        self.songs.push(keys);
        true
    }
}

/// The keys a saved row answers to: its stored key, its names', and those of the names it shows
/// with the user's details (`shown`, when they differ).
pub fn row_keys(
    track_key: &str,
    title: &str,
    artist: Option<&str>,
    shown: Option<(String, Option<String>)>,
) -> Vec<String> {
    let mut keys = vec![track_key.to_owned()];
    let mut add = |more: Vec<String>| {
        for key in more {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    };
    add(keys_of(title, artist));
    if let Some((title, artist)) = shown {
        add(keys_of(&title, artist.as_deref()));
    }
    keys
}

/// What makes a song saved exactly where a `TrackSaves` says (M46.8's Add to playlist).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SavesPlan {
    pub add_favorite: bool,
    pub remove_favorites: Vec<String>,
    pub add_to: Vec<String>,
    pub remove_items: Vec<String>,
}

/// Every live favorite and playlist item with the keys it answers to, oldest first.
pub struct SavedIndex {
    rows: Vec<(SavedRef, Vec<String>)>,
    /// Rows by the titles of their keys.
    by_title: HashMap<String, Vec<usize>>,
}

impl SavedIndex {
    /// `shown` gives the names a row shows with the user's details, when they differ.
    pub fn new(
        rows: Vec<SavedRef>,
        shown: impl Fn(&SavedRef) -> Option<(String, Option<String>)>,
    ) -> Self {
        let rows: Vec<(SavedRef, Vec<String>)> = rows
            .into_iter()
            .map(|row| {
                let keys = row_keys(
                    &row.track_key,
                    &row.title,
                    row.artist.as_deref(),
                    shown(&row),
                );
                (row, keys)
            })
            .collect();
        let mut by_title: HashMap<String, Vec<usize>> = HashMap::new();
        for (at, (_, keys)) in rows.iter().enumerate() {
            for key in keys {
                let bucket = by_title.entry(title_of(key).to_owned()).or_default();
                if bucket.last() != Some(&at) {
                    bucket.push(at);
                }
            }
        }
        Self { rows, by_title }
    }

    /// The saved copies of the song with these keys (from [`keys_of`]), oldest first.
    pub fn copies(&self, keys: &[String]) -> Vec<&SavedRef> {
        let mut found: Vec<usize> = keys
            .iter()
            .filter_map(|key| self.by_title.get(title_of(key)))
            .flatten()
            .copied()
            .filter(|&at| any_same(&self.rows[at].1, keys))
            .collect();
        found.sort_unstable();
        found.dedup();
        found.into_iter().map(|at| &self.rows[at].0).collect()
    }

    /// The stream a saved copy of the song has (as stored, JSON), for a new copy of it.
    pub fn source_of(&self, keys: &[String]) -> Option<String> {
        self.copies(keys)
            .into_iter()
            .rev()
            .find_map(|row| row.source.clone())
    }

    /// Whether Favorites (`None`) or a playlist holds the song.
    pub fn holds(&self, playlist_id: Option<&str>, keys: &[String]) -> bool {
        self.copies(keys)
            .iter()
            .any(|row| row.playlist_id.as_deref() == playlist_id)
    }

    /// Where the song is saved.
    pub fn saves(&self, keys: &[String]) -> TrackSaves {
        let copies = self.copies(keys);
        let mut playlist_ids: Vec<String> = Vec::new();
        for id in copies.iter().filter_map(|row| row.playlist_id.as_ref()) {
            if !playlist_ids.contains(id) {
                playlist_ids.push(id.clone());
            }
        }
        TrackSaves {
            favorite: copies.iter().any(|row| row.is_favorite()),
            playlist_ids,
        }
    }

    /// What to add and take away so the song is saved where `wanted` says: Favorites on or off,
    /// added to listed playlists that don't hold it, every copy out of those not listed.
    pub fn plan(&self, keys: &[String], wanted: &TrackSaves) -> SavesPlan {
        let copies = self.copies(keys);
        let mut plan = SavesPlan::default();
        let favorites: Vec<String> = copies
            .iter()
            .filter(|row| row.is_favorite())
            .map(|row| row.id.clone())
            .collect();
        if wanted.favorite {
            plan.add_favorite = favorites.is_empty();
        } else {
            plan.remove_favorites = favorites;
        }
        for id in &wanted.playlist_ids {
            let held = copies
                .iter()
                .any(|row| row.playlist_id.as_ref() == Some(id));
            if !held && !plan.add_to.contains(id) {
                plan.add_to.push(id.clone());
            }
        }
        plan.remove_items = copies
            .iter()
            .filter(|row| {
                row.playlist_id
                    .as_ref()
                    .is_some_and(|id| !wanted.playlist_ids.contains(id))
            })
            .map(|row| row.id.clone())
            .collect();
        plan
    }

    /// Every saved song as lists mark it: one entry per stored key, with every key its copies
    /// answer to and where they are.
    pub fn songs(&self) -> Vec<SavedSong> {
        let mut songs: Vec<SavedSong> = Vec::new();
        let mut places: HashMap<&str, usize> = HashMap::new();
        for (row, keys) in &self.rows {
            let at = *places.entry(row.track_key.as_str()).or_insert_with(|| {
                songs.push(SavedSong {
                    track_key: row.track_key.clone(),
                    song_keys: Vec::new(),
                    favorite: false,
                    playlist_ids: Vec::new(),
                });
                songs.len() - 1
            });
            let song = &mut songs[at];
            for key in keys {
                if !song.song_keys.contains(key) {
                    song.song_keys.push(key.clone());
                }
            }
            match &row.playlist_id {
                None => song.favorite = true,
                Some(id) if !song.playlist_ids.contains(id) => song.playlist_ids.push(id.clone()),
                Some(_) => {}
            }
        }
        songs
    }

    /// Every key some saved copy answers to.
    pub fn all_keys(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        self.rows
            .iter()
            .flat_map(|(_, keys)| keys.iter())
            .filter(|key| seen.insert(key.as_str()))
            .cloned()
            .collect()
    }

    /// The repair of what saving songs under several names left (M46.48): in Favorites and in
    /// each playlist, the first copy of a song stays and later ones go; a row whose key isn't
    /// its names' key gets it.
    pub fn repair(&self) -> Repair {
        let mut repair = Repair::default();
        let mut kept: HashMap<Option<&str>, SongSet> = HashMap::new();
        for (row, keys) in &self.rows {
            let first = kept
                .entry(row.playlist_id.as_deref())
                .or_default()
                .insert(keys.clone());
            if !first {
                if row.is_favorite() {
                    repair.drop_favorites.push(row.id.clone());
                } else {
                    repair.drop_items.push(row.id.clone());
                }
                continue;
            }
            let expected = track_key(&row.title, row.artist.as_deref());
            if row.track_key != expected {
                let change = (row.id.clone(), expected);
                if row.is_favorite() {
                    repair.rekey_favorites.push(change);
                } else {
                    repair.rekey_items.push(change);
                }
            }
        }
        repair
    }
}

/// See [`SavedIndex::repair`]: ids to tombstone and `(id, key)` pairs to rekey.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Repair {
    pub drop_favorites: Vec<String>,
    pub drop_items: Vec<String>,
    pub rekey_favorites: Vec<(String, String)>,
    pub rekey_items: Vec<(String, String)>,
}

impl Repair {
    pub fn is_empty(&self) -> bool {
        self.drop_favorites.is_empty()
            && self.drop_items.is_empty()
            && self.rekey_favorites.is_empty()
            && self.rekey_items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(id: &str, playlist: Option<&str>, title: &str, artist: Option<&str>) -> SavedRef {
        SavedRef {
            id: id.into(),
            playlist_id: playlist.map(str::to_owned),
            track_key: track_key(title, artist),
            title: title.into(),
            artist: artist.map(str::to_owned),
            added_at: 0,
            source: None,
        }
    }

    #[test]
    fn decorations_after_a_dash_go_like_brackets() {
        for (title, plain) in [
            ("Bohemian Rhapsody - Remastered 2011", "Bohemian Rhapsody"),
            ("Space Oddity - 2015 Remaster", "Space Oddity"),
            ("Shallow - From A Star Is Born Soundtrack", "Shallow"),
            ("One - Live", "One"),
            ("Song – Radio Edit", "Song"),
            ("Tom Sawyer - Live - Remastered", "Tom Sawyer"),
            // Part of the name: kept.
            ("Love Me - Do", "Love Me - Do"),
            ("Ode to Joy - Symphony No 9", "Ode to Joy - Symphony No 9"),
            ("- Live", "- Live"),
            ("Song", "Song"),
        ] {
            assert_eq!(plain_title(title), plain, "{title}");
        }
        assert_eq!(
            song_key("Get Lucky - Radio Edit", Some("Daft Punk")),
            "daft punk|get lucky"
        );
        assert_eq!(
            keys_of("Get Lucky - Radio Edit", Some("Daft Punk")),
            ["daft punk|get lucky", "daft punk|get lucky radio edit"]
        );
        assert_eq!(
            keys_of("Get Lucky", Some("Daft Punk")),
            ["daft punk|get lucky"]
        );
    }

    #[test]
    fn one_song_under_several_artist_credits() {
        // The cases `music/trackKey.test.ts` checks too.
        let spotify = song_key(
            "Get Lucky",
            Some("Daft Punk, Pharrell Williams, Nile Rodgers"),
        );
        assert!(same_song(
            &spotify,
            &song_key("Get Lucky", Some("Daft Punk"))
        ));
        assert!(same_song(
            &song_key("Get Lucky", Some("Pharrell Williams")),
            &spotify
        ));
        assert!(same_song(
            &song_key("Under Pressure", Some("Queen & David Bowie")),
            &song_key("Under Pressure", Some("David Bowie"))
        ));
        assert!(same_song(
            &song_key("Yesterday", Some("Beatles")),
            &song_key("Yesterday", Some("The Beatles"))
        ));
        // Other artists, other titles, and songs without an artist stay apart.
        assert!(!same_song(
            &song_key("Hello", Some("Adele")),
            &song_key("Hello", Some("Lionel Richie"))
        ));
        assert!(!same_song(
            &song_key("Paint It Black", Some("The Rolling Stones")),
            &song_key("Paint It Black", Some("The Beatles"))
        ));
        assert!(!same_song(
            &song_key("Get Lucky", Some("Daft Punk")),
            &song_key("Lose Yourself to Dance", Some("Daft Punk"))
        ));
        assert!(!same_song(
            &song_key("Song", None),
            &song_key("Song", Some("Someone"))
        ));
        assert!(same_song(&song_key("Song", None), &song_key("Song", None)));
        assert!(!same_song("a|", "a b|"));
    }

    #[test]
    fn a_song_set_keeps_each_song_once() {
        let mut set = SongSet::default();
        assert!(set.insert(keys_of("Get Lucky", Some("Daft Punk, Pharrell Williams"))));
        assert!(!set.insert(keys_of("Get Lucky - Radio Edit", Some("Daft Punk"))));
        assert!(set.insert(keys_of("Get Lucky", Some("Someone Else"))));
        assert!(set.contains(&keys_of("Get Lucky", Some("Pharrell Williams"))));
        assert!(!set.contains(&keys_of("Around the World", Some("Daft Punk"))));
    }

    #[test]
    fn an_imported_song_is_found_by_the_names_other_screens_show() {
        let index = SavedIndex::new(
            vec![
                saved(
                    "i1",
                    Some("road"),
                    "Get Lucky - Radio Edit",
                    Some("Daft Punk, Pharrell Williams"),
                ),
                saved(
                    "f1",
                    None,
                    "Get Lucky (feat. Pharrell Williams)",
                    Some("Daft Punk"),
                ),
                saved("i2", Some("gym"), "Other", Some("Daft Punk")),
            ],
            |_| None,
        );
        let deezer = keys_of("Get Lucky", Some("Daft Punk"));
        assert_eq!(
            index.saves(&deezer),
            TrackSaves {
                favorite: true,
                playlist_ids: vec!["road".into()],
            }
        );
        assert!(index.holds(Some("road"), &deezer));
        assert!(!index.holds(Some("gym"), &deezer));

        // Taken out of Road (every copy) and kept in Favorites; added to Gym.
        let plan = index.plan(
            &deezer,
            &TrackSaves {
                favorite: true,
                playlist_ids: vec!["gym".into(), "gym".into()],
            },
        );
        assert_eq!(
            plan,
            SavesPlan {
                add_favorite: false,
                remove_favorites: vec![],
                add_to: vec!["gym".into()],
                remove_items: vec!["i1".into()],
            }
        );
        let off = index.plan(
            &deezer,
            &TrackSaves {
                favorite: false,
                playlist_ids: vec![],
            },
        );
        assert_eq!(off.remove_favorites, ["f1"]);
        assert_eq!(off.remove_items, ["i1"]);
    }

    #[test]
    fn a_renamed_song_answers_to_its_shown_names() {
        // Imported from a screenshot as "Sogn" by "Artst", fixed with Find a source.
        let index = SavedIndex::new(
            vec![saved("i1", Some("road"), "Sogn", Some("Artst"))],
            |row| (row.title == "Sogn").then(|| ("Song".to_owned(), Some("Artist".to_owned()))),
        );
        let shown = keys_of("Song", Some("Artist"));
        assert_eq!(index.saves(&shown).playlist_ids, ["road"]);
        assert_eq!(
            index.saves(&keys_of("Sogn", Some("Artst"))).playlist_ids,
            ["road"]
        );
        assert!(index.all_keys().contains(&"artist|song".to_owned()));
        let songs = index.songs();
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].track_key, "artst|sogn");
        assert_eq!(songs[0].song_keys, ["artst|sogn", "artist|song"]);
        assert_eq!(songs[0].playlist_ids, ["road"]);
    }

    #[test]
    fn the_repair_keeps_the_first_copy_and_fixes_keys() {
        let mut stale = saved("i4", Some("gym"), "Song", Some("Artist"));
        stale.track_key = "old|key".into();
        let index = SavedIndex::new(
            vec![
                saved(
                    "i1",
                    Some("road"),
                    "Get Lucky",
                    Some("Daft Punk, Pharrell Williams"),
                ),
                saved("f1", None, "Get Lucky", Some("Daft Punk")),
                saved(
                    "i2",
                    Some("road"),
                    "Get Lucky - Radio Edit",
                    Some("Daft Punk"),
                ),
                saved("i3", Some("gym"), "Get Lucky", Some("Daft Punk")),
                stale,
                saved(
                    "f2",
                    None,
                    "Get Lucky - Radio Edit",
                    Some("Daft Punk, Pharrell Williams"),
                ),
                // Another artist's song of the same name isn't a copy.
                saved("f3", None, "Get Lucky", Some("Someone Else")),
                saved("i5", Some("road"), "Other", Some("Daft Punk")),
            ],
            |_| None,
        );
        assert_eq!(
            index.repair(),
            Repair {
                drop_favorites: vec!["f2".into()],
                drop_items: vec!["i2".into()],
                rekey_favorites: vec![],
                rekey_items: vec![("i4".into(), "artist|song".into())],
            }
        );
        assert!(SavedIndex::new(vec![], |_| None).repair().is_empty());
    }
}
