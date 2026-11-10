//! MP4 and MOV, read with `mp4-atom`: a plain file through its sample tables, a fragmented one
//! fragment by fragment (`moof` and its `mdat`), and HLS's fragmented MP4 segments after their
//! init section. Samples come out in file order; an edit list moves its track's times.

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use mp4_atom::{Atom, Codec as Entry, Moof, Moov, StszSamples, Trak, Trex};

use super::codec::{self, Ac3};
use super::{Codec, Demux, Error, Packet, Result, Segments, Track};

/// A sample's place and times, in its track's timescale.
#[derive(Debug, Clone, Copy)]
struct Located {
    offset: u64,
    size: u32,
    track: usize,
    dts: i64,
    pts: i64,
    duration: u32,
    key: bool,
}

/// One of the file's tracks that's read.
#[derive(Debug, Clone)]
struct Info {
    /// The edit list's move of its times.
    shift: i64,
    trex: Option<Trex>,
}

enum Input<'a> {
    Segments(Segments<'a>),
    /// A fragmented file, read from `at`.
    Fragments {
        file: BufReader<File>,
        at: u64,
        end: u64,
    },
    /// A plain file's samples in file order.
    Plain {
        file: BufReader<File>,
        samples: Vec<Located>,
        at: usize,
        position: u64,
    },
}

pub struct Mp4Demux<'a> {
    tracks: Vec<Track>,
    infos: Vec<Info>,
    by_id: HashMap<u32, usize>,
    /// Where a track's next fragment starts decoding when it has no `tfdt`.
    next_decode: HashMap<u32, u64>,
    input: Input<'a>,
    ready: VecDeque<Packet>,
}

/// A top-level box: its type, where its body starts and its length (to the end when unsized).
#[derive(Debug, Clone, Copy)]
struct BoxAt {
    kind: [u8; 4],
    start: u64,
    body: u64,
    end: u64,
}

fn box_in(data: &[u8], at: usize) -> Option<BoxAt> {
    let header = data.get(at..at + 8)?;
    let size = u64::from(u32::from_be_bytes(header[..4].try_into().ok()?));
    let kind: [u8; 4] = header[4..8].try_into().ok()?;
    let (body, size) = match size {
        0 => (at as u64 + 8, (data.len() - at) as u64),
        1 => {
            let large = u64::from_be_bytes(data.get(at + 8..at + 16)?.try_into().ok()?);
            (at as u64 + 16, large)
        }
        size => (at as u64 + 8, size),
    };
    if size < body - at as u64 {
        return None;
    }
    Some(BoxAt {
        kind,
        start: at as u64,
        body,
        end: at as u64 + size,
    })
}

fn box_in_file(file: &mut BufReader<File>, at: u64, length: u64) -> Result<Option<BoxAt>> {
    if at + 8 > length {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(at))?;
    let mut header = [0u8; 16];
    file.read_exact(&mut header[..8])?;
    let size = u64::from(u32::from_be_bytes(
        header[..4].try_into().unwrap_or_default(),
    ));
    let kind: [u8; 4] = header[4..8].try_into().unwrap_or_default();
    let (body, size) = match size {
        0 => (at + 8, length - at),
        1 => {
            file.read_exact(&mut header[8..16])?;
            (
                at + 16,
                u64::from_be_bytes(header[8..16].try_into().unwrap_or_default()),
            )
        }
        size => (at + 8, size),
    };
    if size < body - at {
        return Err(Error::Invalid("The file's boxes are damaged".into()));
    }
    Ok(Some(BoxAt {
        kind,
        start: at,
        body,
        end: (at + size).min(length),
    }))
}

fn decode<T: Atom>(body: &[u8]) -> Result<T> {
    T::decode_body(&mut &body[..])
        .map_err(|error| Error::Invalid(format!("The file's index is unreadable ({error})")))
}

