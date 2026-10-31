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
        }
    }

    fn result(provider: &str, tracks: Vec<OnlineTrack>) -> ProviderSearchResult {
        ProviderSearchResult {
            provider: ProviderInfo {
                id: format!("{provider}/search"),
                extension_id: provider.into(),
                kind: "music.metadata".into(),
                name: provider.into(),
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
