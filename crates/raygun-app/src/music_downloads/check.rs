//! Check downloads (M46.49): what's wrong with a saved song now. Its file can be gone, another
//! size than it was saved at, or not decode; the song can have been given another source since
//! (Find a source, a source switch: a newer remembered pick than the download), or other names
//! (Edit details) its stream doesn't carry.

use std::collections::HashMap;
use std::path::Path;

use raygun_db::music_downloads::{Origin, SongDownloadRow};
use raygun_db::streams::CandidateRow;
use raygun_model::{MediaSource, SongDownloadProblem, SongDownloadProblemKind};
use raygun_resolver::CandidateKey;

use super::{covers_title, names_artist, source_name, transcode};

/// Sources' names by their provider id, for saying which one a song has now.
pub type Names = HashMap<String, String>;

/// What's wrong with a saved song now, if anything, in the order it matters: its file (`damaged`,
/// what [`file_problem`] found), too little of the song in it (`end_ms`: its end trim), another
/// source given since ([`source_problem`]), other names given since ([`details_problem`]).
pub fn problem(
    row: &SongDownloadRow,
    damaged: Option<SongDownloadProblem>,
    end_ms: Option<u64>,
    picks: &[CandidateRow],
    names: &Names,
    shown: Option<(String, Option<String>)>,
) -> Option<SongDownloadProblem> {
    if damaged.is_some() {
        return damaged;
    }
    if !super::holds_the_song(row, end_ms) {
        return Some(super::partial_problem(row));
    }
    source_problem(row, picks, names).or_else(|| details_problem(row, shown))
}

/// What's wrong with a saved file: gone, another size than it was saved at (`recorded` bytes),
/// or not audio anymore. Blocking.
pub fn file_problem(path: &Path, recorded: Option<i64>) -> Option<SongDownloadProblem> {
    let problem = |kind, detail: Option<String>| SongDownloadProblem {
        kind,
        was: None,
        now: None,
        detail,
    };
    let Ok(meta) = std::fs::metadata(path) else {
        return Some(problem(SongDownloadProblemKind::Missing, None));
    };
    if let Some(recorded) = recorded.filter(|bytes| *bytes > 0)
        && meta.len() != recorded as u64
    {
        return Some(problem(
            SongDownloadProblemKind::Damaged,
            Some(format!(
                "The file has {} bytes; it was saved with {recorded}",
                meta.len()
            )),
        ));
    }
    transcode::probe(path)
        .err()
        .map(|error| problem(SongDownloadProblemKind::Damaged, Some(error)))
}

/// The pick a song was given after it downloaded, if any: the newest stream remembered for its
/// key since then.
pub fn current_pick<'a>(
    row: &SongDownloadRow,
    picks: &'a [CandidateRow],
) -> Option<&'a CandidateRow> {
    let finished = row.finished_at?;
    picks
        .iter()
        .filter(|pick| pick.source.is_some())
        .filter(|pick| pick.verified_at.is_some_and(|at| at > finished))
        .filter(|pick| {
            pick.failed_at
                .is_none_or(|failed| Some(failed) < pick.verified_at)
        })
        .max_by_key(|pick| pick.verified_at)
}

/// A saved song given another stream than the one it was saved from.
pub fn source_problem(
    row: &SongDownloadRow,
    picks: &[CandidateRow],
    names: &Names,
) -> Option<SongDownloadProblem> {
    let pick = current_pick(row, picks)?;
    let origin = &row.origin;
    let same_stream = origin.provider.as_deref() == Some(pick.provider_id.as_str())
        && origin.stream.as_deref() == Some(pick.candidate_id.as_str());
    let same_source = match (&origin.source, &pick.source) {
        (Some(was), Some(now)) => same_media(was, now),
        _ => false,
    };
    // One YouTube video, its audio file or its embed, whatever address it had (M46.51).
    let same_video = origin_video(origin).is_some_and(|was| pick_video(pick) == Some(was));
    if same_stream || same_source || same_video {
        return None;
    }
    Some(SongDownloadProblem {
        kind: SongDownloadProblemKind::SourceChanged,
        was: row.provider.clone(),
        now: Some(pick_name(pick, names)),
        detail: None,
    })
}

/// The YouTube video a source (JSON) plays in the embed.
fn embedded_video(source: Option<&str>) -> Option<String> {
    match serde_json::from_str::<MediaSource>(source?).ok()? {
        MediaSource::YouTube { video_id } => Some(video_id),
        _ => None,
    }
}

/// The YouTube video a saved copy came from, if it did.
fn origin_video(origin: &Origin) -> Option<String> {
    let key = CandidateKey {
        provider_id: origin.provider.clone()?,
        candidate_id: origin.stream.clone().unwrap_or_default(),
    };
    super::youtube_video_of(&key)
        .map(str::to_owned)
        .or_else(|| embedded_video(origin.source.as_deref()))
}