/// Our track for one of the file's, when Raygun can carry its codec.
fn track_of(trak: &Trak) -> Option<Track> {
    let timescale = trak.mdia.mdhd.timescale.max(1);
    let entry = trak.mdia.minf.stbl.stsd.codecs.first()?;
    let visual = |codec: Codec, width: u16, height: u16| {
        Track::video(codec, timescale, u32::from(width), u32::from(height))
    };
    let audio = |codec: Codec, audio: &mp4_atom::Audio| {
        let rate = match u32::from(audio.sample_rate.integer()) {
            0 => timescale,
            rate => rate,
        };
        let mut track = Track::audio(codec, rate, audio.channel_count.min(255) as u8);
        track.timescale = timescale;
        track
    };
    let mut track = match entry {
        Entry::Avc1(avc1) => visual(
            Codec::H264(avc1.avcc.clone()),
            avc1.visual.width,
            avc1.visual.height,
        ),
        Entry::Avc3(avc3) if !avc3.avcc.sequence_parameter_sets.is_empty() => visual(
            Codec::H264(avc3.avcc.clone()),
            avc3.visual.width,
            avc3.visual.height,
        ),
        Entry::Hvc1(hvc1) => visual(
            Codec::Hevc(hvc1.hvcc.clone()),
            hvc1.visual.width,
            hvc1.visual.height,
        ),
        Entry::Hev1(hev1) if !hev1.hvcc.arrays.is_empty() => visual(
            Codec::Hevc(hev1.hvcc.clone()),
            hev1.visual.width,
            hev1.visual.height,
        ),
        Entry::Vp09(vp09) => visual(Codec::Vp9, vp09.visual.width, vp09.visual.height),
        Entry::Av01(av01) => visual(
            Codec::Av1(codec::atom_body(&av01.av1c)),
            av01.visual.width,
            av01.visual.height,
        ),
        Entry::Mp4a(mp4a) => {
            let config = &mp4a.esds.es_desc.dec_config;
            let codec = match config.object_type_indication {
                0x40 | 0x66..=0x68 => {
                    let raw = config
                        .dec_specific
                        .as_ref()
                        .map(|specific| specific.raw.clone())
                        .filter(|raw| raw.len() >= 2)
                        .unwrap_or_else(|| {
                            codec::aac_config(
                                u32::from(mp4a.audio.sample_rate.integer()),
                                mp4a.audio.channel_count as u8,
                            )
                        });
                    Codec::Aac(raw)
                }
                0x69 => Codec::MpegAudio {
                    layer: 3,
                    mpeg1: false,
                },
                0x6B => Codec::MpegAudio {
                    layer: 3,
                    mpeg1: true,
                },
                _ => return None,
            };
            let mut track = audio(codec.clone(), &mp4a.audio);
            if let Codec::Aac(config) = &codec
                && let Some((_, rate, channels)) = codec::parse_audio_config(config)
            {
                track.rate = rate;
                if channels > 0 {
                    track.channels = channels;
                }
            }
            track
        }
        Entry::Mp3(mp3) => audio(
            Codec::MpegAudio {
                layer: 3,
                mpeg1: true,
            },
            &mp3.audio,
        ),
        Entry::Ac3(ac3) => {
            let dac3 = &ac3.dac3;
            let rate = [48_000, 44_100, 32_000]
                .get(usize::from(dac3.fscod))
                .copied()
                .unwrap_or(48_000);
            let channels =
                [2u8, 1, 2, 3, 3, 4, 4, 5][usize::from(dac3.acmod & 7)] + u8::from(dac3.lfeon);
            audio(
                Codec::Ac3(Ac3 {
                    enhanced: false,
                    rate,
                    channels,
                    length: 0,
                    samples: 1536,
                    fscod: dac3.fscod,
                    bsid: dac3.bsid,
                    bsmod: dac3.bsmod,
                    acmod: dac3.acmod,
                    lfe: dac3.lfeon,
                    bit_rate_code: dac3.bit_rate_code,
                }),
                &ac3.audio,
            )
        }
        Entry::Eac3(eac3) => audio(Codec::Eac3, &eac3.audio),
        Entry::Opus(opus) => {
            let dops = &opus.dops;
            let mut head = b"OpusHead".to_vec();
            head.push(1);
            head.push(dops.output_channel_count);
            head.extend_from_slice(&dops.pre_skip.to_le_bytes());
            head.extend_from_slice(&dops.input_sample_rate.to_le_bytes());
            head.extend_from_slice(&dops.output_gain.to_le_bytes());
            match &dops.channel_mapping {
                Some(mapping) => {
                    head.push(mapping.family);
                    head.push(mapping.stream_count);
                    head.push(mapping.coupled_count);
                    head.extend_from_slice(&mapping.channel_mapping);
                }
                None => head.push(0),
            }
            let mut track = audio(Codec::Opus(head), &opus.audio);
            track.rate = 48_000;
            track
        }
        _ => return None,
    };
    let language = trak.mdia.mdhd.language.trim().to_owned();
    if !language.is_empty() && language != "und" {
        track.language = Some(language);
    }
    Some(track)
}

