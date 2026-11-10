//! MP4 (and M4A) written with `mp4-atom`, its index first: `ftyp`, `moov` with every sample's
//! size and place, then one `mdat` with the samples in chunks of about a second, the tracks'
//! chunks interleaved. An edit list starts each track where it's first shown (B-frames decode
//! before they show) and delays a track that starts after the others.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::atomic::AtomicBool;

use mp4_atom::{
    Ac3 as Ac3Entry, Ac3SpecificBox, Audio, Avc1, Co64, Codec as Entry, CttsEntry, Dinf, Dops,
    Dref, DrefEntry, Edts, Elst, ElstEntry, Encode, Esds, FixedPoint, FourCC, Ftyp, Hdlr, Hvc1,
    Mdhd, Mdia, Minf, Moov, Mp4a, Mvhd, Opus, OpusChannelMapping, Smhd, Stbl, Stco, Stsc,
    StscEntry, Stsd, Stss, Stsz, StszSamples, Stts, SttsEntry, Tkhd, Trak, Url, Visual, Vmhd,
};

use mp4_atom::esds::{DecoderConfig, DecoderSpecific, EsDescriptor, SLConfig};

use super::codec;
use super::plan::{Picker, Plan, Planned};
use super::{Codec, Error, Result, Source, check_cancel};

/// The movie's timescale (milliseconds), for edit lists and track lengths.
const MOVIE_TIMESCALE: u32 = 1000;
/// A chunk holds about this long of one track.
const CHUNK_SECONDS: f64 = 1.0;
const CHUNK_BYTES: u64 = 4 << 20;

/// Whether an MP4 written here can hold `codec`.
pub fn holds(codec: &Codec) -> bool {
    match codec {
        Codec::H264(_) | Codec::Hevc(_) | Codec::Aac(_) | Codec::MpegAudio { .. } => true,
        // `dac3` needs the stream's first frame read.
        Codec::Ac3(info) => info.length > 0 || info.bit_rate_code > 0,
        Codec::Opus(head) => head.len() >= 19,
        Codec::Vp9 | Codec::Av1(_) | Codec::Eac3 => false,
    }
}

/// A run of one track's samples stored together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Chunk {
    track: usize,
    first: usize,
    count: usize,
    bytes: u64,
}

/// The chunks in file order: each track's samples gathered until they span about a second,
/// written when the track's next sample comes.
fn chunks(plan: &Plan) -> Vec<Chunk> {
    let mut next = vec![0usize; plan.tracks.len()];
    let mut open: Vec<Option<(Chunk, i64)>> = vec![None; plan.tracks.len()];
    let mut chunks = Vec::new();
    // Where each open chunk began in the sequence, to close the last ones in order.
    let mut began = vec![0usize; plan.tracks.len()];
    for (position, &entry) in plan.sequence.iter().enumerate() {
        let track = usize::from(entry);
        let planned = &plan.tracks[track];
        let index = next[track];
        next[track] += 1;
        let sample = planned.samples[index];
        if let Some((chunk, start)) = open[track]
            && (planned.seconds(sample.dts - start) >= CHUNK_SECONDS
                || chunk.bytes + u64::from(sample.size) > CHUNK_BYTES)
        {
            chunks.push(chunk);
            open[track] = None;
        }
        match &mut open[track] {
            Some((chunk, _)) => {
                chunk.count += 1;
                chunk.bytes += u64::from(sample.size);
            }
            None => {
                open[track] = Some((
                    Chunk {
                        track,
                        first: index,
                        count: 1,
                        bytes: u64::from(sample.size),
                    },
                    sample.dts,
                ));
                began[track] = position;
            }
        }
    }
    let mut last: Vec<(usize, Chunk)> = open
        .into_iter()
        .enumerate()
        .filter_map(|(track, chunk)| chunk.map(|(chunk, _)| (began[track], chunk)))
        .collect();
    last.sort_by_key(|(position, _)| *position);
    chunks.extend(last.into_iter().map(|(_, chunk)| chunk));
    chunks
}

