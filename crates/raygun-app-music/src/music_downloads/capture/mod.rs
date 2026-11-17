//! Capture (M46.49), the last resort of a download: a stream that plays but gave no file is saved
//! anyway, in the background, without the player people listen to. Bytes come first: the stream
//! is fetched through the stream proxy's `/media/` route as the player gets it, HLS a segment at a
//! time and files by ranges, and kept as they are once they decode to the length expected. Only a
//! stream that can't be fetched or decoded that way has its sound captured as it decodes
//! ([`sound`]), and that sound is encoded as FLAC or MP3 at the Downloads quality, tagged, with its
//! cover. Songs, podcast episodes and audiobook chapters all use it. A song only YouTube's player
//! plays is captured from a hidden YouTube player of its own (M46.52, [`youtube`]).

pub mod bytes;
pub mod pcm;
pub mod sound;
pub mod youtube;

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
    /// A hidden YouTube player couldn't go on: Raygun's window went (Android), or its tap would
    /// take the audio focus from Raygun's player, which plays. It waits to be captured again.
    Interrupted,
    /// Some of it was captured and kept, not all (M46.52): the next try captures only what's
    /// missing. `needed_ms` is what plays of it, when its length is known.
    Partial {
        covered_ms: u64,
        needed_ms: Option<u64>,
        why: String,
    },
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
        None,
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

/// Captures a song only YouTube's player plays (`request.source` its video) from a hidden YouTube
/// player of its own ([`youtube`]), beside whatever plays, and saves it as a captured song is.
/// What it captured is kept between tries: a capture that missed parts ends
/// [`CaptureError::Partial`], and the next one captures only those.
pub async fn youtube(
    app: &AppHandle,
    request: Request<'_>,
    progress: Progress<'_>,
    stop: &Notify,
) -> Result<Captured, CaptureError> {
    let MediaSource::YouTube { video_id } = request.source else {
        return Err(CaptureError::Failed(
            "Only YouTube's videos are captured from its player".into(),
        ));
    };
    let share = |share: f64| progress(0, Some(share.clamp(0.0, 1.0)));
    progress(0, Some(0.0));
    let stitched = youtube_sound(
        app,
        video_id,
        request.dir,
        &request.expected,
        youtube::Purpose::Song,
        &share,
        stop,
    )
    .await?;
    encode(stitched, request).await
}

/// A YouTube video's sound, captured from a hidden YouTube player into `capture.wav` in `dir`
/// (the runs kept in `dir/sound` until it's whole), for `purpose`.
pub async fn youtube_sound(
    app: &AppHandle,
    video_id: &str,
    dir: &Path,
    expected: &Expected,
    purpose: youtube::Purpose,
    share: &(dyn Fn(f64) + Send + Sync),
    stop: &Notify,
) -> Result<sound::Stitched, CaptureError> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|error| CaptureError::Device(error.to_string()))?;
    let key = format!("youtube:{video_id}");
    loop {
        let tap = youtube::Tap::for_purpose(purpose);
        if !youtube::can_start(app, tap) {
            return Err(CaptureError::Interrupted);
        }
        let _turn = tokio::select! {
            turn = youtube::TURN.lock() => turn,
            () = stop.notified() => return Err(CaptureError::Stopped),
        };
        let player = sound::Player::YouTube { video_id, tap };
        match sound::capture(app, player, dir, expected, share, stop, Some(&key)).await {
            // This webview gives no sound from a muted player: tapped as it plays instead.
            Err(CaptureError::Failed(error))
                if error == youtube::SILENT && tap.mode == youtube::TapMode::Muted =>
            {
                youtube::note_silent();
            }
            captured => return captured,
        }
    }
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
/// end trim) or the stream's end, give or take a second and a half, and at least 90% of what its
/// details say plays (M46.52: a copy with less isn't played instead of the stream).
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
        if !covers_enough(captured_ms, needed) {
            return Err(format!(
                "It only plays {} of {}, like a preview",
                clock(captured_ms),
                clock(full)
            ));
        }
    }
    Ok(())
}

/// Whether `saved_ms` of a song is enough to play instead of streaming it: at least 90% of the
/// `needed_ms` that plays of it.
pub fn covers_enough(saved_ms: u64, needed_ms: u64) -> bool {
    saved_ms.saturating_mul(10) >= needed_ms.saturating_mul(9)
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
        // A whole stream shorter than the song's details: 90% of it is enough, less isn't.
        assert!(check_length(181_000, Some(181_000), &song).is_ok());
        assert_eq!(
            check_length(178_000, Some(178_000), &song).unwrap_err(),
            "It only plays 2:58 of 3:20, like a preview"
        );
    }

    #[test]
    fn a_copy_plays_instead_of_the_stream_with_nine_tenths_of_it() {
        assert!(covers_enough(180_000, 200_000));
        assert!(!covers_enough(179_999, 200_000));
        assert!(covers_enough(0, 0));
    }
}
