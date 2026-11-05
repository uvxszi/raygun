//! A stream's sound captured as it decodes (M46.49), for streams that can't be fetched as bytes
//! or that Raygun can't decode itself. A player of its own, never the one people listen to, plays
//! the stream muted and fast: on Android a third ExoPlayer in the `raygun-audio` plugin, whose
//! sound is copied before its speed is applied; on desktop a hidden media element in the webview
//! at 2×, its pitch left free, recorded through Web Audio at twice the stream's rate, so the
//! samples hold the sound at its own pace once read at the context's rate over the speed.
//!
//! Runs are kept by where they start in the stream. A run that stalls is left behind (the next
//! starts further on), and what's missing is captured again afterwards with some sound around
//! it, so the stitched runs overlap and line up.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use raygun_model::{SongCaptureReport, SongCaptureReportKind};
use tauri::{AppHandle, Manager};
use tokio::sync::{Notify, mpsc};

use super::pcm::{Coverage, Piece, frames_at, restored_rate};
use super::{CaptureError, Expected};

/// A run with no sound for this long has stalled.
const STALLED: Duration = Duration::from_secs(15);
/// How long a player may take to start.
const STARTING: Duration = Duration::from_secs(30);
/// A run that stalled starts again this far on (the part between is captured later).
const SKIP_MS: f64 = 8_000.0;
/// What's captured again around a missing part, to line the runs up.
const MARGIN_MS: f64 = 1_500.0;
/// Runs tried in all.
const PASSES: usize = 8;
/// Sound within this many ms of where a run reached goes on from it.
const JOIN_MS: f64 = 60.0;
/// The desktop's capture plays at this speed.
#[cfg(not(target_os = "android"))]
const DESKTOP_SPEED: f64 = 2.0;
/// Android's capture player runs this fast (its sound is copied before the speed applies).
#[cfg(target_os = "android")]
const ANDROID_SPEED: f64 = 4.0;

/// What a capture player says.
#[derive(Debug)]
pub(crate) enum Report {
    /// Sound from `start_ms` in the stream, at the stream's own `rate`.
    Sound {
        start_ms: f64,
        rate: u32,
        channels: u16,
        samples: Vec<i16>,
    },
    Stalled,
    /// The stream's length.
    Length(f64),
    Ended,
    Failed(String),
    /// Raygun's player played something else meanwhile: a recording of what Raygun plays would
    /// hold it, so the capture stops and waits ([`CaptureError::Interrupted`]).
    Interrupted,
}

/// What plays for a capture.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Player<'a> {
    /// A stream through the proxy (`link`; `hls`, `dash` or a file), played by a capture player
    /// of its own, muted and fast.
    Stream {
        link: &'a str,
        format: Option<&'a str>,
    },
    /// A song only YouTube's player plays (Android): played aloud, in real time, in the webview's
    /// recording player, while Raygun's own sound is recorded ([`super::record`]).
    Recording {
        video_id: &'a str,
        download_id: &'a str,
        title: &'a str,
        artist: Option<&'a str>,
    },
}

/// Desktop sessions waiting for the webview's reports.
static SESSIONS: LazyLock<Mutex<HashMap<String, mpsc::UnboundedSender<Report>>>> =
    LazyLock::new(Mutex::default);

/// Whether this device can capture a stream's sound.
pub(crate) fn available(app: &AppHandle) -> bool {
    #[cfg(target_os = "android")]
    {
        app.try_state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
            .is_some()
    }
    #[cfg(not(target_os = "android"))]
    {
        app.get_webview_window("main").is_some()
    }
}

/// A running capture player.
pub(crate) struct Session {
    pub(crate) id: String,
    pub(crate) reports: mpsc::UnboundedReceiver<Report>,
    /// The task following the player (Android's capture player, or a recording), if any.
    pub(crate) poll: Option<tokio::task::JoinHandle<()>>,
    /// It records what Raygun plays.
    pub(crate) recording: bool,
}

