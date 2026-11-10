//! Matroska: read with `matroska-demuxer`; written here. The writer knows every size from the
//! plan, so the file goes out front to back: the EBML header, the segment (its size given) with
//! a seek head, info and tracks, clusters that start at the video's key frames, and cues at the
//! end pointing at those clusters.

use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;
use std::sync::atomic::AtomicBool;

use matroska_demuxer::{Frame, MatroskaFile, TrackType};

use super::codec::{self, Ac3, MpegAudio};
use super::plan::{Picker, Plan};
use super::{AudioClock, Codec, Demux, Error, Packet, Result, Source, Track, check_cancel};

// ---- Reading ----

/// How a source track's frames are timed.
enum Timing {
    /// Presentation times from the blocks, in the track's timescale (video).
    Blocks,
    /// Counted audio frames, moved only by the blocks' jumps.
    Frames(AudioClock),
}

pub struct MkvDemux {
    file: MatroskaFile<BufReader<File>>,
    tracks: Vec<Track>,
    /// Each track's Matroska number and timing.
    numbers: Vec<u64>,
    timings: Vec<Timing>,
    /// Nanoseconds per block timestamp unit.
    scale: u64,
    frame: Frame,
}

impl MkvDemux {
    pub fn open(path: &Path) -> Result<Self> {
        let file = MatroskaFile::open(BufReader::new(File::open(path)?))
            .map_err(|error| Error::Invalid(format!("The file is unreadable ({error})")))?;
        let scale = file.info().timestamp_scale().get();
        let mut tracks = Vec::new();
        let mut numbers = Vec::new();
        let mut timings = Vec::new();
        for entry in file.tracks() {
            if entry
                .content_encodings()
                .is_some_and(|list| !list.is_empty())
            {
                continue;
            }
            let private = entry.codec_private().map(<[u8]>::to_vec);
            let track = match entry.track_type() {
                TrackType::Video => {
                    let Some(video) = entry.video() else {
                        continue;
                    };
                    let codec = match (entry.codec_id(), private) {
                        ("V_MPEG4/ISO/AVC", Some(private)) => {
                            Codec::H264(record(&private, *b"avcC").ok_or_else(|| {
                                Error::Invalid("The video's H.264 settings are unreadable".into())
                            })?)
                        }
                        ("V_MPEGH/ISO/HEVC", Some(private)) => {
                            Codec::Hevc(record(&private, *b"hvcC").ok_or_else(|| {
                                Error::Invalid("The video's HEVC settings are unreadable".into())
                            })?)
                        }
                        ("V_VP9", _) => Codec::Vp9,
                        ("V_AV1", Some(private)) => Codec::Av1(private),
                        _ => continue,
                    };
                    // Block timestamps are in the file's scale (milliseconds, usually).
                    let timescale = (1_000_000_000 / scale.max(1)).clamp(1, u64::from(u32::MAX));
                    Track::video(
                        codec,
                        timescale as u32,
                        video.pixel_width().get() as u32,
                        video.pixel_height().get() as u32,
                    )
                }
                TrackType::Audio => {
                    let Some(audio) = entry.audio() else {
                        continue;
                    };
                    let rate = audio.sampling_frequency().round() as u32;
                    let channels = audio.channels().get().min(255) as u8;
                    let codec = match entry.codec_id() {
                        "A_AAC" | "A_AAC/MPEG4/LC" | "A_AAC/MPEG2/LC" => Codec::Aac(
                            private
                                .filter(|private| private.len() >= 2)
                                .unwrap_or_else(|| codec::aac_config(rate, channels)),
                        ),
                        "A_MPEG/L3" => Codec::MpegAudio {
                            layer: 3,
                            mpeg1: rate >= 32_000,
                        },
                        "A_MPEG/L2" => Codec::MpegAudio {
                            layer: 2,
                            mpeg1: rate >= 32_000,
                        },
                        "A_AC3" => Codec::Ac3(Ac3 {
                            enhanced: false,
                            rate,
                            channels,
                            length: 0,
                            samples: 1536,
                            fscod: 0,
                            bsid: 8,
                            bsmod: 0,
                            acmod: 0,
                            lfe: false,
                            bit_rate_code: 0,
                        }),
                        "A_EAC3" => Codec::Eac3,
                        "A_OPUS" => match private {
                            Some(head) => Codec::Opus(head),
                            None => continue,
                        },
                        _ => continue,
                    };
                    let rate = match &codec {
                        Codec::Opus(_) => 48_000,
                        Codec::Aac(config) => {
                            codec::parse_audio_config(config).map_or(rate, |(_, core, _)| core)
                        }
                        _ => rate,
                    };
                    Track::audio(codec, rate, channels)
                }
                _ => continue,
            };
            let mut track = track;
            if let Some(language) = entry.language().filter(|language| *language != "und") {
                track.language = Some(language.to_owned());
            }
            timings.push(if track.codec.is_video() {
                Timing::Blocks
            } else {
                Timing::Frames(AudioClock::default())
            });
            numbers.push(entry.track_number().get());
            tracks.push(track);
        }
        Ok(Self {
            file,
            tracks,
            numbers,
            timings,
            scale,
            frame: Frame::default(),
        })
    }
}

