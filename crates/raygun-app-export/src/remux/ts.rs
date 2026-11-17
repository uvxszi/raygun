//! MPEG-TS, and the packed audio of HLS audio renditions (raw ADTS, MPEG audio or AC-3 frames
//! after an ID3 tag giving the first frame's time). The PAT and PMT name the streams; PES packets
//! carry H.264 or HEVC access units and audio frames. Each HLS segment is read on its own, so a
//! broken packet at one segment's end doesn't shift the next; timestamps unwrap their 33 bits,
//! and a jump (a discontinuity: an ad, a restarted encoder) is closed up for every stream alike.

use std::collections::{HashMap, VecDeque};

use super::codec::{self, Ac3, Adts, MpegAudio};
use super::{AudioClock, Codec, Demux, Error, Packet, Result, Segments, Track};

const PACKET: usize = 188;
const CLOCK: i64 = 90_000;
const WRAP: i64 = 1 << 33;
/// Bigger jumps than these, back or forward, are discontinuities.
const JUMP_BACK: i64 = CLOCK / 2;
const JUMP_FORWARD: i64 = 10 * CLOCK;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Es {
    H264,
    Hevc,
    Adts,
    Mpeg,
    Ac3,
}

impl Es {
    fn video(self) -> bool {
        matches!(self, Self::H264 | Self::Hevc)
    }
}

/// The 90 kHz clock of every stream of one source, unwrapped and with its jumps closed.
#[derive(Debug, Default)]
struct Timeline {
    last: Option<i64>,
    offset: i64,
    /// Each stream's last time and step, to see jumps.
    streams: HashMap<usize, (i64, i64)>,
}

impl Timeline {
    fn unwrap(&mut self, raw: i64) -> i64 {
        let raw = raw.rem_euclid(WRAP);
        let value = match self.last {
            Some(last) => {
                let base = last - last.rem_euclid(WRAP);
                [base - WRAP, base, base + WRAP]
                    .into_iter()
                    .map(|base| base + raw)
                    .min_by_key(|candidate| (candidate - last).abs())
                    .unwrap_or(raw)
            }
            None => raw,
        };
        self.last = Some(value);
        value
    }

    /// `(pts, dts)` of a stream's PES on the source's one timeline.
    fn stamp(&mut self, stream: usize, pts: i64, dts: Option<i64>) -> (i64, i64) {
        let pts = self.unwrap(pts);
        let dts = dts.map_or(pts, |dts| {
            let dts = self.unwrap(dts);
            // A DTS is never after its PTS; a wrap between them would say so.
            if dts > pts { dts - WRAP } else { dts }
        });
        let mut mapped = dts + self.offset;
        if let Some(&(last, step)) = self.streams.get(&stream)
            && (mapped < last - JUMP_BACK || mapped > last + JUMP_FORWARD)
        {
            let expected = last + step.max(1);
            self.offset += expected - mapped;
            mapped = expected;
        }
        let step = self
            .streams
            .get(&stream)
            .map_or(CLOCK / 25, |&(last, step)| {
                let gap = mapped - last;
                if gap > 0 && gap < CLOCK { gap } else { step }
            });
        self.streams.insert(stream, (mapped, step));
        (mapped + (pts - dts), mapped)
    }
}

/// H.264 or HEVC access units into samples.
#[derive(Debug, Default)]
struct VideoEs {
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
    vps: Option<Vec<u8>>,
    track: Option<usize>,
}

