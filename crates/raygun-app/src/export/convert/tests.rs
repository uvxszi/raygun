use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use super::ffmpeg::{self, Abilities, Expect, Input, Job, SubtitleFile, VideoEncoder};
use super::pipeline::{self, iso639_2};
use super::*;

fn video(codec: &str, plain: bool) -> VideoInfo {
    VideoInfo {
        codec: codec.into(),
        plain,
        hdr: false,
        width: 1920,
        height: 1080,
        fps: Some(60.0),
        remux: REMUX_CODECS.contains(&codec),
    }
}

fn audio(codec: &str, channels: u32) -> AudioInfo {
    AudioInfo {
        codec: codec.into(),
        channels,
        remux: REMUX_CODECS.contains(&codec),
    }
}

fn media(video: Option<VideoInfo>, audio: Option<AudioInfo>) -> MediaInfo {
    MediaInfo {
        video,
        audio,
        seconds: Some(634.6),
        bytes: 300_000_000,
        subtitles: 0,
    }
}

/// Big Buck Bunny from Wikimedia Commons: VP9 at 1080p60 and 5.1 Opus in WebM.
fn bunny() -> MediaInfo {
    media(Some(video("vp9", true)), Some(audio("opus", 6)))
}

fn plain_h264() -> MediaInfo {
    media(Some(video("h264", true)), Some(audio("aac", 2)))
}

const DESKTOP: Engines = Engines {
    ffmpeg: true,
    transformer: false,
};
const BARE_DESKTOP: Engines = Engines {
    ffmpeg: false,
    transformer: false,
};
const PHONE: Engines = Engines {
    ffmpeg: false,
    transformer: true,
};
const COPY_BOTH: Decision = Decision {
    video: Some(Step::Copy),
    audio: Some(Step::Copy),
};
const ENCODE_BOTH: Decision = Decision {
    video: Some(Step::Encode),
    audio: Some(Step::Encode),
};

#[test]
fn formats_copy_what_they_hold_and_re_encode_the_rest() {
    // MP4 holds H.264 (8-bit 4:2:0) and AAC as every player plays them.
    assert_eq!(decide(&plain_h264(), Target::Mp4, true), COPY_BOTH);
    assert_eq!(decide(&bunny(), Target::Mp4, true), ENCODE_BOTH);
    let high10 = media(Some(video("h264", false)), Some(audio("aac", 2)));
    assert_eq!(
        decide(&high10, Target::Mp4, true).video,
        Some(Step::Encode),
        "High 10 doesn't play in most players"
    );
    let hevc = media(Some(video("hevc", true)), Some(audio("ac3", 6)));
    assert_eq!(decide(&hevc, Target::Mp4, true), ENCODE_BOTH);
    let seven_one = media(Some(video("h264", true)), Some(audio("aac", 8)));
    assert_eq!(
        decide(&seven_one, Target::Mp4, true).audio,
        Some(Step::Encode)
    );
    // Matroska holds anything FFmpeg copies; the remux only what it reads.
    assert_eq!(decide(&bunny(), Target::Mkv, true), COPY_BOTH);
    assert_eq!(decide(&bunny(), Target::Mkv, false), COPY_BOTH);
    let webm_vp8 = media(Some(video("vp8", true)), Some(audio("vorbis", 2)));
    assert_eq!(decide(&webm_vp8, Target::Mkv, true), COPY_BOTH);
    assert_eq!(decide(&webm_vp8, Target::Mkv, false), ENCODE_BOTH);
    // The sound alone.
    let sound = |info: &MediaInfo, target| decide(info, target, true);
    assert_eq!(
        sound(&plain_h264(), Target::M4a),
        Decision {
            video: None,
            audio: Some(Step::Copy)
        }
    );
    assert_eq!(sound(&bunny(), Target::M4a).audio, Some(Step::Encode));
    let mp3 = media(None, Some(audio("mp3", 2)));
    assert_eq!(
        sound(&mp3, Target::Mp3 { kbps: 192 }).audio,
        Some(Step::Copy)
    );
    assert_eq!(
        sound(&plain_h264(), Target::Mp3 { kbps: 192 }).audio,
        Some(Step::Encode)
    );
    assert_eq!(sound(&mp3, Target::Mp4).video, None, "no picture to keep");
}