/// A configuration record (Matroska's CodecPrivate) read as its MP4 box.
fn record<T: mp4_atom::Atom>(private: &[u8], kind: [u8; 4]) -> Option<T> {
    use mp4_atom::Decode;

    let mut boxed = ((private.len() + 8) as u32).to_be_bytes().to_vec();
    boxed.extend_from_slice(&kind);
    boxed.extend_from_slice(private);
    T::decode(&mut boxed.as_slice()).ok()
}

/// How many samples an audio frame of `codec` holds.
fn frame_samples(codec: &Codec, data: &[u8]) -> u32 {
    match codec {
        Codec::Aac(_) => 1024,
        Codec::MpegAudio { .. } => MpegAudio::parse(data).map_or(1152, |frame| frame.samples),
        Codec::Ac3(_) | Codec::Eac3 => Ac3::parse(data).map_or(1536, |frame| frame.samples),
        Codec::Opus(_) => codec::opus_samples(data),
        _ => 0,
    }
}

impl Demux for MkvDemux {
    fn next(&mut self) -> Result<Option<Packet>> {
        loop {
            let more = self
                .file
                .next_frame(&mut self.frame)
                .map_err(|error| Error::Invalid(format!("The file is unreadable ({error})")))?;
            if !more {
                return Ok(None);
            }
            let Some(index) = self
                .numbers
                .iter()
                .position(|number| *number == self.frame.track)
            else {
                continue;
            };
            let track = &mut self.tracks[index];
            let nanos = i128::from(self.frame.timestamp) * i128::from(self.scale);
            let time = (nanos * i128::from(track.timescale) / 1_000_000_000) as i64;
            let data = std::mem::take(&mut self.frame.data);
            let packet = match &mut self.timings[index] {
                Timing::Blocks => {
                    let key = self
                        .frame
                        .is_keyframe
                        .unwrap_or_else(|| match &track.codec {
                            Codec::H264(_) => avcc_units(&data)
                                .iter()
                                .any(|unit| codec::avc_type(unit) == codec::AVC_IDR),
                            Codec::Hevc(_) => codec::hevc_random_access(&avcc_units(&data)),
                            _ => false,
                        });
                    Packet {
                        track: index,
                        dts: None,
                        pts: time,
                        duration: None,
                        key,
                        data,
                    }
                }
                Timing::Frames(clock) => {
                    // AC-3 learns its `dac3` fields from its first frame.
                    if let Codec::Ac3(info) = &mut track.codec
                        && info.length == 0
                        && let Some(found) = Ac3::parse(&data)
                    {
                        *info = found;
                    }
                    let samples = frame_samples(&track.codec, &data);
                    let time = clock.stamp(Some(time), samples);
                    Packet {
                        track: index,
                        dts: Some(time),
                        pts: time,
                        duration: Some(samples),
                        key: true,
                        data,
                    }
                }
            };
            return Ok(Some(packet));
        }
    }

    fn tracks(&self) -> &[Track] {
        &self.tracks
    }
}

/// The NAL units of a sample with four-byte lengths.
fn avcc_units(data: &[u8]) -> Vec<&[u8]> {
    let mut units = Vec::new();
    let mut at = 0;
    while at + 4 <= data.len() {
        let length = u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
        let end = at + 4 + length as usize;
        if end > data.len() {
            break;
        }
        units.push(&data[at + 4..end]);
        at = end;
    }
    units
}

// ---- Writing ----

const EBML: u32 = 0x1A45_DFA3;
const SEGMENT: u32 = 0x1853_8067;
const SEEK_HEAD: u32 = 0x114D_9B74;
const SEEK: u32 = 0x4DBB;
const SEEK_ID: u32 = 0x53AB;
const SEEK_POSITION: u32 = 0x53AC;
const INFO: u32 = 0x1549_A966;
const TRACKS: u32 = 0x1654_AE6B;
const CLUSTER: u32 = 0x1F43_B675;
const CUES: u32 = 0x1C53_BB6B;
const TIMESTAMP: u32 = 0xE7;
const SIMPLE_BLOCK: u32 = 0xA3;

