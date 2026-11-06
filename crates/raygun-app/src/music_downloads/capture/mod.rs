//! Capture (M46.49), the last resort of a download: a stream that plays but gave no file is saved
//! anyway, in the background, without the player people listen to. Bytes come first: the stream
//! is fetched through the stream proxy's `/media/` route as the player gets it, HLS a segment at a
//! time and files by ranges, and kept as they are once they decode to the length expected. Only a
//! stream that can't be fetched or decoded that way has its sound captured as it decodes
//! ([`sound`]), and that sound is encoded as FLAC or MP3 at the Downloads quality, tagged, with its
//! cover. Songs, podcast episodes and audiobook chapters all use it.

pub mod bytes;
pub mod pcm;
pub mod record;
pub mod sound;

use std::path::{Path, PathBuf};

use raygun_model::{AudioFileFormat, AudioQuality, MediaSource};
use tauri::{AppHandle, Manager};
use tokio::sync::Notify;

use super::transcode::{self, SongTags};

/// One capture at a time captures sound: there's one capture player.
static SOUND_TURN: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// How long the stream should be.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Expected {
    /// The length its details give, ms.
    pub duration_ms: Option<u64>,
    /// What plays of it (its trims), ms: what must be there.
    pub start_ms: u64,
    pub end_ms: Option<u64>,
}

#[derive(Debug)]
pub enum CaptureError {
    Stopped,
    /// It couldn't be written here: nothing else would work either.
    Device(String),
    Failed(String),
    /// A recording of what Raygun plays had to stop: Raygun's player played something else, or
    /// the consent to record went away. It waits to be recorded again.
    Interrupted,
}

/// A saved capture: the file's name in the folder, how it was made and how long it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captured {
    pub file: String,
    /// Its sound was captured and encoded (else the stream's own bytes were kept).
    pub encoded: bool,
    pub duration_ms: Option<u64>,
}

/// What a capture asks for.
pub struct Request<'a> {
    pub source: &'a MediaSource,
    pub dir: &'a Path,
    /// The saved file's name, without its extension.
    pub stem: &'a str,
    pub expected: Expected,
    /// The Downloads quality: FLAC for Best, else MP3 at its bitrate.
    pub quality: AudioQuality,
    pub tags: Option<SongTags>,
}

/// Bytes captured so far, and the share done when known.
pub type Progress<'a> = &'a (dyn Fn(u64, Option<f64>) + Send + Sync);

/// Saves what plays of `request.source` into `request.dir`.
pub async fn capture(
    app: &AppHandle,
    client: &reqwest::Client,
    request: Request<'_>,
    progress: Progress<'_>,
    stop: &Notify,
) -> Result<Captured, CaptureError> {
    let MediaSource::Url { url, headers } = request.source else {
        return Err(CaptureError::Failed("It has no stream to capture".into()));
    };
    let link = app
        .try_state::<raygun_net::MediaLinks>()
        .and_then(|media| media.link(url, headers))
        .ok_or_else(|| CaptureError::Failed("Only web streams can be captured".into()))?;
    tokio::fs::create_dir_all(request.dir)
        .await
        .map_err(|error| CaptureError::Device(error.to_string()))?;

    // Bytes first.
    let mut why: Option<String> = None;
    let kind = match bytes::sniff(client, &link, stop).await {
        Ok(kind) => Some(kind),
        Err(bytes::BytesError::Stopped) => return Err(CaptureError::Stopped),
        Err(bytes::BytesError::Failed(error) | bytes::BytesError::Device(error)) => {
            why = Some(error);
            None
        }
    };
    if let Some(kind) = kind.as_ref().filter(|kind| **kind != bytes::Kind::Dash) {
        let fetched = match kind {
            bytes::Kind::Hls => bytes::hls(client, &link, request.dir, progress, stop).await,
            _ => bytes::file(client, &link, kind, request.dir, progress, stop).await,
        };
        match fetched {
            Ok(fetched) => match keep(&fetched, &request).await {
                Ok(captured) => return Ok(captured),
                Err(error) => why = Some(error),
            },
            Err(bytes::BytesError::Stopped) => return Err(CaptureError::Stopped),
            Err(bytes::BytesError::Device(error)) => return Err(CaptureError::Device(error)),
            Err(bytes::BytesError::Failed(error)) => why = Some(error),
        }
    }

    // Then the sound, as it decodes.
    if !sound::available(app) {
        return Err(CaptureError::Failed(
            why.unwrap_or_else(|| "This device can't capture it".into()),
        ));
    }
    let format = match kind {
        Some(bytes::Kind::Hls) => Some("hls"),
        Some(bytes::Kind::Dash) => Some("dash"),
        _ => None,
    };
    let _turn = tokio::select! {
        turn = SOUND_TURN.lock() => turn,
        () = stop.notified() => return Err(CaptureError::Stopped),
    };
    progress(0, Some(0.0));
    let share = |share: f64| progress(0, Some(share.clamp(0.0, 1.0)));
    let stitched = sound::capture(
        app,
        sound::Player::Stream {
            link: &link,
            format,
        },
        request.dir,
        &request.expected,
        &share,
        stop,
    )
    .await
    .map_err(|error| match (error, &why) {
        (CaptureError::Failed(error), Some(before)) => {
            CaptureError::Failed(format!("{before}; capturing its sound: {error}"))
        }
        (error, _) => error,
    })?;
    encode(stitched, request).await
}