impl VideoEs {
    fn push(
        &mut self,
        hevc: bool,
        data: &[u8],
        times: Option<(i64, i64)>,
        tracks: &mut Vec<Track>,
        out: &mut VecDeque<Packet>,
    ) {
        let units = codec::nal_units(data);
        for unit in &units {
            let slot = if hevc {
                match codec::hevc_type(unit) {
                    codec::HEVC_VPS => &mut self.vps,
                    codec::HEVC_SPS => &mut self.sps,
                    codec::HEVC_PPS => &mut self.pps,
                    _ => continue,
                }
            } else {
                match codec::avc_type(unit) {
                    codec::AVC_SPS => &mut self.sps,
                    codec::AVC_PPS => &mut self.pps,
                    _ => continue,
                }
            };
            if slot.is_none() {
                *slot = Some(unit.to_vec());
            }
        }
        let key = if hevc {
            codec::hevc_random_access(&units)
        } else {
            codec::avc_random_access(&units)
        };
        let track = match self.track {
            Some(track) => track,
            // Pictures before the first one a decoder can start from can't be shown.
            None if !key => return,
            None => {
                let made = if hevc {
                    match (&self.vps, &self.sps, &self.pps) {
                        (Some(vps), Some(sps), Some(pps)) => codec::hevc_config(vps, sps, pps)
                            .map(|(hvcc, info)| (Codec::Hevc(hvcc), info.width, info.height)),
                        _ => None,
                    }
                } else {
                    match (&self.sps, &self.pps) {
                        (Some(sps), Some(pps)) => codec::avc_config(sps, pps)
                            .map(|(avcc, info)| (Codec::H264(avcc), info.width, info.height)),
                        _ => None,
                    }
                };
                let Some((codec, width, height)) = made else {
                    return;
                };
                tracks.push(Track::video(codec, CLOCK as u32, width, height));
                self.track = Some(tracks.len() - 1);
                tracks.len() - 1
            }
        };
        let Some((pts, dts)) = times else {
            return;
        };
        let kept = units.into_iter().filter(|unit| {
            if hevc {
                !matches!(codec::hevc_type(unit), codec::HEVC_AUD | codec::HEVC_FILLER)
            } else {
                !matches!(codec::avc_type(unit), codec::AVC_AUD | codec::AVC_FILLER)
            }
        });
        out.push_back(Packet {
            track,
            dts: Some(dts),
            pts,
            duration: None,
            key,
            data: codec::length_prefixed(kept),
        });
    }
}

/// One audio frame found in a byte stream.
struct Frame {
    /// The frame's bytes to keep (an ADTS header is left out).
    start: usize,
    length: usize,
    samples: u32,
    rate: u32,
    channels: u8,
    codec: Codec,
}

/// Audio frames into samples; a frame cut by the PES's end continues in the next.
#[derive(Debug, Default)]
struct AudioEs {
    rest: Vec<u8>,
    clock: AudioClock,
    track: Option<usize>,
}

/// The longest header any audio frame needs read.
const AUDIO_HEADER: usize = 9;

fn audio_frame(es: Es, data: &[u8]) -> Option<Frame> {
    match es {
        Es::Adts => {
            let adts = Adts::parse(data)?;
            Some(Frame {
                start: adts.header,
                length: adts.length,
                samples: 1024 * u32::from(adts.blocks),
                rate: adts.rate(),
                channels: adts.channels,
                codec: Codec::Aac(adts.config()),
            })
        }
        Es::Mpeg => {
            let frame = MpegAudio::parse(data)?;
            Some(Frame {
                start: 0,
                length: frame.length,
                samples: frame.samples,
                rate: frame.rate,
                channels: frame.channels,
                codec: Codec::MpegAudio {
                    layer: frame.layer,
                    mpeg1: frame.mpeg1,
                },
            })
        }
        Es::Ac3 => {
            let frame = Ac3::parse(data)?;
            Some(Frame {
                start: 0,
                length: frame.length,
                samples: frame.samples,
                rate: frame.rate,
                channels: frame.channels,
                codec: if frame.enhanced {
                    Codec::Eac3
                } else {
                    Codec::Ac3(frame)
                },
            })
        }
        Es::H264 | Es::Hevc => None,
    }
}

impl AudioEs {
    /// `pts` is the 90 kHz time of the first frame starting in `data`.
    fn push(
        &mut self,
        es: Es,
        data: &[u8],
        pts: Option<i64>,
        tracks: &mut Vec<Track>,
        out: &mut VecDeque<Packet>,
    ) {
        let mut buffer = std::mem::take(&mut self.rest);
        let new_from = buffer.len();
        buffer.extend_from_slice(data);
        let mut at = 0;
        let mut stamped = false;
        loop {
            let remaining = buffer.len() - at;
            if remaining < 4 {
                break;
            }
            let Some(frame) = audio_frame(es, &buffer[at..]) else {
                // A header cut short waits for the rest; anything else isn't a frame.
                if remaining < AUDIO_HEADER {
                    break;
                }
                at += 1;
                continue;
            };
            if frame.length == 0 || frame.length > remaining {
                break;
            }
            // More than one AAC block per ADTS frame can't be told apart without decoding.
            let whole = !matches!(es, Es::Adts) || frame.samples == 1024;
            let track = match self.track {
                Some(track) => Some(track),
                None if whole => {
                    tracks.push(Track::audio(
                        frame.codec.clone(),
                        frame.rate,
                        frame.channels,
                    ));
                    self.track = Some(tracks.len() - 1);
                    self.track
                }
                None => None,
            };
            let timescale = track
                .and_then(|track| tracks.get(track))
                .map_or(frame.rate, |track| track.timescale);
            // The PES's time belongs to the first frame that starts in it.
            let source = if !stamped && at >= new_from {
                stamped = true;
                pts.map(|pts| pts * i64::from(timescale) / CLOCK)
            } else {
                None
            };
            let time = self.clock.stamp(source, frame.samples);
            if let (Some(track), true) = (track, whole) {
                out.push_back(Packet {
                    track,
                    dts: Some(time),
                    pts: time,
                    duration: Some(frame.samples),
                    key: true,
                    data: buffer[at + frame.start..at + frame.length].to_vec(),
                });
            }
            at += frame.length;
        }
        self.rest = buffer.split_off(at.min(buffer.len()));
    }
}