fn runs<T: PartialEq + Copy>(values: impl Iterator<Item = T>) -> Vec<(u32, T)> {
    let mut runs: Vec<(u32, T)> = Vec::new();
    for value in values {
        match runs.last_mut() {
            Some((count, last)) if *last == value => *count += 1,
            _ => runs.push((1, value)),
        }
    }
    runs
}

fn sample_entry(planned: &Planned, track_id: u32, average_kbps: u32) -> Result<Entry> {
    let track = &planned.track;
    let visual = Visual {
        data_reference_index: 1,
        width: track.width.min(u32::from(u16::MAX)) as u16,
        height: track.height.min(u32::from(u16::MAX)) as u16,
        ..Visual::default()
    };
    let audio = Audio {
        data_reference_index: 1,
        channel_count: u16::from(track.channels.max(1)),
        sample_size: 16,
        sample_rate: FixedPoint::from(track.rate.min(u32::from(u16::MAX)) as u16),
    };
    let esds = |object: u8, specific: Option<Vec<u8>>| Esds {
        es_desc: EsDescriptor {
            es_id: track_id as u16,
            dec_config: DecoderConfig {
                object_type_indication: object,
                stream_type: 5,
                up_stream: 0,
                buffer_size_db: Default::default(),
                max_bitrate: average_kbps * 1000,
                avg_bitrate: average_kbps * 1000,
                dec_specific: specific.map(|raw| {
                    let (profile, rate, channels) = codec::parse_audio_config(&raw)
                        .map(|(object, rate, channels)| {
                            let index = codec::AAC_RATES
                                .iter()
                                .position(|known| *known == rate)
                                .unwrap_or(15) as u8;
                            (object, index, channels)
                        })
                        .unwrap_or_default();
                    DecoderSpecific {
                        profile,
                        freq_index: rate,
                        chan_conf: channels,
                        raw,
                    }
                }),
            },
            sl_config: SLConfig {},
            ..EsDescriptor::default()
        },
    };
    Ok(match &track.codec {
        Codec::H264(avcc) => Entry::Avc1(Avc1 {
            visual,
            avcc: avcc.clone(),
            ..Avc1::default()
        }),
        Codec::Hevc(hvcc) => Entry::Hvc1(Hvc1 {
            visual,
            hvcc: hvcc.clone(),
            ..Hvc1::default()
        }),
        Codec::Aac(config) => Entry::Mp4a(Mp4a {
            audio,
            esds: esds(0x40, Some(config.clone())),
            btrt: None,
            taic: None,
        }),
        Codec::MpegAudio { mpeg1, .. } => Entry::Mp4a(Mp4a {
            audio,
            esds: esds(if *mpeg1 { 0x6B } else { 0x69 }, None),
            btrt: None,
            taic: None,
        }),
        Codec::Ac3(info) => Entry::Ac3(Ac3Entry {
            audio,
            dac3: Ac3SpecificBox {
                fscod: info.fscod,
                bsid: info.bsid,
                bsmod: info.bsmod,
                acmod: info.acmod,
                lfeon: info.lfe,
                bit_rate_code: info.bit_rate_code,
            },
        }),
        Codec::Opus(head) => {
            let byte = |at: usize| head.get(at).copied().unwrap_or(0);
            let family = byte(18);
            Entry::Opus(Opus {
                audio: Audio {
                    sample_rate: FixedPoint::from(48_000u16),
                    ..audio
                },
                dops: Dops {
                    output_channel_count: byte(9),
                    pre_skip: u16::from_le_bytes([byte(10), byte(11)]),
                    input_sample_rate: u32::from_le_bytes([byte(12), byte(13), byte(14), byte(15)]),
                    output_gain: i16::from_le_bytes([byte(16), byte(17)]),
                    channel_mapping: (family != 0).then(|| OpusChannelMapping {
                        family,
                        stream_count: byte(19),
                        coupled_count: byte(20),
                        channel_mapping: head.get(21..).unwrap_or_default().to_vec(),
                    }),
                },
                btrt: None,
            })
        }
        other => {
            return Err(Error::Unsupported(format!(
                "{} can't go in an MP4 file here",
                other.name()
            )));
        }
    })
}