async fn start(
    app: &AppHandle,
    player: Player<'_>,
    from_ms: f64,
    dir: &Path,
) -> Result<Session, String> {
    let (link, format) = match player {
        Player::Stream { link, format } => (link, format),
        Player::Recording {
            video_id,
            download_id,
            title,
            artist,
        } => {
            return super::record::start_run(
                app,
                super::record::Run {
                    video_id,
                    download_id,
                    title,
                    artist,
                    from_ms,
                    dir,
                },
            )
            .await;
        }
    };
    let id = uuid::Uuid::now_v7().simple().to_string();
    #[cfg(target_os = "android")]
    {
        let (sender, reports) = mpsc::unbounded_channel();
        let request = serde_json::json!({
            "session": id,
            "url": link,
            "mime": match format {
                Some("hls") => Some("application/x-mpegURL"),
                Some("dash") => Some("application/dash+xml"),
                _ => None,
            },
            "fromMs": from_ms,
            "speed": ANDROID_SPEED,
            "dir": dir.to_string_lossy(),
        });
        let native = app.clone();
        tokio::task::spawn_blocking(move || {
            native
                .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .capture_start(&request)
        })
        .await
        .map_err(|error| error.to_string())??;
        let poll = tokio::spawn(android_poll(app.clone(), id.clone(), sender));
        Ok(Session {
            id,
            reports,
            poll: Some(poll),
            recording: false,
        })
    }
    #[cfg(not(target_os = "android"))]
    {
        use tauri_specta::Event as _;

        let _ = dir;
        let (sender, reports) = mpsc::unbounded_channel();
        SESSIONS
            .lock()
            .expect("capture sessions poisoned")
            .insert(id.clone(), sender);
        crate::events::SongCaptureEvent(raygun_model::SongCaptureRequest {
            session: id.clone(),
            url: Some(link.to_owned()),
            format: format.map(str::to_owned),
            from_ms,
            speed: DESKTOP_SPEED,
        })
        .emit(app)
        .map_err(|error| error.to_string())?;
        Ok(Session {
            id,
            reports,
            poll: None,
            recording: false,
        })
    }
}

async fn end_session(app: &AppHandle, session: Session) {
    if let Some(poll) = &session.poll {
        poll.abort();
    }
    if session.recording {
        super::record::end_run(app, &session.id).await;
        drop(session.reports);
        return;
    }
    #[cfg(target_os = "android")]
    {
        let (native, id) = (app.clone(), session.id.clone());
        let _ = tokio::task::spawn_blocking(move || {
            native
                .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .capture_stop(&id)
        })
        .await;
    }
    #[cfg(not(target_os = "android"))]
    {
        use tauri_specta::Event as _;

        SESSIONS
            .lock()
            .expect("capture sessions poisoned")
            .remove(&session.id);
        let _ = crate::events::SongCaptureEvent(raygun_model::SongCaptureRequest {
            session: session.id.clone(),
            url: None,
            format: None,
            from_ms: 0.0,
            speed: DESKTOP_SPEED,
        })
        .emit(app);
    }
    drop(session.reports);
}

/// A desktop capture's report from the webview. An error tells it the session is over.
pub fn report(report: SongCaptureReport) -> Result<(), String> {
    let sender = SESSIONS
        .lock()
        .expect("capture sessions poisoned")
        .get(&report.session)
        .cloned()
        .ok_or("That capture was stopped")?;
    for message in from_webview(report)? {
        sender
            .send(message)
            .map_err(|_| "That capture was stopped".to_owned())?;
    }
    Ok(())
}

/// The webview's report as the capture hears it: its sound at the stream's own rate.
fn from_webview(report: SongCaptureReport) -> Result<Vec<Report>, String> {
    let mut messages = Vec::new();
    if let Some(length) = report.duration_ms.filter(|ms| ms.is_finite() && *ms > 0.0) {
        messages.push(Report::Length(length));
    }
    messages.push(match report.kind {
        SongCaptureReportKind::Sound => {
            let rate = restored_rate(report.rate, report.speed)
                .ok_or("The capture's rate and speed don't make a sound rate")?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(report.pcm.as_deref().unwrap_or_default())
                .map_err(|error| error.to_string())?;
            Report::Sound {
                start_ms: report.start_ms,
                rate,
                channels: u16::try_from(report.channels.clamp(1, 8)).unwrap_or(2),
                samples: bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| i16::from_le_bytes(*pair))
                    .collect(),
            }
        }
        SongCaptureReportKind::Stalled => Report::Stalled,
        SongCaptureReportKind::Ended => Report::Ended,
        SongCaptureReportKind::Failed => Report::Failed(
            report
                .error
                .unwrap_or_else(|| "The stream didn't play".into()),
        ),
    });
    Ok(messages)
}

/// One PCM piece an Android player writes (its capture player, or a recording): 16-bit
/// interleaved samples in `file`, from `start_ms` in the stream, `frames` long so far.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PieceState {
    pub(crate) file: String,
    pub(crate) start_ms: f64,
    pub(crate) rate: u32,
    pub(crate) channels: u16,
    pub(crate) frames: u64,
}

