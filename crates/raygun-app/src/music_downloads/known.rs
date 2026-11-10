//! Downloads start from the song's known source (M46.53): the stream playing it now, a source
//! picked for it (Find a source, a source switch), or one that played it before. A fresh search
//! can come up empty ("no source has it") for a song that plays fine, so it comes only after
//! these gave no file. They play this song, so they aren't held to the search's matching rules.

use std::path::Path;

use raygun_db::music_downloads::{Origin, SongDownloadRow};
use raygun_model::{MediaSource, TrackQuery};
use tauri::Manager;
use tokio::sync::Notify;

use super::{
    Attempts, Candidate, Failure, Saved, SongDownloadService, StreamError, device_message,
};
use crate::resolution::Exclusions;
use crate::resolver::ResolverService;

impl SongDownloadService {
    /// Saves the song from a source it's known to play from, best first: `None` when none gave a
    /// file. Each one tried is left out of the search after, and a stream of it that didn't save
    /// is kept to capture when nothing else works.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn save_known(
        &self,
        row: &SongDownloadRow,
        dir: &Path,
        query: &TrackQuery,
        exclusions: &mut Exclusions,
        attempts: &mut Attempts,
        resolver: &ResolverService,
        using: raygun_ext_host::StreamUse,
        stop: &Notify,
    ) -> Result<Option<Saved>, Failure> {
        let keys = song_keys(row, query);
        let playing = self
            .app
            .try_state::<crate::playback::PlaybackService>()
            .and_then(|playback| playback.playing_pick(&keys));
        for known in resolver.known_sources(&keys, playing.as_ref()).await {
            if self.stopped(&row.id) {
                return Err(Failure::Stopped);
            }
            let tried_source = known
                .source
                .as_ref()
                .is_some_and(|source| exclusions.failed_sources.contains(source));
            if exclusions.tried.contains(&known.key) || tried_source {
                continue;
            }
            exclusions.tried.push(known.key.clone());
            let origin = |source: Option<&MediaSource>| Origin {
                provider: Some(known.key.provider_id.clone()),
                stream: Some(known.key.candidate_id.clone()),
                source: source.and_then(|source| serde_json::to_string(source).ok()),
                title: Some(known.title.clone()),
                artist: known.artist.clone(),
            };
            tracing::info!(song = %row.song.track_key, provider = %known.key.provider_id, "downloading from the song's known source");

            // A YouTube video: its own audio file (M46.51), unless the song was asked with it.
            if let Some(video_id) = known.video() {
                let tried = attempts
                    .youtube
                    .iter()
                    .any(|copy| copy.video_id == video_id && copy.tried);
                if tried {
                    continue;
                }
                let mut file = None;
                let found = self
                    .save_youtube(row, dir, &video_id, &mut file, resolver, using, stop)
                    .await;
                match found {
                    Ok(saved) => return Ok(Some(saved.from(origin(known.source.as_ref())))),
                    Err(StreamError::Stopped) => return Err(Failure::Stopped),
                    Err(StreamError::Device(error)) => {
                        return Err(Failure::Error(device_message(&error)));
                    }
                    Err(error) => attempts.own_video_failed(
                        &video_id,
                        file,
                        origin(known.source.as_ref()),
                        &error,
                    ),
                }
                continue;
            }

            let pick = tokio::select! {
                pick = resolver.known_stream(&known, using) => pick,
                () = stop.notified() => return Err(Failure::Stopped),
            };
            let pick = match pick {
                Ok(pick) => pick,
                Err(error) => {
                    let name = resolver.provider_name(&known.key.provider_id).await;
                    attempts.report.note(&name, &StreamError::Error(error));
                    continue;
                }
            };
            let MediaSource::Url { url, headers } = &pick.source else {
                continue;
            };
            let name = pick.provider_name.clone();
            self.trying(&row.id, Some(name.clone()));
            let saved = self
                .save_retrying(row, dir, url, headers, Some(name.clone()), stop)
                .await;
            match saved {
                Ok(saved) => {
                    // A new stream from the pick's source worked: it's remembered with it.
                    if !pick.from_cache {
                        resolver.mark_verified(&pick).await;
                    }
                    return Ok(Some(saved.from(origin(Some(&pick.source)))));
                }
                Err(StreamError::Stopped) => return Err(Failure::Stopped),
                Err(StreamError::Device(error)) => {
                    return Err(Failure::Error(device_message(&error)));
                }
                Err(error) => {
                    // Only a stream's own error counts against its pick.
                    if let StreamError::Error(message) = &error
                        && !pick.from_cache
                    {
                        resolver.mark_failed(&pick, message).await;
                    }
                    exclusions.failed_sources.push(pick.source.clone());
                    attempts.file_failed(
                        Candidate {
                            name,
                            source: pick.source.clone(),
                            origin: origin(Some(&pick.source)),
                        },
                        &error,
                    );
                }
            }
        }
        Ok(None)
    }
}

/// The keys a download's song is remembered under: the key it was saved under, the names it
/// shows now (`query`, Edit details) and the keys saved songs know it by.
fn song_keys(row: &SongDownloadRow, query: &TrackQuery) -> Vec<String> {
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
    use raygun_db::music_downloads::NewSong;
    use raygun_model::{MediaKind, QueueItem};

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