/// One track's `trak`, its chunks at `offsets`.
fn trak(
    planned: &Planned,
    track_id: u32,
    origin: f64,
    chunks: &[(usize, u64)],
    wide: bool,
) -> Result<(Trak, u64)> {
    let track = &planned.track;
    let samples = &planned.samples;
    let timescale = track.timescale.max(1);
    let length = planned.length();
    let bytes: u64 = samples.iter().map(|sample| u64::from(sample.size)).sum();
    let seconds = planned.seconds(length as i64).max(0.001);
    let average_kbps = ((bytes as f64 * 8.0 / seconds) / 1000.0).round() as u32;
    let entry = sample_entry(planned, track_id, average_kbps)?;

    let stts = Stts {
        entries: runs(samples.iter().map(|sample| sample.duration))
            .into_iter()
            .map(|(sample_count, sample_delta)| SttsEntry {
                sample_count,
                sample_delta,
            })
            .collect(),
    };
    let offsets = runs(samples.iter().map(|sample| sample.pts - sample.dts));
    let ctts =
        (offsets.len() > 1 || offsets.first().is_some_and(|(_, offset)| *offset != 0)).then(|| {
            mp4_atom::Ctts {
                entries: offsets
                    .into_iter()
                    .map(|(sample_count, sample_offset)| CttsEntry {
                        sample_count,
                        sample_offset,
                    })
                    .collect(),
            }
        });
    let stss = samples.iter().any(|sample| !sample.key).then(|| Stss {
        entries: samples
            .iter()
            .enumerate()
            .filter(|(_, sample)| sample.key)
            .map(|(index, _)| index as u32 + 1)
            .collect(),
    });
    let mut stsc = Vec::new();
    for (number, (count, _)) in chunks.iter().enumerate() {
        if stsc
            .last()
            .is_none_or(|last: &StscEntry| last.samples_per_chunk != *count as u32)
        {
            stsc.push(StscEntry {
                first_chunk: number as u32 + 1,
                samples_per_chunk: *count as u32,
                sample_description_index: 1,
            });
        }
    }
    let first_size = samples.first().map_or(0, |sample| sample.size);
    let stsz = Stsz {
        samples: if samples.iter().all(|sample| sample.size == first_size) {
            StszSamples::Identical {
                count: samples.len() as u32,
                size: first_size,
            }
        } else {
            StszSamples::Different {
                sizes: samples.iter().map(|sample| sample.size).collect(),
            }
        },
    };
    let (stco, co64) = if wide {
        (
            None,
            Some(Co64 {
                entries: chunks.iter().map(|(_, offset)| *offset).collect(),
            }),
        )
    } else {
        (
            Some(Stco {
                entries: chunks.iter().map(|(_, offset)| *offset as u32).collect(),
            }),
            None,
        )
    };

    // Shown from its first presented sample, which may come after its first decoded one.
    let start = planned.start();
    let media_time = (start - samples.first().map_or(0, |sample| sample.dts)).max(0) as u64;
    let delay =
        ((planned.seconds(start) - origin).max(0.0) * f64::from(MOVIE_TIMESCALE)).round() as u64;
    let shown = (planned.seconds(length as i64) * f64::from(MOVIE_TIMESCALE)).round() as u64;
    let mut edits = Vec::new();
    if delay > 0 {
        edits.push(ElstEntry {
            segment_duration: delay,
            media_time: None,
            media_rate: FixedPoint::from(1),
        });
    }
    if delay > 0 || media_time > 0 {
        edits.push(ElstEntry {
            segment_duration: shown,
            media_time: Some(media_time),
            media_rate: FixedPoint::from(1),
        });
    }
    let video = track.codec.is_video();
    let trak = Trak {
        tkhd: Tkhd {
            track_id,
            duration: delay + shown,
            enabled: true,
            in_movie: true,
            alternate_group: u16::from(!video),
            volume: FixedPoint::from(u8::from(!video)),
            width: FixedPoint::from(track.width.min(u32::from(u16::MAX)) as u16),
            height: FixedPoint::from(track.height.min(u32::from(u16::MAX)) as u16),
            ..Tkhd::default()
        },
        edts: (!edits.is_empty()).then_some(Edts {
            elst: Some(Elst { entries: edits }),
        }),
        mdia: Mdia {
            mdhd: Mdhd {
                timescale,
                duration: length,
                // ISO 639-2: three lowercase letters, else undetermined.
                language: track
                    .language
                    .clone()
                    .filter(|code| {
                        code.len() == 3 && code.bytes().all(|byte| byte.is_ascii_lowercase())
                    })
                    .unwrap_or_else(|| "und".into()),
                ..Mdhd::default()
            },
            hdlr: Hdlr {
                handler: FourCC::new(if video { b"vide" } else { b"soun" }),
                name: if video {
                    "VideoHandler".into()
                } else {
                    "SoundHandler".into()
                },
            },
            minf: Minf {
                vmhd: video.then(Vmhd::default),
                smhd: (!video).then(Smhd::default),
                dinf: Dinf {
                    dref: Dref {
                        entries: vec![DrefEntry::Url(Url {
                            location: String::new(),
                        })],
                    },
                },
                stbl: Stbl {
                    stsd: Stsd {
                        codecs: vec![entry],
                    },
                    stts,
                    ctts,
                    stss,
                    stsc: Stsc { entries: stsc },
                    stsz,
                    stco,
                    co64,
                    ..Stbl::default()
                },
                ..Minf::default()
            },
        },
        ..Trak::default()
    };
    Ok((trak, delay + shown))
}

