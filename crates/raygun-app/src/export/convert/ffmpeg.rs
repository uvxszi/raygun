//! FFmpeg on desktop: what a file holds (`ffprobe`), the command line that copies or re-encodes
//! it, running it with progress and cancelling, and checking what it wrote.
//!
//! Re-encoded pictures are H.264 High in 8-bit 4:2:0 (`libx264 -preset veryfast -crf 20`; a
//! hardware or Media Foundation encoder only when the FFmpeg found has no libx264), HDR tone-mapped
//! to SDR; sound is AAC (192 kb/s for stereo, 384 kb/s for 5.1) or MP3 at the Downloads quality.
//! MP4 and M4A start with their index (`+faststart`). Subtitle files are embedded: `mov_text` in
//! MP4, SubRip (or ASS as it is) in MKV.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};

use super::{AAC_CHANNELS, AudioInfo, Decision, MediaInfo, REMUX_CODECS, Step, VideoInfo};
use crate::export::remux::Target;

/// FFmpeg's programs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

impl Tools {
    /// `ffmpeg` and `ffprobe` in `dir`, when both are there.
    pub fn in_dir(dir: &Path) -> Option<Self> {
        let tools = Self {
            ffmpeg: dir.join(exe("ffmpeg")),
            ffprobe: dir.join(exe("ffprobe")),
        };
        (tools.ffmpeg.is_file() && tools.ffprobe.is_file()).then_some(tools)
    }
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// A program with no console window and no input, ended if Raygun lets go of it.
fn command(program: &Path) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(program);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW: no console flashes up for each run.
        command.creation_flags(0x0800_0000);
    }
    command
}

// ---- What a file holds ----

/// The pixel formats every decoder takes (8-bit 4:2:0).
const PLAIN_FORMATS: &[&str] = &["yuv420p", "yuvj420p", "nv12"];

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// `30000/1001` as a number of frames per second.
fn rate(value: &Value) -> Option<f64> {
    let text = value.as_str()?;
    let (top, bottom) = text.split_once('/').unwrap_or((text, "1"));
    let (top, bottom) = (
        top.trim().parse::<f64>().ok()?,
        bottom.trim().parse::<f64>().ok()?,
    );
    (top > 0.0 && bottom > 0.0).then(|| top / bottom)
}

/// What `ffprobe -print_format json -show_format -show_streams` says a file holds.
pub fn parse_probe(json: &str) -> Option<MediaInfo> {
    let root: Value = serde_json::from_str(json).ok()?;
    let streams = root.get("streams")?.as_array()?;
    let mut info = MediaInfo::default();
    let mut longest = 0.0f64;
    for stream in streams {
        let codec = stream["codec_name"]
            .as_str()
            .unwrap_or("unknown")
            .to_owned();
        if let Some(seconds) = number(&stream["duration"]) {
            longest = longest.max(seconds);
        }
        match stream["codec_type"].as_str() {
            Some("video") if info.video.is_none() => {
                // Cover pictures are video streams too.
                if stream["disposition"]["attached_pic"].as_i64() == Some(1) {
                    continue;
                }
                let profile = stream["profile"].as_str().unwrap_or_default();
                let pixels = stream["pix_fmt"].as_str();
                let deep_profile = ["10", "4:2:2", "4:4:4", "422", "444"]
                    .iter()
                    .any(|mark| profile.contains(mark));
                let transfer = stream["color_transfer"].as_str().unwrap_or_default();
                info.video = Some(VideoInfo {
                    remux: REMUX_CODECS.contains(&codec.as_str()),
                    plain: !deep_profile
                        && pixels.is_none_or(|pixels| PLAIN_FORMATS.contains(&pixels)),
                    hdr: matches!(transfer, "smpte2084" | "arib-std-b67"),
                    width: stream["width"].as_u64().unwrap_or(0) as u32,
                    height: stream["height"].as_u64().unwrap_or(0) as u32,
                    fps: rate(&stream["avg_frame_rate"]).or_else(|| rate(&stream["r_frame_rate"])),
                    codec,
                });
            }
            Some("audio") if info.audio.is_none() => {
                info.audio = Some(AudioInfo {
                    remux: REMUX_CODECS.contains(&codec.as_str()),
                    channels: stream["channels"].as_u64().unwrap_or(0) as u32,
                    codec,
                });
            }
            Some("subtitle") => info.subtitles += 1,
            _ => {}
        }
    }
    let format = &root["format"];
    info.seconds = number(&format["duration"])
        .filter(|seconds| *seconds > 0.0)
        .or((longest > 0.0).then_some(longest));
    info.bytes = number(&format["size"]).unwrap_or(0.0) as u64;
    Some(info)
}

