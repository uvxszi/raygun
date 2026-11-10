//! Conversions of real segments and files (made by FFmpeg from its test patterns), read back
//! with `mp4-atom`, `matroska-demuxer`, symphonia and Raygun's own readers.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockEncryptMut, KeyIvInit};
use mp4_atom::{Atom, Codec as Entry, Moov};

use super::*;
use crate::music_downloads::transcode;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/export/remux/fixtures")
}

fn convert_to(source: &Source, target: Target) -> Vec<u8> {
    let mut out = Vec::new();
    convert(source, target, &mut out, &AtomicBool::new(false)).expect("converts");
    out
}

fn hls(dir: &str) -> Source {
    let dir = fixtures().join(dir);
    Source::of_download(&dir, "index.m3u8").unwrap()
}

/// Every packet of a source: track kind (video?), data.
fn packets(source: &Source) -> Vec<(bool, Vec<u8>)> {
    let mut demux = source.open().unwrap();
    let mut found = Vec::new();
    while let Some(packet) = demux.next().unwrap() {
        let video = demux.tracks()[packet.track].codec.is_video();
        found.push((video, packet.data));
    }
    found
}

fn of_kind(packets: &[(bool, Vec<u8>)], video: bool) -> Vec<Vec<u8>> {
    packets
        .iter()
        .filter(|(kind, _)| *kind == video)
        .map(|(_, data)| data.clone())
        .collect()
}

/// Top-level boxes: kind and length.
fn boxes(file: &[u8]) -> Vec<(String, u64)> {
    let mut at = 0usize;
    let mut found = Vec::new();
    while at + 8 <= file.len() {
        let mut size = u64::from(u32::from_be_bytes(file[at..at + 4].try_into().unwrap()));
        if size == 1 {
            size = u64::from_be_bytes(file[at + 8..at + 16].try_into().unwrap());
        }
        found.push((
            String::from_utf8_lossy(&file[at + 4..at + 8]).into_owned(),
            size,
        ));
        at += size as usize;
    }
    found
}

fn moov_of(file: &[u8]) -> Moov {
    let size = u32::from_be_bytes(file[..4].try_into().unwrap()) as usize;
    let moov = &file[size..];
    let moov_size = u32::from_be_bytes(moov[..4].try_into().unwrap()) as usize;
    Moov::decode_body(&mut &moov[8..moov_size]).unwrap()
}

