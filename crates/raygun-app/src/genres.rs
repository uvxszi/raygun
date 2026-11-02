//! Genres and moods for songs (M46.9). Sources tag songs in their own words ("Rap/Hip Hop",
//! "hip hop", "Electro"); this module maps them onto one vocabulary, drops tags that aren't
//! genres ("seen live", "british"), and derives moods (Chill, Energetic…) from genres, as
//! Spotify's Browse groups music.

use std::collections::HashMap;
use std::sync::OnceLock;

use raygun_model::{MusicTag, TagKind};

/// Each genre and the spellings sources use for it (lowercase words, punctuation as spaces).
const GENRES: &[(&str, &[&str])] = &[
    (
        "Pop",
        &[
            "pop",
            "dance pop",
            "electropop",
            "electro pop",
            "teen pop",
            "international pop",
        ],
    ),
    ("Dance", &["dance", "dance music"]),
    (
        "Electronic",
        &["electronic", "electronica", "electro", "electronic music"],
    ),
    (
        "House",
        &[
            "house",
            "deep house",
            "tech house",
            "progressive house",
            "future house",
        ],
    ),
    ("Techno", &["techno", "minimal techno"]),
    ("Trance", &["trance", "psytrance", "psychedelic trance"]),
    (
        "Drum & Bass",
        &[
            "drum and bass",
            "dnb",
            "drum n bass",
            "jungle",
            "liquid funk",
        ],
    ),
    ("Dubstep", &["dubstep", "brostep", "riddim"]),
    ("EDM", &["edm", "big room", "electro house"]),
    ("Hardstyle", &["hardstyle", "gabber", "hardcore techno"]),
    ("Breakcore", &["breakcore"]),
    ("Phonk", &["phonk", "drift phonk"]),
    ("Hyperpop", &["hyperpop", "digicore"]),
    ("Synth-pop", &["synthpop", "synth pop"]),
    (
        "Synthwave",
        &["synthwave", "retrowave", "outrun", "darksynth"],
    ),
    ("Vaporwave", &["vaporwave"]),
    (
        "Lo-fi",
        &["lo fi", "lofi", "lo fi hip hop", "lofi hip hop", "chillhop"],
    ),
    ("Ambient", &["ambient", "drone", "dark ambient"]),
    (
        "Downtempo",
        &["downtempo", "trip hop", "chillout", "chill out"],
    ),
    (
        "Hip-Hop",
        &[
            "hip hop",
            "hiphop",
            "rap",
            "rap hip hop",
            "hip hop rap",
            "conscious hip hop",
            "boom bap",
        ],
    ),
    ("Trap", &["trap", "trap music", "rage"]),
    ("Drill", &["drill", "uk drill"]),
    ("Grime", &["grime"]),
    (
        "R&B",
        &[
            "r and b",
            "rnb",
            "r b",
            "rhythm and blues",
            "contemporary r and b",
        ],
    ),
    ("Soul", &["soul", "neo soul", "soul and funk", "motown"]),
    ("Funk", &["funk"]),
    ("Disco", &["disco", "nu disco"]),
    (
        "Rock",
        &[
            "rock",
            "classic rock",
            "hard rock",
            "pop rock",
            "rock and roll",
            "rock n roll",
            "electro rock",
        ],
    ),
    (
        "Alternative",
        &["alternative", "alternative rock", "alt rock"],
    ),
    ("Indie", &["indie", "indie rock", "indie pop"]),
    ("Punk", &["punk", "punk rock", "pop punk", "hardcore punk"]),
    ("Emo", &["emo", "midwest emo"]),
    (
        "Metal",
        &[
            "metal",
            "heavy metal",
            "metalcore",
            "thrash metal",
            "death metal",
            "black metal",
            "doom metal",
            "nu metal",
            "deathcore",
        ],
    ),
    ("Grunge", &["grunge"]),
    ("Shoegaze", &["shoegaze"]),
    ("Dream Pop", &["dream pop"]),
    ("Post-rock", &["post rock"]),
    ("Post-punk", &["post punk"]),
    ("New Wave", &["new wave"]),
    ("Gothic", &["gothic", "goth", "darkwave", "gothic rock"]),
    ("Industrial", &["industrial", "ebm"]),
    ("Witch House", &["witch house"]),
    ("Folk", &["folk", "indie folk", "folk rock", "folk pop"]),
    ("Country", &["country", "country pop", "americana"]),
    ("Singer-Songwriter", &["singer songwriter"]),
    ("Acoustic", &["acoustic"]),
    ("Blues", &["blues", "blues rock"]),
    (
        "Jazz",
        &["jazz", "smooth jazz", "jazz fusion", "bebop", "swing"],
    ),
    (
        "Classical",
        &[
            "classical",
            "classical music",
            "orchestral",
            "opera",
            "baroque",
            "piano",
        ],
    ),
    (
        "Soundtrack",
        &[
            "soundtrack",
            "film score",
            "films games",
            "film soundtrack",
            "video game music",
            "game soundtrack",
            "ost",
            "score",
        ],
    ),
    ("Anime", &["anime", "anisong"]),
    ("J-Pop", &["j pop", "jpop"]),
    ("J-Rock", &["j rock", "jrock"]),
    ("K-Pop", &["k pop", "kpop"]),
    ("City Pop", &["city pop"]),
    (
        "Asian",
        &["asian music", "asian", "c pop", "mandopop", "cantopop"],
    ),
    ("Vocaloid", &["vocaloid"]),
    (
        "Latin",
        &["latin", "latin music", "latin pop", "musica latina"],
    ),
    ("Reggaeton", &["reggaeton", "reggaetón", "urbano latino"]),
    ("Salsa", &["salsa"]),
    ("Cumbia", &["cumbia"]),
    (
        "Regional Mexican",
        &[
            "traditional mexicano",
            "regional mexican",
            "corridos",
            "corridos tumbados",
        ],
    ),
    (
        "Brazilian",
        &[
            "brazilian music",
            "brazilian",
            "mpb",
            "funk carioca",
            "sertanejo",
        ],
    ),
    ("Bossa Nova", &["bossa nova"]),
    (
        "Afrobeats",
        &[
            "afrobeats",
            "afrobeat",
            "african music",
            "amapiano",
            "afropop",
        ],
    ),
    ("Reggae", &["reggae", "dancehall", "dub", "roots reggae"]),
    ("Ska", &["ska", "ska punk"]),
    (
        "Gospel",
        &["gospel", "christian", "christian music", "worship", "ccm"],
    ),
    (
        "Indian",
        &["indian music", "bollywood", "desi", "indian pop", "filmi"],
    ),
    ("Kids", &["kids", "children s music", "childrens music"]),
    ("Experimental", &["experimental", "noise", "avant garde"]),
    ("Instrumental", &["instrumental"]),
    ("Chiptune", &["chiptune", "8 bit", "8bit"]),
    ("Jersey Club", &["jersey club"]),
    ("UK Garage", &["uk garage", "2 step", "garage"]),
];

