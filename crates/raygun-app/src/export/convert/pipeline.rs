//! Running a conversion: its passes one after another in a folder of the cache's `convert/`, then
//! a check of what came out. A file that doesn't check out (or a pass that fails) is made again
//! with everything re-encoded, when that's something else to try.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tauri::{AppHandle, Manager};

use super::ffmpeg::{self, Expect, Input, SubtitleFile, Tools};
use super::{Engines, MediaInfo, Pass, Route, Speeds, Step, android, reencoded, route};
use crate::export::remux::matroska::TextTrack;
use crate::export::remux::{self, Target};
use crate::startup::AppDb;
use crate::ytdlp::YtDlpService;

/// A downloaded video to convert: `file` (a video or a saved HLS playlist) in `dir`.
#[derive(Debug, Clone)]
pub struct Source {
    pub dir: PathBuf,
    pub file: String,
    pub info: MediaInfo,
}

impl Source {
    pub fn hls(&self) -> bool {
        self.file.ends_with(".m3u8")
    }

    fn path(&self) -> PathBuf {
        self.dir.join(&self.file)
    }

    fn matroska(&self) -> bool {
        self.file.ends_with(".mkv") || self.file.ends_with(".webm")
    }
}

/// One conversion.
#[derive(Debug, Clone)]
pub struct Request {
    pub source: Source,
    pub target: Target,
    /// Subtitle files to put inside, where the format holds them.
    pub subtitles: Vec<SubtitleFile>,
    /// The file's title (the episode's).
    pub title: String,
    /// Where the passes write: a folder of this conversion's own.
    pub work: PathBuf,
}

/// A converted file, checked.
#[derive(Debug, Clone)]
pub struct Converted {
    pub path: PathBuf,
    /// Which of the request's subtitle files are inside it (the rest go beside it).
    pub inside: Vec<bool>,
    pub route: Route,
}

pub type Progress = Arc<dyn Fn(f64) + Send + Sync>;

/// FFmpeg on this computer, as yt-dlp's service finds it.
pub fn ffmpeg_tools(app: &AppHandle) -> Option<Tools> {
    app.try_state::<Arc<YtDlpService>>()?.ffmpeg_tools()
}

/// `local_meta` key of the speeds measured here.
const SPEEDS: &str = "export.convertSpeeds";

/// How fast this device converted before, else a guess.
pub async fn speeds(app: &AppHandle) -> Speeds {
    let guessed = Speeds::guessed();
    let Some(db) = app.try_state::<AppDb>() else {
        return guessed;
    };
    let stored = raygun_db::local_value(&db.0, SPEEDS).await.ok().flatten();
    let Some(value) = stored.and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
    else {
        return guessed;
    };
    let read = |key: &str, fallback: f64| {
        value[key]
            .as_f64()
            .filter(|speed| speed.is_finite() && *speed > 0.0)
            .unwrap_or(fallback)
    };
    Speeds {
        pixels: read("pixels", guessed.pixels),
        sound: read("sound", guessed.sound),
        bytes: read("bytes", guessed.bytes),
    }
}

/// Folds a measured speed into the stored ones (each new one counts for half).
async fn remember_speed(app: &AppHandle, change: impl FnOnce(&mut Speeds)) {
    let mut known = speeds(app).await;
    let before = known;
    change(&mut known);
    let blended = Speeds {
        pixels: (before.pixels + known.pixels) / 2.0,
        sound: (before.sound + known.sound) / 2.0,
        bytes: (before.bytes + known.bytes) / 2.0,
    };
    let Some(db) = app.try_state::<AppDb>() else {
        return;
    };
    let value = serde_json::json!({
        "pixels": blended.pixels,
        "sound": blended.sound,
        "bytes": blended.bytes,
    })
    .to_string();
    if let Err(error) = raygun_db::set_local_value(&db.0, SPEEDS, &value).await {
        tracing::debug!(%error, "could not keep the conversion speed");
    }
}

/// Converts `request.source` into `request.target`, checked; FFmpeg is fetched first when a
/// computer needs it to re-encode.
pub async fn convert(
    app: &AppHandle,
    request: &Request,
    cancel: Arc<AtomicBool>,
    progress: Progress,
) -> Result<Converted, String> {
    let mut tools = ffmpeg_tools(app);
    let needs_ffmpeg = route(
        &request.source.info,
        request.target,
        request.source.hls(),
        Engines::here(tools.is_some()),
    )
    .needs_ffmpeg;
    if needs_ffmpeg {
        let ytdlp = app
            .try_state::<Arc<YtDlpService>>()
            .ok_or("Re-encoding this video needs FFmpeg")?
            .inner()
            .clone();
        ytdlp
            .install_ffmpeg()
            .await
            .map_err(|why| format!("Re-encoding this video needs FFmpeg ({why})"))?;
        tools = ffmpeg_tools(app);
    }
    convert_with(Some(app), tools, request, cancel, progress).await
}