/// An Android player's state, as its status call answers.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlayerState {
    pub(crate) state: String,
    pub(crate) duration_ms: Option<f64>,
    pub(crate) error: Option<String>,
    #[serde(default)]
    pub(crate) pieces: Vec<PieceState>,
}

/// Sends the sound its pieces gained since they were last read (`read`: frames read per file).
/// Returns whether any came.
pub(crate) fn send_new_sound(
    pieces: &[PieceState],
    read: &mut HashMap<String, u64>,
    sender: &mpsc::UnboundedSender<Report>,
) -> bool {
    use std::io::{Read, Seek, SeekFrom};

    let mut heard = false;
    for piece in pieces {
        let done = read.entry(piece.file.clone()).or_insert(0);
        if piece.frames <= *done {
            continue;
        }
        let channels = u64::from(piece.channels.max(1));
        let count = piece.frames - *done;
        let samples = std::fs::File::open(&piece.file).and_then(|mut file| {
            file.seek(SeekFrom::Start(*done * channels * 2))?;
            let mut bytes = vec![0u8; (count * channels * 2) as usize];
            file.read_exact(&mut bytes)?;
            Ok(bytes)
        });
        let Ok(bytes) = samples else {
            continue;
        };
        let start_ms = piece.start_ms + *done as f64 * 1000.0 / f64::from(piece.rate.max(1));
        *done = piece.frames;
        heard = true;
        let _ = sender.send(Report::Sound {
            start_ms,
            rate: piece.rate,
            channels: piece.channels,
            samples: bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| i16::from_le_bytes(*pair))
                .collect(),
        });
    }
    heard
}

/// Follows Android's capture player: its pieces (PCM files it writes) and its state, four times
/// a second.
#[cfg(target_os = "android")]
async fn android_poll(app: AppHandle, id: String, sender: mpsc::UnboundedSender<Report>) {
    let mut read: HashMap<String, u64> = HashMap::new();
    let mut stalled = false;
    let mut length_told = false;
    loop {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let (native, session) = (app.clone(), id.clone());
        let status = tokio::task::spawn_blocking(move || {
            native
                .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .capture_status(&session)
        })
        .await;
        let state: PlayerState = match status
            .map_err(|error| error.to_string())
            .and_then(|status| status)
            .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()))
        {
            Ok(state) => state,
            Err(error) => {
                let _ = sender.send(Report::Failed(error));
                return;
            }
        };
        if !length_told && let Some(length) = state.duration_ms.filter(|ms| *ms > 0.0) {
            length_told = true;
            let _ = sender.send(Report::Length(length));
        }
        if send_new_sound(&state.pieces, &mut read, &sender) {
            stalled = false;
        }
        match state.state.as_str() {
            "ended" => {
                let _ = sender.send(Report::Ended);
                return;
            }
            "failed" => {
                let _ = sender.send(Report::Failed(
                    state
                        .error
                        .unwrap_or_else(|| "The stream didn't play".into()),
                ));
                return;
            }
            "buffering" if !stalled => {
                stalled = true;
                let _ = sender.send(Report::Stalled);
            }
            _ => {}
        }
    }
}

/// How a run ended.
enum RunEnd {
    /// The stream ended.
    Ended,
    /// It got as far as asked.
    Reached,
    /// No sound came for a while, from where it had reached (ms).
    Stalled(f64),
    Failed(String),
}

/// The runs written so far: where each piece goes, and the one being written.
struct Runs {
    dir: PathBuf,
    coverage: Option<Coverage>,
    open: Option<(Piece, std::io::BufWriter<std::fs::File>)>,
    count: usize,
}

impl Runs {
    /// Adds sound at `start_ms`: onto the open run when it goes on from it, else as a new one.
    fn add(
        &mut self,
        start_ms: f64,
        rate: u32,
        channels: u16,
        samples: &[i16],
    ) -> Result<(), CaptureError> {
        let coverage = self
            .coverage
            .get_or_insert_with(|| Coverage::new(rate, channels));
        if coverage.rate != rate || coverage.channels != channels {
            return Err(CaptureError::Failed(
                "The stream changed its sound midway".into(),
            ));
        }
        let start = frames_at(start_ms, rate);
        let joins = self
            .open
            .as_ref()
            .is_some_and(|(piece, _)| piece.end().abs_diff(start) <= frames_at(JOIN_MS, rate));
        if !joins {
            self.close()?;
            self.count += 1;
            let path = self.dir.join(format!("run-{}.raw", self.count));
            let file = std::fs::File::create(&path)
                .map_err(|error| CaptureError::Device(error.to_string()))?;
            self.open = Some((
                Piece {
                    start,
                    frames: 0,
                    path,
                },
                std::io::BufWriter::new(file),
            ));
        }
        let (piece, writer) = self.open.as_mut().expect("a run is open");
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        writer
            .write_all(&bytes)
            .map_err(|error| CaptureError::Device(error.to_string()))?;
        piece.frames += (samples.len() / usize::from(channels.max(1))) as u64;
        Ok(())
    }