/// Moods, and the words sources use for them directly.
const MOODS: &[(&str, &[&str])] = &[
    ("Chill", &["chill", "relaxing", "mellow", "calm", "chilled"]),
    (
        "Energetic",
        &["energetic", "upbeat", "energy", "hype", "high energy"],
    ),
    (
        "Happy",
        &["happy", "feel good", "uplifting", "cheerful", "fun"],
    ),
    (
        "Sad",
        &[
            "sad",
            "melancholic",
            "melancholy",
            "depressing",
            "heartbreak",
        ],
    ),
    ("Dark", &["dark", "eerie", "sinister"]),
    (
        "Romantic",
        &["romantic", "love", "love song", "love songs", "sensual"],
    ),
    ("Party", &["party"]),
    ("Focus", &["focus", "study", "concentration"]),
    ("Workout", &["workout", "gym", "running"]),
    ("Sleep", &["sleep", "sleepy", "lullaby"]),
];

/// The moods each genre suggests.
const GENRE_MOODS: &[(&str, &[&str])] = &[
    ("Pop", &["Happy"]),
    ("Dance", &["Energetic", "Party"]),
    ("Electronic", &["Energetic"]),
    ("House", &["Party"]),
    ("Techno", &["Energetic", "Party"]),
    ("Trance", &["Energetic"]),
    ("Drum & Bass", &["Energetic", "Workout"]),
    ("Dubstep", &["Energetic", "Workout"]),
    ("EDM", &["Energetic", "Party", "Workout"]),
    ("Hardstyle", &["Energetic", "Workout"]),
    ("Breakcore", &["Energetic"]),
    ("Phonk", &["Energetic", "Dark", "Workout"]),
    ("Hyperpop", &["Energetic"]),
    ("Synth-pop", &["Happy"]),
    ("Synthwave", &["Focus"]),
    ("Vaporwave", &["Chill"]),
    ("Lo-fi", &["Chill", "Focus"]),
    ("Ambient", &["Chill", "Focus", "Sleep"]),
    ("Downtempo", &["Chill"]),
    ("Hip-Hop", &["Party", "Workout"]),
    ("Trap", &["Energetic", "Party", "Workout"]),
    ("Drill", &["Dark", "Energetic"]),
    ("Grime", &["Energetic"]),
    ("R&B", &["Romantic", "Chill"]),
    ("Soul", &["Romantic", "Chill"]),
    ("Funk", &["Happy", "Party"]),
    ("Disco", &["Happy", "Party"]),
    ("Rock", &["Energetic"]),
    ("Punk", &["Energetic"]),
    ("Emo", &["Sad"]),
    ("Metal", &["Energetic", "Dark", "Workout"]),
    ("Shoegaze", &["Sad", "Chill"]),
    ("Dream Pop", &["Chill"]),
    ("Post-rock", &["Focus", "Sad"]),
    ("Post-punk", &["Dark"]),
    ("Gothic", &["Dark"]),
    ("Industrial", &["Dark", "Energetic"]),
    ("Witch House", &["Dark"]),
    ("Singer-Songwriter", &["Sad"]),
    ("Acoustic", &["Chill"]),
    ("Blues", &["Sad"]),
    ("Jazz", &["Chill"]),
    ("Classical", &["Focus", "Sleep"]),
    ("Soundtrack", &["Focus"]),
    ("K-Pop", &["Happy", "Party"]),
    ("J-Pop", &["Happy"]),
    ("City Pop", &["Chill", "Happy"]),
    ("Latin", &["Happy", "Party"]),
    ("Reggaeton", &["Party"]),
    ("Bossa Nova", &["Chill", "Romantic"]),
    ("Afrobeats", &["Happy", "Party"]),
    ("Reggae", &["Chill"]),
    ("Ska", &["Happy"]),
    ("Kids", &["Happy"]),
    ("Instrumental", &["Focus"]),
    ("Chiptune", &["Focus", "Happy"]),
    ("Jersey Club", &["Party", "Energetic"]),
    ("UK Garage", &["Party"]),
];