fn written(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

/// How long the sound plays, decoding all of it with symphonia.
fn sound_seconds(bytes: Vec<u8>, extension: &str) -> f64 {
    let mut decoding =
        transcode::open_source(Box::new(Cursor::new(bytes)), Some(extension)).unwrap();
    let mut samples = Vec::new();
    let mut frames = 0usize;
    while decoding.next(&mut samples).unwrap().is_some() {
        frames += samples.len() / decoding.channels.max(1);
    }
    frames as f64 / f64::from(decoding.rate)
}

#[test]
fn hls_mpeg_ts_becomes_an_mp4_with_its_index_first() {
    let source = hls("ts");
    let mp4 = convert_to(&source, Target::Mp4);
    let kinds: Vec<String> = boxes(&mp4).into_iter().map(|(kind, _)| kind).collect();
    assert_eq!(kinds, vec!["ftyp", "moov", "mdat"]);
    assert_eq!(
        boxes(&mp4).iter().map(|(_, size)| size).sum::<u64>(),
        mp4.len() as u64
    );

    let moov = moov_of(&mp4);
    assert_eq!(moov.trak.len(), 2);
    assert!(moov.mvex.is_none(), "a plain MP4, not fragments");
    let video = &moov.trak[0];
    let Some(Entry::Avc1(avc1)) = video.mdia.minf.stbl.stsd.codecs.first() else {
        panic!("the first track is H.264");
    };
    assert_eq!((avc1.visual.width, avc1.visual.height), (128, 72));
    // 2 s at 25 fps; B-frames start shown two frames in.
    let frames = video.mdia.minf.stbl.stsz.samples.clone();
    let count = match frames {
        mp4_atom::StszSamples::Different { sizes } => sizes.len(),
        mp4_atom::StszSamples::Identical { count, .. } => count as usize,
    };
    assert_eq!(count, 50);
    assert!(video.mdia.minf.stbl.ctts.is_some());
    assert!(
        video
            .mdia
            .minf
            .stbl
            .stss
            .as_ref()
            .is_some_and(|stss| stss.entries[0] == 1)
    );
    let elst = video
        .edts
        .as_ref()
        .and_then(|edts| edts.elst.as_ref())
        .unwrap();
    assert!(elst.entries.last().unwrap().media_time.unwrap() > 0);
    let seconds = moov.mvhd.duration as f64 / f64::from(moov.mvhd.timescale);
    assert!((seconds - 2.0).abs() < 0.1, "{seconds} s");

    // The samples read back are the ones the segments carry.
    let dir = tempfile::tempdir().unwrap();
    let again = Source::File(written(&dir, "out.mp4", &mp4));
    let before = packets(&source);
    let after = packets(&again);
    assert_eq!(of_kind(&after, true), of_kind(&before, true));
    assert_eq!(of_kind(&after, false), of_kind(&before, false));
    // And symphonia plays its sound: two seconds of it.
    let sound = sound_seconds(mp4, "mp4");
    assert!((sound - 2.0).abs() < 0.1, "{sound} s");
}

#[test]
fn encrypted_segments_are_decrypted_on_the_way() {
    let dir = tempfile::tempdir().unwrap();
    let key = [7u8; 16];
    std::fs::write(dir.path().join("key.bin"), key).unwrap();
    for name in ["seg0.m2t", "seg1.m2t"] {
        let mut data = std::fs::read(fixtures().join("ts").join(name)).unwrap();
        let length = data.len();
        data.resize(length + 16, 0);
        let mut iv = [0u8; 16];
        iv[15] = u8::from(name == "seg1.m2t");
        let encrypted = cbc::Encryptor::<aes::Aes128>::new(&key.into(), &iv.into())
            .encrypt_padded_mut::<Pkcs7>(&mut data, length)
            .unwrap()
            .to_vec();
        std::fs::write(dir.path().join(name), encrypted).unwrap();
    }
    std::fs::write(
        dir.path().join("index.m3u8"),
        "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\"\n\
         seg0.m2t\nseg1.m2t\n#EXT-X-ENDLIST\n",
    )
    .unwrap();
    let encrypted = Source::of_download(dir.path(), "index.m3u8").unwrap();
    assert_eq!(
        convert_to(&encrypted, Target::Mkv),
        convert_to(&hls("ts"), Target::Mkv)
    );
}

#[test]
fn fragmented_mp4_segments_become_one_plain_mp4_and_a_matroska_file() {
    let source = hls("fmp4");
    let before = packets(&source);
    assert_eq!(of_kind(&before, true).len(), 50);

    let mp4 = convert_to(&source, Target::Mp4);
    let moov = moov_of(&mp4);
    assert!(moov.mvex.is_none());
    let dir = tempfile::tempdir().unwrap();
    let after = packets(&Source::File(written(&dir, "out.mp4", &mp4)));
    assert_eq!(of_kind(&after, true), of_kind(&before, true));
    assert_eq!(of_kind(&after, false), of_kind(&before, false));

    let mkv = convert_to(&source, Target::Mkv);
    let path = written(&dir, "out.mkv", &mkv);
    let file = matroska_demuxer::MatroskaFile::open(std::fs::File::open(&path).unwrap()).unwrap();
    let codecs: Vec<&str> = file.tracks().iter().map(|track| track.codec_id()).collect();
    assert_eq!(codecs, vec!["V_MPEG4/ISO/AVC", "A_AAC"]);
    let duration = file.info().duration().unwrap();
    assert!((duration - 2000.0).abs() < 100.0, "{duration} ms");
    let back = packets(&Source::File(path));
    assert_eq!(of_kind(&back, true), of_kind(&before, true));
    assert_eq!(of_kind(&back, false), of_kind(&before, false));
    // Cues let players seek: one per cluster that starts with a key frame.
    assert!(
        mkv.windows(4)
            .any(|window| window == [0x1C, 0x53, 0xBB, 0x6B])
    );
}

#[test]
fn hevc_gets_its_hvcc_record() {
    let source = Source::File(fixtures().join("hevc.m2t"));
    let mp4 = convert_to(&source, Target::Mp4);
    let moov = moov_of(&mp4);
    let Some(Entry::Hvc1(hvc1)) = moov.trak[0].mdia.minf.stbl.stsd.codecs.first() else {
        panic!("the first track is HEVC");
    };
    assert_eq!((hvc1.visual.width, hvc1.visual.height), (128, 72));
    let types: Vec<u8> = hvc1
        .hvcc
        .arrays
        .iter()
        .map(|array| array.nal_unit_type)
        .collect();
    assert_eq!(types, vec![32, 33, 34]);
    assert_eq!(hvc1.hvcc.general_profile_idc, 1);
    let dir = tempfile::tempdir().unwrap();
    let back = packets(&Source::File(written(&dir, "out.mp4", &mp4)));
    assert_eq!(of_kind(&back, true), of_kind(&packets(&source), true));
}

#[test]
fn mp3_sound_is_copied_and_aac_is_encoded() {
    let mp3 = Source::File(fixtures().join("mp3.m2t"));
    let copied = convert_to(&mp3, Target::Mp3 { kbps: 128 });
    assert_eq!(copied, of_kind(&packets(&mp3), false).concat());
    let mp4 = moov_of(&convert_to(&mp3, Target::Mp4));
    let Some(Entry::Mp4a(mp4a)) = mp4.trak[1].mdia.minf.stbl.stsd.codecs.first() else {
        panic!("MP3 goes in an mp4a entry");
    };
    assert_eq!(mp4a.esds.es_desc.dec_config.object_type_indication, 0x6B);

    let encoded = convert_to(&hls("ts"), Target::Mp3 { kbps: 96 });
    let seconds = sound_seconds(encoded, "mp3");
    assert!((seconds - 2.0).abs() < 0.15, "{seconds} s");
}

#[test]
fn m4a_holds_the_sound_alone() {
    let m4a = convert_to(&hls("ts"), Target::M4a);
    let moov = moov_of(&m4a);
    assert_eq!(moov.trak.len(), 1);
    assert_eq!(&m4a[8..12], b"M4A ");
    let seconds = sound_seconds(m4a, "m4a");
    assert!((seconds - 2.0).abs() < 0.1, "{seconds} s");
    // M4A takes AAC only.
    let mp3 = Source::File(fixtures().join("mp3.m2t"));
    let probed = probe(&mp3).unwrap();
    assert!(refusal(&probed, Target::M4a).is_some());
    assert!(refusal(&probed, Target::Mp3 { kbps: 128 }).is_none());
}

#[test]
fn single_files_convert_between_mp4_and_matroska() {
    let dir = tempfile::tempdir().unwrap();
    let mp4 = Source::File(fixtures().join("plain.mp4"));
    let mkv = convert_to(&mp4, Target::Mkv);
    let back = packets(&Source::File(written(&dir, "from-mp4.mkv", &mkv)));
    assert_eq!(of_kind(&back, true), of_kind(&packets(&mp4), true));

    let matroska = Source::File(fixtures().join("plain.mkv"));
    let converted = convert_to(&matroska, Target::Mp4);
    let moov = moov_of(&converted);
    assert_eq!(moov.trak.len(), 2);
    let path = written(&dir, "from-mkv.mp4", &converted);
    assert_eq!(
        of_kind(&packets(&Source::File(path)), true),
        of_kind(&packets(&matroska), true)
    );
    let seconds = sound_seconds(converted, "mp4");
    assert!((seconds - 2.0).abs() < 0.1, "{seconds} s");
}

#[test]
fn a_separate_audio_stream_joins_its_video() {
    let source = hls("pair");
    let probed = probe(&source).unwrap();
    assert!(matches!(probed.video, Some(Codec::H264(_))));
    assert!(matches!(probed.audio, Some(Codec::Aac(_))));
    let mp4 = convert_to(&source, Target::Mp4);
    let moov = moov_of(&mp4);
    assert_eq!(moov.trak.len(), 2);
    let seconds = sound_seconds(mp4, "mp4");
    assert!((seconds - 2.0).abs() < 0.15, "{seconds} s");
}

#[test]
fn packed_audio_takes_its_time_from_id3() {
    // An HLS audio rendition of raw ADTS frames, each segment after an ID3 tag with its time.
    let adts = std::fs::read(fixtures().join("pair/tone.adts")).unwrap();
    let mut frames = Vec::new();
    let mut at = 0;
    while let Some(frame) = codec::Adts::parse(&adts[at..]) {
        frames.push(adts[at..at + frame.length].to_vec());
        at += frame.length;
        if at >= adts.len() {
            break;
        }
    }
    let half = frames.len() / 2;
    let tag = |time: u64| {
        let owner = b"com.apple.streaming.transportStreamTimestamp\0";
        let mut frame = b"PRIV".to_vec();
        frame.extend_from_slice(&[0, 0, 0, (owner.len() + 8) as u8, 0, 0]);
        frame.extend_from_slice(owner);
        frame.extend_from_slice(&time.to_be_bytes());
        let mut tag = b"ID3\x04\x00\x00".to_vec();
        tag.extend_from_slice(&[0, 0, 0, frame.len() as u8]);
        tag.extend_from_slice(&frame);
        tag
    };
    let dir = tempfile::tempdir().unwrap();
    let first = [tag(900_000), frames[..half].concat()].concat();
    let second_time = 900_000 + (half as u64 * 1024 * 90_000) / 44_100;
    let second = [tag(second_time), frames[half..].concat()].concat();
    std::fs::write(dir.path().join("a0.aac"), first).unwrap();
    std::fs::write(dir.path().join("a1.aac"), second).unwrap();
    std::fs::write(
        dir.path().join("index.m3u8"),
        "#EXTM3U\na0.aac\na1.aac\n#EXT-X-ENDLIST\n",
    )
    .unwrap();
    let source = Source::of_download(dir.path(), "index.m3u8").unwrap();
    let mut demux = source.open().unwrap();
    let mut times = Vec::new();
    while let Some(packet) = demux.next().unwrap() {
        times.push(packet.pts);
    }
    assert_eq!(times.len(), frames.len());
    // 10 s at 44.1 kHz, then a frame every 1024 samples.
    assert_eq!(times[0], 441_000);
    assert_eq!(times[1], 441_000 + 1024);
    let m4a = convert_to(&source, Target::M4a);
    let seconds = sound_seconds(m4a, "m4a");
    assert!((seconds - 1.0).abs() < 0.1, "{seconds} s");
}

#[test]
fn targets_say_what_they_cannot_hold() {
    let vp9 = Probe {
        video: Some(Codec::Vp9),
        audio: Some(Codec::Opus(
            b"OpusHead\x01\x02\x38\x01\x80\xBB\0\0\0\0\0".to_vec(),
        )),
    };
    assert!(refusal(&vp9, Target::Mp4).is_some());
    assert!(refusal(&vp9, Target::Mkv).is_none());
    assert!(refusal(&vp9, Target::M4a).is_some());
    assert!(refusal(&vp9, Target::Mp3 { kbps: 128 }).is_some());
    assert!(refusal(&Probe::default(), Target::Mkv).is_some());
}

#[test]
fn cancelling_stops_a_conversion() {
    let mut out = Vec::new();
    let result = convert(&hls("ts"), Target::Mp4, &mut out, &AtomicBool::new(true));
    assert!(matches!(result, Err(Error::Cancelled)));
}

/// Writes every target for an HLS folder (`REMUX_HLS_DIR`, its `index.m3u8`) or a file
/// (`REMUX_FILE`) into `REMUX_OUT`, to check with other players or ffprobe.
#[test]
#[ignore = "writes files for inspection"]
fn remux_for_inspection() {
    let source = match (std::env::var("REMUX_HLS_DIR"), std::env::var("REMUX_FILE")) {
        (Ok(dir), _) => Source::of_download(Path::new(&dir), "index.m3u8").unwrap(),
        (_, Ok(file)) => Source::File(PathBuf::from(file)),
        _ => Source::of_download(&fixtures().join("ts"), "index.m3u8").unwrap(),
    };
    let out = PathBuf::from(std::env::var("REMUX_OUT").expect("REMUX_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    println!("{:?}", probe(&source).unwrap());
    for target in [
        Target::Mp4,
        Target::Mkv,
        Target::M4a,
        Target::Mp3 { kbps: 128 },
    ] {
        let mut bytes = Vec::new();
        match convert(&source, target, &mut bytes, &AtomicBool::new(false)) {
            Ok(()) => {
                std::fs::write(out.join(format!("out.{}", target.extension())), bytes).unwrap();
            }
            Err(error) => println!("{}: {error}", target.extension()),
        }
    }
}