/// [`convert`] with the FFmpeg given (none: the remux, or Transformer on Android); `app` keeps the
/// speeds measured and reaches the plugin on Android.
pub async fn convert_with(
    app: Option<&AppHandle>,
    tools: Option<Tools>,
    request: &Request,
    cancel: Arc<AtomicBool>,
    progress: Progress,
) -> Result<Converted, String> {
    let hls = request.source.hls();
    let info = &request.source.info;
    let engines = Engines::here(tools.is_some());
    let mut tried = route(info, request.target, hls, engines);
    if tried.needs_ffmpeg {
        return Err("Re-encoding this video needs FFmpeg".into());
    }
    std::fs::create_dir_all(&request.work).map_err(|error| error.to_string())?;
    loop {
        let made = run(app, request, &tried, tools.as_ref(), &cancel, &progress).await;
        let checked = match made {
            Ok((path, expect)) => check(app, tools.as_ref(), &path, request.target, &expect)
                .await
                .map(|()| path),
            Err(why) => Err(why),
        };
        match checked {
            Ok(path) => {
                return Ok(Converted {
                    inside: inside(&tried, request),
                    path,
                    route: tried,
                });
            }
            Err(_) if cancel.load(Ordering::SeqCst) => return Err("Cancelled".into()),
            Err(why) => {
                let Some(next) = reencoded(&tried, info, request.target, hls, engines) else {
                    return Err(why);
                };
                tracing::warn!(%why, "a converted video didn't check out: re-encoding it");
                tried = next;
            }
        }
    }
}

/// Subtitle formats FFmpeg reads.
fn ffmpeg_reads(subtitle: &SubtitleFile) -> bool {
    matches!(subtitle.format.as_str(), "srt" | "vtt" | "ass" | "ssa")
}

/// Subtitle formats the remux puts in Matroska as text.
fn remux_reads(subtitle: &SubtitleFile) -> bool {
    matches!(subtitle.format.as_str(), "srt" | "vtt")
}

/// Which subtitle files the route's last pass puts inside.
fn inside(route: &Route, request: &Request) -> Vec<bool> {
    let last = route.passes.last();
    request
        .subtitles
        .iter()
        .map(|subtitle| match last {
            _ if request.target.audio_only() => false,
            Some(Pass::Ffmpeg { .. }) => ffmpeg_reads(subtitle),
            Some(Pass::Remux(Target::Mkv)) => remux_reads(subtitle),
            _ => false,
        })
        .collect()
}

/// A subtitle file's cues as a Matroska text track.
fn text_track(subtitle: &SubtitleFile) -> Option<TextTrack> {
    let text = std::fs::read_to_string(&subtitle.path).ok()?;
    let kind = if subtitle.format == "vtt" {
        "text/vtt"
    } else {
        "application/srt"
    };
    let cues = crate::captions::cues::parse(kind, &text)?
        .into_iter()
        .filter(|row| !row.text.trim().is_empty())
        .map(|row| (row.start_ms, row.end_ms, plain_tags(&row.text)))
        .collect::<Vec<_>>();
    (!cues.is_empty()).then(|| TextTrack {
        language: subtitle.language.as_deref().map(iso639_2),
        cues,
    })
}

