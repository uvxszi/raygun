//! Check downloads (M46.49): what's wrong with a saved song now. Its file can be gone, another
//! size than it was saved at, or not decode; the song can have been given another source since
//! (Find a source, a source switch: a newer remembered pick than the download), or other names
//! (Edit details) its stream doesn't carry.

use std::path::Path;

use raygun_db::music_downloads::SongDownloadRow;
use raygun_db::streams::CandidateRow;
use raygun_model::{MediaSource, SongDownloadProblem, SongDownloadProblemKind};

use super::{covers_title, names_artist, source_name, transcode};

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
) -> Option<SongDownloadProblem> {
    let pick = current_pick(row, picks)?;
    let origin = &row.origin;
    let same_stream = origin.provider.as_deref() == Some(pick.provider_id.as_str())
        && origin.stream.as_deref() == Some(pick.candidate_id.as_str());
    let same_source = match (&origin.source, &pick.source) {
        (Some(was), Some(now)) => same_media(was, now),
        _ => false,
    };
    if same_stream || same_source {
        return None;
    }
    Some(SongDownloadProblem {
        kind: SongDownloadProblemKind::SourceChanged,
        was: row.provider.clone(),
        now: Some(pick_name(pick)),
        detail: None,
    })
}

/// Whether two sources (JSON) are one stream: the same address or video.
fn same_media(was: &str, now: &str) -> bool {
    match (
        serde_json::from_str::<MediaSource>(was),
        serde_json::from_str::<MediaSource>(now),
    ) {
        (Ok(MediaSource::Url { url: was, .. }), Ok(MediaSource::Url { url: now, .. })) => {
            was == now
        }
        (Ok(was), Ok(now)) => was == now,
        _ => was == now,
    }
}

/// What a pick is called: its source's name (an extension's id, or the name it was picked
/// under), else its site.
pub fn pick_name(pick: &CandidateRow) -> String {
    if pick.provider_id != "picked" {
        return pick.provider_id.clone();
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
            provider: Some("Audius".into()),
            file: Some("track.mp3".into()),
            done_bytes: 4_000,
            total_bytes: Some(4_000),
            error: None,
            error_detail: None,
            created_at: 1,
            queued_at: 1,
            finished_at: Some(1_000),
            origin: Origin {
                provider: Some("audius".into()),
                stream: Some("track-1".into()),
                source: Some(r#"{"kind":"url","url":"https://audius.co/1","headers":{}}"#.into()),
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
        let saved = row();
        // Picks from before it downloaded don't count, nor the stream it came from.
        assert_eq!(
            source_problem(
                &saved,
                &[pick("jamendo", "9", "https://jamendo.com/9", 500)]
            ),
            None
        );
        assert_eq!(
            source_problem(
                &saved,
                &[pick("audius", "track-1", "https://audius.co/1", 2_000)]
            ),
            None
        );
        assert_eq!(
            source_problem(&saved, &[pick("other", "x", "https://audius.co/1", 2_000)]),
            None,
            "the same address under another name is the same stream"
        );
        // Find a source picked another stream since.
        let problem = source_problem(
            &saved,
            &[
                pick("audius", "track-1", "https://audius.co/1", 1_500),
                pick("picked", "p", "https://archive.org/details/swim.mp3", 3_000),
            ],
        )
        .unwrap();
        assert_eq!(problem.kind, SongDownloadProblemKind::SourceChanged);
        assert_eq!(
            (problem.was.as_deref(), problem.now.as_deref()),
            (Some("Audius"), Some("archive.org"))
        );
        // A newer pick that failed since isn't the song's source.
        let mut failed = pick("jamendo", "9", "https://jamendo.com/9", 2_000);
        failed.failed_at = Some(2_500);
        assert_eq!(source_problem(&saved, &[failed]), None);
        // A download from before origins were kept: any pick since is another source.
        let mut old = row();
        old.origin = Origin::default();
        assert!(
            source_problem(
                &old,
                &[pick("audius", "track-1", "https://audius.co/1", 2_000)]
            )
            .is_some()
        );
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