#[test]
fn routes_pick_ffmpeg_the_remux_or_transformer() {
    // A computer with FFmpeg: one FFmpeg pass for a file; a saved HLS stream is joined first.
    let mp4 = route(&bunny(), Target::Mp4, false, DESKTOP);
    assert_eq!(
        mp4.passes,
        vec![Pass::Ffmpeg {
            target: Target::Mp4,
            decision: ENCODE_BOTH
        }]
    );
    assert!(mp4.encodes_video() && mp4.encodes_audio() && !mp4.needs_ffmpeg);
    assert_eq!(
        route(&plain_h264(), Target::Mp4, true, DESKTOP).passes,
        vec![
            Pass::Remux(Target::Mkv),
            Pass::Ffmpeg {
                target: Target::Mp4,
                decision: COPY_BOTH
            }
        ]
    );
    assert_eq!(
        route(&plain_h264(), Target::Mkv, true, DESKTOP).passes,
        vec![Pass::Remux(Target::Mkv)],
        "Matroska from a saved stream is the remux's own file"
    );
    // Without FFmpeg, copies still work; re-encoding asks for FFmpeg.
    assert_eq!(
        route(&plain_h264(), Target::Mp4, true, BARE_DESKTOP).passes,
        vec![Pass::Remux(Target::Mp4)]
    );
    let bare = route(&bunny(), Target::Mp4, false, BARE_DESKTOP);
    assert!(bare.needs_ffmpeg && bare.passes.is_empty());
    assert_eq!(
        route(
            &plain_h264(),
            Target::Mp3 { kbps: 320 },
            false,
            BARE_DESKTOP
        )
        .passes,
        vec![Pass::Remux(Target::Mp3 { kbps: 320 })],
        "the remux makes MP3 from stereo AAC itself"
    );
    // A phone: Transformer re-encodes, the remux copies and makes MKV and MP3 from its file.
    assert_eq!(
        route(&bunny(), Target::Mp4, false, PHONE).passes,
        vec![Pass::Transformer { audio_only: false }]
    );
    assert_eq!(
        route(&bunny(), Target::Mkv, false, PHONE).passes,
        vec![Pass::Remux(Target::Mkv)]
    );
    let webm_vp8 = media(Some(video("vp8", true)), Some(audio("vorbis", 2)));
    assert_eq!(
        route(&webm_vp8, Target::Mkv, false, PHONE).passes,
        vec![
            Pass::Transformer { audio_only: false },
            Pass::Remux(Target::Mkv)
        ]
    );
    // Transformer's file has H.264 and AAC: Opus beside VP8 is re-encoded too.
    let vp8_opus = media(Some(video("vp8", true)), Some(audio("opus", 2)));
    assert_eq!(
        route(&vp8_opus, Target::Mkv, false, PHONE).decision,
        ENCODE_BOTH
    );
    assert_eq!(
        route(&bunny(), Target::Mp3 { kbps: 192 }, false, PHONE).passes,
        vec![
            Pass::Transformer { audio_only: true },
            Pass::Remux(Target::Mp3 { kbps: 192 })
        ]
    );
    assert_eq!(
        route(&plain_h264(), Target::M4a, false, PHONE).passes,
        vec![Pass::Remux(Target::M4a)]
    );
    // A file nothing could read is re-encoded by the engine that reads it.
    let unknown = route(&MediaInfo::unknown(10), Target::Mp4, false, PHONE);
    assert_eq!(
        unknown.passes,
        vec![Pass::Transformer { audio_only: false }]
    );
}