/// What a file holds, by ffprobe.
pub async fn probe(tools: &Tools, input: &Path) -> Result<MediaInfo, String> {
    let output = tokio::time::timeout(
        Duration::from_secs(60),
        command(&tools.ffprobe)
            .args([
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
            ])
            .arg(input)
            .output(),
    )
    .await
    .map_err(|_| "ffprobe took too long".to_owned())?
    .map_err(|error| format!("ffprobe didn't start: {error}"))?;
    if !output.status.success() {
        let why = String::from_utf8_lossy(&output.stderr);
        return Err(last_line(&why).unwrap_or_else(|| "ffprobe can't read the file".into()));
    }
    parse_probe(&String::from_utf8_lossy(&output.stdout))
        .ok_or_else(|| "ffprobe's answer was unreadable".to_owned())
}

// ---- What this FFmpeg can do ----

/// The encoders and filters a build has.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Abilities {
    pub encoders: HashSet<String>,
    /// zscale and tonemap: HDR pictures turned into SDR.
    pub tone_mapping: bool,
    /// Its HLS reader checks segments' names (`extension_picky`, FFmpeg 7.1 and later).
    pub picky_hls: bool,
}

/// Encoder names from `ffmpeg -encoders` (lines like ` V....D libx264   libx264 H.264…`).
pub fn parse_encoders(text: &str) -> HashSet<String> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let flags = parts.next()?;
            let name = parts.next()?;
            (flags.len() == 6
                && flags
                    .chars()
                    .next()
                    .is_some_and(|kind| "VAS".contains(kind))
                && name != "=")
                .then(|| name.to_owned())
        })
        .collect()
}

/// What FFmpeg at `tools` can do, asked once per program.
pub async fn abilities(tools: &Tools) -> Abilities {
    static KNOWN: OnceLock<Mutex<HashMap<PathBuf, Abilities>>> = OnceLock::new();
    let known = KNOWN.get_or_init(Mutex::default);
    if let Some(found) = known
        .lock()
        .ok()
        .and_then(|known| known.get(&tools.ffmpeg).cloned())
    {
        return found;
    }
    let ask = |args: &'static [&'static str]| {
        let program = tools.ffmpeg.clone();
        async move {
            let output = tokio::time::timeout(
                Duration::from_secs(30),
                command(&program).args(args).output(),
            )
            .await;
            match output {
                Ok(Ok(output)) => String::from_utf8_lossy(&output.stdout).into_owned(),
                _ => String::new(),
            }
        }
    };
    let encoders = parse_encoders(&ask(&["-hide_banner", "-encoders"]).await);
    let filters = ask(&["-hide_banner", "-filters"]).await;
    let hls = ask(&["-hide_banner", "-h", "demuxer=hls"]).await;
    let found = Abilities {
        encoders,
        tone_mapping: filters.contains(" zscale ") && filters.contains(" tonemap "),
        picky_hls: hls.contains("extension_picky"),
    };
    if let Ok(mut known) = known.lock() {
        known.insert(tools.ffmpeg.clone(), found.clone());
    }
    found
}

/// The H.264 encoder a conversion uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoEncoder {
    X264,
    Nvenc,
    Qsv,
    Amf,
    MediaFoundation,
}

impl VideoEncoder {
    fn name(self) -> &'static str {
        match self {
            Self::X264 => "libx264",
            Self::Nvenc => "h264_nvenc",
            Self::Qsv => "h264_qsv",
            Self::Amf => "h264_amf",
            Self::MediaFoundation => "h264_mf",
        }
    }

    /// Its options for a picture as good as `-crf 20`, in a profile every player takes.
    fn args(self) -> Vec<String> {
        let args: &[&str] = match self {
            Self::X264 => &["-preset", "veryfast", "-crf", "20", "-profile:v", "high"],
            Self::Nvenc => &[
                "-preset",
                "p4",
                "-rc",
                "vbr",
                "-cq",
                "23",
                "-b:v",
                "0",
                "-maxrate",
                "20M",
                "-bufsize",
                "40M",
                "-profile:v",
                "high",
            ],
            Self::Qsv => &["-global_quality", "23", "-profile:v", "high"],
            Self::Amf => &[
                "-rc",
                "qvbr",
                "-qvbr_quality_level",
                "23",
                "-profile:v",
                "high",
            ],
            Self::MediaFoundation => &[
                "-rate_control",
                "quality",
                "-quality",
                "70",
                "-hw_encoding",
                "1",
            ],
        };
        let mut all = vec!["-c:v".to_owned(), self.name().to_owned()];
        all.extend(args.iter().map(|arg| (*arg).to_owned()));
        all
    }
}

