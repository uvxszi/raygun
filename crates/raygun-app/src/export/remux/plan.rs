//! The first pass: every kept sample measured and timed, so the writers know the whole file
//! before writing its first byte.

use std::sync::atomic::AtomicBool;

use super::{Error, Packet, Result, Source, Track, check_cancel};

/// One sample as written: its size and times in its track's timescale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    pub size: u32,
    pub dts: i64,
    pub pts: i64,
    pub duration: u32,
    pub key: bool,
}

/// A track that's written, with its samples in decoding order.
#[derive(Debug, Clone, PartialEq)]
pub struct Planned {
    pub track: Track,
    pub samples: Vec<Sample>,
}

impl Planned {
    pub fn is_video(&self) -> bool {
        self.track.codec.is_video()
    }

    /// Its first shown time, in its timescale.
    pub fn start(&self) -> i64 {
        self.samples
            .iter()
            .map(|sample| sample.pts)
            .min()
            .unwrap_or(0)
    }

    /// How long it plays, in its timescale.
    pub fn length(&self) -> u64 {
        self.samples
            .iter()
            .map(|sample| u64::from(sample.duration))
            .sum()
    }

    pub fn seconds(&self, time: i64) -> f64 {
        time as f64 / f64::from(self.track.timescale.max(1))
    }
}

/// Which of a source's tracks are written: the first video (unless only the sound is wanted) and
/// the first audio stream. The same packets come in the same order on both passes, so the choice
/// is the same.
#[derive(Debug, Clone, Default)]
pub struct Picker {
    audio_only: bool,
    /// By the source's track: its place among the picked, or `None` when left out.
    map: Vec<Option<Option<usize>>>,
    picked: usize,
    video: bool,
    audio: bool,
}

impl Picker {
    pub fn new(audio_only: bool) -> Self {
        Self {
            audio_only,
            ..Self::default()
        }
    }

    /// Where a packet of the source's `track` goes, or `None` when it's left out.
    pub fn pick(&mut self, tracks: &[Track], track: usize) -> Option<usize> {
        if self.map.len() <= track {
            self.map.resize(track + 1, None);
        }
        if let Some(known) = self.map[track] {
            return known;
        }
        let video = tracks.get(track)?.codec.is_video();
        let take = if video {
            !self.audio_only && !self.video
        } else {
            !self.audio
        };
        let place = take.then(|| {
            if video {
                self.video = true;
            } else {
                self.audio = true;
            }
            self.picked += 1;
            self.picked - 1
        });
        self.map[track] = Some(place);
        place
    }
}

/// The whole file, measured.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// Written tracks, video first.
    pub tracks: Vec<Planned>,
    /// Each written packet's track (an index into `tracks`), in the source's order.
    pub sequence: Vec<u8>,
    /// Picked place (as `Picker` numbers them) → index into `tracks`.
    pub order: Vec<usize>,
    pub audio_only: bool,
}

impl Plan {
    /// Reads `source` once, measuring what's written.
    pub fn scan(source: &Source, audio_only: bool, cancel: &AtomicBool) -> Result<Self> {
        let mut demux = source.open()?;
        let mut picker = Picker::new(audio_only);
        let mut tracks: Vec<Planned> = Vec::new();
        let mut raw: Vec<Vec<RawTimes>> = Vec::new();
        let mut sequence = Vec::new();
        let mut count = 0usize;
        while let Some(packet) = demux.next()? {
            count += 1;
            check_cancel(cancel, count)?;
            let Some(place) = picker.pick(demux.tracks(), packet.track) else {
                continue;
            };
            if place == tracks.len() {
                tracks.push(Planned {
                    track: demux.tracks()[packet.track].clone(),
                    samples: Vec::new(),
                });
                raw.push(Vec::new());
            }
            let Packet {
                dts,
                pts,
                duration,
                key,
                data,
                ..
            } = packet;
            tracks[place].samples.push(Sample {
                size: data.len() as u32,
                dts: 0,
                pts,
                duration: 0,
                key,
            });
            raw[place].push((dts, pts, duration));
            sequence.push(place as u8);
            if tracks.len() > u8::MAX as usize {
                return Err(Error::Invalid("Too many streams".into()));
            }
        }
        for (planned, times) in tracks.iter_mut().zip(&raw) {
            time(planned, times);
        }
        // Video first: players take the first track as the main one.
        let mut order: Vec<usize> = (0..tracks.len()).collect();
        order.sort_by_key(|place| !tracks[*place].is_video());
        let mut placed = vec![0; tracks.len()];
        for (index, place) in order.iter().enumerate() {
            placed[*place] = index;
        }
        let mut sorted: Vec<Option<Planned>> = tracks.into_iter().map(Some).collect();
        let tracks: Vec<Planned> = order
            .iter()
            .map(|place| sorted[*place].take().expect("each place once"))
            .collect();
        for entry in &mut sequence {
            *entry = placed[usize::from(*entry)] as u8;
        }
        Ok(Self {
            tracks,
            sequence,
            order: placed,
            audio_only,
        })
    }

    /// The earliest shown time of all tracks, in seconds: zero on the written timeline.
    pub fn origin(&self) -> f64 {
        self.tracks
            .iter()
            .map(|planned| planned.seconds(planned.start()))
            .fold(f64::INFINITY, f64::min)
            .min(f64::MAX)
    }

    /// How long the whole file plays, in seconds.
    pub fn seconds(&self) -> f64 {
        let origin = self.origin();
        self.tracks
            .iter()
            .map(|planned| {
                planned.seconds(planned.start()) - origin + planned.seconds(planned.length() as i64)
            })
            .fold(0.0, f64::max)
    }
}

