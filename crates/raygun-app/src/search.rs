//! "Top results" for a search (M18): songs that several providers found, each merged into one
//! track. Songs more providers agree on rank first; a song only one provider knows stays in that
//! provider's own list.

use std::collections::HashMap;

use raygun_model::{OnlineTrack, ProviderSearchResult, TrackQuery};

/// Tracks in the merged list.
pub const TOP_LIMIT: usize = 5;
/// Only each provider's first results take part; deep results are rarely the song asked for.
const CONSIDERED_PER_PROVIDER: usize = 10;

struct Merged {
    track: OnlineTrack,
    providers: usize,
    best_rank: usize,
    first_seen: usize,
}

/// Up to [`TOP_LIMIT`] tracks found by at least two providers, merged by track key.
pub fn merge_top(results: &[ProviderSearchResult]) -> Vec<OnlineTrack> {
    let mut merged: Vec<Merged> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for result in results {
        // A provider listing the same song twice only votes once for it.
        let mut voted: Vec<usize> = Vec::new();
        for (rank, track) in result
            .tracks
            .iter()
            .take(CONSIDERED_PER_PROVIDER)
            .enumerate()
        {
            let key = key(track);
            match index.get(&key) {
                Some(&at) => {
                    let entry = &mut merged[at];
                    fill(&mut entry.track, track);
                    entry.best_rank = entry.best_rank.min(rank);
                    if !voted.contains(&at) {
                        entry.providers += 1;
                        voted.push(at);
                    }
                }
                None => {
                    index.insert(key, merged.len());
                    voted.push(merged.len());
                    merged.push(Merged {
                        track: track.clone(),
                        providers: 1,
                        best_rank: rank,
                        first_seen: merged.len(),
                    });
                }
            }
        }
    }
    merged.sort_by(|a, b| {
        b.providers
            .cmp(&a.providers)
            .then(a.best_rank.cmp(&b.best_rank))
            .then(a.first_seen.cmp(&b.first_seen))
    });
    merged
        .into_iter()
        .filter(|entry| entry.providers >= 2)
        .take(TOP_LIMIT)
        .map(|entry| entry.track)
        .collect()
}

/// The artists named in a credit such as "A, B & C feat. D" (M46.8), trimmed.
pub fn credited_artists(credit: &str) -> Vec<String> {
    const SEPARATORS: [&str; 9] = [
        ", ",
        " & ",
        " feat. ",
        " feat ",
        " ft. ",
        " ft ",
        " featuring ",
        " x ",
        " and ",
    ];
    let mut parts = vec![credit.to_owned()];
    for separator in SEPARATORS {
        parts = parts
            .iter()
            .flat_map(|part| {
                let lower = part.to_lowercase();
                // Split case-insensitively, keeping the part's own spelling.
                let mut pieces = Vec::new();
                let mut start = 0;
                while let Some(found) = lower[start..].find(separator) {
                    pieces.push(part[start..start + found].to_owned());
                    start += found + separator.len();
                }
                pieces.push(part[start..].to_owned());
                pieces
            })
            .collect();
    }
    parts
        .into_iter()
        .map(|part| part.trim().trim_matches(['(', ')']).trim().to_owned())
        .filter(|part| !part.is_empty())
        .collect()
}

/// Whether `credit` names `artist` (case and spacing aside): as one of its artists, or whole
/// between separators, so "Simon & Garfunkel" is found as a duo and as Simon.
pub fn credits_include(credit: &str, artist: &str) -> bool {
    const BEFORE: [&str; 9] = [
        ", ",
        " & ",
        " feat. ",
        " feat ",
        " ft. ",
        " ft ",
        " featuring ",
        " x ",
        " and ",
    ];
    let wanted = artist.trim().to_lowercase();
    if wanted.is_empty() {
        return false;
    }
    if credited_artists(credit)
        .iter()
        .any(|part| part.to_lowercase() == wanted)
    {
        return true;
    }
    let lower = credit.trim().to_lowercase();
    lower.match_indices(&wanted).any(|(at, _)| {
        let before = &lower[..at];
        let after = &lower[at + wanted.len()..];
        (before.is_empty() || BEFORE.iter().any(|separator| before.ends_with(separator)))
            && (after.is_empty() || BEFORE.iter().any(|separator| after.starts_with(separator)))
    })
}