/// An element ID's bytes (IDs carry their own length marker).
fn id_bytes(id: u32) -> Vec<u8> {
    let bytes = id.to_be_bytes();
    let skip = bytes.iter().take_while(|byte| **byte == 0).count();
    bytes[skip..].to_vec()
}

/// A size as an EBML variable-length integer, as short as it can be.
fn size_bytes(size: u64) -> Vec<u8> {
    let length = (1..=8)
        .find(|length| size < (1u64 << (7 * length)) - 1)
        .unwrap_or(8);
    sized(size, length)
}

fn sized(size: u64, length: usize) -> Vec<u8> {
    let marked = size | 1u64 << (7 * length);
    marked.to_be_bytes()[8 - length..].to_vec()
}

fn element(id: u32, body: &[u8]) -> Vec<u8> {
    let mut out = id_bytes(id);
    out.extend(size_bytes(body.len() as u64));
    out.extend_from_slice(body);
    out
}

fn unsigned(id: u32, value: u64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let skip = bytes.iter().take_while(|byte| **byte == 0).count().min(7);
    element(id, &bytes[skip..])
}

/// An unsigned integer always eight bytes long, so a position can be filled in without moving
/// anything after it.
fn unsigned8(id: u32, value: u64) -> Vec<u8> {
    element(id, &value.to_be_bytes())
}

fn float(id: u32, value: f64) -> Vec<u8> {
    element(id, &value.to_be_bytes())
}

fn text(id: u32, value: &str) -> Vec<u8> {
    element(id, value.as_bytes())
}

/// The block timestamps' unit: a millisecond.
const SCALE_NS: u64 = 1_000_000;
/// Clusters run this long at least before a key frame starts the next.
const CLUSTER_MS: i64 = 2_000;
/// Block times are 16-bit offsets from their cluster's time.
const CLUSTER_SPAN_MS: i64 = 30_000;

/// Codec ID and CodecPrivate.
fn codec_id(codec: &Codec) -> (&'static str, Option<Vec<u8>>) {
    match codec {
        Codec::H264(avcc) => ("V_MPEG4/ISO/AVC", Some(codec::atom_body(avcc))),
        Codec::Hevc(hvcc) => ("V_MPEGH/ISO/HEVC", Some(codec::atom_body(hvcc))),
        Codec::Vp9 => ("V_VP9", None),
        Codec::Av1(config) => ("V_AV1", Some(config.clone())),
        Codec::Aac(config) => ("A_AAC", Some(config.clone())),
        Codec::MpegAudio { layer: 3, .. } => ("A_MPEG/L3", None),
        Codec::MpegAudio { layer: 2, .. } => ("A_MPEG/L2", None),
        Codec::MpegAudio { .. } => ("A_MPEG/L1", None),
        Codec::Ac3(_) => ("A_AC3", None),
        Codec::Eac3 => ("A_EAC3", None),
        Codec::Opus(head) => ("A_OPUS", Some(head.clone())),
    }
}

fn track_entry(number: u64, track: &Track) -> Vec<u8> {
    let (id, private) = codec_id(&track.codec);
    let mut body = unsigned(0xD7, number);
    body.extend(unsigned(0x73C5, number));
    body.extend(unsigned(0x83, if track.codec.is_video() { 1 } else { 2 }));
    body.extend(unsigned(0x9C, 0));
    body.extend(text(
        0x0022_B59C,
        track.language.as_deref().unwrap_or("und"),
    ));
    body.extend(text(0x86, id));
    if let Some(private) = private {
        body.extend(element(0x63A2, &private));
    }
    if let Codec::Opus(head) = &track.codec {
        let pre_skip = head
            .get(10..12)
            .map_or(0, |bytes| u16::from_le_bytes([bytes[0], bytes[1]]));
        body.extend(unsigned(
            0x56AA,
            u64::from(pre_skip) * 1_000_000_000 / 48_000,
        ));
        body.extend(unsigned(0x56BB, 80_000_000));
    }
    if track.codec.is_video() {
        let mut video = unsigned(0xB0, u64::from(track.width));
        video.extend(unsigned(0xBA, u64::from(track.height)));
        body.extend(element(0xE0, &video));
    } else {
        let mut audio = float(0xB5, f64::from(track.rate));
        audio.extend(unsigned(0x9F, u64::from(track.channels.max(1))));
        body.extend(element(0xE1, &audio));
    }
    element(0xAE, &body)
}

