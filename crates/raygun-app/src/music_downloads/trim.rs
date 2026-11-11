//! Song downloads of YouTube videos without their non-music parts (SponsorBlock's
//! `music_offtopic`, Settings → SponsorBlock → Leave non-music parts out of downloads): an M4A's
//! packets copied without them, nothing encoded again. A trimmed file's name says so
//! (`<name>.trimmed.m4a`), since SponsorBlock's times no longer fit it: its other parts aren't
//! skipped as it plays. Other files (captures, recordings, Opus) keep their parts, which are
//! skipped as they play.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use raygun_model::SegmentCategory;

use crate::export::remux::{self, Source, Target};
use crate::sponsorblock::Segment;

/// What a trimmed file's name has before its extension.
const TRIMMED: &str = ".trimmed";

/// Whether a saved file had its non-music parts cut out.
pub fn is_trimmed(file: &str) -> bool {
    Path::new(file)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.ends_with(TRIMMED))
}

/// Whether a saved file is one Raygun can trim: AAC in MP4, as downloads ask YouTube for.
pub fn can_trim(file: &str) -> bool {
    !is_trimmed(file)
        && Path::new(file)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("m4a"))
}

/// The name of `file` trimmed.
pub fn trimmed_name(file: &str) -> String {
    let path = Path::new(file);
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(file);
    format!("{stem}{TRIMMED}.m4a")
}

/// The stretches to cut from a file of `duration_ms` (seconds): its video's non-music parts that
/// fit its length, in order.
pub fn cuts(segments: &[Segment], duration_ms: Option<f64>) -> Vec<(f64, f64)> {
    let mut cuts: Vec<(f64, f64)> = segments
        .iter()
        .filter(|segment| segment.category == SegmentCategory::MusicOfftopic)
        .filter(|segment| segment.fits(duration_ms))
        .map(|segment| (segment.start_ms / 1000.0, segment.end_ms / 1000.0))
        .collect();
    cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
    cuts
}

/// Writes `source` without `cuts` as the M4A `target`, beside it first. Blocking.
pub fn cut_m4a(source: &Path, cuts: &[(f64, f64)], target: &Path) -> Result<(), String> {
    use std::io::Write as _;

    let partial = target.with_extension("m4a.part");
    let written = (|| -> Result<(), remux::Error> {
        let mut out = std::io::BufWriter::new(std::fs::File::create(&partial)?);
        remux::convert_with(
            &Source::Cut {
                file: source.to_owned(),
                cuts: cuts.to_vec(),
            },
            Target::M4a,
            &mut out,
            &AtomicBool::new(false),
            &[],
        )?;
        out.flush()?;
        drop(out);
        std::fs::rename(&partial, target)?;
        Ok(())
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    written.map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(category: SegmentCategory, start: f64, end: f64) -> Segment {
        Segment {
            id: format!("{start}"),
            category,
            start_ms: start,
            end_ms: end,
            video_ms: 200_000.0,
        }
    }

    #[test]
    fn only_non_music_parts_that_fit_the_file_are_cut_and_trimmed_files_are_known() {
        let segments = [
            part(SegmentCategory::MusicOfftopic, 180_000.0, 200_000.0),
            part(SegmentCategory::Sponsor, 30_000.0, 40_000.0),
            part(SegmentCategory::MusicOfftopic, 0.0, 12_500.0),
        ];
        assert_eq!(
            cuts(&segments, Some(200_400.0)),
            [(0.0, 12.5), (180.0, 200.0)]
        );
        // A file of another cut of the video: nothing fits.
        assert!(cuts(&segments, Some(230_000.0)).is_empty());

        assert!(can_trim("0193.m4a"));
        assert!(!can_trim("0193.webm"));
        assert_eq!(trimmed_name("0193.m4a"), "0193.trimmed.m4a");
        assert!(is_trimmed("0193.trimmed.m4a"));
        assert!(!can_trim("0193.trimmed.m4a"));
    }

    #[test]
    fn a_files_stretches_are_cut_into_a_new_m4a() {
        let dir = tempfile::tempdir().unwrap();
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/export/remux/fixtures/plain.mp4");
        let target = dir.path().join(trimmed_name("song.m4a"));
        cut_m4a(&fixture, &[(0.0, 1.0)], &target).unwrap();
        let ms = crate::music_downloads::transcode::length_ms(&target).unwrap();
        assert!((800..=1_200).contains(&ms), "{ms} ms");
        assert!(!dir.path().join("song.trimmed.m4a.part").exists());
    }

    #[test]
    fn saved_files_know_the_youtube_video_they_came_from() {
        use raygun_db::music_downloads::Origin;
        use raygun_model::MediaSource;

        let audio_file = Origin {
            provider: Some(crate::resolver::YOUTUBE_STREAMS.into()),
            stream: Some("audio:kJQP7kiw5Fk".into()),
            ..Origin::default()
        };
        assert_eq!(
            super::super::youtube_origin(&audio_file).as_deref(),
            Some("kJQP7kiw5Fk")
        );
        let captured = Origin {
            provider: Some("own".into()),
            source: serde_json::to_string(&MediaSource::YouTube {
                video_id: "kJQP7kiw5Fk".into(),
            })
            .ok(),
            ..Origin::default()
        };
        assert_eq!(
            super::super::youtube_origin(&captured).as_deref(),
            Some("kJQP7kiw5Fk")
        );
        let elsewhere = Origin {
            provider: Some("indie".into()),
            stream: Some("1".into()),
            ..Origin::default()
        };
        assert_eq!(super::super::youtube_origin(&elsewhere), None);
    }
}