/// Records a song only YouTube's player plays (Android, `request.source` its video): it plays
/// aloud, in real time, in the webview's recording player while Raygun's own sound is recorded
/// ([`record`]), once Raygun's player is quiet, and is saved as a captured song is. The person
/// must have allowed recording ([`record::ready`]).
pub async fn record(
    app: &AppHandle,
    request: Request<'_>,
    recording: record::Song<'_>,
    progress: Progress<'_>,
    stop: &Notify,
) -> Result<Captured, CaptureError> {
    let MediaSource::YouTube { video_id } = request.source else {
        return Err(CaptureError::Failed(
            "Only YouTube's player's songs are recorded".into(),
        ));
    };
    tokio::fs::create_dir_all(request.dir)
        .await
        .map_err(|error| CaptureError::Device(error.to_string()))?;
    record::wait_until_quiet(app, stop).await?;
    let _turn = tokio::select! {
        turn = SOUND_TURN.lock() => turn,
        () = stop.notified() => return Err(CaptureError::Stopped),
    };
    progress(0, Some(0.0));
    let share = |share: f64| progress(0, Some(share.clamp(0.0, 1.0)));
    let player = sound::Player::Recording {
        video_id,
        download_id: recording.download_id,
        title: recording.title,
        artist: recording.artist,
    };
    let recorded = sound::capture(app, player, request.dir, &request.expected, &share, stop).await;
    let stitched = match recorded {
        Ok(stitched) => stitched,
        Err(error) => {
            // What was recorded of it doesn't carry over to the next try.
            let _ = tokio::fs::remove_dir_all(request.dir.join("sound")).await;
            return Err(error);
        }
    };
    encode(stitched, request).await
}

/// A capture's stitched sound checked against the song's length, then saved as FLAC (Best) or
/// MP3 at the Downloads quality, tagged, with its cover.
async fn encode(stitched: sound::Stitched, request: Request<'_>) -> Result<Captured, CaptureError> {
    if let Err(error) = check_length(stitched.ms, stitched.stream_ms, &request.expected) {
        let _ = tokio::fs::remove_file(&stitched.wav).await;
        return Err(CaptureError::Failed(error));
    }
    let (wav, target) = (stitched.wav.clone(), request.dir.join(request.stem));
    let (format, kbps) = match request.quality.target_kbps() {
        None => (AudioFileFormat::Flac, 320),
        Some(kbps) => (AudioFileFormat::Mp3, kbps),
    };
    let tags = request.tags;
    let encoded = tokio::task::spawn_blocking(move || {
        let (path, _) = transcode::write_file(&wav, format, &target, kbps)?;
        let _ = std::fs::remove_file(&wav);
        if let Some(tags) = tags
            && let Err(error) = transcode::tag(&path, &tags)
        {
            tracing::info!(%error, "couldn't tag a captured song");
        }
        Ok::<PathBuf, String>(path)
    })
    .await
    .map_err(|error| CaptureError::Failed(error.to_string()))?
    .map_err(CaptureError::Device)?;
    Ok(Captured {
        file: file_name(&encoded),
        encoded: true,
        duration_ms: Some(stitched.ms),
    })
}