/// Songs the providers found for an artist's name whose credits include them: songs several
/// providers agree on first, then each provider's in its order, without repeats (M46.8).
pub fn artist_tracks(
    results: &[ProviderSearchResult],
    artist: &str,
    limit: usize,
) -> Vec<OnlineTrack> {
    let mut seen = std::collections::HashSet::new();
    let agreed = merge_top(results);
    let each = results
        .iter()
        .flat_map(|result| result.tracks.iter().cloned());
    agreed
        .into_iter()
        .chain(each)
        .filter(|track| {
            track
                .artist
                .as_deref()
                .is_some_and(|credit| credits_include(credit, artist))
        })
        .filter(|track| seen.insert(key(track)))
        .take(limit)
        .collect()
}

fn key(track: &OnlineTrack) -> String {
    raygun_resolver::track_key(&TrackQuery {
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: None,
        duration_ms: None,
    })
}

/// Fills what `track` lacks from another provider's copy of the same song.
fn fill(track: &mut OnlineTrack, other: &OnlineTrack) {
    if track.album.is_none() {
        track.album.clone_from(&other.album);
    }
    if track.duration_ms.is_none() {
        track.duration_ms = other.duration_ms;
    }
    if track.artwork_url.is_none() {
        track.artwork_url.clone_from(&other.artwork_url);
    }
    if track.source.is_none() {
        track.source.clone_from(&other.source);
        track.external_id.clone_from(&other.external_id);
    }
}

#[cfg(test)]
mod artist_tests {
    use raygun_model::{OnlineTrack, ProviderInfo, ProviderSearchResult};

    use super::{artist_tracks, credited_artists, credits_include};

    #[test]
    fn credits_split_into_artists() {
        assert_eq!(
            credited_artists("Daft Punk, Pharrell Williams & Nile Rodgers"),
            ["Daft Punk", "Pharrell Williams", "Nile Rodgers"]
        );
        assert_eq!(
            credited_artists("Calvin Harris feat. Rihanna"),
            ["Calvin Harris", "Rihanna"]
        );
        assert_eq!(
            credited_artists("Simon & Garfunkel"),
            ["Simon", "Garfunkel"]
        );
        assert!(credits_include("Simon & Garfunkel", "simon & garfunkel"));
        assert!(credits_include(
            "Simon & Garfunkel feat. X",
            "Simon & Garfunkel"
        ));
        assert!(credits_include(
            "Daft Punk FT. Julian Casablancas",
            "julian casablancas"
        ));
        assert!(!credits_include("Daft Punk Tribute Band", "Daft Punk"));
        assert!(!credits_include("Daft Punk", ""));
    }

    #[test]
    fn an_artists_songs_keep_to_them_without_repeats() {
        let track = |title: &str, artist: &str| OnlineTrack {
            external_id: None,
            title: title.into(),
            artist: Some(artist.into()),
            album: None,
            duration_ms: None,
            artwork_url: None,
            source: None,
            explicit: false,
        };
        let provider = |id: &str, tracks| ProviderSearchResult {
            provider: ProviderInfo {
                adult: false,
                id: id.into(),
                extension_id: id.into(),
                kind: "music.metadata".into(),
                name: id.into(),
                search_pill: None,
            },
            tracks,
            error: None,
        };
        let results = [
            provider(
                "a",
                vec![
                    track("Get Lucky", "Daft Punk, Pharrell Williams"),
                    track("Get Lucky (Cover)", "Some Band"),
                    track("One More Time", "Daft Punk"),
                ],
            ),
            provider(
                "b",
                vec![
                    track("One More Time", "Daft Punk"),
                    track("Around the World", "Daft Punk"),
                ],
            ),
        ];
        let titles: Vec<String> = artist_tracks(&results, "daft punk", 10)
            .into_iter()
            .map(|track| track.title)
            .collect();
        // The song both providers found leads; the cover isn't theirs; no repeats.
        assert_eq!(titles, ["One More Time", "Get Lucky", "Around the World"]);
    }
}