/// A packet's times as the source gave them: decoding (when it says), presentation, duration.
type RawTimes = (Option<i64>, i64, Option<u32>);

/// Fills in decoding times and durations: increasing decoding times (worked out from the order
/// of presentation times when the source has none), presentation never before decoding, and
/// each sample lasting until the next.
fn time(planned: &mut Planned, raw: &[RawTimes]) {
    let samples = &mut planned.samples;
    if samples.is_empty() {
        return;
    }
    let has_dts = raw.iter().all(|(dts, _, _)| dts.is_some());
    if has_dts {
        for (sample, (dts, _, _)) in samples.iter_mut().zip(raw) {
            sample.dts = dts.unwrap_or(sample.pts);
        }
    } else {
        // Decoding order with presentation times only: the n-th decoded sample decodes at the
        // n-th smallest presentation time, moved back so none decodes after it's shown.
        let mut sorted: Vec<i64> = samples.iter().map(|sample| sample.pts).collect();
        sorted.sort_unstable();
        let delay = sorted
            .iter()
            .zip(samples.iter())
            .map(|(dts, sample)| dts - sample.pts)
            .max()
            .unwrap_or(0)
            .max(0);
        for (sample, dts) in samples.iter_mut().zip(sorted) {
            sample.dts = dts - delay;
        }
    }
    for index in 1..samples.len() {
        if samples[index].dts <= samples[index - 1].dts {
            samples[index].dts = samples[index - 1].dts + 1;
        }
    }
    // Shown never before decoded (MP4's composition offsets are unsigned): decoding moves
    // earlier, so what's shown keeps its time.
    let lead = samples
        .iter()
        .map(|sample| sample.dts - sample.pts)
        .max()
        .unwrap_or(0);
    if lead > 0 {
        for sample in samples.iter_mut() {
            sample.dts -= lead;
        }
    }
    let fallback = raw
        .last()
        .and_then(|(_, _, duration)| *duration)
        .unwrap_or_else(|| {
            if samples.len() > 1 {
                let span = samples[samples.len() - 1].dts - samples[0].dts;
                (span / (samples.len() as i64 - 1)).max(1) as u32
            } else {
                (planned.track.timescale / 25).max(1)
            }
        });
    let last = samples.len() - 1;
    for index in 0..last {
        samples[index].duration = (samples[index + 1].dts - samples[index].dts) as u32;
    }
    samples[last].duration = fallback;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::remux::Codec;

    fn video() -> Track {
        Track::video(Codec::Vp9, 1000, 16, 16)
    }

    #[test]
    fn decoding_times_come_from_presentation_order() {
        // I P B B as a Matroska file stores them: presentation times only.
        let pts = [0, 120, 40, 80, 160];
        let mut planned = Planned {
            track: video(),
            samples: pts
                .iter()
                .map(|pts| Sample {
                    size: 1,
                    dts: 0,
                    pts: *pts,
                    duration: 0,
                    key: false,
                })
                .collect(),
        };
        let raw: Vec<_> = pts.iter().map(|pts| (None, *pts, None)).collect();
        time(&mut planned, &raw);
        let dts: Vec<i64> = planned.samples.iter().map(|sample| sample.dts).collect();
        assert_eq!(dts, vec![-40, 0, 40, 80, 120]);
        assert!(
            planned
                .samples
                .iter()
                .all(|sample| sample.pts >= sample.dts)
        );
        let durations: Vec<u32> = planned
            .samples
            .iter()
            .map(|sample| sample.duration)
            .collect();
        assert_eq!(durations, vec![40, 40, 40, 40, 40]);
    }

    #[test]
    fn decoding_moves_earlier_when_a_sample_shows_before_it_decodes() {
        let raw = [(Some(0), -2, None), (Some(1), 3, None), (Some(2), 2, None)];
        let mut planned = Planned {
            track: video(),
            samples: raw
                .iter()
                .map(|(_, pts, _)| Sample {
                    size: 1,
                    dts: 0,
                    pts: *pts,
                    duration: 0,
                    key: true,
                })
                .collect(),
        };
        time(&mut planned, &raw);
        let pts: Vec<i64> = planned.samples.iter().map(|sample| sample.pts).collect();
        assert_eq!(pts, vec![-2, 3, 2], "what's shown keeps its time");
        let dts: Vec<i64> = planned.samples.iter().map(|sample| sample.dts).collect();
        assert_eq!(dts, vec![-2, -1, 0]);
        assert!(
            planned
                .samples
                .iter()
                .all(|sample| sample.dts <= sample.pts)
        );
    }

    #[test]
    fn the_picker_keeps_one_video_and_one_sound() {
        let tracks = vec![
            Track::audio(Codec::Eac3, 48_000, 2),
            video(),
            Track::audio(Codec::Eac3, 48_000, 2),
            video(),
        ];
        let mut picker = Picker::new(false);
        assert_eq!(picker.pick(&tracks, 0), Some(0));
        assert_eq!(picker.pick(&tracks, 2), None);
        assert_eq!(picker.pick(&tracks, 1), Some(1));
        assert_eq!(picker.pick(&tracks, 3), None);
        assert_eq!(picker.pick(&tracks, 0), Some(0));
        let mut sound = Picker::new(true);
        assert_eq!(sound.pick(&tracks, 1), None);
        assert_eq!(sound.pick(&tracks, 2), Some(0));
    }
}