#[test]
fn a_copy_that_does_not_check_out_is_re_encoded_once() {
    let copied = route(&plain_h264(), Target::Mp4, false, DESKTOP);
    let again = reencoded(&copied, &plain_h264(), Target::Mp4, false, DESKTOP).unwrap();
    assert_eq!(
        again.passes,
        vec![Pass::Ffmpeg {
            target: Target::Mp4,
            decision: ENCODE_BOTH
        }]
    );
    assert_eq!(
        reencoded(&again, &plain_h264(), Target::Mp4, false, DESKTOP),
        None,
        "nothing else to try"
    );
    let joined = route(&plain_h264(), Target::Mp4, true, DESKTOP);
    assert_eq!(
        reencoded(&joined, &plain_h264(), Target::Mp4, true, DESKTOP)
            .unwrap()
            .passes,
        vec![
            Pass::Remux(Target::Mkv),
            Pass::Ffmpeg {
                target: Target::Mp4,
                decision: ENCODE_BOTH
            }
        ]
    );
    let phone = route(&plain_h264(), Target::Mp4, false, PHONE);
    assert_eq!(
        reencoded(&phone, &plain_h264(), Target::Mp4, false, PHONE)
            .unwrap()
            .passes,
        vec![Pass::Transformer { audio_only: false }]
    );
    let transformed = route(&bunny(), Target::Mp4, false, PHONE);
    assert_eq!(
        reencoded(&transformed, &bunny(), Target::Mp4, false, PHONE),
        None
    );
    assert_eq!(
        reencoded(&copied, &plain_h264(), Target::Mp4, false, BARE_DESKTOP),
        None
    );
}

#[test]
fn estimates_follow_size_length_and_speed() {
    let speeds = Speeds {
        pixels: 500e6,
        sound: 200.0,
        bytes: 250e6,
    };
    let bunny = bunny();
    let encode = route(&bunny, Target::Mp4, false, DESKTOP);
    // 634.6 s of 1920×1080 at 60 fps at 500 Mpx/s.
    let seconds = estimate(&encode, &bunny, &speeds);
    assert!((150.0..170.0).contains(&seconds), "{seconds}");
    let copy = route(&bunny, Target::Mkv, false, DESKTOP);
    assert!((estimate(&copy, &bunny, &speeds) - 1.2).abs() < 0.01);
    let sound = route(&bunny, Target::M4a, false, DESKTOP);
    assert!((estimate(&sound, &bunny, &speeds) - 634.6 / 200.0).abs() < 0.01);
    assert_eq!(shown_codec("vp9"), "VP9");
    assert_eq!(shown_codec("eac3"), "E-AC-3");
    assert_eq!(shown_codec("pcm_s16le"), "PCM");
}

fn job_args(
    info: &MediaInfo,
    target: Target,
    decision: Decision,
    subtitles: &[SubtitleFile],
    input: Input,
) -> Vec<String> {
    ffmpeg::args(&Job {
        input,
        info,
        target,
        decision,
        encoder: VideoEncoder::X264,
        abilities: &Abilities {
            tone_mapping: true,
            ..Abilities::default()
        },
        subtitles,
        title: Some("Big Buck Bunny"),
        output: Path::new("out.mp4"),
    })
}

/// Whether `args` has `run` as consecutive arguments.
fn has(args: &[String], run: &[&str]) -> bool {
    args.windows(run.len())
        .any(|window| window.iter().zip(run).all(|(arg, want)| arg == want))
}

fn file(path: &str) -> Input {
    Input::File {
        path: PathBuf::from(path),
        matroska: path.ends_with(".webm") || path.ends_with(".mkv"),
    }
}