/// The YouTube video a pick plays, if it does.
fn pick_video(pick: &CandidateRow) -> Option<String> {
    let key = CandidateKey {
        provider_id: pick.provider_id.clone(),
        candidate_id: pick.candidate_id.clone(),
    };
    super::youtube_video_of(&key)
        .map(str::to_owned)
        .or_else(|| embedded_video(pick.source.as_deref()))
}

/// Whether two sources (JSON) are one stream: the same address or video.
pub(super) fn same_media(was: &str, now: &str) -> bool {
    match (
        serde_json::from_str::<MediaSource>(was),
        serde_json::from_str::<MediaSource>(now),
    ) {
        // One of YouTube's files has another address with every lookup.
        (Ok(MediaSource::Url { url: was, .. }), Ok(MediaSource::Url { url: now, .. })) => {
            super::same_file(&was, &now)
        }
        (Ok(was), Ok(now)) => was == now,
        _ => was == now,
    }
}

/// What a pick is called: its source's name (from `names`), else its site, or what it's titled.
pub fn pick_name(pick: &CandidateRow, names: &Names) -> String {
    if let Some(name) = names.get(&pick.provider_id) {
        return name.clone();
    }
    match pick
        .source
        .as_deref()
        .and_then(|json| serde_json::from_str::<MediaSource>(json).ok())
    {
        Some(MediaSource::Url { url, .. }) => source_name(&url),
        Some(MediaSource::YouTube { .. }) => "YouTube".into(),
        _ => pick.title.clone(),
    }
}

/// A saved song shown under other names (`shown`, from Edit details) than its stream carries.
pub fn details_problem(
    row: &SongDownloadRow,
    shown: Option<(String, Option<String>)>,
) -> Option<SongDownloadProblem> {
    let (title, artist) = shown?;
    // The stream's own names, else the ones it was saved for.
    let stream_title = row.origin.title.as_deref().unwrap_or(&row.song.title);
    let stream_artist = row
        .origin
        .artist
        .as_deref()
        .or(row.song.artist.as_deref())
        .unwrap_or_default();
    let credited = format!("{stream_artist} {stream_title}");
    if covers_title(&title, stream_title) && names_artist(artist.as_deref(), &credited) {
        return None;
    }
    let named = |title: &str, artist: Option<&str>| match artist {
        Some(artist) if !artist.trim().is_empty() => format!("{title} \u{2013} {artist}"),
        _ => title.to_owned(),
    };
    Some(SongDownloadProblem {
        kind: SongDownloadProblemKind::DetailsChanged,
        was: Some(named(stream_title, Some(stream_artist))),
        now: Some(named(&title, artist.as_deref())),
        detail: None,
    })
}

#[cfg(test)]
mod tests {
    use raygun_db::music_downloads::{NewSong, Origin};

    use super::*;

    fn row() -> SongDownloadRow {
        SongDownloadRow {
            id: "d".into(),
            song: NewSong {
                track_key: "bts|swim".into(),
                title: "Swim".into(),
                artist: Some("BTS".into()),
                album: None,
                duration_ms: Some(180_000),
                artwork_url: None,
                source: None,
            },
            state: "done".into(),
            kind: Some("file".into()),
            provider: Some("Indie Tracks".into()),
            file: Some("track.mp3".into()),
            done_bytes: 4_000,
            total_bytes: Some(4_000),
            error: None,
            error_detail: None,
            created_at: 1,
            queued_at: 1,
            finished_at: Some(1_000),
            origin: Origin {
                provider: Some("indie".into()),
                stream: Some("track-1".into()),
                source: Some(
                    r#"{"kind":"url","url":"https://indie.example/1","headers":{}}"#.into(),
                ),
                title: Some("BTS - Swim (Official)".into()),
                artist: None,
            },
            file_ms: Some(180_000),
            replacing: None,
            problem: None,
        }
    }