/// One cluster: which packets (a run of the plan's sequence) and its time.
#[derive(Debug, Clone)]
struct Cluster {
    first: usize,
    count: usize,
    time: i64,
    /// The cluster's body length, and the whole element's.
    body: u64,
    length: u64,
    /// A video key frame starts it (or there's no video): a place to seek to.
    cue: Option<i64>,
}

/// Each packet's block time (ms on the written timeline) and the clusters they fall in.
fn layout(plan: &Plan) -> (Vec<i64>, Vec<Cluster>) {
    let origin = plan.origin();
    let mut next = vec![0usize; plan.tracks.len()];
    let mut times = Vec::with_capacity(plan.sequence.len());
    let mut keys = Vec::with_capacity(plan.sequence.len());
    for &entry in &plan.sequence {
        let planned = &plan.tracks[usize::from(entry)];
        let sample = planned.samples[next[usize::from(entry)]];
        next[usize::from(entry)] += 1;
        let seconds = planned.seconds(sample.pts) - origin;
        times.push((seconds * 1000.0).round() as i64);
        keys.push(sample.key && (planned.is_video() || !plan.tracks[0].is_video()));
    }
    let video = plan
        .tracks
        .first()
        .is_some_and(|planned| planned.is_video());
    let mut clusters: Vec<Cluster> = Vec::new();
    let (mut low, mut high) = (0i64, 0i64);
    for (index, &time) in times.iter().enumerate() {
        let start_new = match clusters.last() {
            None => true,
            Some(cluster) => {
                let started = times[cluster.first];
                let long = time - started >= CLUSTER_MS;
                let span = time.max(high) - time.min(low) > CLUSTER_SPAN_MS;
                span || (long && keys[index] && (video || time - started >= 2 * CLUSTER_MS))
            }
        };
        if start_new {
            clusters.push(Cluster {
                first: index,
                count: 0,
                time,
                body: 0,
                length: 0,
                cue: keys[index].then_some(time),
            });
            (low, high) = (time, time);
        }
        let cluster = clusters.last_mut().expect("one was just made");
        cluster.count += 1;
        cluster.time = cluster.time.min(time);
        low = low.min(time);
        high = high.max(time);
    }
    (times, clusters)
}

fn block_header(track: u64, relative: i64, key: bool) -> [u8; 4] {
    let relative = (relative as i16).to_be_bytes();
    [
        0x80 | track as u8,
        relative[0],
        relative[1],
        if key { 0x80 } else { 0 },
    ]
}