    /// Where the open run has reached, in ms.
    fn reached_ms(&self) -> Option<f64> {
        let (piece, _) = self.open.as_ref()?;
        let rate = self.coverage.as_ref()?.rate;
        Some(piece.end() as f64 * 1000.0 / f64::from(rate.max(1)))
    }

    fn close(&mut self) -> Result<(), CaptureError> {
        if let Some((piece, mut writer)) = self.open.take() {
            writer
                .flush()
                .map_err(|error| CaptureError::Device(error.to_string()))?;
            if piece.frames > 0
                && let Some(coverage) = self.coverage.as_mut()
            {
                coverage.add(piece);
            }
        }
        Ok(())
    }

    /// Everything captured, the open run included.
    fn snapshot(&self) -> Option<Coverage> {
        let mut coverage = self.coverage.clone()?;
        if let Some((piece, _)) = &self.open {
            coverage.add(piece.clone());
        }
        Some(coverage)
    }
}

/// What a finished capture holds: the stitched WAV, its length and the stream's.
pub(crate) struct Stitched {
    pub wav: PathBuf,
    pub ms: u64,
    pub stream_ms: Option<u64>,
}

/// Captures the sound `player` plays into `capture.wav` in `dir`, in as many runs as it takes to
/// cover what plays of it; `share` hears how much is covered.
pub(crate) async fn capture(
    app: &AppHandle,
    player: Player<'_>,
    dir: &Path,
    expected: &Expected,
    share: &(dyn Fn(f64) + Send + Sync),
    stop: &Notify,
) -> Result<Stitched, CaptureError> {
    let work = dir.join("sound");
    tokio::fs::create_dir_all(&work)
        .await
        .map_err(|error| CaptureError::Device(error.to_string()))?;
    let mut runs = Runs {
        dir: work.clone(),
        coverage: None,
        open: None,
        count: 0,
    };
    let mut length_ms: Option<f64> = None;
    let mut ended_at: Option<f64> = None;
    let mut stalls: Vec<f64> = Vec::new();
    let mut last_error: Option<String> = None;
    let (mut from_ms, mut until_ms) = (0.0, None::<f64>);
    for _ in 0..PASSES {
        let mut session = start(app, player, from_ms, &work)
            .await
            .map_err(CaptureError::Failed)?;
        let end = follow(
            &mut session,
            &mut runs,
            &mut length_ms,
            until_ms,
            expected,
            share,
            stop,
        )
        .await;
        end_session(app, session).await;
        runs.close()?;
        let end = end?;
        let Some(coverage) = runs.coverage.clone() else {
            let why = match end {
                RunEnd::Failed(error) => error,
                _ => "No sound came from the stream".into(),
            };
            return Err(CaptureError::Failed(why));
        };
        match &end {
            RunEnd::Ended => {
                ended_at.get_or_insert(coverage.ms_at(last_end(&coverage)) as f64);
            }
            RunEnd::Failed(error) => last_error = Some(error.clone()),
            RunEnd::Stalled(_) | RunEnd::Reached => {}
        }
        let total_ms = length_ms
            .or(ended_at)
            .or(expected.duration_ms.map(|ms| ms as f64));
        let required = required_frames(&coverage, expected, total_ms);
        let Some((gap_start, gap_end)) = coverage.missing(required.0, required.1).first().copied()
        else {
            let wav = dir.join("capture.wav");
            let (target, rate) = (wav.clone(), coverage.rate);
            let to = covered_end(&coverage, required.1);
            let frames = tokio::task::spawn_blocking(move || {
                coverage.stitch_padded(0, Some(to), required.0, &target)
            })
            .await
            .map_err(|error| CaptureError::Failed(error.to_string()))?
            .map_err(CaptureError::Failed)?;
            let _ = tokio::fs::remove_dir_all(&work).await;
            return Ok(Stitched {
                wav,
                ms: frames * 1000 / u64::from(rate.max(1)),
                stream_ms: total_ms.map(|ms| ms as u64),
            });
        };
        let (gap_start, gap_end) = (
            coverage.ms_at(gap_start) as f64,
            coverage.ms_at(gap_end) as f64,
        );
        match end {
            // It stopped in the part that's missing: past it, unless it stops there every time.
            RunEnd::Stalled(at) if at >= gap_start - MARGIN_MS && at < gap_end => {
                if stalls.iter().any(|stall| (stall - at).abs() < 2_000.0) {
                    return Err(CaptureError::Failed(format!(
                        "The stream stops at {} every time",
                        clock(at as u64)
                    )));
                }
                stalls.push(at);
                from_ms = at + SKIP_MS;
                until_ms = until_ms.map(|until| until.max(from_ms + MARGIN_MS));
            }
            _ => {
                from_ms = (gap_start - MARGIN_MS).max(0.0);
                until_ms = Some(gap_end + MARGIN_MS);
            }
        }
    }
    let _ = tokio::fs::remove_dir_all(&work).await;
    Err(CaptureError::Failed(last_error.unwrap_or_else(|| {
        "The stream kept stopping, so parts of it couldn't be captured".into()
    })))
}