#[test]
fn ffmpeg_re_encodes_to_h264_and_aac_that_every_player_takes() {
    let subtitles = [SubtitleFile {
        path: PathBuf::from("sub1.vtt"),
        format: "vtt".into(),
        language: Some("en".into()),
    }];
    let args = job_args(
        &bunny(),
        Target::Mp4,
        ENCODE_BOTH,
        &subtitles,
        file("video.webm"),
    );
    assert!(has(&args, &["-progress", "pipe:1"]));
    assert!(has(&args, &["-i", "video.webm", "-i", "sub1.vtt"]));
    assert!(has(
        &args,
        &["-map", "0:V:0", "-map", "0:a:0", "-map", "1:s:0"]
    ));
    assert!(has(
        &args,
        &[
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "20",
            "-profile:v",
            "high"
        ]
    ));
    assert!(has(&args, &["-vf", "format=yuv420p"]), "{args:?}");
    assert!(
        has(&args, &["-c:a", "aac", "-b:a", "384k"]),
        "5.1 keeps its channels"
    );
    assert!(has(&args, &["-c:s", "mov_text"]));
    assert!(has(&args, &["-metadata:s:s:0", "language=en"]));
    assert!(has(&args, &["-metadata", "title=Big Buck Bunny"]));
    assert!(has(
        &args,
        &["-movflags", "+faststart", "-f", "mp4", "out.mp4"]
    ));
    assert_eq!(args.last().map(String::as_str), Some("out.mp4"));

    // HDR is tone-mapped, odd sizes made even.
    let mut hdr = bunny();
    if let Some(video) = hdr.video.as_mut() {
        video.hdr = true;
        video.width = 1921;
    }
    let args = job_args(&hdr, Target::Mp4, ENCODE_BOTH, &[], file("video.mkv"));
    let filters = &args[args.iter().position(|arg| arg == "-vf").unwrap() + 1];
    assert!(filters.starts_with("zscale=t=linear"), "{filters}");
    assert!(filters.contains("tonemap=tonemap=hable"));
    assert!(filters.contains("scale=trunc(iw/2)*2:trunc(ih/2)*2"));
    assert!(filters.ends_with("format=yuv420p"));
    assert!(has(&args, &["-color_trc", "bt709"]));
    assert!(has(&args, &["-sn"]), "no subtitle files, none kept");

    // 7.1 sound is folded into 5.1.
    let wide = media(Some(video("h264", true)), Some(audio("eac3", 8)));
    let args = job_args(
        &wide,
        Target::Mp4,
        decide(&wide, Target::Mp4, true),
        &[],
        file("a.mp4"),
    );
    assert!(has(&args, &["-c:v", "copy"]));
    assert!(has(&args, &["-c:a", "aac", "-b:a", "384k", "-ac", "6"]));
}

#[test]
fn ffmpeg_copies_into_matroska_with_its_subtitles() {
    let subtitles = [
        SubtitleFile {
            path: PathBuf::from("en.vtt"),
            format: "vtt".into(),
            language: Some("en".into()),
        },
        SubtitleFile {
            path: PathBuf::from("ja.ass"),
            format: "ass".into(),
            language: None,
        },
    ];
    let mut info = bunny();
    info.subtitles = 1;
    let args = job_args(
        &info,
        Target::Mkv,
        COPY_BOTH,
        &subtitles,
        file("video.webm"),
    );
    assert!(has(
        &args,
        &["-map", "0:V:0", "-map", "0:a", "-map", "0:s?"]
    ));
    assert!(has(&args, &["-map", "1:s:0", "-map", "2:s:0"]));
    assert!(has(&args, &["-c:v", "copy"]));
    assert!(has(&args, &["-c:a", "copy"]));
    // The file's own subtitle comes first; WebVTT goes in as SubRip, ASS as it is.
    assert!(has(&args, &["-c:s", "copy", "-c:s:1", "srt"]));
    assert!(!args.iter().any(|arg| arg == "-c:s:2"));
    assert!(has(&args, &["-metadata:s:s:1", "language=en"]));
    assert!(has(&args, &["-f", "matroska"]));
    assert!(!args.iter().any(|arg| arg == "+faststart"));

    // A saved playlist read by FFmpeg's own HLS reader takes any segment name.
    let picky = Abilities {
        picky_hls: true,
        ..Abilities::default()
    };
    let args = ffmpeg::args(&Job {
        input: Input::Playlist(PathBuf::from("index.m3u8")),
        info: &plain_h264(),
        target: Target::Mkv,
        decision: COPY_BOTH,
        encoder: VideoEncoder::X264,
        abilities: &picky,
        subtitles: &[],
        title: None,
        output: Path::new("out.mkv"),
    });
    assert!(has(
        &args,
        &[
            "-allowed_extensions",
            "ALL",
            "-extension_picky",
            "0",
            "-i",
            "index.m3u8"
        ]
    ));
}

#[test]
fn ffmpeg_saves_the_sound_alone() {
    let subtitles = [SubtitleFile {
        path: PathBuf::from("en.srt"),
        format: "srt".into(),
        language: None,
    }];
    let args = job_args(
        &plain_h264(),
        Target::M4a,
        decide(&plain_h264(), Target::M4a, true),
        &subtitles,
        file("a.mp4"),
    );
    assert!(has(&args, &["-map", "0:a:0"]));
    assert!(!args.iter().any(|arg| arg == "0:V:0" || arg == "en.srt"));
    assert!(has(&args, &["-vn", "-c:a", "copy", "-sn"]));
    assert!(has(&args, &["-movflags", "+faststart", "-f", "ipod"]));
    let args = job_args(
        &bunny(),
        Target::Mp3 { kbps: 320 },
        decide(&bunny(), Target::Mp3 { kbps: 320 }, true),
        &[],
        file("video.webm"),
    );
    assert!(has(
        &args,
        &["-c:a", "libmp3lame", "-b:a", "320k", "-ac", "2"]
    ));
    assert!(has(&args, &["-f", "mp3"]));
}