/// A fetched file kept as the capture once it decodes to the length expected.
async fn keep(fetched: &bytes::Fetched, request: &Request<'_>) -> Result<Captured, String> {
    let path = fetched.path.clone();
    let measured = tokio::task::spawn_blocking(move || transcode::measure(&path))
        .await
        .map_err(|error| error.to_string())
        .and_then(|measured| measured);
    let checked = measured.and_then(|ms| {
        if let Some(ms) = ms {
            check_length(ms, fetched.listed_ms, &request.expected)?;
        }
        Ok(ms)
    });
    let duration_ms = match checked {
        Ok(ms) => ms,
        Err(error) => {
            let _ = tokio::fs::remove_file(&fetched.path).await;
            return Err(error);
        }
    };
    let extension = fetched
        .path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("mp3")
        .to_owned();
    let target = request.dir.join(format!("{}.{extension}", request.stem));
    tokio::fs::rename(&fetched.path, &target)
        .await
        .map_err(|error| error.to_string())?;
    Ok(Captured {
        file: file_name(&target),
        encoded: false,
        duration_ms,
    })
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Whether a capture of `captured_ms` holds the song: up to the end of what plays of it (its
/// end trim) or the stream's end, and more than a preview of what its details say it lasts.
pub fn check_length(
    captured_ms: u64,
    stream_ms: Option<u64>,
    expected: &Expected,
) -> Result<(), String> {
    if let Some(stream) = stream_ms.filter(|stream| *stream > 0) {
        let needed = expected.end_ms.map_or(stream, |end| end.min(stream));
        if captured_ms + 1_500 < needed {
            return Err(format!(
                "Only {} of {} was saved",
                clock(captured_ms),
                clock(stream)
            ));
        }
    }
    if let Some(full) = expected.duration_ms.filter(|full| *full > 0) {
        let needed = expected.end_ms.map_or(full, |end| end.min(full));
        let slack = (needed * 15 / 100).max(3_000);
        if captured_ms + slack < needed {
            return Err(format!(
                "It only plays {} of {}, like a preview",
                clock(captured_ms),
                clock(full)
            ));
        }
    }
    Ok(())
}

/// `m:ss`.
fn clock(ms: u64) -> String {
    let seconds = ms / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capture_must_hold_what_plays_of_the_song() {
        let song = Expected {
            duration_ms: Some(200_000),
            start_ms: 0,
            end_ms: None,
        };
        assert!(check_length(199_000, Some(199_500), &song).is_ok());
        assert_eq!(
            check_length(30_000, Some(30_000), &song).unwrap_err(),
            "It only plays 0:30 of 3:20, like a preview"
        );
        assert_eq!(
            check_length(150_000, Some(199_000), &song).unwrap_err(),
            "Only 2:30 of 3:19 was saved"
        );
        // Trimmed at 2:40: what's after it doesn't matter.
        let trimmed = Expected {
            end_ms: Some(160_000),
            ..song
        };
        assert!(check_length(161_000, Some(199_000), &trimmed).is_ok());
        assert!(check_length(150_000, Some(199_000), &trimmed).is_err());
        // Nothing to go by but the stream.
        let unknown = Expected::default();
        assert!(check_length(10_000, None, &unknown).is_ok());
        assert!(check_length(10_000, Some(60_000), &unknown).is_err());
    }
}
