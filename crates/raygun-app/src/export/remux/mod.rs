//! Downloaded videos saved in another format: MP4, Matroska, or their sound alone as M4A or MP3.
//! Packets are copied as they are (nothing is re-encoded) except for an MP3 made from other
//! sound, which is decoded and encoded again.
//!
//! What's read: a saved HLS stream (MPEG-TS, fragmented MP4 or packed audio segments, AES-128
//! decrypted on the way; a separate audio rendition joins its video) or one downloaded file
//! (MPEG-TS, MP4/MOV, Matroska/WebM). Each file is written in two passes over the source: the
//! first measures every sample, so the file then goes out front to back with its index first (an
//! MP4's `moov` before `mdat`, every Matroska size known). Nothing seeks in the output, which is
//! what Android's document descriptors allow, and an MP4 plays before it has fully loaded.

pub mod codec;
pub mod isobmff;
pub mod matroska;
pub mod mp3;
pub mod mp4;
pub mod plan;
pub mod ts;

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use mp4_atom::{Avcc, Hvcc};

use super::video::Join;

/// A file a conversion writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Mp4,
    Mkv,
    /// The sound alone, in an MP4 container (AAC only).
    M4a,
    /// The sound alone as MP3, encoded at this rate unless it's MP3 already.
    Mp3 {
        kbps: u32,
    },
}

impl Target {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mkv => "mkv",
            Self::M4a => "m4a",
            Self::Mp3 { .. } => "mp3",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Self::Mp4 => "video/mp4",
            Self::Mkv => "video/x-matroska",
            Self::M4a => "audio/mp4",
            Self::Mp3 { .. } => "audio/mpeg",
        }
    }

    fn audio_only(self) -> bool {
        matches!(self, Self::M4a | Self::Mp3 { .. })
    }
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Cancelled,
    /// The target can't hold this video (a codec or container it doesn't take): nothing was
    /// written, so the video can still be saved as it is.
    Unsupported(String),
    /// The source is damaged or changed while it was read.
    Invalid(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Unsupported(why) | Self::Invalid(why) => write!(f, "{why}"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// A stream's codec and what decoders need to start it.
#[derive(Debug, Clone, PartialEq)]
pub enum Codec {
    H264(Avcc),
    Hevc(Hvcc),
    Vp9,
    /// With its `av1C` record.
    Av1(Vec<u8>),
    /// With its AudioSpecificConfig.
    Aac(Vec<u8>),
    /// MPEG-1/2 audio layers I to III.
    MpegAudio {
        layer: u8,
        mpeg1: bool,
    },
    Ac3(codec::Ac3),
    Eac3,
    /// With its `OpusHead`.
    Opus(Vec<u8>),
}

impl Codec {
    pub fn is_video(&self) -> bool {
        matches!(
            self,
            Self::H264(_) | Self::Hevc(_) | Self::Vp9 | Self::Av1(_)
        )
    }

    /// How people know it.
    pub fn name(&self) -> &'static str {
        match self {
            Self::H264(_) => "H.264",
            Self::Hevc(_) => "HEVC",
            Self::Vp9 => "VP9",
            Self::Av1(_) => "AV1",
            Self::Aac(_) => "AAC",
            Self::MpegAudio { layer: 3, .. } => "MP3",
            Self::MpegAudio { .. } => "MPEG audio",
            Self::Ac3(_) => "AC-3",
            Self::Eac3 => "E-AC-3",
            Self::Opus(_) => "Opus",
        }
    }
}

/// One stream of a video.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub codec: Codec,
    /// Units of its timestamps per second.
    pub timescale: u32,
    pub width: u32,
    pub height: u32,
    pub rate: u32,
    pub channels: u8,
    /// ISO 639-2, when the source says.
    pub language: Option<String>,
}

impl Track {
    pub fn video(codec: Codec, timescale: u32, width: u32, height: u32) -> Self {
        Self {
            codec,
            timescale,
            width,
            height,
            rate: 0,
            channels: 0,
            language: None,
        }
    }

    pub fn audio(codec: Codec, rate: u32, channels: u8) -> Self {
        Self {
            codec,
            timescale: rate.max(1),
            width: 0,
            height: 0,
            rate,
            channels,
            language: None,
        }
    }
}

/// One sample, in its track's timescale. H.264 and HEVC samples are length-prefixed.
#[derive(Debug, Clone, PartialEq)]
pub struct Packet {
    pub track: usize,
    /// Decoding time; Matroska stores none, and it's worked out from the order of `pts`.
    pub dts: Option<i64>,
    pub pts: i64,
    /// How long it plays, when the codec says (audio frames).
    pub duration: Option<u32>,
    pub key: bool,
    pub data: Vec<u8>,
}