const BUNNY_PROBE: &str = r#"{
  "streams": [
    {"index": 0, "codec_name": "vp9", "codec_type": "video", "profile": "Profile 0",
     "width": 1920, "height": 1080, "pix_fmt": "yuv420p", "color_transfer": "bt709",
     "avg_frame_rate": "60/1", "r_frame_rate": "60/1", "disposition": {"attached_pic": 0}},
    {"index": 1, "codec_name": "opus", "codec_type": "audio", "channels": 6,
     "sample_rate": "48000"},
    {"index": 2, "codec_name": "webvtt", "codec_type": "subtitle"},
    {"index": 3, "codec_name": "mjpeg", "codec_type": "video", "width": 600, "height": 600,
     "disposition": {"attached_pic": 1}}
  ],
  "format": {"duration": "634.571000", "size": "300466178"}
}"#;

#[test]
fn ffprobe_answers_are_read() {
    let info = ffmpeg::parse_probe(BUNNY_PROBE).unwrap();
    let video = info.video.as_ref().unwrap();
    assert_eq!(
        (video.codec.as_str(), video.width, video.height, video.fps),
        ("vp9", 1920, 1080, Some(60.0))
    );
    assert!(video.plain && !video.hdr && video.remux);
    assert_eq!(info.audio, Some(audio("opus", 6)));
    assert_eq!(info.subtitles, 1);
    assert_eq!(info.seconds, Some(634.571));
    assert_eq!(info.bytes, 300_466_178);
    let hi10 = r#"{"streams": [{"codec_name": "h264", "codec_type": "video", "profile": "High 10",
        "pix_fmt": "yuv420p10le", "color_transfer": "smpte2084", "avg_frame_rate": "24000/1001"}],
        "format": {}}"#;
    let info = ffmpeg::parse_probe(hi10).unwrap();
    let video = info.video.unwrap();
    assert!(!video.plain && video.hdr);
    assert!((video.fps.unwrap() - 23.976).abs() < 0.001);
    assert_eq!(info.seconds, None);
    assert!(ffmpeg::parse_probe("not json").is_none());
}

#[test]
fn ffmpeg_lists_and_progress_lines_are_read() {
    let encoders = ffmpeg::parse_encoders(
        "Encoders:\n V..... = Video\n A..... = Audio\n ------\n V....D libx264              libx264 H.264\n A....D aac                  AAC\n V....D h264_mf              H264 via MediaFoundation\n",
    );
    assert!(encoders.contains("libx264") && encoders.contains("aac"));
    assert!(encoders.contains("h264_mf"));
    assert!(!encoders.contains("="));
    assert_eq!(ffmpeg::progress_seconds("out_time_us=1500000"), Some(1.5));
    assert_eq!(ffmpeg::progress_seconds("out_time_us=N/A"), None);
    assert_eq!(ffmpeg::progress_seconds("progress=end"), None);
}

#[test]
fn converted_files_must_be_as_long_as_their_source() {
    let expect = Expect {
        video: true,
        audio: true,
        seconds: Some(634.571),
        approximate: false,
    };
    assert!(expect.length_fits(634.6));
    assert!(expect.length_fits(635.4));
    assert!(!expect.length_fits(632.0));
    let found = media(Some(video("h264", true)), None);
    assert_eq!(
        ffmpeg::shortfall(&found, &expect).as_deref(),
        Some("The converted file has no sound")
    );
    let mut short = plain_h264();
    short.seconds = Some(300.0);
    assert!(ffmpeg::shortfall(&short, &expect).is_some_and(|why| why.contains("300.0 s")));
    // A playlist's sum of rounded segment lengths gets some slack.
    let playlist = Expect {
        approximate: true,
        seconds: Some(1440.0),
        ..expect
    };
    assert!(playlist.length_fits(1460.0));
    assert!(!playlist.length_fits(1300.0));
}