/// How a track's edit list moves its times: an empty edit delays it, a media time skips into it.
fn shift_of(trak: &Trak, movie_timescale: u32) -> i64 {
    let Some(elst) = trak.edts.as_ref().and_then(|edts| edts.elst.as_ref()) else {
        return 0;
    };
    let timescale = i64::from(trak.mdia.mdhd.timescale.max(1));
    let mut delay = 0i64;
    for entry in &elst.entries {
        match entry.media_time {
            None => {
                delay +=
                    entry.segment_duration as i64 * timescale / i64::from(movie_timescale.max(1));
            }
            Some(start) => return delay - start as i64,
        }
    }
    delay
}

impl<'a> Mp4Demux<'a> {
    fn with(moov: &Moov, input: Input<'a>) -> Self {
        let mut tracks = Vec::new();
        let mut infos = Vec::new();
        let mut by_id = HashMap::new();
        for trak in &moov.trak {
            let Some(track) = track_of(trak) else {
                continue;
            };
            let id = trak.tkhd.track_id;
            by_id.insert(id, tracks.len());
            tracks.push(track);
            infos.push(Info {
                shift: shift_of(trak, moov.mvhd.timescale),
                trex: moov
                    .mvex
                    .as_ref()
                    .and_then(|mvex| mvex.trex.iter().find(|trex| trex.track_id == id))
                    .cloned(),
            });
        }
        Self {
            tracks,
            infos,
            by_id,
            next_decode: HashMap::new(),
            input,
            ready: VecDeque::new(),
        }
    }

    /// HLS segments: the first carries the init section.
    pub fn segments(mut segments: Segments<'a>) -> Result<Self> {
        let first = segments
            .next()
            .ok_or_else(|| Error::Invalid("The saved stream has no segments".into()))??;
        let init = first
            .init
            .as_deref()
            .ok_or_else(|| Error::Invalid("The stream's init section is missing".into()))?;
        let moov = moov_in(init)?;
        let mut demux = Self::with(&moov, Input::Segments(segments));
        demux.fragments(&first.data, 0)?;
        Ok(demux)
    }

    /// A file, plain or fragmented.
    pub fn file(path: &Path) -> Result<Self> {
        let mut file = BufReader::new(File::open(path)?);
        let length = file.get_ref().metadata()?.len();
        let mut at = 0;
        let mut moov = None;
        let mut first_moof = None;
        while let Some(found) = box_in_file(&mut file, at, length)? {
            match &found.kind {
                b"moov" => {
                    let mut body = vec![0u8; (found.end - found.body) as usize];
                    file.seek(SeekFrom::Start(found.body))?;
                    file.read_exact(&mut body)?;
                    moov = Some(decode::<Moov>(&body)?);
                }
                b"moof" if first_moof.is_none() => first_moof = Some(found.start),
                _ => {}
            }
            if found.end <= at {
                break;
            }
            at = found.end;
        }
        let moov = moov.ok_or_else(|| Error::Invalid("The file has no index (moov)".into()))?;
        if moov.mvex.is_some()
            && let Some(start) = first_moof
        {
            return Ok(Self::with(
                &moov,
                Input::Fragments {
                    file,
                    at: start,
                    end: length,
                },
            ));
        }
        let mut demux = Self::with(&moov, Input::Segments(Box::new(std::iter::empty())));
        let mut samples = Vec::new();
        for trak in &moov.trak {
            if let Some(&index) = demux.by_id.get(&trak.tkhd.track_id) {
                samples.extend(table_samples(trak, index, demux.infos[index].shift)?);
            }
        }
        samples.sort_by_key(|sample| sample.offset);
        demux.input = Input::Plain {
            file,
            samples,
            at: 0,
            position: u64::MAX,
        };
        Ok(demux)
    }