#[cfg(test)]
mod tests {
    use raygun_model::{MediaSource, ProviderInfo};

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

    fn result(provider: &str, tracks: Vec<OnlineTrack>) -> ProviderSearchResult {
        ProviderSearchResult {
            provider: ProviderInfo {
                adult: false,
                id: format!("{provider}/search"),
                extension_id: provider.into(),
                kind: "music.metadata".into(),
                name: provider.into(),
                search_pill: None,
            },
            tracks,
            error: None,
        }
    }

    fn titles(tracks: &[OnlineTrack]) -> Vec<&str> {
        tracks.iter().map(|track| track.title.as_str()).collect()
    }

    #[test]
    fn songs_several_providers_found_rank_first() {
        let top = merge_top(&[
            result("a", vec![track("Solo A", "X"), track("Shared", "Band")]),
            result(
                "b",
                vec![
                    track("Solo B", "Y"),
                    track("Shared (Remastered 2011)", "BAND"),
                ],
            ),
        ]);
        assert_eq!(
            titles(&top),
            ["Shared"],
            "songs one provider found stay in its list"
        );
    }

    #[test]
    fn more_agreement_ranks_higher_then_position() {
        let top = merge_top(&[
            result("a", vec![track("Pair", "X"), track("Trio", "Y")]),
            result("b", vec![track("Trio", "Y"), track("Pair", "X")]),
            result("c", vec![track("Trio", "Y")]),
        ]);
        assert_eq!(titles(&top), ["Trio", "Pair"]);
    }

    #[test]
    fn copies_fill_each_others_gaps() {
        let mut bare = track("Song", "Artist");
        bare.album = Some("First album".into());
        let mut rich = track("Song", "Artist");
        rich.album = Some("Other album".into());
        rich.duration_ms = Some(200_000);
        rich.artwork_url = Some("https://img/cover.jpg".into());
        rich.source = Some(MediaSource::YouTube {
            video_id: "abc".into(),
        });
        rich.external_id = Some("abc".into());

        let top = merge_top(&[result("a", vec![bare]), result("b", vec![rich])]);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].album.as_deref(), Some("First album"), "kept first");
        assert_eq!(top[0].duration_ms, Some(200_000));
        assert_eq!(top[0].artwork_url.as_deref(), Some("https://img/cover.jpg"));
        assert_eq!(top[0].external_id.as_deref(), Some("abc"));
    }

    #[test]
    fn nothing_to_merge_from_a_single_provider() {
        let lone = result("a", vec![track("One", "X"), track("Two", "X")]);
        let silent = result("b", vec![]);
        assert!(merge_top(&[lone, silent]).is_empty());
    }

    #[test]
    fn duplicates_within_a_provider_vote_once_and_the_list_is_capped() {
        let twice = merge_top(&[
            result("a", vec![track("Twice", "Y"), track("Twice", "Y")]),
            result("b", vec![track("Other", "Z")]),
        ]);
        assert!(
            twice.is_empty(),
            "one provider listing a song twice isn't agreement"
        );

        let many: Vec<_> = (0..8).map(|n| track(&format!("Song {n}"), "X")).collect();
        let top = merge_top(&[result("a", many.clone()), result("b", many)]);
        assert_eq!(top.len(), TOP_LIMIT);
        assert_eq!(top[0].title, "Song 0");
    }
}