/// `m:ss`.
fn clock(ms: u64) -> String {
    let seconds = ms / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// Where the coverage ends.
fn last_end(coverage: &Coverage) -> u64 {
    coverage.covered().last().map_or(0, |(_, end)| *end)
}

/// The end of the covered stretch that reaches `to` (else the last one's).
fn covered_end(coverage: &Coverage, to: u64) -> u64 {
    let gap = frames_at(super::pcm::GAP_MS as f64, coverage.rate);
    coverage
        .covered()
        .into_iter()
        .find(|(start, end)| *start <= to && *end + gap >= to)
        .map_or_else(|| last_end(coverage), |(_, end)| end)
}

/// The part that must be captured, in frames: what plays of the song (its trims), up to the
/// stream's end.
fn required_frames(coverage: &Coverage, expected: &Expected, total_ms: Option<f64>) -> (u64, u64) {
    let start = coverage.frames_at(expected.start_ms as f64);
    let end_ms = match (expected.end_ms, total_ms) {
        (Some(end), Some(total)) => (end as f64).min(total),
        (Some(end), None) => end as f64,
        (None, Some(total)) => total,
        (None, None) => coverage.ms_at(last_end(coverage)) as f64,
    };
    (start, coverage.frames_at(end_ms).max(start))
}

/// Takes one run's reports until it ends, stalls, fails or reaches `until_ms`.
#[allow(clippy::too_many_arguments)]
async fn follow(
    session: &mut Session,
    runs: &mut Runs,
    length_ms: &mut Option<f64>,
    until_ms: Option<f64>,
    expected: &Expected,
    share: &(dyn Fn(f64) + Send + Sync),
    stop: &Notify,
) -> Result<RunEnd, CaptureError> {
    let started = Instant::now();
    let mut heard: Option<Instant> = None;
    let mut told: Option<Instant> = None;
    loop {
        let wait = match heard {
            Some(at) => STALLED.saturating_sub(at.elapsed()),
            None => STARTING.saturating_sub(started.elapsed()),
        };
        let report = tokio::select! {
            report = tokio::time::timeout(wait, session.reports.recv()) => report,
            () = stop.notified() => return Err(CaptureError::Stopped),
        };
        let report = match report {
            Ok(Some(report)) => report,
            Ok(None) => return Ok(RunEnd::Failed("The capture player stopped".into())),
            Err(_) if heard.is_some() => {
                return Ok(RunEnd::Stalled(runs.reached_ms().unwrap_or(0.0)));
            }
            Err(_) => return Ok(RunEnd::Failed("The stream didn't start".into())),
        };
        match report {
            Report::Sound {
                start_ms,
                rate,
                channels,
                samples,
            } => {
                heard = Some(Instant::now());
                runs.add(start_ms, rate, channels, &samples)?;
                let total = length_ms.or(expected.duration_ms.map(|ms| ms as f64));
                if told.is_none_or(|at| at.elapsed() >= Duration::from_millis(250))
                    && let Some(total) = total.filter(|total| *total > 0.0)
                    && let Some(coverage) = runs.snapshot()
                {
                    told = Some(Instant::now());
                    let to = coverage.frames_at(total);
                    share(coverage.covered_within(0, to) as f64 / to.max(1) as f64);
                }
                if let (Some(until), Some(reached)) = (until_ms, runs.reached_ms())
                    && reached >= until
                {
                    return Ok(RunEnd::Reached);
                }
            }
            Report::Stalled => {}
            Report::Length(length) => *length_ms = Some(length),
            Report::Ended => return Ok(RunEnd::Ended),
            Report::Failed(error) => return Ok(RunEnd::Failed(error)),
            Report::Interrupted => return Err(CaptureError::Interrupted),
        }
    }
}