/// The encoder for re-encoded pictures: libx264, else the first hardware (or Media Foundation)
/// encoder that encodes a test frame here.
pub async fn video_encoder(tools: &Tools, abilities: &Abilities) -> Result<VideoEncoder, String> {
    if abilities.encoders.contains("libx264") {
        return Ok(VideoEncoder::X264);
    }
    for encoder in [
        VideoEncoder::Nvenc,
        VideoEncoder::Qsv,
        VideoEncoder::Amf,
        VideoEncoder::MediaFoundation,
    ] {
        if !abilities.encoders.contains(encoder.name()) {
            continue;
        }
        let output = tokio::time::timeout(
            Duration::from_secs(30),
            command(&tools.ffmpeg)
                .args(["-hide_banner", "-v", "error", "-f", "lavfi", "-i"])
                .arg("color=c=black:s=320x240:d=0.2")
                .args(["-pix_fmt", "yuv420p"])
                .args(encoder.args())
                .args(["-f", "null", "-"])
                .output(),
        )
        .await;
        if matches!(output, Ok(Ok(output)) if output.status.success()) {
            return Ok(encoder);
        }
    }
    Err("This FFmpeg has no H.264 encoder that works here".into())
}

// ---- The command line ----

/// A subtitle file embedded in the output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleFile {
    pub path: PathBuf,
    /// `srt`, `vtt`, `ass`…
    pub format: String,
    pub language: Option<String>,
}

/// Where the main input comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// A file (Matroska keeps its own subtitle streams when it is Matroska too).
    File { path: PathBuf, matroska: bool },
    /// A saved HLS playlist, read by FFmpeg's own HLS reader.
    Playlist(PathBuf),
}

/// One run of FFmpeg.
#[derive(Debug, Clone)]
pub struct Job<'a> {
    pub input: Input,
    pub info: &'a MediaInfo,
    pub target: Target,
    pub decision: Decision,
    pub encoder: VideoEncoder,
    pub abilities: &'a Abilities,
    pub subtitles: &'a [SubtitleFile],
    pub title: Option<&'a str>,
    pub output: &'a Path,
}

/// AAC's rate for `channels`.
fn aac_rate(channels: u32) -> &'static str {
    if channels > 2 { "384k" } else { "192k" }
}

/// The video filters a re-encoded picture goes through: HDR to SDR, even sizes (4:2:0 needs
/// them), 8-bit 4:2:0.
fn video_filters(video: Option<&VideoInfo>, abilities: &Abilities) -> String {
    let mut filters = Vec::new();
    if video.is_some_and(|video| video.hdr) && abilities.tone_mapping {
        filters.push(
            "zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=hable:desat=0,\
             zscale=t=bt709:m=bt709:r=tv"
                .to_owned(),
        );
    }
    let odd = video.is_none_or(|video| video.width % 2 == 1 || video.height % 2 == 1);
    if odd {
        filters.push("scale=trunc(iw/2)*2:trunc(ih/2)*2".to_owned());
    }
    filters.push("format=yuv420p".to_owned());
    filters.join(",")
}