    /// Reads the fragments in `data`, a buffer starting at `base` in its file.
    fn fragments(&mut self, data: &[u8], base: u64) -> Result<()> {
        let mut at = 0usize;
        while let Some(found) = box_in(data, at) {
            if &found.kind == b"moof" {
                let body = data
                    .get(found.body as usize..found.end as usize)
                    .ok_or_else(|| Error::Invalid("A fragment is cut short".into()))?;
                let moof: Moof = decode(body)?;
                self.fragment(&moof, found.start, data, base)?;
            }
            if found.end as usize <= at {
                break;
            }
            at = found.end as usize;
        }
        Ok(())
    }

    fn fragment(&mut self, moof: &Moof, moof_start: u64, data: &[u8], base: u64) -> Result<()> {
        let mut located = Vec::new();
        for traf in &moof.traf {
            let tfhd = &traf.tfhd;
            let Some(&index) = self.by_id.get(&tfhd.track_id) else {
                continue;
            };
            let info = &self.infos[index];
            let defaults = info.trex.clone().unwrap_or_default();
            let mut cursor = match tfhd.base_data_offset {
                Some(offset) => offset.saturating_sub(base),
                None => moof_start,
            };
            let mut decode = traf
                .tfdt
                .as_ref()
                .map(|tfdt| tfdt.base_media_decode_time)
                .or_else(|| self.next_decode.get(&tfhd.track_id).copied())
                .unwrap_or(0);
            let run_base = cursor;
            for trun in &traf.trun {
                if let Some(offset) = trun.data_offset {
                    cursor = (run_base as i64 + i64::from(offset)).max(0) as u64;
                }
                for entry in trun.samples.iter() {
                    let duration = entry
                        .duration
                        .or(tfhd.default_sample_duration)
                        .unwrap_or(defaults.default_sample_duration);
                    let size = entry
                        .size
                        .or(tfhd.default_sample_size)
                        .unwrap_or(defaults.default_sample_size);
                    let flags = entry
                        .flags
                        .or(tfhd.default_sample_flags)
                        .unwrap_or(defaults.default_sample_flags);
                    let dts = decode as i64 + info.shift;
                    located.push(Located {
                        offset: cursor,
                        size,
                        track: index,
                        dts,
                        pts: dts + entry.cts.unwrap_or(0),
                        duration,
                        key: flags >> 16 & 1 == 0,
                    });
                    cursor += u64::from(size);
                    decode += u64::from(duration);
                }
            }
            self.next_decode.insert(tfhd.track_id, decode);
        }
        located.sort_by_key(|sample| sample.offset);
        for sample in located {
            let start = sample.offset as usize;
            let bytes = data
                .get(start..start + sample.size as usize)
                .ok_or_else(|| Error::Invalid("A fragment's data is cut short".into()))?;
            self.ready.push_back(packet(&sample, bytes.to_vec()));
        }
        Ok(())
    }
}

fn packet(sample: &Located, data: Vec<u8>) -> Packet {
    Packet {
        track: sample.track,
        dts: Some(sample.dts),
        pts: sample.pts,
        duration: Some(sample.duration),
        key: sample.key,
        data,
    }
}

/// The `moov` in an init section.
fn moov_in(init: &[u8]) -> Result<Moov> {
    let mut at = 0;
    while let Some(found) = box_in(init, at) {
        if &found.kind == b"moov" {
            let body = init
                .get(found.body as usize..found.end as usize)
                .ok_or_else(|| Error::Invalid("The init section is cut short".into()))?;
            return decode(body);
        }
        if found.end as usize <= at {
            break;
        }
        at = found.end as usize;
    }
    Err(Error::Invalid("The init section has no moov".into()))
}