fn moov(plan: &Plan, chunks: &[Chunk], offsets: &[u64], wide: bool) -> Result<Moov> {
    let origin = plan.origin();
    let mut traks = Vec::new();
    let mut duration = 0;
    for (index, planned) in plan.tracks.iter().enumerate() {
        let own: Vec<(usize, u64)> = chunks
            .iter()
            .zip(offsets)
            .filter(|(chunk, _)| chunk.track == index)
            .map(|(chunk, offset)| (chunk.count, *offset))
            .collect();
        let (trak, length) = trak(planned, index as u32 + 1, origin, &own, wide)?;
        duration = duration.max(length);
        traks.push(trak);
    }
    Ok(Moov {
        mvhd: Mvhd {
            timescale: MOVIE_TIMESCALE,
            duration,
            rate: FixedPoint::from(1),
            volume: FixedPoint::from(1),
            next_track_id: plan.tracks.len() as u32 + 1,
            ..Mvhd::default()
        },
        trak: traks,
        ..Moov::default()
    })
}

fn encoded(atom: &impl Encode) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    atom.encode(&mut buffer).map_err(|error| {
        Error::Invalid(format!("The file's index couldn't be written ({error})"))
    })?;
    Ok(buffer)
}

/// Writes the plan's tracks as MP4 (`audio`: as an M4A).
pub fn write(
    source: &Source,
    plan: &Plan,
    audio: bool,
    out: &mut dyn Write,
    cancel: &AtomicBool,
) -> Result<()> {
    let chunks = chunks(plan);
    let data: u64 = chunks.iter().map(|chunk| chunk.bytes).sum();
    let ftyp = encoded(&if audio {
        Ftyp {
            major_brand: FourCC::new(b"M4A "),
            minor_version: 0,
            compatible_brands: vec![
                FourCC::new(b"M4A "),
                FourCC::new(b"isom"),
                FourCC::new(b"mp42"),
            ],
        }
    } else {
        Ftyp {
            major_brand: FourCC::new(b"isom"),
            minor_version: 512,
            compatible_brands: vec![
                FourCC::new(b"isom"),
                FourCC::new(b"iso2"),
                FourCC::new(b"avc1"),
                FourCC::new(b"mp41"),
            ],
        }
    })?;
    // 32-bit chunk offsets unless the file may pass 4 GB.
    let wide = data + (64 << 20) > u64::from(u32::MAX);
    let mdat_header: u64 = if data + 8 > u64::from(u32::MAX) {
        16
    } else {
        8
    };
    // The index's length doesn't depend on the offsets in it.
    let placeholder = encoded(&moov(plan, &chunks, &vec![0; chunks.len()], wide)?)?;
    let mut offset = ftyp.len() as u64 + placeholder.len() as u64 + mdat_header;
    let mut offsets = Vec::with_capacity(chunks.len());
    for chunk in &chunks {
        offsets.push(offset);
        offset += chunk.bytes;
    }
    let moov = encoded(&moov(plan, &chunks, &offsets, wide)?)?;
    if moov.len() != placeholder.len() {
        return Err(Error::Invalid("The file's index changed size".into()));
    }
    out.write_all(&ftyp)?;
    out.write_all(&moov)?;
    if mdat_header == 16 {
        out.write_all(&1u32.to_be_bytes())?;
        out.write_all(b"mdat")?;
        out.write_all(&(data + 16).to_be_bytes())?;
    } else {
        out.write_all(&((data + 8) as u32).to_be_bytes())?;
        out.write_all(b"mdat")?;
    }

    // The second pass: each track's samples wait until their chunk's turn.
    let mut demux = source.open()?;
    let mut picker = Picker::new(plan.audio_only);
    let mut waiting: Vec<VecDeque<Vec<u8>>> = vec![VecDeque::new(); plan.tracks.len()];
    let mut next_chunk = 0usize;
    let mut position = 0usize;
    let mut count = 0usize;
    let mut next_sample = vec![0usize; plan.tracks.len()];
    while let Some(packet) = demux.next()? {
        count += 1;
        check_cancel(cancel, count)?;
        let Some(place) = picker.pick(demux.tracks(), packet.track) else {
            continue;
        };
        let track = plan.order[place];
        let planned = plan
            .tracks
            .get(track)
            .and_then(|planned| planned.samples.get(next_sample[track]));
        if plan.sequence.get(position).map(|entry| usize::from(*entry)) != Some(track)
            || planned.is_none_or(|sample| sample.size as usize != packet.data.len())
        {
            return Err(changed());
        }
        next_sample[track] += 1;
        position += 1;
        waiting[track].push_back(packet.data);
        flush(&chunks, &mut next_chunk, &mut waiting, out)?;
    }
    flush(&chunks, &mut next_chunk, &mut waiting, out)?;
    if next_chunk != chunks.len() || position != plan.sequence.len() {
        return Err(changed());
    }
    Ok(())
}