/// A cue's text with only the tags SubRip players show (italic, bold, underline).
fn plain_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('>') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let tag = &rest[open..open + close + 1];
        let name = tag
            .trim_start_matches('<')
            .trim_start_matches('/')
            .trim_end_matches('>');
        if matches!(name, "i" | "b" | "u") {
            out.push_str(tag);
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

/// A two-letter language as Matroska's three letters (its bibliographic ISO 639-2 codes).
pub fn iso639_2(code: &str) -> String {
    let lower = code.trim().to_lowercase();
    let known = [
        ("en", "eng"),
        ("es", "spa"),
        ("fr", "fre"),
        ("de", "ger"),
        ("pt", "por"),
        ("it", "ita"),
        ("ja", "jpn"),
        ("ru", "rus"),
        ("zh", "chi"),
        ("ko", "kor"),
        ("ar", "ara"),
        ("nl", "dut"),
        ("pl", "pol"),
        ("tr", "tur"),
        ("sv", "swe"),
        ("no", "nor"),
        ("da", "dan"),
        ("fi", "fin"),
        ("cs", "cze"),
        ("el", "gre"),
        ("he", "heb"),
        ("hu", "hun"),
        ("ro", "rum"),
        ("hi", "hin"),
        ("id", "ind"),
        ("vi", "vie"),
        ("th", "tha"),
        ("uk", "ukr"),
        ("bg", "bul"),
        ("hr", "hrv"),
        ("sr", "srp"),
        ("sk", "slo"),
        ("sl", "slv"),
        ("fa", "per"),
        ("ms", "may"),
    ];
    known
        .iter()
        .find(|(short, _)| *short == lower)
        .map_or(lower.clone(), |(_, long)| (*long).to_owned())
}

/// How much of a conversion's time a pass takes.
fn weight(pass: &Pass) -> f64 {
    if pass.encodes() { 10.0 } else { 1.0 }
}

/// Runs a route's passes; the last one's file comes back with what it must hold.
async fn run(
    app: Option<&AppHandle>,
    request: &Request,
    route: &Route,
    tools: Option<&Tools>,
    cancel: &Arc<AtomicBool>,
    progress: &Progress,
) -> Result<(PathBuf, Expect), String> {
    let source = &request.source;
    let info = &source.info;
    let total: f64 = route.passes.iter().map(weight).sum::<f64>().max(1.0);
    let mut done = 0.0;
    let mut last_file: Option<PathBuf> = None;
    // The source's length: measured again from a joined stream when there is one.
    let mut expected = info.seconds;
    let mut approximate = source.hls();
    for (index, pass) in route.passes.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            return Err("Cancelled".into());
        }
        let last = index + 1 == route.passes.len();
        let out = request
            .work
            .join(format!("pass{index}.{}", pass.target().extension()));
        let _ = std::fs::remove_file(&out);
        let started = Instant::now();
        let share = {
            let progress = progress.clone();
            let (done, weight) = (done, weight(pass));
            move |part: f64| progress((done + part.clamp(0.0, 1.0) * weight) / total)
        };
        match pass {
            Pass::Remux(target) => {
                let input = match &last_file {
                    Some(path) => remux::Source::File(path.clone()),
                    None => remux::Source::of_download(&source.dir, &source.file)
                        .map_err(|error| error.to_string())?,
                };
                let texts: Vec<TextTrack> = if last && *target == Target::Mkv {
                    request
                        .subtitles
                        .iter()
                        .filter(|subtitle| remux_reads(subtitle))
                        .filter_map(text_track)
                        .collect()
                } else {
                    Vec::new()
                };
                let (target, written, cancel) = (*target, out.clone(), cancel.clone());
                tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
                    let file = File::create(&written).map_err(|error| error.to_string())?;
                    let mut writer = BufWriter::with_capacity(1 << 20, file);
                    remux::convert_with(&input, target, &mut writer, &cancel, &texts)
                        .map_err(|error| error.to_string())?;
                    writer.flush().map_err(|error| error.to_string())
                })
                .await
                .map_err(|error| error.to_string())??;
                if !last
                    && let Some(tools) = tools
                    && let Ok(measured) = ffmpeg::probe(tools, &out).await
                    && measured.seconds.is_some()
                {
                    expected = measured.seconds;
                    approximate = false;
                }
            }
            Pass::Ffmpeg { target, decision } => {
                let tools = tools.ok_or("FFmpeg is missing")?;
                let (input, job_info) = match &last_file {
                    Some(path) => (
                        Input::File {
                            path: path.clone(),
                            matroska: true,
                        },
                        ffmpeg::probe(tools, path)
                            .await
                            .unwrap_or_else(|_| info.clone()),
                    ),
                    None if source.hls() => (Input::Playlist(source.path()), info.clone()),
                    None => (
                        Input::File {
                            path: source.path(),
                            matroska: source.matroska(),
                        },
                        info.clone(),
                    ),
                };
                let abilities = ffmpeg::abilities(tools).await;
                let encoder = if decision.video == Some(Step::Encode) {
                    ffmpeg::video_encoder(tools, &abilities).await?
                } else {
                    ffmpeg::VideoEncoder::X264
                };
                let subtitles: Vec<SubtitleFile> = if last {
                    request
                        .subtitles
                        .iter()
                        .filter(|subtitle| ffmpeg_reads(subtitle))
                        .cloned()
                        .collect()
                } else {
                    Vec::new()
                };
                let args = ffmpeg::args(&ffmpeg::Job {
                    input,
                    info: &job_info,
                    target: *target,
                    decision: *decision,
                    encoder,
                    abilities: &abilities,
                    subtitles: &subtitles,
                    title: Some(&request.title),
                    output: &out,
                });
                ffmpeg::run(tools, &args, job_info.seconds, share, cancel).await?;
                if pass.encodes()
                    && let Some(app) = app
                {
                    measured(
                        app,
                        &job_info,
                        decision.video == Some(Step::Encode),
                        started,
                    )
                    .await;
                }
            }
            Pass::Transformer { audio_only } => {
                let app = app.ok_or("Re-encoding with the phone's encoders needs the app")?;
                let input = last_file.clone().unwrap_or_else(|| source.path());
                let job = android::Transform {
                    input: &input,
                    output: &out,
                    audio_only: *audio_only,
                    // MP3 (made from this pass's AAC) holds two channels at most.
                    stereo: matches!(request.target, Target::Mp3 { .. }),
                    video: info.video.is_some(),
                    audio: info.audio.is_some(),
                };
                let result = android::transform(app, &job, cancel, &share).await;
                // Some phones' AAC encoders take two channels only.
                let surround =
                    !job.stereo && info.audio.as_ref().is_some_and(|audio| audio.channels > 2);
                match result {
                    Err(why) if surround && !cancel.load(Ordering::SeqCst) => {
                        tracing::info!(%why, "converting again with the sound mixed to stereo");
                        let stereo = android::Transform {
                            stereo: true,
                            ..job
                        };
                        android::transform(app, &stereo, cancel, &share).await?;
                    }
                    other => other?,
                }
                measured(app, info, !audio_only && route.encodes_video(), started).await;
            }
        }
        if let Some(previous) = last_file.replace(out) {
            let _ = std::fs::remove_file(previous);
        }
        done += weight(pass);
        progress(done / total);
    }
    let path = last_file.ok_or("Nothing was converted")?;
    let expect = Expect {
        video: route.decision.video.is_some() && !request.target.audio_only(),
        audio: route.decision.audio.is_some(),
        seconds: expected,
        approximate,
    };
    Ok((path, expect))
}