#[test]
fn subtitle_languages_and_tags_suit_matroska() {
    assert_eq!(iso639_2("en"), "eng");
    assert_eq!(iso639_2("FR"), "fre");
    assert_eq!(iso639_2("tlh"), "tlh");
}

#[test]
fn the_remux_puts_subtitle_files_in_matroska() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/export/remux/fixtures");
    let source = remux::Source::of_download(&fixtures.join("ts"), "index.m3u8").unwrap();
    let texts = [remux::matroska::TextTrack {
        language: Some("eng".into()),
        cues: vec![
            (0, 900, "<i>First</i> line".into()),
            (1200, 1900, "Second".into()),
        ],
    }];
    let mut out = Vec::new();
    remux::convert_with(
        &source,
        Target::Mkv,
        &mut out,
        &AtomicBool::new(false),
        &texts,
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.mkv");
    std::fs::write(&path, &out).unwrap();
    let mut file =
        matroska_demuxer::MatroskaFile::open(std::fs::File::open(&path).unwrap()).unwrap();
    let subtitle = file
        .tracks()
        .iter()
        .find(|track| track.codec_id() == "S_TEXT/UTF8")
        .map(|track| {
            (
                track.track_number().get(),
                track.language().map(str::to_owned),
            )
        })
        .expect("a text track");
    assert_eq!(subtitle.1.as_deref(), Some("eng"));
    let mut frame = matroska_demuxer::Frame::default();
    let mut cues = Vec::new();
    while file.next_frame(&mut frame).unwrap() {
        if frame.track == subtitle.0 {
            cues.push((
                frame.timestamp,
                String::from_utf8_lossy(&frame.data).into_owned(),
            ));
        }
    }
    assert_eq!(
        cues,
        vec![
            (0, "<i>First</i> line".to_owned()),
            (1200, "Second".to_owned())
        ]
    );
    // The picture and sound are all still there.
    let back = remux::probe(&remux::Source::File(path)).unwrap();
    assert!(back.video.is_some() && back.audio.is_some());
}

// ---- Live: FFmpeg fetched as the app fetches it ----

/// FFmpeg for the live checks: `FFMPEG_DIR`, else Raygun's Windows build fetched into
/// `FFMPEG_TOOLS` (default `N:\Raygun\cache\transcode-tools`) as the app fetches it.
async fn live_tools() -> ffmpeg::Tools {
    if let Ok(dir) = std::env::var("FFMPEG_DIR") {
        return ffmpeg::Tools::in_dir(Path::new(&dir)).expect("FFMPEG_DIR has ffmpeg and ffprobe");
    }
    let dir = PathBuf::from(
        std::env::var("FFMPEG_TOOLS").unwrap_or_else(|_| r"N:\Raygun\cache\transcode-tools".into()),
    );
    if let Some(tools) = ffmpeg::Tools::in_dir(&dir) {
        return tools;
    }
    let client = raygun_net::http_client().unwrap();
    let started = std::time::Instant::now();
    let go = tokio_util::sync::CancellationToken::new();
    crate::ytdlp::fetch_ffmpeg_into(&client, &dir, &go, |_| {})
        .await
        .expect("FFmpeg fetched");
    println!("FFmpeg fetched in {:.1} s", started.elapsed().as_secs_f64());
    ffmpeg::Tools::in_dir(&dir).expect("FFmpeg unpacked")
}

async fn live_convert(
    tools: Option<ffmpeg::Tools>,
    source: pipeline::Source,
    target: Target,
    subtitles: Vec<SubtitleFile>,
    work: &Path,
) -> pipeline::Converted {
    let request = pipeline::Request {
        source,
        target,
        subtitles,
        title: "Live check".into(),
        work: work.to_owned(),
    };
    let started = std::time::Instant::now();
    let converted = pipeline::convert_with(
        None,
        tools,
        &request,
        Arc::new(AtomicBool::new(false)),
        Arc::new(|_| {}),
    )
    .await
    .expect("converted and checked");
    println!(
        "{} → {}: {:?} in {:.1} s ({} bytes)",
        request.source.file,
        target.extension(),
        converted.route.passes,
        started.elapsed().as_secs_f64(),
        std::fs::metadata(&converted.path).map_or(0, |meta| meta.len()),
    );
    converted
}

