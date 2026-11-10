//! Video exports as files every common player plays. A stream is copied when the chosen
//! container holds it that way (H.264 in 8-bit 4:2:0 and AAC for MP4 and M4A, MP3 for MP3, anything
//! Matroska takes for MKV); otherwise it's re-encoded: the picture to H.264, the sound to AAC (MP3
//! for MP3).
//!
//! Who does the work ([`route`]):
//! - Raygun's own remux (`super::remux`) copies packets, and makes MP3 from AAC or MP3 sound.
//! - On desktop, FFmpeg (fetched like yt-dlp's, `ffmpeg`) copies (`-c copy`) or re-encodes
//!   (libx264, AAC, LAME); a saved HLS stream goes through the remux into Matroska first, which
//!   joins its segments, keys and separate sound. Without FFmpeg, exports that only copy still
//!   run through the remux.
//! - On Android, Media3's Transformer (the `raygun-audio` plugin, `android`) re-encodes to H.264
//!   and AAC in MP4 with the phone's encoders, copying what already fits; MKV and MP3 then go
//!   through the remux.
//!
//! Every pass writes into the cache's `convert/` folder; the result is checked (both streams there,
//! as long as the source, decodable, and on Windows opened by Media Foundation) before it's copied
//! where it goes.

pub mod android;
pub mod ffmpeg;
#[cfg(windows)]
pub mod media_foundation;
pub mod pipeline;

#[cfg(test)]
mod tests;

use super::remux::{self, Codec, Target};

/// A picture stream, by FFmpeg's names.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoInfo {
    /// `h264`, `hevc`, `vp9`, `av1`, `vp8`…
    pub codec: String,
    /// 8-bit 4:2:0 in a profile every decoder plays (H.264 High 10 or 4:4:4 isn't).
    pub plain: bool,
    /// HDR (PQ or HLG): tone-mapped to SDR when re-encoded.
    pub hdr: bool,
    pub width: u32,
    pub height: u32,
    pub fps: Option<f64>,
    /// Raygun's own remux carries it.
    pub remux: bool,
}

/// A sound stream, by FFmpeg's names.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioInfo {
    /// `aac`, `mp3`, `opus`, `ac3`, `vorbis`…
    pub codec: String,
    pub channels: u32,
    /// Raygun's own remux carries it.
    pub remux: bool,
}

/// What a downloaded video holds, as conversions see it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaInfo {
    pub video: Option<VideoInfo>,
    pub audio: Option<AudioInfo>,
    /// How long it plays.
    pub seconds: Option<f64>,
    /// Its size on disk.
    pub bytes: u64,
    /// Subtitle streams inside it.
    pub subtitles: u32,
}

/// The codecs Raygun's remux reads and writes.
const REMUX_CODECS: &[&str] = &[
    "h264", "hevc", "vp9", "av1", "aac", "mp3", "mp2", "mp1", "ac3", "eac3", "opus",
];

impl MediaInfo {
    /// What the remux found in a source.
    pub fn from_probe(probe: &remux::Probe, bytes: u64) -> Self {
        let video = probe
            .video
            .as_ref()
            .map(|codec| VideoInfo {
                codec: codec_name(codec).to_owned(),
                plain: plain_picture(codec),
                hdr: false,
                width: probe.width,
                height: probe.height,
                fps: probe.fps,
                remux: true,
            })
            .or_else(|| {
                probe
                    .skipped
                    .iter()
                    .find(|(video, _)| *video)
                    .map(|(_, codec)| VideoInfo {
                        codec: codec.clone(),
                        plain: false,
                        hdr: false,
                        width: 0,
                        height: 0,
                        fps: None,
                        remux: false,
                    })
            });
        let audio = probe
            .audio
            .as_ref()
            .map(|codec| AudioInfo {
                codec: codec_name(codec).to_owned(),
                channels: match codec {
                    Codec::Aac(config) => remux::codec::parse_audio_config(config)
                        .map(|(_, _, channels)| u32::from(channels))
                        .filter(|channels| *channels > 0)
                        .unwrap_or(u32::from(probe.channels)),
                    _ => u32::from(probe.channels),
                },
                remux: true,
            })
            .or_else(|| {
                probe
                    .skipped
                    .iter()
                    .find(|(video, _)| !*video)
                    .map(|(_, codec)| AudioInfo {
                        codec: codec.clone(),
                        channels: 0,
                        remux: false,
                    })
            });
        Self {
            video,
            audio,
            seconds: probe.seconds,
            bytes,
            subtitles: 0,
        }
    }