/// FFmpeg's arguments for `job`, the output last.
pub fn args(job: &Job<'_>) -> Vec<String> {
    let mut args: Vec<String> = ["-hide_banner", "-nostdin", "-y", "-v", "error", "-nostats"]
        .map(str::to_owned)
        .to_vec();
    args.extend(["-progress".into(), "pipe:1".into()]);
    let audio_only = job.target.audio_only();
    let (main, keeps_subtitles) = match &job.input {
        Input::File { path, matroska } => (path.clone(), *matroska),
        Input::Playlist(path) => {
            args.extend(["-allowed_extensions".into(), "ALL".into()]);
            if job.abilities.picky_hls {
                args.extend(["-extension_picky".into(), "0".into()]);
            }
            (path.clone(), false)
        }
    };
    args.extend(["-i".into(), main.to_string_lossy().into_owned()]);
    let embedded: &[SubtitleFile] = if audio_only { &[] } else { job.subtitles };
    for subtitle in embedded {
        args.extend(["-i".into(), subtitle.path.to_string_lossy().into_owned()]);
    }

    // Streams: the first picture (not a cover) and the first sound; Matroska keeps every sound
    // and a Matroska source's own subtitles.
    let mkv = job.target == Target::Mkv;
    if !audio_only && job.decision.video.is_some() {
        args.extend(["-map".into(), "0:V:0".into()]);
    }
    if job.decision.audio.is_some() {
        let audio = if mkv && job.decision.audio == Some(Step::Copy) {
            "0:a"
        } else {
            "0:a:0"
        };
        args.extend(["-map".into(), audio.into()]);
    }
    let own_subtitles = if mkv && keeps_subtitles {
        job.info.subtitles
    } else {
        0
    };
    if own_subtitles > 0 {
        args.extend(["-map".into(), "0:s?".into()]);
    }
    for index in 0..embedded.len() {
        args.extend(["-map".into(), format!("{}:s:0", index + 1)]);
    }

    match job.decision.video {
        Some(Step::Copy) if !audio_only => args.extend(["-c:v".into(), "copy".into()]),
        Some(Step::Encode) if !audio_only => {
            args.extend(job.encoder.args());
            args.extend([
                "-vf".into(),
                video_filters(job.info.video.as_ref(), job.abilities),
            ]);
            if job.info.video.as_ref().is_some_and(|video| video.hdr) {
                args.extend(
                    [
                        "-color_primaries",
                        "bt709",
                        "-color_trc",
                        "bt709",
                        "-colorspace",
                        "bt709",
                    ]
                    .map(str::to_owned),
                );
            }
        }
        _ => args.push("-vn".into()),
    }
    let channels = job.info.audio.as_ref().map_or(2, |audio| audio.channels);
    match (job.decision.audio, job.target) {
        (Some(Step::Copy), _) => args.extend(["-c:a".into(), "copy".into()]),
        (Some(Step::Encode), Target::Mp3 { kbps }) => {
            args.extend([
                "-c:a".into(),
                "libmp3lame".into(),
                "-b:a".into(),
                format!("{kbps}k"),
            ]);
            if channels > 2 || channels == 0 {
                args.extend(["-ac".into(), "2".into()]);
            }
        }
        (Some(Step::Encode), _) => {
            let channels = if channels == 0 {
                2
            } else {
                channels.min(AAC_CHANNELS)
            };
            args.extend([
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                aac_rate(channels).into(),
            ]);
            if job
                .info
                .audio
                .as_ref()
                .is_some_and(|audio| audio.channels > AAC_CHANNELS)
            {
                args.extend(["-ac".into(), AAC_CHANNELS.to_string()]);
            }
        }
        (None, _) => args.push("-an".into()),
    }
    if audio_only || (embedded.is_empty() && own_subtitles == 0) {
        args.push("-sn".into());
    } else if mkv {
        args.extend(["-c:s".into(), "copy".into()]);
        for (index, subtitle) in embedded.iter().enumerate() {
            // WebVTT goes in as SubRip, which every Matroska player shows.
            if !matches!(subtitle.format.as_str(), "srt" | "ass" | "ssa") {
                args.extend([
                    format!("-c:s:{}", own_subtitles as usize + index),
                    "srt".into(),
                ]);
            }
        }
    } else {
        args.extend(["-c:s".into(), "mov_text".into()]);
    }
    for (index, subtitle) in embedded.iter().enumerate() {
        if let Some(language) = &subtitle.language {
            args.extend([
                format!("-metadata:s:s:{}", own_subtitles as usize + index),
                format!("language={language}"),
            ]);
        }
    }
    args.push("-dn".into());
    if let Some(title) = job.title {
        args.extend(["-metadata".into(), format!("title={title}")]);
    }
    // Streams far apart in the source (a sound stream that starts late) wait in the muxer.
    args.extend(["-max_muxing_queue_size".into(), "4096".into()]);
    let format = match job.target {
        Target::Mp4 => "mp4",
        Target::M4a => "ipod",
        Target::Mkv => "matroska",
        Target::Mp3 { .. } => "mp3",
    };
    if matches!(job.target, Target::Mp4 | Target::M4a) {
        args.extend(["-movflags".into(), "+faststart".into()]);
    }
    args.extend(["-f".into(), format.into()]);
    args.push(job.output.to_string_lossy().into_owned());
    args
}

// ---- Running it ----

/// The last line of an error output worth showing.
fn last_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(str::to_owned)
}

/// How far a `-progress` line says FFmpeg got, in seconds.
pub fn progress_seconds(line: &str) -> Option<f64> {
    let value = line.strip_prefix("out_time_us=")?;
    let micros: i64 = value.trim().parse().ok()?;
    (micros >= 0).then(|| micros as f64 / 1_000_000.0)
}