/// Reads a source's samples in file order. Tracks may be found as reading goes (MPEG-TS learns
/// a stream's configuration from its first frames); a packet's track is always known.
pub trait Demux {
    fn next(&mut self) -> Result<Option<Packet>>;
    fn tracks(&self) -> &[Track];
}

/// One segment of a saved HLS stream, decrypted.
pub struct Segment {
    /// The init section (fragmented MP4), given with the first segment that uses it.
    pub init: Option<Vec<u8>>,
    pub data: Vec<u8>,
}

pub type Segments<'a> = Box<dyn Iterator<Item = std::io::Result<Segment>> + 'a>;

/// What a conversion reads.
pub enum Source {
    /// A saved HLS stream in `dir`, and its separate audio stream, if any.
    Hls {
        dir: PathBuf,
        video: Join,
        audio: Option<Join>,
    },
    /// One downloaded file.
    File(PathBuf),
}

impl Source {
    /// A saved download: `file` in `dir` (a playlist or a video file).
    pub fn of_download(dir: &Path, file: &str) -> Result<Self> {
        if !file.ends_with(".m3u8") {
            return Ok(Self::File(dir.join(file)));
        }
        let playlist = std::fs::read_to_string(dir.join(file))?;
        if playlist.contains("#EXT-X-STREAM-INF") {
            let (video, audio) = master_parts(&playlist).ok_or_else(|| {
                Error::Invalid("The saved stream's playlist is unreadable".into())
            })?;
            let read = |name: &str| -> Result<Join> {
                if name.contains(['/', '\\']) || name.contains("..") {
                    return Err(Error::Invalid(
                        "The saved playlist points outside its folder".into(),
                    ));
                }
                Join::parse(&std::fs::read_to_string(dir.join(name))?).map_err(Error::Invalid)
            };
            return Ok(Self::Hls {
                dir: dir.to_owned(),
                video: read(&video)?,
                audio: audio.as_deref().map(read).transpose()?,
            });
        }
        Ok(Self::Hls {
            dir: dir.to_owned(),
            video: Join::parse(&playlist).map_err(Error::Invalid)?,
            audio: None,
        })
    }

    /// A fresh reader from the start.
    pub fn open(&self) -> Result<Box<dyn Demux + '_>> {
        match self {
            Self::Hls { dir, video, audio } => {
                let main = open_segments(video.segments(dir))?;
                Ok(match audio {
                    Some(audio) => Box::new(Merge::new(main, open_segments(audio.segments(dir))?)),
                    None => main,
                })
            }
            Self::File(path) => open_file(path),
        }
    }
}

/// The video playlist (a `#EXT-X-STREAM-INF` line's URI) and the audio one (`#EXT-X-MEDIA`) of
/// a saved master playlist.
fn master_parts(playlist: &str) -> Option<(String, Option<String>)> {
    let mut lines = playlist.lines().map(str::trim);
    let mut video = None;
    let mut audio = None;
    while let Some(line) = lines.next() {
        if let Some(attributes) = line.strip_prefix("#EXT-X-MEDIA:")
            && super::video::attribute(attributes, "TYPE").as_deref() == Some("AUDIO")
            && audio.is_none()
        {
            audio = super::video::attribute(attributes, "URI");
        } else if line.starts_with("#EXT-X-STREAM-INF") && video.is_none() {
            video = lines
                .by_ref()
                .find(|next| !next.is_empty() && !next.starts_with('#'))
                .map(str::to_owned);
        }
    }
    Some((video?, audio))
}

/// A reader for HLS segments, by what the first one holds.
fn open_segments<'a>(mut segments: Segments<'a>) -> Result<Box<dyn Demux + 'a>> {
    let first = match segments.next() {
        Some(first) => first?,
        None => return Err(Error::Invalid("The saved stream has no segments".into())),
    };
    let sniffed = if first.init.is_some() {
        Kind::Mp4
    } else {
        sniff(&first.data)
    };
    let segments: Segments<'a> = Box::new(std::iter::once(Ok(first)).chain(segments));
    match sniffed {
        Kind::Ts => Ok(Box::new(ts::TsDemux::new(segments))),
        Kind::Mp4 => Ok(Box::new(isobmff::Mp4Demux::segments(segments)?)),
        Kind::PackedAudio => Ok(Box::new(ts::PackedAudio::new(segments))),
        Kind::Matroska | Kind::Unknown => Err(Error::Unsupported(
            "This stream's segments are in a format Raygun can't convert".into(),
        )),
    }
}