    /// A source nothing could read: everything is re-encoded by an engine that reads it.
    pub fn unknown(bytes: u64) -> Self {
        Self {
            video: Some(VideoInfo {
                codec: "unknown".into(),
                plain: false,
                hdr: false,
                width: 0,
                height: 0,
                fps: None,
                remux: false,
            }),
            audio: Some(AudioInfo {
                codec: "unknown".into(),
                channels: 0,
                remux: false,
            }),
            seconds: None,
            bytes,
            subtitles: 0,
        }
    }
}

/// The remux's codec by FFmpeg's name.
pub fn codec_name(codec: &Codec) -> &'static str {
    match codec {
        Codec::H264(_) => "h264",
        Codec::Hevc(_) => "hevc",
        Codec::Vp9 => "vp9",
        Codec::Av1(_) => "av1",
        Codec::Aac(_) => "aac",
        Codec::MpegAudio { layer: 3, .. } => "mp3",
        Codec::MpegAudio { layer: 2, .. } => "mp2",
        Codec::MpegAudio { .. } => "mp1",
        Codec::Ac3(_) => "ac3",
        Codec::Eac3 => "eac3",
        Codec::Opus(_) => "opus",
    }
}

/// An H.264 picture every decoder plays: Baseline, Main or High in 8-bit 4:2:0.
fn plain_picture(codec: &Codec) -> bool {
    match codec {
        Codec::H264(avcc) => {
            let profile_fits = matches!(avcc.avc_profile_indication, 66 | 77 | 88 | 100);
            let format_fits = avcc
                .ext
                .as_ref()
                .is_none_or(|ext| ext.chroma_format == 1 && ext.bit_depth_luma == 8);
            profile_fits && format_fits
        }
        _ => false,
    }
}

/// A codec as people know it.
pub fn shown_codec(codec: &str) -> String {
    match codec {
        "h264" => "H.264".into(),
        "hevc" => "HEVC".into(),
        "vp8" => "VP8".into(),
        "vp9" => "VP9".into(),
        "av1" => "AV1".into(),
        "mpeg2video" => "MPEG-2".into(),
        "mpeg4" => "MPEG-4".into(),
        "theora" => "Theora".into(),
        "aac" => "AAC".into(),
        "mp3" => "MP3".into(),
        "mp2" | "mp1" => "MPEG audio".into(),
        "ac3" => "AC-3".into(),
        "eac3" => "E-AC-3".into(),
        "opus" => "Opus".into(),
        "vorbis" => "Vorbis".into(),
        "flac" => "FLAC".into(),
        "dts" => "DTS".into(),
        "truehd" => "TrueHD".into(),
        "pcm" => "PCM".into(),
        "unknown" => String::new(),
        other if other.starts_with("pcm_") => "PCM".into(),
        other => other.to_uppercase(),
    }
}

/// What happens to a stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Copy,
    Encode,
}

/// What a format does with a source's picture and sound (`None`: left out or not there).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Decision {
    pub video: Option<Step>,
    pub audio: Option<Step>,
}

impl Decision {
    pub fn encodes(&self) -> bool {
        self.video == Some(Step::Encode) || self.audio == Some(Step::Encode)
    }
}

/// Channels AAC keeps (Windows' AAC decoder takes up to 5.1).
pub const AAC_CHANNELS: u32 = 6;