/// What Media Foundation opens (Windows only), printed.
fn windows_opens(path: &Path, video: bool) {
    #[cfg(windows)]
    {
        let opened = media_foundation::open(path).expect("Media Foundation opens it");
        println!("Media Foundation: {opened:?}");
        assert_eq!(opened.video, video);
        assert!(opened.audio);
    }
    #[cfg(not(windows))]
    let _ = (path, video);
}

/// Opens `MF_FILE` with Media Foundation, as Windows' players would.
#[cfg(windows)]
#[test]
#[ignore = "reads a file named by MF_FILE"]
fn media_foundation_opens_live() {
    let path = PathBuf::from(std::env::var("MF_FILE").expect("MF_FILE"));
    println!("{:?}", media_foundation::open(&path));
}

/// `cargo test -p raygun-app --lib -- --ignored big_buck_bunny_live --nocapture` (`BBB_WEBM`: the
/// WebM, default `N:\Raygun\cache\transcode-fixtures\bbb.webm`).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "fetches FFmpeg and converts a 300 MB video"]
async fn big_buck_bunny_live() {
    let input = PathBuf::from(
        std::env::var("BBB_WEBM")
            .unwrap_or_else(|_| r"N:\Raygun\cache\transcode-fixtures\bbb.webm".into()),
    );
    let tools = live_tools().await;
    let info = ffmpeg::probe(&tools, &input).await.unwrap();
    println!("source: {info:?}");
    let source = pipeline::Source {
        dir: input.parent().unwrap().to_owned(),
        file: input.file_name().unwrap().to_string_lossy().into_owned(),
        info,
    };
    let work = tempfile::tempdir_in(input.parent().unwrap()).unwrap();
    let subtitles: Vec<SubtitleFile> = std::env::var("BBB_VTT")
        .ok()
        .map(|path| SubtitleFile {
            path: PathBuf::from(path),
            format: "vtt".into(),
            language: Some("en".into()),
        })
        .into_iter()
        .collect();
    let mp4 = live_convert(
        Some(tools.clone()),
        source.clone(),
        Target::Mp4,
        subtitles.clone(),
        &work.path().join("mp4"),
    )
    .await;
    assert!(mp4.route.encodes_video() && mp4.route.encodes_audio());
    let found = ffmpeg::probe(&tools, &mp4.path).await.unwrap();
    println!("MP4: {found:?}");
    assert_eq!(
        found.video.as_ref().map(|video| video.codec.as_str()),
        Some("h264")
    );
    assert_eq!(
        found.audio.as_ref().map(|audio| audio.codec.as_str()),
        Some("aac")
    );
    windows_opens(&mp4.path, true);
    let mkv = live_convert(
        Some(tools.clone()),
        source,
        Target::Mkv,
        subtitles,
        &work.path().join("mkv"),
    )
    .await;
    assert!(!mkv.route.encodes_video() && !mkv.route.encodes_audio());
    let found = ffmpeg::probe(&tools, &mkv.path).await.unwrap();
    println!("MKV: {found:?}");
    assert_eq!(
        found.video.as_ref().map(|video| video.codec.as_str()),
        Some("vp9")
    );
}

/// A short H.264 HLS stream (the remux's fixture) to MP4: through FFmpeg, and through the remux
/// alone (a computer without FFmpeg), both opened by Media Foundation.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "fetches FFmpeg"]
async fn hls_to_mp4_live() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/export/remux/fixtures");
    let tools = live_tools().await;
    let work = tempfile::tempdir().unwrap();
    for stream in ["ts", "pair", "fmp4"] {
        let dir = fixtures.join(stream);
        let probe = remux::probe(&remux::Source::of_download(&dir, "index.m3u8").unwrap()).unwrap();
        let source = pipeline::Source {
            dir: dir.clone(),
            file: "index.m3u8".into(),
            info: MediaInfo::from_probe(&probe, 1),
        };
        for (tools, name) in [(Some(tools.clone()), "ffmpeg"), (None, "remux")] {
            let converted = live_convert(
                tools,
                source.clone(),
                Target::Mp4,
                Vec::new(),
                &work.path().join(format!("{stream}-{name}")),
            )
            .await;
            assert!(!converted.route.encodes_video());
            windows_opens(&converted.path, source.info.video.is_some());
        }
    }
}
