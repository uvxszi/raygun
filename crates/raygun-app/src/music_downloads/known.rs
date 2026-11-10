//! Downloads start from the song's known source (M46.53): the stream playing it now, a source
//! picked for it (Find a source, a source switch), or one that played it before. A fresh search
//! can come up empty ("no source has it") for a song that plays fine, so it comes only after
//! these gave no file. They play this song, so they aren't held to the search's matching rules.

use raygun_db::music_downloads::SongDownloadRow;
use raygun_model::TrackQuery;

/// The keys a download's song is remembered under: the key it was saved under, the names it
/// shows now (`query`, Edit details) and the keys saved songs know it by.
pub(super) fn song_keys(row: &SongDownloadRow, query: &TrackQuery) -> Vec<String> {
    let mut keys =
        crate::collections::identity::keys_of(&row.song.title, row.song.artist.as_deref());
    keys.push(row.song.track_key.clone());
    keys.push(raygun_resolver::track_key(query));
    keys.sort_unstable();
    keys.dedup();
    keys
}

#[cfg(test)]
mod tests {
    use raygun_db::music_downloads::{NewSong, Origin};
    use raygun_model::{MediaKind, MediaSource, QueueItem};

    use super::super::offline_keys;
    use super::*;

    fn row(title: &str, artist: &str) -> SongDownloadRow {
        SongDownloadRow {
            id: "d".into(),
            song: NewSong {
                track_key: raygun_resolver::track_key(&TrackQuery {
                    title: title.into(),
                    artist: Some(artist.into()),
                    album: None,
                    duration_ms: None,
                }),
                title: title.into(),
                artist: Some(artist.into()),
                album: None,
                duration_ms: Some(180_000),
                artwork_url: None,
                source: None,
            },
            state: "queued".into(),
            kind: None,
            provider: None,
            file: None,
            done_bytes: 0,
            total_bytes: None,
            error: None,
            error_detail: None,
            created_at: 1,
            queued_at: 1,
            finished_at: None,
            origin: Origin::default(),
            file_ms: None,
            replacing: None,
            problem: None,
        }
    }

    #[test]
    fn a_download_looks_for_picks_under_every_name_its_song_goes_by() {
        // Shown under other names now (Edit details): picks under both count.
        let shown = TrackQuery {
            title: "Feeling Good (Live)".into(),
            artist: Some("Nina".into()),
            album: None,
            duration_ms: None,
        };
        let keys = song_keys(&row("Feeling Good - Remastered", "Nina Simone"), &shown);
        assert!(keys.contains(&"nina simone|feeling good remastered".to_owned()));
        assert!(
            keys.contains(&"nina simone|feeling good".to_owned()),
            "without the decoration"
        );
        assert!(
            keys.contains(&"nina|feeling good".to_owned()),
            "the names shown now"
        );
        let mut sorted = keys.clone();
        sorted.dedup();
        assert_eq!(sorted, keys, "each once");
    }

    #[test]
    fn a_downloaded_song_plays_its_copy_whatever_names_it_plays_by() {
        let item = |source: MediaSource| QueueItem {
            explicit: false,
            id: "i".into(),
            title: "Song".into(),
            artist: Some("Band".into()),
            album: None,
            duration_ms: Some(200_000),
            artwork_url: None,
            media_kind: MediaKind::Audio,
            source,
        };
        // Played by a picked source's names: its copy, saved under the names it shows, comes
        // first, then one saved under the names it plays by.
        let picked = item(MediaSource::Lookup {
            title: "Song (Official Audio)".into(),
            artist: Some("Band VEVO".into()),
            album: None,
            duration_ms: None,
        });
        assert_eq!(offline_keys(&picked), ["band|song", "band vevo|song"]);
        // Its own names: once.
        let own = item(MediaSource::Url {
            url: "https://example.org/song.mp3".into(),
            headers: Default::default(),
        });
        assert_eq!(offline_keys(&own), ["band|song"]);
    }
}