/// What `target` does with each of `info`'s streams: copied when the container holds it as a file
/// every common player plays, else re-encoded. `any_codec`: the engine copies any codec into
/// Matroska (FFmpeg); else only what the remux carries.
pub fn decide(info: &MediaInfo, target: Target, any_codec: bool) -> Decision {
    let carried = |remux: bool| any_codec || remux;
    let step = |copy: bool| if copy { Step::Copy } else { Step::Encode };
    let aac_fits = |audio: &AudioInfo| {
        audio.codec == "aac" && audio.channels <= AAC_CHANNELS && carried(audio.remux)
    };
    match target {
        Target::Mkv => Decision {
            video: info.video.as_ref().map(|video| step(carried(video.remux))),
            audio: info.audio.as_ref().map(|audio| step(carried(audio.remux))),
        },
        Target::Mp4 => Decision {
            video: info
                .video
                .as_ref()
                .map(|video| step(video.codec == "h264" && video.plain && carried(video.remux))),
            audio: info.audio.as_ref().map(|audio| step(aac_fits(audio))),
        },
        Target::M4a => Decision {
            video: None,
            audio: info.audio.as_ref().map(|audio| step(aac_fits(audio))),
        },
        Target::Mp3 { .. } => Decision {
            video: None,
            audio: info
                .audio
                .as_ref()
                .map(|audio| step(audio.codec == "mp3" && carried(audio.remux))),
        },
    }
}

/// The converters a device has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Engines {
    /// FFmpeg was found (desktop).
    pub ffmpeg: bool,
    /// Media3's Transformer (Android).
    pub transformer: bool,
}

impl Engines {
    /// This device's: Android has Transformer; elsewhere FFmpeg when it's there.
    pub fn here(ffmpeg: bool) -> Self {
        let android = cfg!(target_os = "android");
        Self {
            ffmpeg: ffmpeg && !android,
            transformer: android,
        }
    }
}

/// One pass of a conversion, reading the source or the last pass's file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    /// Raygun's remux: packets copied, MP3 made from AAC.
    Remux(Target),
    /// FFmpeg copies or re-encodes each stream as `decision` says.
    Ffmpeg { target: Target, decision: Decision },
    /// Media3's Transformer: H.264 and AAC in MP4 (the sound alone in M4A), what fits copied.
    Transformer { audio_only: bool },
}

impl Pass {
    /// The file this pass writes.
    pub fn target(&self) -> Target {
        match self {
            Self::Remux(target) | Self::Ffmpeg { target, .. } => *target,
            Self::Transformer { audio_only: true } => Target::M4a,
            Self::Transformer { audio_only: false } => Target::Mp4,
        }
    }

    /// It decodes and encodes (slow), rather than copying packets.
    pub fn encodes(&self) -> bool {
        match self {
            Self::Remux(Target::Mp3 { .. }) | Self::Transformer { .. } => true,
            Self::Remux(_) => false,
            Self::Ffmpeg { decision, .. } => decision.encodes(),
        }
    }
}

/// How a download becomes a file in a format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub passes: Vec<Pass>,
    /// What happens to the picture and the sound.
    pub decision: Decision,
    /// Re-encoding needs FFmpeg, which isn't here (desktop).
    pub needs_ffmpeg: bool,
}

impl Route {
    /// The picture is re-encoded.
    pub fn encodes_video(&self) -> bool {
        self.decision.video == Some(Step::Encode)
    }

    /// The sound is re-encoded.
    pub fn encodes_audio(&self) -> bool {
        self.decision.audio == Some(Step::Encode)
    }
}

/// The remux reads every stream `info` has (and it has one).
fn remux_reads_all(info: &MediaInfo) -> bool {
    info.video.iter().all(|video| video.remux)
        && info.audio.iter().all(|audio| audio.remux)
        && (info.video.is_some() || info.audio.is_some())
}

/// FFmpeg's passes: a saved HLS stream the remux reads goes into Matroska first (the remux joins
/// its segments, keys and separate sound), and that's the file when Matroska copies it all.
fn ffmpeg_passes(info: &MediaInfo, target: Target, hls: bool, decision: Decision) -> Vec<Pass> {
    if !(hls && remux_reads_all(info)) {
        return vec![Pass::Ffmpeg { target, decision }];
    }
    if target == Target::Mkv && !decision.encodes() {
        return vec![Pass::Remux(Target::Mkv)];
    }
    vec![Pass::Remux(Target::Mkv), Pass::Ffmpeg { target, decision }]
}

/// Transformer's passes: MKV and MP3 are made from its MP4 or M4A by the remux.
fn transformer_passes(target: Target) -> Vec<Pass> {
    let mut passes = vec![Pass::Transformer {
        audio_only: target.audio_only(),
    }];
    if matches!(target, Target::Mkv | Target::Mp3 { .. }) {
        passes.push(Pass::Remux(target));
    }
    passes
}