/// Most genres and moods kept for one song.
const MAX_GENRES: usize = 4;
const MAX_MOODS: usize = 3;
/// A genre needs this much weight across sources to count (one source's main genre is 1).
const MIN_WEIGHT: f64 = 0.3;

/// Lowercase words separated by single spaces; `&` reads "and", apostrophes join.
fn simplify(text: &str) -> String {
    let lowered = text
        .to_lowercase()
        .replace('&', " and ")
        .replace(['\'', '’'], "");
    lowered
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn lookup() -> &'static HashMap<String, (&'static str, TagKind)> {
    static TABLE: OnceLock<HashMap<String, (&'static str, TagKind)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = HashMap::new();
        for (kind, list) in [(TagKind::Genre, GENRES), (TagKind::Mood, MOODS)] {
            for (name, spellings) in list {
                table.insert(simplify(name), (*name, kind));
                for spelling in *spellings {
                    table.insert(simplify(spelling), (*name, kind));
                }
            }
        }
        // Deezer's combined genres.
        table.insert(simplify("Rap/Hip Hop"), ("Hip-Hop", TagKind::Genre));
        table.insert(simplify("Soul & Funk"), ("Soul", TagKind::Genre));
        table.insert(simplify("Films/Games"), ("Soundtrack", TagKind::Genre));
        table
    })
}

/// A source's tag in the app's vocabulary, or `None` when it isn't a genre or a mood.
pub fn canonical(tag: &str) -> Option<MusicTag> {
    lookup().get(&simplify(tag)).map(|(name, kind)| MusicTag {
        name: (*name).to_owned(),
        kind: *kind,
    })
}

/// A genre's or a mood's other spellings, as sources use them (M46.19: asking sources for its
/// songs); empty for a name outside the vocabulary.
pub fn aliases(name: &str) -> Vec<String> {
    GENRES
        .iter()
        .chain(MOODS)
        .find(|(known, _)| *known == name)
        .map(|(known, spellings)| {
            let own = simplify(known);
            spellings
                .iter()
                .map(|spelling| (*spelling).to_owned())
                .filter(|spelling| *spelling != own)
                .collect()
        })
        .unwrap_or_default()
}

/// Every genre and mood, in the order Browse shows them.
pub fn vocabulary() -> Vec<MusicTag> {
    MOODS
        .iter()
        .map(|(name, _)| (*name, TagKind::Mood))
        .chain(GENRES.iter().map(|(name, _)| (*name, TagKind::Genre)))
        .map(|(name, kind)| MusicTag {
            name: name.to_owned(),
            kind,
        })
        .collect()
}