    fn pick(provider: &str, id: &str, url: &str, verified_at: i64) -> CandidateRow {
        CandidateRow {
            provider_id: provider.into(),
            candidate_id: id.into(),
            title: "Swim".into(),
            source: Some(format!(r#"{{"kind":"url","url":"{url}","headers":{{}}}}"#)),
            expires_at: None,
            verified_at: Some(verified_at),
            failed_at: None,
            bitrate_kbps: None,
        }
    }

    #[test]
    fn a_song_given_another_source_after_it_downloaded_is_told() {
        let names = Names::from([("jamendo".to_owned(), "Jamendo".to_owned())]);
        let saved = row();
        // Picks from before it downloaded don't count, nor the stream it came from.
        assert_eq!(
            source_problem(
                &saved,
                &[pick("jamendo", "9", "https://jamendo.com/9", 500)],
                &names
            ),
            None
        );
        assert_eq!(
            source_problem(
                &saved,
                &[pick("indie", "track-1", "https://indie.example/1", 2_000)],
                &names
            ),
            None
        );
        assert_eq!(
            source_problem(
                &saved,
                &[pick("other", "x", "https://indie.example/1", 2_000)],
                &names
            ),
            None,
            "the same address under another name is the same stream"
        );
        // Find a source picked another stream since.
        let problem = source_problem(
            &saved,
            &[
                pick("indie", "track-1", "https://indie.example/1", 1_500),
                pick("picked", "p", "https://archive.org/details/swim.mp3", 3_000),
            ],
            &names,
        )
        .unwrap();
        assert_eq!(problem.kind, SongDownloadProblemKind::SourceChanged);
        assert_eq!(
            (problem.was.as_deref(), problem.now.as_deref()),
            (Some("Indie Tracks"), Some("archive.org"))
        );
        // A newer pick that failed since isn't the song's source.
        let mut failed = pick("jamendo", "9", "https://jamendo.com/9", 2_000);
        failed.failed_at = Some(2_500);
        assert_eq!(source_problem(&saved, &[failed], &names), None);
        // A download from before origins were kept: any pick since is another source.
        let mut old = row();
        old.origin = Origin::default();
        assert!(
            source_problem(
                &old,
                &[pick("indie", "track-1", "https://indie.example/1", 2_000)],
                &names
            )
            .is_some()
        );
    }

    #[test]
    fn a_youtube_copy_stays_the_same_video_whatever_address_it_plays_from() {
        let youtube = crate::resolver::YOUTUBE_STREAMS;
        let names = Names::from([(youtube.to_owned(), "YouTube".to_owned())]);
        let audio_file = |at: i64| {
            CandidateRow {
            provider_id: youtube.into(),
            candidate_id: "audio:dQw4w9WgXcQ".into(),
            title: "Swim".into(),
            source: Some(
                r#"{"kind":"url","url":"https://rr1---sn-x.googlevideo.com/videoplayback?sig=2","headers":{}}"#
                    .into(),
            ),
            expires_at: Some(9_000),
            verified_at: Some(at),
            failed_at: None,
            bitrate_kbps: None,
        }
        };
        // Saved from the song's own YouTube video, then played as its audio file.
        let mut saved = row();
        saved.origin = Origin {
            provider: Some("own".into()),
            stream: None,
            source: Some(r#"{"kind":"youTube","videoId":"dQw4w9WgXcQ"}"#.into()),
            title: Some("Swim".into()),
            artist: None,
        };
        assert_eq!(source_problem(&saved, &[audio_file(2_000)], &names), None);
        // Saved from another address of that file.
        saved.origin = Origin {
            provider: Some(youtube.into()),
            stream: Some("audio:dQw4w9WgXcQ".into()),
            source: Some(
                r#"{"kind":"url","url":"https://rr9---sn-y.googlevideo.com/videoplayback?sig=1","headers":{}}"#
                    .into(),
            ),
            title: Some("Swim".into()),
            artist: None,
        };
        assert_eq!(source_problem(&saved, &[audio_file(2_000)], &names), None);
        // Another video is another source, named as the listener knows it.
        let mut other = audio_file(2_000);
        other.candidate_id = "audio:otherVideo1".into();
        let problem = source_problem(&saved, &[other], &names).unwrap();
        assert_eq!(problem.now.as_deref(), Some("YouTube"));
    }

    #[test]
    fn a_song_given_names_its_stream_doesnt_carry_is_told() {
        let saved = row();
        assert_eq!(details_problem(&saved, None), None, "names as they were");
        assert_eq!(
            details_problem(&saved, Some(("SWIM".into(), Some("BTS".into())))),
            None,
            "the stream carries them"
        );
        let problem = details_problem(
            &saved,
            Some(("Swim Good".into(), Some("Frank Ocean".into()))),
        )
        .unwrap();
        assert_eq!(problem.kind, SongDownloadProblemKind::DetailsChanged);
        assert_eq!(
            problem.was.as_deref(),
            Some("BTS - Swim (Official) \u{2013} BTS")
        );
        assert_eq!(
            problem.now.as_deref(),
            Some("Swim Good \u{2013} Frank Ocean")
        );
    }

    #[test]
    fn files_gone_resized_or_unreadable_are_told() {
        let temp = tempfile::TempDir::new().unwrap();
        let missing = temp.path().join("track.mp3");
        assert_eq!(
            file_problem(&missing, Some(10)).unwrap().kind,
            SongDownloadProblemKind::Missing
        );
        let wav = temp.path().join("track.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wav, spec).unwrap();
        for frame in 0..8_000i32 {
            writer
                .write_sample(((frame % 100) * 300 - 15_000) as i16)
                .unwrap();
        }
        writer.finalize().unwrap();
        let size = std::fs::metadata(&wav).unwrap().len() as i64;
        assert_eq!(file_problem(&wav, Some(size)), None, "a good file");
        assert_eq!(file_problem(&wav, None), None);
        let resized = file_problem(&wav, Some(size + 100)).unwrap();
        assert_eq!(resized.kind, SongDownloadProblemKind::Damaged);
        assert!(resized.detail.unwrap().contains("saved with"));
        let broken = temp.path().join("broken.mp3");
        std::fs::write(&broken, b"<html>gone</html>").unwrap();
        assert_eq!(
            file_problem(&broken, Some(17)).unwrap().kind,
            SongDownloadProblemKind::Damaged
        );
    }
}