#[derive(Debug)]
struct Stream {
    pid: u16,
    es: Es,
    pes: Vec<u8>,
    gathering: bool,
    /// The PES's payload length, when its header says.
    expected: Option<usize>,
    pts: Option<i64>,
    dts: Option<i64>,
    video: VideoEs,
    audio: AudioEs,
}

/// A PES header: its length, PTS, DTS and the payload's length when given.
type PesHeader = (usize, Option<i64>, Option<i64>, Option<usize>);

fn pes_header(payload: &[u8]) -> Option<PesHeader> {
    if payload.len() < 9 || payload[..3] != [0, 0, 1] {
        return None;
    }
    let stream_id = payload[3];
    let length = usize::from(u16::from_be_bytes([payload[4], payload[5]]));
    if matches!(
        stream_id,
        0xBC | 0xBE | 0xBF | 0xF0 | 0xF1 | 0xF2 | 0xF8 | 0xFF
    ) {
        return None;
    }
    let flags = payload[7];
    let header = 9 + usize::from(payload[8]);
    let time = |at: usize| -> Option<i64> {
        let bytes = payload.get(at..at + 5)?;
        Some(
            (i64::from(bytes[0] >> 1 & 7) << 30)
                | (i64::from(bytes[1]) << 22)
                | (i64::from(bytes[2] >> 1) << 15)
                | (i64::from(bytes[3]) << 7)
                | i64::from(bytes[4] >> 1),
        )
    };
    let pts = (flags & 0x80 != 0).then(|| time(9)).flatten();
    let dts = (flags & 0x40 != 0).then(|| time(14)).flatten();
    let size = (length > 0).then(|| (length + 6).saturating_sub(header));
    Some((header.min(payload.len()), pts, dts, size))
}

/// MPEG-TS from segments (or a file in pieces, `continuous`).
pub struct TsDemux<'a> {
    segments: Segments<'a>,
    continuous: bool,
    carry: Vec<u8>,
    pmt: Option<u16>,
    sections: HashMap<u16, Vec<u8>>,
    streams: Vec<Stream>,
    /// The PMT named only protected (SAMPLE-AES) streams.
    protected: bool,
    tracks: Vec<Track>,
    ready: VecDeque<Packet>,
    time: Timeline,
    emitted: usize,
    ended: bool,
}

impl<'a> TsDemux<'a> {
    /// HLS segments, each read on its own.
    pub fn new(segments: Segments<'a>) -> Self {
        Self::with(segments, false)
    }

    /// One file read in pieces that continue each other.
    pub fn continuous(segments: Segments<'a>) -> Self {
        Self::with(segments, true)
    }

    fn with(segments: Segments<'a>, continuous: bool) -> Self {
        Self {
            segments,
            continuous,
            carry: Vec::new(),
            pmt: None,
            sections: HashMap::new(),
            streams: Vec::new(),
            protected: false,
            tracks: Vec::new(),
            ready: VecDeque::new(),
            time: Timeline::default(),
            emitted: 0,
            ended: false,
        }
    }

    fn read(&mut self, data: Vec<u8>) {
        let data = if self.continuous && !self.carry.is_empty() {
            let mut joined = std::mem::take(&mut self.carry);
            joined.extend_from_slice(&data);
            joined
        } else {
            data
        };
        let mut at = sync(&data, 0);
        while at + PACKET <= data.len() {
            if data[at] != 0x47 {
                at = sync(&data, at + 1);
                continue;
            }
            self.packet(&data[at..at + PACKET]);
            at += PACKET;
        }
        if self.continuous {
            self.carry = data[at.min(data.len())..].to_vec();
        }
    }