/// A song's tags from every source's `(tag, weight)` answers: genres that enough weight agrees on
/// (the strongest first, at most four), then moods named by sources or suggested by the genres.
pub fn merge(answers: &[Vec<(String, f64)>]) -> Vec<MusicTag> {
    let mut genres: Vec<(String, f64)> = Vec::new();
    let mut moods: Vec<(String, f64)> = Vec::new();
    for answer in answers {
        for (tag, weight) in answer {
            // Combined genres ("Techno/House") count as each part.
            let found: Vec<MusicTag> = match canonical(tag) {
                Some(found) => vec![found],
                None => tag.split('/').filter_map(canonical).collect(),
            };
            for found in found {
                let list = match found.kind {
                    TagKind::Genre => &mut genres,
                    TagKind::Mood => &mut moods,
                };
                match list.iter_mut().find(|(name, _)| *name == found.name) {
                    Some((_, total)) => *total += weight,
                    None => list.push((found.name, *weight)),
                }
            }
        }
    }
    let by_weight = |a: &(String, f64), b: &(String, f64)| b.1.total_cmp(&a.1);
    genres.retain(|(_, weight)| *weight >= MIN_WEIGHT);
    genres.sort_by(by_weight);
    genres.truncate(MAX_GENRES);
    // The top genres each suggest their moods, weaker than a source naming the mood itself.
    for (genre, weight) in genres.iter().take(2) {
        let suggested = GENRE_MOODS
            .iter()
            .find(|(name, _)| name == genre)
            .map_or(&[][..], |(_, moods)| *moods);
        for mood in suggested {
            match moods.iter_mut().find(|(name, _)| name == mood) {
                Some((_, total)) => *total += weight * 0.5,
                None => moods.push(((*mood).to_owned(), weight * 0.5)),
            }
        }
    }
    moods.sort_by(by_weight);
    moods.truncate(MAX_MOODS);
    genres
        .into_iter()
        .map(|(name, _)| MusicTag {
            name,
            kind: TagKind::Genre,
        })
        .chain(moods.into_iter().map(|(name, _)| MusicTag {
            name,
            kind: TagKind::Mood,
        }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(tags: &[MusicTag]) -> Vec<(&str, TagKind)> {
        tags.iter()
            .map(|tag| (tag.name.as_str(), tag.kind))
            .collect()
    }

    #[test]
    fn spellings_meet_in_one_vocabulary() {
        for (spelling, name) in [
            ("Rap/Hip Hop", "Hip-Hop"),
            ("hip hop", "Hip-Hop"),
            ("Hip-Hop", "Hip-Hop"),
            ("R&B", "R&B"),
            ("rnb", "R&B"),
            ("Drum & Bass", "Drum & Bass"),
            ("drum and bass", "Drum & Bass"),
            ("Electro", "Electronic"),
            ("Films/Games", "Soundtrack"),
            ("lo-fi", "Lo-fi"),
            ("K-Pop", "K-Pop"),
            ("breakcore", "Breakcore"),
        ] {
            assert_eq!(canonical(spelling).unwrap().name, name, "{spelling}");
        }
        assert_eq!(canonical("chill").unwrap().kind, TagKind::Mood);
        assert!(canonical("seen live").is_none());
        assert!(canonical("british").is_none());
    }

    #[test]
    fn genres_agree_across_sources_and_suggest_moods() {
        let tags = merge(&[
            vec![("Dance".into(), 1.0), ("Pop".into(), 0.7)],
            vec![
                ("funk".into(), 1.0),
                ("happy".into(), 1.0),
                ("seen live".into(), 1.0),
                ("disco".into(), 0.2),
            ],
        ]);
        assert_eq!(
            names(&tags),
            [
                ("Dance", TagKind::Genre),
                ("Funk", TagKind::Genre),
                ("Pop", TagKind::Genre),
                ("Happy", TagKind::Mood),
                ("Party", TagKind::Mood),
                ("Energetic", TagKind::Mood),
            ]
        );
        assert!(merge(&[vec![("seen live".into(), 1.0)]]).is_empty());
        let combined = merge(&[vec![("Techno/House".into(), 1.0)]]);
        assert_eq!(combined[0].name, "Techno");
        assert_eq!(combined[1].name, "House");
    }

    #[test]
    fn every_mood_a_genre_suggests_exists() {
        let moods: Vec<_> = MOODS.iter().map(|(name, _)| *name).collect();
        for (genre, suggested) in GENRE_MOODS {
            assert!(canonical(genre).is_some(), "{genre}");
            for mood in *suggested {
                assert!(moods.contains(mood), "{genre}: {mood}");
            }
        }
        assert_eq!(vocabulary().len(), GENRES.len() + MOODS.len());
    }

    #[test]
    fn names_come_with_their_other_spellings() {
        let hip_hop = aliases("Hip-Hop");
        assert!(hip_hop.iter().any(|alias| alias == "rap"), "{hip_hop:?}");
        assert!(
            !hip_hop.contains(&"hip hop".to_owned()),
            "its own simple spelling is left out"
        );
        assert!(aliases("Chill").contains(&"relaxing".to_owned()));
        assert!(aliases("Polka dots").is_empty());
    }
}