/// Writes every chunk whose samples have all arrived, in order.
fn flush(
    chunks: &[Chunk],
    next: &mut usize,
    waiting: &mut [VecDeque<Vec<u8>>],
    out: &mut dyn Write,
) -> Result<()> {
    while let Some(chunk) = chunks.get(*next) {
        if waiting[chunk.track].len() < chunk.count {
            break;
        }
        for _ in 0..chunk.count {
            let data = waiting[chunk.track].pop_front().expect("counted above");
            out.write_all(&data)?;
        }
        *next += 1;
    }
    Ok(())
}

fn changed() -> Error {
    Error::Invalid("The video changed while it was being saved".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::remux::Track;
    use crate::export::remux::plan::Sample;

    fn planned(timescale: u32, count: usize, step: i64) -> Planned {
        Planned {
            track: Track::audio(Codec::Aac(vec![0x12, 0x10]), timescale, 2),
            samples: (0..count)
                .map(|index| Sample {
                    size: 10,
                    dts: index as i64 * step,
                    pts: index as i64 * step,
                    duration: step as u32,
                    key: true,
                })
                .collect(),
        }
    }

    #[test]
    fn chunks_hold_about_a_second_of_each_track_in_turn() {
        // Two tracks of 0.25 s samples, alternating.
        let plan = Plan {
            tracks: vec![planned(4, 8, 1), planned(4, 8, 1)],
            sequence: (0..16).map(|index| (index % 2) as u8).collect(),
            order: vec![0, 1],
            audio_only: false,
        };
        let found = chunks(&plan);
        let shape: Vec<(usize, usize, usize)> = found
            .iter()
            .map(|chunk| (chunk.track, chunk.first, chunk.count))
            .collect();
        assert_eq!(shape, vec![(0, 0, 4), (1, 0, 4), (0, 4, 4), (1, 4, 4)]);
        assert_eq!(runs([3, 3, 4, 3].into_iter()), vec![(2, 3), (1, 4), (1, 3)]);
    }
}