/// How `info` (from a saved HLS stream when `hls`) becomes `target` with `engines`.
pub fn route(info: &MediaInfo, target: Target, hls: bool, engines: Engines) -> Route {
    if engines.ffmpeg {
        let decision = decide(info, target, true);
        return Route {
            passes: ffmpeg_passes(info, target, hls, decision),
            decision,
            needs_ffmpeg: false,
        };
    }
    let decision = decide(info, target, false);
    // The remux's MP3 writer decodes AAC (stereo at most) itself.
    let mp3_by_remux = matches!(target, Target::Mp3 { .. })
        && info.audio.as_ref().is_some_and(|audio| {
            audio.remux && (audio.codec == "mp3" || (audio.codec == "aac" && audio.channels <= 2))
        });
    if (!decision.encodes() && remux_reads_all(info)) || mp3_by_remux {
        return Route {
            passes: vec![Pass::Remux(target)],
            decision,
            needs_ffmpeg: false,
        };
    }
    if engines.transformer {
        // Transformer writes H.264 and AAC: Matroska made from its file has those too.
        let decision = if target == Target::Mkv {
            decide(info, Target::Mp4, false)
        } else {
            decision
        };
        return Route {
            passes: transformer_passes(target),
            decision,
            needs_ffmpeg: false,
        };
    }
    Route {
        passes: Vec::new(),
        decision,
        needs_ffmpeg: true,
    }
}

/// Re-encoding everything, after `tried` made a file players couldn't open; `None` when there's
/// nothing else to try.
pub fn reencoded(
    tried: &Route,
    info: &MediaInfo,
    target: Target,
    hls: bool,
    engines: Engines,
) -> Option<Route> {
    let all = Decision {
        video: info
            .video
            .as_ref()
            .filter(|_| !target.audio_only())
            .map(|_| Step::Encode),
        audio: info.audio.as_ref().map(|_| Step::Encode),
    };
    if tried.decision == all
        && !tried
            .passes
            .iter()
            .all(|pass| matches!(pass, Pass::Remux(_)))
    {
        return None;
    }
    let passes = if engines.ffmpeg {
        ffmpeg_passes(info, target, hls, all)
    } else if engines.transformer {
        transformer_passes(target)
    } else {
        return None;
    };
    (passes != tried.passes).then_some(Route {
        passes,
        decision: all,
        needs_ffmpeg: false,
    })
}

/// How fast a device converts, to estimate how long an export takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Speeds {
    /// Picture pixels re-encoded per second (width × height × frames).
    pub pixels: f64,
    /// Seconds of sound re-encoded per second.
    pub sound: f64,
    /// Bytes copied per second.
    pub bytes: f64,
}

impl Speeds {
    /// A first guess before anything was measured here: a phone's hardware encoder, or libx264's
    /// `veryfast` on a desktop processor.
    pub fn guessed() -> Self {
        if cfg!(target_os = "android") {
            Self {
                pixels: 90e6,
                sound: 40.0,
                bytes: 80e6,
            }
        } else {
            Self {
                pixels: 400e6,
                sound: 200.0,
                bytes: 300e6,
            }
        }
    }
}

/// About how long `route` takes for `info`, in seconds.
pub fn estimate(route: &Route, info: &MediaInfo, speeds: &Speeds) -> f64 {
    let seconds = info.seconds.unwrap_or(0.0).max(0.0);
    let copying = info.bytes as f64 / speeds.bytes.max(1.0);
    let mut total = 0.0;
    for pass in &route.passes {
        let video = match pass {
            Pass::Ffmpeg { decision, .. } => decision.video == Some(Step::Encode),
            Pass::Transformer { audio_only } => !audio_only && route.encodes_video(),
            Pass::Remux(_) => false,
        };
        total += if video {
            let picture = info.video.as_ref();
            let width = picture.map_or(1280, |video| video.width.max(1));
            let height = picture.map_or(720, |video| video.height.max(1));
            let fps = picture.and_then(|video| video.fps).unwrap_or(30.0);
            seconds * f64::from(width) * f64::from(height) * fps / speeds.pixels.max(1.0)
        } else if pass.encodes() {
            seconds / speeds.sound.max(1.0)
        } else {
            copying
        };
    }
    total
}