fn open_file(path: &Path) -> Result<Box<dyn Demux + 'static>> {
    let mut head = [0u8; 1024];
    let read = {
        let mut file = File::open(path)?;
        let mut filled = 0;
        while filled < head.len() {
            let count = file.read(&mut head[filled..])?;
            if count == 0 {
                break;
            }
            filled += count;
        }
        filled
    };
    match sniff(&head[..read]) {
        Kind::Ts => {
            let file = BufReader::new(File::open(path)?);
            Ok(Box::new(ts::TsDemux::continuous(chunked(file))))
        }
        Kind::Mp4 => Ok(Box::new(isobmff::Mp4Demux::file(path)?)),
        Kind::Matroska => Ok(Box::new(matroska::MkvDemux::open(path)?)),
        Kind::PackedAudio | Kind::Unknown => Err(Error::Unsupported(
            "This video's format can't be converted".into(),
        )),
    }
}

/// A file read in pieces, for the MPEG-TS reader (whole packets: 188 bytes each).
fn chunked(mut file: impl Read + 'static) -> Segments<'static> {
    const PIECE: usize = 188 * 4096;
    let mut done = false;
    Box::new(std::iter::from_fn(move || {
        if done {
            return None;
        }
        let mut data = vec![0u8; PIECE];
        let mut filled = 0;
        while filled < PIECE {
            match file.read(&mut data[filled..]) {
                Ok(0) => break,
                Ok(count) => filled += count,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Some(Err(error)),
            }
        }
        if filled == 0 {
            done = true;
            return None;
        }
        data.truncate(filled);
        // A piece that ends mid-packet continues in the next: the reader keeps the rest.
        Some(Ok(Segment { init: None, data }))
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Ts,
    Mp4,
    Matroska,
    PackedAudio,
    Unknown,
}

fn sniff(data: &[u8]) -> Kind {
    let ts_at = |offset: usize| data.get(offset) == Some(&0x47);
    if ts_at(0) && (data.len() <= 188 || ts_at(188)) {
        return Kind::Ts;
    }
    if data.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Kind::Matroska;
    }
    if data.len() >= 8
        && [
            b"ftyp", b"styp", b"moov", b"moof", b"free", b"skip", b"mdat", b"sidx", b"wide",
        ]
        .contains(&&[data[4], data[5], data[6], data[7]])
    {
        return Kind::Mp4;
    }
    if data.starts_with(b"ID3")
        || codec::Adts::parse(data).is_some()
        || codec::MpegAudio::parse(data).is_some()
        || codec::Ac3::parse(data).is_some()
    {
        return Kind::PackedAudio;
    }
    // Some segments start with stray bytes before their first packet.
    if let Some(start) = data.iter().position(|byte| *byte == 0x47)
        && data.get(start + 188) == Some(&0x47)
        && data.get(start + 376) == Some(&0x47)
    {
        return Kind::Ts;
    }
    Kind::Unknown
}

/// Audio timestamps follow the frames (each as long as its samples) and move only when the source
/// jumps: a gap the source leaves keeps its place; frames that would overlap carry on after the
/// last one.
#[derive(Debug, Default, Clone, Copy)]
pub struct AudioClock {
    next: Option<i64>,
}

impl AudioClock {
    /// The time of a frame `samples` long whose source says it starts at `source`, if it says.
    pub fn stamp(&mut self, source: Option<i64>, samples: u32) -> i64 {
        let length = i64::from(samples.max(1));
        let time = match (self.next, source) {
            (Some(next), Some(source)) if source > next + length => source,
            (Some(next), _) => next,
            (None, source) => source.unwrap_or(0),
        };
        self.next = Some(time + length);
        time
    }
}

/// Two readers as one, in time order (an HLS video and its separate audio).
pub struct Merge<'a> {
    parts: [Box<dyn Demux + 'a>; 2],
    heads: [Option<Packet>; 2],
    ended: [bool; 2],
    /// Each part's tracks by their place in `tracks`.
    maps: [Vec<Option<usize>>; 2],
    tracks: Vec<Track>,
}

impl<'a> Merge<'a> {
    pub fn new(first: Box<dyn Demux + 'a>, second: Box<dyn Demux + 'a>) -> Self {
        Self {
            parts: [first, second],
            heads: [None, None],
            ended: [false, false],
            maps: [Vec::new(), Vec::new()],
            tracks: Vec::new(),
        }
    }

    fn seconds(&self, part: usize, packet: &Packet) -> f64 {
        let timescale = self.parts[part]
            .tracks()
            .get(packet.track)
            .map_or(1, |track| track.timescale.max(1));
        packet.dts.unwrap_or(packet.pts) as f64 / f64::from(timescale)
    }
}

impl Demux for Merge<'_> {
    fn next(&mut self) -> Result<Option<Packet>> {
        for part in 0..2 {
            if self.heads[part].is_none() && !self.ended[part] {
                self.heads[part] = self.parts[part].next()?;
                self.ended[part] = self.heads[part].is_none();
            }
        }
        let part = match (&self.heads[0], &self.heads[1]) {
            (None, None) => return Ok(None),
            (Some(_), None) => 0,
            (None, Some(_)) => 1,
            (Some(first), Some(second)) => {
                usize::from(self.seconds(1, second) < self.seconds(0, first))
            }
        };
        let mut packet = self.heads[part].take().expect("a head was chosen");
        let map = &mut self.maps[part];
        if map.len() <= packet.track {
            map.resize(packet.track + 1, None);
        }
        let global = match map[packet.track] {
            Some(global) => global,
            None => {
                let track = self.parts[part]
                    .tracks()
                    .get(packet.track)
                    .cloned()
                    .ok_or_else(|| Error::Invalid("A packet has no stream".into()))?;
                self.tracks.push(track);
                map[packet.track] = Some(self.tracks.len() - 1);
                self.tracks.len() - 1
            }
        };
        packet.track = global;
        Ok(Some(packet))
    }

    fn tracks(&self) -> &[Track] {
        &self.tracks
    }
}

/// How often long loops look at the cancel flag.
const CANCEL_EVERY: usize = 256;

pub(crate) fn check_cancel(cancel: &AtomicBool, count: usize) -> Result<()> {
    if count % CANCEL_EVERY == 1 && cancel.load(Ordering::SeqCst) {
        return Err(Error::Cancelled);
    }
    Ok(())
}

/// The codecs a source has (its first video and audio streams), read from its start.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Probe {
    pub video: Option<Codec>,
    pub audio: Option<Codec>,
}

/// Packets read at most while probing.
const PROBE_PACKETS: usize = 2000;

/// What a source holds, from its first packets.
pub fn probe(source: &Source) -> Result<Probe> {
    let mut demux = source.open()?;
    let mut found = Probe::default();
    for _ in 0..PROBE_PACKETS {
        let Some(packet) = demux.next()? else {
            break;
        };
        let Some(track) = demux.tracks().get(packet.track) else {
            continue;
        };
        let slot = if track.codec.is_video() {
            &mut found.video
        } else {
            &mut found.audio
        };
        if slot.is_none() {
            *slot = Some(track.codec.clone());
        }
        if found.video.is_some() && found.audio.is_some() {
            break;
        }
    }
    Ok(found)
}

/// Why `target` can't hold what `probe` found, if so.
pub fn refusal(probe: &Probe, target: Target) -> Option<String> {
    let audio = probe.audio.as_ref();
    let video = probe.video.as_ref();
    match target {
        Target::M4a => match audio {
            None => Some("This video has no sound to save".into()),
            Some(Codec::Aac(_)) => None,
            Some(codec) => Some(format!(
                "Its sound is {}, which M4A can't hold: choose MP3",
                codec.name()
            )),
        },
        Target::Mp3 { .. } => match audio {
            None => Some("This video has no sound to save".into()),
            Some(Codec::Aac(config))
                if codec::parse_audio_config(config)
                    .is_some_and(|(_, _, channels)| channels <= 2) =>
            {
                None
            }
            Some(Codec::MpegAudio { layer: 3, .. }) => None,
            Some(codec) => Some(format!(
                "Its sound is {}, which Raygun can't turn into MP3",
                codec.name()
            )),
        },
        Target::Mp4 => {
            if video.is_none() && audio.is_none() {
                return Some("No video or sound was found in it".into());
            }
            if let Some(codec) = video.filter(|codec| !mp4::holds(codec)) {
                return Some(format!(
                    "Its picture is {}, which MP4 can't hold here: choose MKV",
                    codec.name()
                ));
            }
            audio.filter(|codec| !mp4::holds(codec)).map(|codec| {
                format!(
                    "Its sound is {}, which MP4 can't hold here: choose MKV",
                    codec.name()
                )
            })
        }
        Target::Mkv => {
            (video.is_none() && audio.is_none()).then(|| "No video or sound was found in it".into())
        }
    }
}

/// Converts `source` into `target`, written to `out` front to back.
pub fn convert(
    source: &Source,
    target: Target,
    out: &mut dyn std::io::Write,
    cancel: &AtomicBool,
) -> Result<()> {
    if let Target::Mp3 { kbps } = target {
        return mp3::write(source, kbps, out, cancel);
    }
    let plan = plan::Plan::scan(source, target.audio_only(), cancel)?;
    if plan.tracks.is_empty() {
        return Err(Error::Unsupported(
            "No video or sound was found in it".into(),
        ));
    }
    match target {
        Target::Mkv => matroska::write(source, &plan, out, cancel),
        _ => {
            for track in &plan.tracks {
                if !mp4::holds(&track.track.codec)
                    || (target == Target::M4a && !matches!(track.track.codec, Codec::Aac(_)))
                {
                    return Err(Error::Unsupported(format!(
                        "{} can't go in this file",
                        track.track.codec.name()
                    )));
                }
            }
            mp4::write(source, &plan, target == Target::M4a, out, cancel)
        }
    }
}

#[cfg(test)]
mod tests;