/// Writes the plan's tracks as Matroska.
pub fn write(source: &Source, plan: &Plan, out: &mut dyn Write, cancel: &AtomicBool) -> Result<()> {
    let (times, mut clusters) = layout(plan);
    let mut sizes = Vec::with_capacity(plan.sequence.len());
    let mut keys = Vec::with_capacity(plan.sequence.len());
    let mut next = vec![0usize; plan.tracks.len()];
    for &entry in &plan.sequence {
        let sample = plan.tracks[usize::from(entry)].samples[next[usize::from(entry)]];
        next[usize::from(entry)] += 1;
        sizes.push(u64::from(sample.size));
        keys.push(sample.key || !plan.tracks[usize::from(entry)].is_video());
    }
    for cluster in &mut clusters {
        let mut body = unsigned(TIMESTAMP, cluster.time.max(0) as u64).len() as u64;
        for size in &sizes[cluster.first..cluster.first + cluster.count] {
            let block = 4 + size;
            body += 1 + size_bytes(block).len() as u64 + block;
        }
        cluster.body = body;
        cluster.length = id_bytes(CLUSTER).len() as u64 + size_bytes(body).len() as u64 + body;
    }

    let mut info = unsigned(0x002A_D7B1, SCALE_NS);
    info.extend(float(0x4489, plan.seconds() * 1000.0));
    info.extend(text(0x4D80, "Raygun"));
    info.extend(text(0x5741, "Raygun"));
    let info = element(INFO, &info);
    let tracks: Vec<u8> = plan
        .tracks
        .iter()
        .enumerate()
        .flat_map(|(index, planned)| track_entry(index as u64 + 1, &planned.track))
        .collect();
    let tracks = element(TRACKS, &tracks);
    let seek_entry = |id: u32, position: u64| {
        let mut body = element(SEEK_ID, &id_bytes(id));
        body.extend(unsigned8(SEEK_POSITION, position));
        element(SEEK, &body)
    };
    let seek_head_length = element(
        SEEK_HEAD,
        &[
            seek_entry(INFO, 0),
            seek_entry(TRACKS, 0),
            seek_entry(CUES, 0),
        ]
        .concat(),
    )
    .len() as u64;
    let info_at = seek_head_length;
    let tracks_at = info_at + info.len() as u64;
    let clusters_at = tracks_at + tracks.len() as u64;
    let mut cue_points = Vec::new();
    let mut position = clusters_at;
    let cue_track = 1u64;
    for cluster in &clusters {
        if let Some(time) = cluster.cue {
            let mut positions = unsigned(0xF7, cue_track);
            positions.extend(unsigned8(0xF1, position));
            let mut point = unsigned(0xB3, time.max(0) as u64);
            point.extend(element(0xB7, &positions));
            cue_points.extend(element(0xBB, &point));
        }
        position += cluster.length;
    }
    let cues_at = position;
    let cues = element(CUES, &cue_points);
    let seek_head = element(
        SEEK_HEAD,
        &[
            seek_entry(INFO, info_at),
            seek_entry(TRACKS, tracks_at),
            seek_entry(CUES, cues_at),
        ]
        .concat(),
    );
    let segment_length = cues_at + cues.len() as u64;

    let mut header = unsigned(0x4286, 1);
    header.extend(unsigned(0x42F7, 1));
    header.extend(unsigned(0x42F2, 4));
    header.extend(unsigned(0x42F3, 8));
    header.extend(text(0x4282, "matroska"));
    header.extend(unsigned(0x4287, 4));
    header.extend(unsigned(0x4285, 2));
    out.write_all(&element(EBML, &header))?;
    out.write_all(&id_bytes(SEGMENT))?;
    out.write_all(&sized(segment_length, 8))?;
    out.write_all(&seek_head)?;
    out.write_all(&info)?;
    out.write_all(&tracks)?;

    // The second pass: the same packets again, now written.
    let mut demux = source.open()?;
    let mut picker = Picker::new(plan.audio_only);
    let mut index = 0usize;
    let mut cluster_at = 0usize;
    let mut count = 0usize;
    while let Some(packet) = demux.next()? {
        count += 1;
        check_cancel(cancel, count)?;
        let Some(place) = picker.pick(demux.tracks(), packet.track) else {
            continue;
        };
        let track = plan.order[place];
        if index >= plan.sequence.len()
            || usize::from(plan.sequence[index]) != track
            || sizes[index] != packet.data.len() as u64
        {
            return Err(changed());
        }
        if clusters
            .get(cluster_at)
            .is_some_and(|cluster| cluster.first == index)
        {
            let cluster = &clusters[cluster_at];
            out.write_all(&id_bytes(CLUSTER))?;
            out.write_all(&size_bytes(cluster.body))?;
            out.write_all(&unsigned(TIMESTAMP, cluster.time.max(0) as u64))?;
            cluster_at += 1;
        }
        let cluster = &clusters[cluster_at - 1];
        let block = 4 + packet.data.len() as u64;
        out.write_all(&id_bytes(SIMPLE_BLOCK))?;
        out.write_all(&size_bytes(block))?;
        out.write_all(&block_header(
            track as u64 + 1,
            times[index] - cluster.time,
            keys[index],
        ))?;
        out.write_all(&packet.data)?;
        index += 1;
    }
    if index != plan.sequence.len() {
        return Err(changed());
    }
    out.write_all(&cues)?;
    Ok(())
}

fn changed() -> Error {
    Error::Invalid("The video changed while it was being saved".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ebml_sizes_and_ids() {
        assert_eq!(size_bytes(0), vec![0x80]);
        assert_eq!(size_bytes(126), vec![0xFE]);
        // 127 would read as "unknown" in one byte.
        assert_eq!(size_bytes(127), vec![0x40, 0x7F]);
        assert_eq!(sized(5, 8), vec![1, 0, 0, 0, 0, 0, 0, 5]);
        assert_eq!(id_bytes(SEGMENT), vec![0x18, 0x53, 0x80, 0x67]);
        assert_eq!(id_bytes(TIMESTAMP), vec![0xE7]);
        assert_eq!(unsigned(0xD7, 1), vec![0xD7, 0x81, 1]);
        assert_eq!(block_header(1, -2, true), [0x81, 0xFF, 0xFE, 0x80]);
    }
}