/// Runs FFmpeg with `args` until it ends or `cancel` is set, telling `progress` the share of
/// `seconds` written so far.
pub async fn run(
    tools: &Tools,
    args: &[String],
    seconds: Option<f64>,
    progress: impl Fn(f64) + Send + Sync + 'static,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let mut child = command(&tools.ffmpeg)
        .args(args)
        .spawn()
        .map_err(|error| format!("FFmpeg didn't start: {error}"))?;
    let stdout = child.stdout.take().ok_or("FFmpeg gave no output")?;
    let stderr = child.stderr.take().ok_or("FFmpeg gave no output")?;
    let progress = Arc::new(progress);
    let reader = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let (Some(done), Some(total)) = (progress_seconds(&line), seconds)
                && total > 0.0
            {
                progress((done / total).clamp(0.0, 1.0));
            }
        }
    });
    let errors = tokio::spawn(async move {
        let mut kept: Vec<String> = Vec::new();
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            kept.push(line);
            if kept.len() > 20 {
                kept.remove(0);
            }
        }
        kept.join("\n")
    });
    let status = loop {
        tokio::select! {
            status = child.wait() => break status.map_err(|error| error.to_string())?,
            () = tokio::time::sleep(Duration::from_millis(200)) => {
                if cancel.load(Ordering::SeqCst) {
                    let _ = child.kill().await;
                    return Err("Cancelled".into());
                }
            }
        }
    };
    let _ = reader.await;
    let errors = errors.await.unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    Err(last_line(&errors).map_or_else(
        || format!("FFmpeg stopped ({status})"),
        |line| format!("FFmpeg couldn't convert it: {line}"),
    ))
}

// ---- Checking the result ----

/// What a converted file must hold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Expect {
    pub video: bool,
    pub audio: bool,
    /// The source's length.
    pub seconds: Option<f64>,
    /// That length is a playlist's sum of its segments' lengths, which may be rounded.
    pub approximate: bool,
}

impl Expect {
    /// Whether a converted file `found` seconds long is as long as the source.
    pub fn length_fits(&self, found: f64) -> bool {
        match self.seconds {
            None => true,
            Some(expected) if self.approximate => {
                (found - expected).abs() <= (expected.abs() * 0.02).max(1.5)
            }
            Some(expected) => same_length(found, expected),
        }
    }
}

/// Lengths that match: within a second (and a thousandth, for long videos).
pub fn same_length(found: f64, expected: f64) -> bool {
    (found - expected).abs() <= 1.0 + expected.abs() * 0.001
}

/// What's wrong with `found` for `expect`, if anything.
pub fn shortfall(found: &MediaInfo, expect: &Expect) -> Option<String> {
    if expect.video && found.video.is_none() {
        return Some("The converted file has no picture".into());
    }
    if expect.audio && found.audio.is_none() {
        return Some("The converted file has no sound".into());
    }
    if let (Some(expected), Some(seconds)) = (expect.seconds, found.seconds)
        && !expect.length_fits(seconds)
    {
        return Some(format!(
            "The converted file plays {seconds:.1} s of the video's {expected:.1} s"
        ));
    }
    if expect.seconds.is_some() && found.seconds.is_none() {
        return Some("The converted file has no length".into());
    }
    None
}

/// Checks a converted file: ffprobe finds its streams and length, and its start and end decode.
pub async fn check(tools: &Tools, path: &Path, expect: &Expect) -> Result<MediaInfo, String> {
    let found = probe(tools, path).await?;
    if let Some(problem) = shortfall(&found, expect) {
        return Err(problem);
    }
    let seconds = found.seconds.unwrap_or(0.0);
    let ends = if seconds > 8.0 {
        vec![0.0, seconds - 4.0]
    } else {
        vec![0.0]
    };
    for from in ends {
        let output = tokio::time::timeout(
            Duration::from_secs(120),
            command(&tools.ffmpeg)
                .args(["-hide_banner", "-nostdin", "-v", "error", "-xerror"])
                .args(["-ss", &format!("{from:.3}")])
                .arg("-i")
                .arg(path)
                .args([
                    "-t", "3", "-map", "0:V:0?", "-map", "0:a:0?", "-f", "null", "-",
                ])
                .output(),
        )
        .await
        .map_err(|_| "Checking the converted file took too long".to_owned())?
        .map_err(|error| format!("FFmpeg didn't start: {error}"))?;
        if !output.status.success() {
            let why = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "The converted file doesn't decode: {}",
                last_line(&why).unwrap_or_default()
            ));
        }
    }
    Ok(found)
}