/// A plain track's samples from its sample tables.
fn table_samples(trak: &Trak, track: usize, shift: i64) -> Result<Vec<Located>> {
    let stbl = &trak.mdia.minf.stbl;
    let sizes: Vec<u32> = match &stbl.stsz.samples {
        StszSamples::Identical { count, size } => vec![*size; *count as usize],
        StszSamples::Different { sizes } => sizes.clone(),
    };
    let chunks: Vec<u64> = match (&stbl.co64, &stbl.stco) {
        (Some(co64), _) => co64.entries.clone(),
        (None, Some(stco)) => stco
            .entries
            .iter()
            .map(|offset| u64::from(*offset))
            .collect(),
        (None, None) => Vec::new(),
    };
    let mut durations = Vec::with_capacity(sizes.len());
    for entry in &stbl.stts.entries {
        durations.extend(std::iter::repeat_n(
            entry.sample_delta,
            entry.sample_count as usize,
        ));
    }
    let mut offsets = Vec::with_capacity(sizes.len());
    if let Some(ctts) = &stbl.ctts {
        for entry in &ctts.entries {
            offsets.extend(std::iter::repeat_n(
                entry.sample_offset,
                entry.sample_count as usize,
            ));
        }
    }
    let sync: Option<std::collections::HashSet<u32>> = stbl
        .stss
        .as_ref()
        .map(|stss| stss.entries.iter().copied().collect());
    let mut samples = Vec::with_capacity(sizes.len());
    let entries = &stbl.stsc.entries;
    let mut number = 0usize;
    let mut dts = shift;
    for (place, entry) in entries.iter().enumerate() {
        let first = entry.first_chunk.max(1) as usize;
        let last = entries.get(place + 1).map_or(chunks.len(), |next| {
            (next.first_chunk as usize).saturating_sub(1)
        });
        for chunk in first..=last.min(chunks.len()) {
            let mut offset = chunks[chunk - 1];
            for _ in 0..entry.samples_per_chunk {
                let Some(&size) = sizes.get(number) else {
                    break;
                };
                let duration = durations.get(number).copied().unwrap_or(0);
                let composition = offsets.get(number).copied().unwrap_or(0);
                samples.push(Located {
                    offset,
                    size,
                    track,
                    dts,
                    pts: dts + composition,
                    duration,
                    key: sync
                        .as_ref()
                        .is_none_or(|sync| sync.contains(&(number as u32 + 1))),
                });
                offset += u64::from(size);
                dts += i64::from(duration);
                number += 1;
            }
        }
    }
    Ok(samples)
}

impl Demux for Mp4Demux<'_> {
    fn next(&mut self) -> Result<Option<Packet>> {
        loop {
            if let Some(packet) = self.ready.pop_front() {
                return Ok(Some(packet));
            }
            match &mut self.input {
                Input::Segments(segments) => {
                    let Some(segment) = segments.next() else {
                        return Ok(None);
                    };
                    let segment = segment?;
                    self.fragments(&segment.data, 0)?;
                }
                Input::Fragments { file, at, end } => {
                    let Some(moof) = box_in_file(file, *at, *end)? else {
                        return Ok(None);
                    };
                    if moof.end <= *at {
                        return Ok(None);
                    }
                    if &moof.kind != b"moof" {
                        *at = moof.end;
                        continue;
                    }
                    // The fragment: its moof and the boxes up to the next moof (its mdat).
                    let mut stop = moof.end;
                    while let Some(next) = box_in_file(file, stop, *end)? {
                        if &next.kind == b"moof" || next.end <= stop {
                            break;
                        }
                        stop = next.end;
                        if &next.kind == b"mdat" {
                            break;
                        }
                    }
                    let start = moof.start;
                    let mut data = vec![0u8; (stop - start) as usize];
                    file.seek(SeekFrom::Start(start))?;
                    file.read_exact(&mut data)?;
                    *at = stop;
                    self.fragments(&data, start)?;
                }
                Input::Plain {
                    file,
                    samples,
                    at,
                    position,
                } => {
                    let Some(sample) = samples.get(*at).copied() else {
                        return Ok(None);
                    };
                    *at += 1;
                    if *position != sample.offset {
                        file.seek(SeekFrom::Start(sample.offset))?;
                    }
                    let mut data = vec![0u8; sample.size as usize];
                    file.read_exact(&mut data)?;
                    *position = sample.offset + u64::from(sample.size);
                    return Ok(Some(packet(&sample, data)));
                }
            }
        }
    }

    fn tracks(&self) -> &[Track] {
        &self.tracks
    }
}