    fn packet(&mut self, packet: &[u8]) {
        if packet[1] & 0x80 != 0 {
            return;
        }
        let start = packet[1] & 0x40 != 0;
        let pid = u16::from(packet[1] & 0x1F) << 8 | u16::from(packet[2]);
        let control = packet[3] >> 4 & 3;
        let mut at = 4;
        if control & 2 != 0 {
            at += 1 + usize::from(packet[4]);
        }
        if control & 1 == 0 || at >= PACKET {
            return;
        }
        let payload = &packet[at..];
        if pid == 0 {
            if let Some(section) = self.section(pid, start, payload) {
                self.pat(&section);
            }
        } else if Some(pid) == self.pmt {
            if let Some(section) = self.section(pid, start, payload) {
                self.pmt(&section);
            }
        } else if let Some(index) = self.streams.iter().position(|stream| stream.pid == pid) {
            self.pes(index, start, payload);
        }
    }

    fn section(&mut self, pid: u16, start: bool, payload: &[u8]) -> Option<Vec<u8>> {
        let buffer = self.sections.entry(pid).or_default();
        if start {
            let pointer = usize::from(*payload.first()?);
            buffer.clear();
            buffer.extend_from_slice(payload.get(1 + pointer..)?);
        } else if !buffer.is_empty() {
            buffer.extend_from_slice(payload);
        } else {
            return None;
        }
        if buffer.len() < 3 {
            return None;
        }
        let length = 3 + (usize::from(buffer[1] & 0x0F) << 8 | usize::from(buffer[2]));
        if buffer.len() < length {
            return None;
        }
        let section = buffer[..length].to_vec();
        buffer.clear();
        Some(section)
    }

    fn pat(&mut self, section: &[u8]) {
        if self.pmt.is_some() || section.first() != Some(&0) || section.len() < 12 {
            return;
        }
        let end = section.len() - 4;
        let mut at = 8;
        while at + 4 <= end {
            let program = u16::from_be_bytes([section[at], section[at + 1]]);
            let pid = u16::from(section[at + 2] & 0x1F) << 8 | u16::from(section[at + 3]);
            if program != 0 {
                self.pmt = Some(pid);
                return;
            }
            at += 4;
        }
    }

    fn pmt(&mut self, section: &[u8]) {
        if !self.streams.is_empty() || section.first() != Some(&2) || section.len() < 16 {
            return;
        }
        let end = section.len() - 4;
        let info = usize::from(section[10] & 0x0F) << 8 | usize::from(section[11]);
        let mut at = 12 + info;
        let (mut video, mut audio) = (false, false);
        while at + 5 <= end {
            let kind = section[at];
            let pid = u16::from(section[at + 1] & 0x1F) << 8 | u16::from(section[at + 2]);
            let length = usize::from(section[at + 3] & 0x0F) << 8 | usize::from(section[at + 4]);
            let descriptors = section
                .get(at + 5..(at + 5 + length).min(end))
                .unwrap_or_default();
            at += 5 + length;
            let es = match kind {
                0x1B => Some(Es::H264),
                0x24 => Some(Es::Hevc),
                0x0F => Some(Es::Adts),
                0x03 | 0x04 => Some(Es::Mpeg),
                0x81 | 0x87 => Some(Es::Ac3),
                0x06 if dolby(descriptors) => Some(Es::Ac3),
                0xDB | 0xCF | 0xC1 | 0xC2 => {
                    self.protected = true;
                    None
                }
                _ => None,
            };
            let Some(es) = es else {
                continue;
            };
            let slot = if es.video() { &mut video } else { &mut audio };
            if *slot {
                continue;
            }
            *slot = true;
            self.streams.push(Stream {
                pid,
                es,
                pes: Vec::new(),
                gathering: false,
                expected: None,
                pts: None,
                dts: None,
                video: VideoEs::default(),
                audio: AudioEs::default(),
            });
        }
    }

    fn pes(&mut self, index: usize, start: bool, payload: &[u8]) {
        if start {
            self.finish(index);
            let stream = &mut self.streams[index];
            let Some((header, pts, dts, expected)) = pes_header(payload) else {
                stream.gathering = false;
                return;
            };
            stream.gathering = true;
            stream.pes.clear();
            stream.pes.extend_from_slice(&payload[header..]);
            stream.pts = pts;
            stream.dts = dts;
            stream.expected = expected;
        } else if self.streams[index].gathering {
            self.streams[index].pes.extend_from_slice(payload);
        }
        let stream = &mut self.streams[index];
        if stream.gathering
            && let Some(expected) = stream.expected
            && stream.pes.len() >= expected
        {
            stream.pes.truncate(expected);
            self.finish(index);
        }
    }