/// Keeps how fast an encoding pass went, for later estimates.
async fn measured(app: &AppHandle, info: &MediaInfo, video: bool, started: Instant) {
    let elapsed = started.elapsed().as_secs_f64();
    let Some(seconds) = info.seconds.filter(|seconds| *seconds > 0.0) else {
        return;
    };
    if elapsed < 1.0 {
        return;
    }
    if video {
        let Some(picture) = info.video.as_ref().filter(|video| video.width > 0) else {
            return;
        };
        let pixels = f64::from(picture.width)
            * f64::from(picture.height)
            * picture.fps.unwrap_or(30.0)
            * seconds
            / elapsed;
        remember_speed(app, |speeds| speeds.pixels = pixels).await;
    } else {
        remember_speed(app, |speeds| speeds.sound = seconds / elapsed).await;
    }
}

/// Checks a converted file: with FFmpeg (and Media Foundation on Windows) on desktop, Android's
/// own extractor and decoders on a phone.
async fn check(
    app: Option<&AppHandle>,
    tools: Option<&Tools>,
    path: &Path,
    target: Target,
    expect: &Expect,
) -> Result<(), String> {
    if cfg!(target_os = "android")
        && let Some(app) = app
    {
        let found = android::check(app, path).await?;
        if expect.video && !(found.video && found.frame) {
            return Err("The converted file's picture doesn't decode on this phone".into());
        }
        if expect.audio && !found.audio {
            return Err("The converted file has no sound this phone plays".into());
        }
        if let Some(seconds) = expect.seconds
            && !expect.length_fits(found.seconds)
        {
            return Err(format!(
                "The converted file plays {:.1} s of the video's {seconds:.1} s",
                found.seconds
            ));
        }
        return Ok(());
    }
    match tools {
        Some(tools) => {
            ffmpeg::check(tools, path, expect).await?;
        }
        None => {
            // Without FFmpeg, the remux reads its own MP4 and Matroska back.
            if !matches!(target, Target::Mp3 { .. }) {
                let file = path.to_owned();
                let probe = tauri::async_runtime::spawn_blocking(move || {
                    remux::probe(&remux::Source::File(file))
                })
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| format!("The converted file is unreadable: {error}"))?;
                let found = MediaInfo::from_probe(&probe, 0);
                if let Some(problem) = ffmpeg::shortfall(&found, expect) {
                    return Err(problem);
                }
            }
        }
    }
    #[cfg(windows)]
    if target != Target::Mkv {
        let file = path.to_owned();
        let opened =
            tauri::async_runtime::spawn_blocking(move || super::media_foundation::open(&file))
                .await
                .map_err(|error| error.to_string())??;
        if expect.video && !opened.video {
            return Err("Windows' players don't show the converted file's picture".into());
        }
        if expect.audio && !opened.audio {
            return Err("Windows' players don't play the converted file's sound".into());
        }
        if let Some(seconds) = expect.seconds
            && !expect.length_fits(opened.seconds)
        {
            return Err(format!(
                "Windows' players find {:.1} s in the converted file, not {seconds:.1} s",
                opened.seconds
            ));
        }
    }
    Ok(())
}