    /// Hands a gathered PES to its stream.
    fn finish(&mut self, index: usize) {
        let stream = &mut self.streams[index];
        if !stream.gathering {
            return;
        }
        stream.gathering = false;
        let data = std::mem::take(&mut stream.pes);
        let (pts, dts, es) = (stream.pts, stream.dts, stream.es);
        let before = self.ready.len();
        if es.video() {
            let times = pts.map(|pts| self.time.stamp(index, pts, dts));
            let stream = &mut self.streams[index];
            stream.video.push(
                es == Es::Hevc,
                &data,
                times,
                &mut self.tracks,
                &mut self.ready,
            );
        } else {
            let pts = pts.map(|pts| self.time.stamp(index, pts, None).0);
            let stream = &mut self.streams[index];
            stream
                .audio
                .push(es, &data, pts, &mut self.tracks, &mut self.ready);
        }
        self.emitted += self.ready.len() - before;
    }
}

/// Where the next packet starts: a sync byte with another one a packet later (or the end).
fn sync(data: &[u8], from: usize) -> usize {
    (from..data.len())
        .find(|&at| data[at] == 0x47 && (at + PACKET >= data.len() || data[at + PACKET] == 0x47))
        .unwrap_or(data.len())
}

/// A private stream's descriptors naming AC-3 or E-AC-3.
fn dolby(descriptors: &[u8]) -> bool {
    let mut at = 0;
    while at + 2 <= descriptors.len() {
        let (tag, length) = (descriptors[at], usize::from(descriptors[at + 1]));
        let body = descriptors.get(at + 2..at + 2 + length).unwrap_or_default();
        if tag == 0x6A || tag == 0x7A || (tag == 0x05 && (body == b"AC-3" || body == b"EAC3")) {
            return true;
        }
        at += 2 + length;
    }
    false
}

impl Demux for TsDemux<'_> {
    fn next(&mut self) -> Result<Option<Packet>> {
        loop {
            if let Some(packet) = self.ready.pop_front() {
                return Ok(Some(packet));
            }
            if self.ended {
                if self.emitted == 0 && self.protected {
                    return Err(Error::Unsupported("This video is protected".into()));
                }
                return Ok(None);
            }
            match self.segments.next() {
                Some(segment) => self.read(segment?.data),
                None => {
                    for index in 0..self.streams.len() {
                        self.finish(index);
                    }
                    self.ended = true;
                }
            }
        }
    }

    fn tracks(&self) -> &[Track] {
        &self.tracks
    }
}

/// The 90 kHz time an ID3 tag gives the frames after it (Apple's transport stream timestamp),
/// and where the tags end.
fn id3(data: &[u8]) -> (Option<i64>, usize) {
    const OWNER: &[u8] = b"com.apple.streaming.transportStreamTimestamp\0";
    let mut at = 0;
    let mut time = None;
    while data.len() >= at + 10 && &data[at..at + 3] == b"ID3" {
        let version = data[at + 3];
        let syncsafe = |bytes: &[u8]| {
            bytes
                .iter()
                .fold(0usize, |size, byte| size << 7 | usize::from(byte & 0x7F))
        };
        let size = syncsafe(&data[at + 6..at + 10]);
        let footer = if data[at + 5] & 0x10 != 0 { 10 } else { 0 };
        let end = (at + 10 + size + footer).min(data.len());
        let mut frame = at + 10;
        while frame + 10 <= end {
            let id = &data[frame..frame + 4];
            let length = if version >= 4 {
                syncsafe(&data[frame + 4..frame + 8])
            } else {
                u32::from_be_bytes([
                    data[frame + 4],
                    data[frame + 5],
                    data[frame + 6],
                    data[frame + 7],
                ]) as usize
            };
            let body = data
                .get(frame + 10..(frame + 10 + length).min(end))
                .unwrap_or_default();
            if id == b"PRIV"
                && let Some(stamp) = body.strip_prefix(OWNER)
                && stamp.len() >= 8
            {
                let value = u64::from_be_bytes(stamp[..8].try_into().unwrap_or_default());
                time = Some((value & ((1 << 33) - 1)) as i64);
            }
            if id == [0, 0, 0, 0] || length == 0 {
                break;
            }
            frame += 10 + length;
        }
        at = end;
    }
    (time, at)
}

/// HLS packed audio: each segment's frames after its ID3 tag.
pub struct PackedAudio<'a> {
    segments: Segments<'a>,
    es: Option<Es>,
    audio: AudioEs,
    time: Timeline,
    tracks: Vec<Track>,
    ready: VecDeque<Packet>,
}

impl<'a> PackedAudio<'a> {
    pub fn new(segments: Segments<'a>) -> Self {
        Self {
            segments,
            es: None,
            audio: AudioEs::default(),
            time: Timeline::default(),
            tracks: Vec::new(),
            ready: VecDeque::new(),
        }
    }
}

impl Demux for PackedAudio<'_> {
    fn next(&mut self) -> Result<Option<Packet>> {
        loop {
            if let Some(packet) = self.ready.pop_front() {
                return Ok(Some(packet));
            }
            let Some(segment) = self.segments.next() else {
                return Ok(None);
            };
            let data = segment?.data;
            let (time, start) = id3(&data);
            let body = &data[start..];
            let es = match self.es {
                Some(es) => es,
                None => {
                    let found = [Es::Adts, Es::Mpeg, Es::Ac3]
                        .into_iter()
                        .find(|es| audio_frame(*es, body).is_some());
                    let Some(es) = found else {
                        continue;
                    };
                    self.es = Some(es);
                    es
                }
            };
            let pts = time.map(|time| self.time.stamp(0, time, None).0);
            // Each segment holds whole frames.
            self.audio.rest.clear();
            self.audio
                .push(es, body, pts, &mut self.tracks, &mut self.ready);
        }
    }

    fn tracks(&self) -> &[Track] {
        &self.tracks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_unwrap_and_close_jumps() {
        let mut wrapping = Timeline::default();
        let near_wrap = WRAP - 3600;
        assert_eq!(wrapping.stamp(0, near_wrap, None), (near_wrap, near_wrap));
        // 33 bits wrap: the next frame is later, not 26 hours earlier.
        assert_eq!(wrapping.stamp(0, 0, None).0, WRAP);

        let mut time = Timeline::default();
        let at = 100 * CLOCK;
        assert_eq!(time.stamp(0, at, None).0, at);
        assert_eq!(time.stamp(0, at + 3600, None).0, at + 3600);
        // A discontinuity back to 5 s goes on from where the stream was.
        assert_eq!(time.stamp(0, 5 * CLOCK, None).0, at + 7200);
        // Another stream follows the same jump.
        assert_eq!(time.stamp(1, 5 * CLOCK + 90, None).0, at + 7200 + 90);
        // B-frames: the PTS keeps its distance from the DTS.
        assert_eq!(
            time.stamp(0, 5 * CLOCK + 3 * 3600, Some(5 * CLOCK + 3600)),
            (at + 7200 + 3 * 3600, at + 7200 + 3600)
        );
    }

    #[test]
    fn id3_tags_give_packed_audio_its_time() {
        let mut tag = b"ID3\x04\x00\x00".to_vec();
        let owner = b"com.apple.streaming.transportStreamTimestamp\0";
        let mut frame = b"PRIV".to_vec();
        let body_len = owner.len() + 8;
        frame.extend_from_slice(&[0, 0, 0, body_len as u8, 0, 0]);
        frame.extend_from_slice(owner);
        frame.extend_from_slice(&(90_000u64 * 10).to_be_bytes());
        tag.extend_from_slice(&[0, 0, 0, frame.len() as u8]);
        tag.extend_from_slice(&frame);
        let header_end = tag.len();
        tag.extend_from_slice(&[0xFF, 0xF1]);
        assert_eq!(id3(&tag), (Some(900_000), header_end));
        assert_eq!(id3(&[0xFF, 0xF1]), (None, 0));
    }

    #[test]
    fn pes_headers_give_their_times() {
        // PTS 900000 and DTS 896400, header data length 10.
        let encode = |marker: u8, time: i64| {
            [
                marker << 4 | ((time >> 29) & 0x0E) as u8 | 1,
                (time >> 22) as u8,
                ((time >> 14) & 0xFE) as u8 | 1,
                (time >> 7) as u8,
                ((time << 1) & 0xFE) as u8 | 1,
            ]
        };
        let mut pes = vec![0, 0, 1, 0xE0, 0, 0, 0x80, 0xC0, 10];
        pes.extend_from_slice(&encode(3, 900_000));
        pes.extend_from_slice(&encode(1, 896_400));
        pes.extend_from_slice(&[0, 0, 0, 1, 0x09]);
        assert_eq!(
            pes_header(&pes),
            Some((19, Some(900_000), Some(896_400), None))
        );
    }
}
