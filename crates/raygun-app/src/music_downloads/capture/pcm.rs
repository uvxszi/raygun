//! Sound captured from a stream as it decodes (M46.49): runs of PCM placed by where they start in
//! the stream, the parts still missing, and the runs stitched in order into one WAV. Overlaps are
//! trimmed by sample position, and each seam is lined up on the sound itself, as the positions a
//! player reports are only close. Blocking: call from `spawn_blocking`.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Gaps shorter than this many ms are timing, not missing sound.
pub const GAP_MS: u64 = 15;
/// How far a seam may move to line up with the sound, in ms.
const ALIGN_MS: u64 = 120;
/// The stretch of sound compared at a seam, in ms.
const WINDOW_MS: u64 = 40;
/// Below this mean level a stretch is too quiet to line anything up on.
const QUIET: f64 = 4.0;

/// A run of captured sound: interleaved little-endian 16-bit samples in a file, from frame `start`
/// of the stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub start: u64,
    pub frames: u64,
    pub path: PathBuf,
}

impl Piece {
    pub fn end(&self) -> u64 {
        self.start + self.frames
    }
}

/// The runs captured so far, at one rate and channel count.
#[derive(Debug, Clone)]
pub struct Coverage {
    pub rate: u32,
    pub channels: u16,
    pub pieces: Vec<Piece>,
}

impl Coverage {
    pub fn new(rate: u32, channels: u16) -> Self {
        Self {
            rate,
            channels: channels.max(1),
            pieces: Vec::new(),
        }
    }

    pub fn add(&mut self, piece: Piece) {
        self.pieces.push(piece);
    }

    pub fn frames_at(&self, ms: f64) -> u64 {
        frames_at(ms, self.rate)
    }

    pub fn ms_at(&self, frames: u64) -> u64 {
        frames * 1000 / u64::from(self.rate.max(1))
    }

    fn gap(&self) -> u64 {
        frames_at(GAP_MS as f64, self.rate)
    }

    /// What's covered, in order: `(start, end)` frames, runs closer than [`GAP_MS`] joined.
    pub fn covered(&self) -> Vec<(u64, u64)> {
        let mut ranges: Vec<(u64, u64)> = self
            .pieces
            .iter()
            .filter(|piece| piece.frames > 0)
            .map(|piece| (piece.start, piece.end()))
            .collect();
        ranges.sort_unstable();
        let mut merged: Vec<(u64, u64)> = Vec::new();
        for (start, end) in ranges {
            match merged.last_mut() {
                Some(last) if start <= last.1 + self.gap() => last.1 = last.1.max(end),
                _ => merged.push((start, end)),
            }
        }
        merged
    }

    /// The parts of `from..to` (frames) not covered yet.
    pub fn missing(&self, from: u64, to: u64) -> Vec<(u64, u64)> {
        let mut missing = Vec::new();
        let mut at = from;
        for (start, end) in self.covered() {
            if end <= at {
                continue;
            }
            if start >= to {
                break;
            }
            if start > at + self.gap() {
                missing.push((at, start.min(to)));
            }
            at = at.max(end);
            if at >= to {
                break;
            }
        }
        if at + self.gap() < to {
            missing.push((at, to));
        }
        missing
    }

    /// How much of `from..to` is covered, in frames.
    pub fn covered_within(&self, from: u64, to: u64) -> u64 {
        let missing: u64 = self
            .missing(from, to)
            .iter()
            .map(|(start, end)| end - start)
            .sum();
        to.saturating_sub(from).saturating_sub(missing)
    }

    /// Writes the runs in order as one 16-bit WAV, from frame `from` to `to` (or as far as they
    /// go): what a run repeats of the one before is trimmed by sample position, the seam moved
    /// to where the two runs' sound lines up. A gap is an error. Returns the frames written.
    #[cfg(test)]
    pub fn stitch(&self, from: u64, to: Option<u64>, wav: &Path) -> Result<u64, String> {
        self.stitch_padded(from, to, 0, wav)
    }

    /// [`Self::stitch`], where a gap that ends by frame `silence_until` (before what plays of a
    /// trimmed song) is silence instead of an error, keeping every frame after it in its place.
    pub fn stitch_padded(
        &self,
        from: u64,
        to: Option<u64>,
        silence_until: u64,
        wav: &Path,
    ) -> Result<u64, String> {
        let channels = usize::from(self.channels);
        let spec = hound::WavSpec {
            channels: self.channels,
            sample_rate: self.rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(
            BufWriter::new(File::create(wav).map_err(|error| error.to_string())?),
            spec,
        )
        .map_err(|error| error.to_string())?;
        let mut pieces: Vec<&Piece> = self
            .pieces
            .iter()
            .filter(|piece| piece.frames > 0)
            .collect();
        pieces.sort_by_key(|piece| (piece.start, std::cmp::Reverse(piece.frames)));
        let window = frames_at(WINDOW_MS as f64, self.rate) as usize;
        let reach = frames_at(ALIGN_MS as f64, self.rate) as usize;
        let gap = self.gap();
        let missing = |at: u64| {
            format!(
                "{} is missing from the capture",
                clock(at * 1000 / u64::from(self.rate.max(1)))
            )
        };
        // Where the output reached in the stream, and its last frames.
        let mut written: Option<u64> = None;
        let mut tail: Vec<i16> = Vec::new();
        let mut buffer = vec![0u8; 64 * 1024];
        for piece in pieces {
            let reached = written.unwrap_or(from);
            if to.is_some_and(|to| reached >= to) {
                break;
            }
            if piece.end() <= reached {
                continue;
            }
            let mut reached = reached;
            if piece.start > reached + gap {
                if piece.start > silence_until {
                    return Err(missing(reached));
                }
                for _ in 0..(piece.start - reached) * channels as u64 {
                    writer
                        .write_sample(0i16)
                        .map_err(|error| error.to_string())?;
                }
                tail.clear();
                reached = piece.start;
                written = Some(reached);
            }
            // What the run repeats, by position; then moved by as much as its sound says. Its
            // frame `k` is then the stream's `start + k - shift`.
            let overlap = reached.saturating_sub(piece.start);
            let mut shift = 0i64;
            if written.is_some() && overlap >= window as u64 && tail.len() >= window * channels {
                let before = reach.min(overlap as usize - window);
                let first = overlap - (window + before) as u64;
                let head = read_frames(piece, first, (before + window + reach) as u64)?;
                let compared = &tail[tail.len() - window * channels..];
                shift = best_shift(compared, &head, channels, before, reach) as i64;
            }
            let skip = (overlap as i64 + shift).max(0) as u64;
            let mut end = piece.frames;
            if let Some(to) = to {
                end = end.min((to as i64 - piece.start as i64 + shift).max(0) as u64);
            }
            if skip >= end {
                continue;
            }
            let mut reader = open_at(piece, skip)?;
            let mut left = (end - skip) as usize * channels * 2;
            while left > 0 {
                let take = left.min(buffer.len());
                reader
                    .read_exact(&mut buffer[..take])
                    .map_err(|error| format!("a captured piece is unreadable ({error})"))?;
                for pair in buffer[..take].as_chunks::<2>().0 {
                    let sample = i16::from_le_bytes(*pair);
                    writer
                        .write_sample(sample)
                        .map_err(|error| error.to_string())?;
                    tail.push(sample);
                }
                if tail.len() > window * channels * 4 {
                    tail.drain(..tail.len() - window * channels);
                }
                left -= take;
            }
            written = Some(reached + (end - skip));
        }
        let Some(reached) = written else {
            return Err("Nothing was captured".into());
        };
        if let Some(to) = to
            && reached + gap < to
        {
            return Err(missing(reached));
        }
        let frames = u64::from(writer.len()) / channels as u64;
        writer.finalize().map_err(|error| error.to_string())?;
        Ok(frames)
    }
}

/// Frames at `rate` in `ms`.
pub fn frames_at(ms: f64, rate: u32) -> u64 {
    (ms.max(0.0) * f64::from(rate) / 1000.0).round() as u64
}

/// The rate a capture's samples play at to sound as the stream did: the audio context's rate over
/// the speed it played at, as a 96 kHz capture of a stream played at 2× holds 48 kHz sound. The
/// samples aren't stretched or resampled, so nothing of the sound is lost; `None` when the two
/// don't give a whole rate.
pub fn restored_rate(context_rate: u32, speed: f64) -> Option<u32> {
    if context_rate == 0 || !speed.is_finite() || speed < 1.0 {
        return None;
    }
    let rate = f64::from(context_rate) / speed;
    let whole = rate.round();
    ((rate - whole).abs() < 0.01 && whole >= 8_000.0).then_some(whole as u32)
}

/// How far (in frames, from `-before` to `after`) the run `head` must move for its sound to line
/// up with `tail`, the last frames written: `head` holds `before + tail's length + after` frames,
/// its middle part being where positions alone put `tail`. 0 when it's too quiet to tell.
pub fn best_shift(
    tail: &[i16],
    head: &[i16],
    channels: usize,
    before: usize,
    after: usize,
) -> isize {
    let channels = channels.max(1);
    let mono = |samples: &[i16]| -> Vec<f64> {
        samples
            .chunks_exact(channels)
            .map(|frame| {
                frame.iter().map(|sample| f64::from(*sample)).sum::<f64>() / channels as f64
            })
            .collect()
    };
    let wanted = mono(tail);
    let candidates = mono(head);
    let window = wanted.len();
    if window == 0 || candidates.len() < window {
        return 0;
    }
    let level = wanted.iter().map(|sample| sample.abs()).sum::<f64>() / window as f64;
    if level < QUIET {
        return 0;
    }
    let wanted_energy: f64 = wanted.iter().map(|sample| sample * sample).sum();
    let mut best = (f64::MIN, 0isize);
    let last = candidates.len() - window;
    for offset in 0..=last.min(before + after) {
        let slice = &candidates[offset..offset + window];
        let energy: f64 = slice.iter().map(|sample| sample * sample).sum();
        if energy <= 0.0 {
            continue;
        }
        let dot: f64 = slice.iter().zip(&wanted).map(|(a, b)| a * b).sum();
        let score = dot / (energy * wanted_energy).sqrt();
        if score > best.0 {
            best = (score, offset as isize - before as isize);
        }
    }
    best.1
}

/// Frames `first..first + count` of a run (fewer at its end).
fn read_frames(piece: &Piece, first: u64, count: u64) -> Result<Vec<i16>, String> {
    let count = count.min(piece.frames.saturating_sub(first));
    let mut reader = open_at(piece, first)?;
    let mut bytes = vec![0u8; count as usize * channels_of(piece)? * 2];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| format!("a captured piece is unreadable ({error})"))?;
    Ok(bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_le_bytes(*pair))
        .collect())
}

/// A run's channels, from its file's size and frames.
fn channels_of(piece: &Piece) -> Result<usize, String> {
    let bytes = std::fs::metadata(&piece.path)
        .map_err(|error| format!("a captured piece is missing ({error})"))?
        .len();
    if piece.frames == 0 {
        return Ok(1);
    }
    Ok(((bytes / piece.frames) / 2).max(1) as usize)
}

fn open_at(piece: &Piece, frame: u64) -> Result<BufReader<File>, String> {
    let channels = channels_of(piece)? as u64;
    let mut file = File::open(&piece.path)
        .map_err(|error| format!("a captured piece is missing ({error})"))?;
    file.seek(SeekFrom::Start(frame * channels * 2))
        .map_err(|error| error.to_string())?;
    Ok(BufReader::new(file))
}

/// `m:ss`.
fn clock(ms: u64) -> String {
    let seconds = ms / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 8_000;

    /// A stereo signal that never repeats: frame `n` of the stream.
    fn signal(frame: u64) -> [i16; 2] {
        let mut state = frame
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        state ^= state >> 29;
        let left = ((state % 20_000) as i16) - 10_000;
        [left, left / 2]
    }

    /// A run of the signal written to a file, from `from` for `frames`, placed at `said` (where
    /// its player said it started).
    fn piece(dir: &Path, name: &str, from: u64, frames: u64, said: u64) -> Piece {
        let path = dir.join(name);
        let bytes: Vec<u8> = (from..from + frames)
            .flat_map(signal)
            .flat_map(i16::to_le_bytes)
            .collect();
        std::fs::write(&path, bytes).unwrap();
        Piece {
            start: said,
            frames,
            path,
        }
    }

    fn read_wav(path: &Path) -> (u32, Vec<i16>) {
        let mut reader = hound::WavReader::open(path).unwrap();
        let rate = reader.spec().sample_rate;
        (rate, reader.samples::<i16>().map(Result::unwrap).collect())
    }

    #[test]
    fn coverage_says_what_is_missing() {
        let temp = tempfile::TempDir::new().unwrap();
        let mut coverage = Coverage::new(1_000, 2);
        coverage.add(piece(temp.path(), "a", 0, 1_000, 0));
        coverage.add(piece(temp.path(), "b", 900, 1_100, 900));
        // 10 frames (10 ms) apart: timing, not a gap.
        coverage.add(piece(temp.path(), "c", 2_010, 490, 2_010));
        coverage.add(piece(temp.path(), "d", 3_000, 500, 3_000));
        assert_eq!(coverage.covered(), vec![(0, 2_500), (3_000, 3_500)]);
        assert_eq!(
            coverage.missing(0, 4_000),
            vec![(2_500, 3_000), (3_500, 4_000)]
        );
        assert_eq!(coverage.missing(100, 2_400), Vec::<(u64, u64)>::new());
        assert_eq!(
            coverage.missing(0, 3_505),
            vec![(2_500, 3_000)],
            "a tiny end is timing"
        );
        assert_eq!(coverage.covered_within(0, 4_000), 3_000);
    }

    #[test]
    fn runs_are_stitched_in_order_with_overlaps_trimmed() {
        let temp = tempfile::TempDir::new().unwrap();
        let mut coverage = Coverage::new(RATE, 2);
        // The first pass stalled at 3 s and went on from 4 s; the gap was captured again with a
        // second of sound around it, from a player that put its start 7 frames late; then the end.
        coverage.add(piece(temp.path(), "first", 0, 24_000, 0));
        coverage.add(piece(temp.path(), "after", 32_000, 16_000, 32_000));
        coverage.add(piece(temp.path(), "fill", 16_000, 24_000, 16_007));
        assert!(coverage.missing(0, 48_000).is_empty());
        let wav = temp.path().join("out.wav");
        let frames = coverage.stitch(0, Some(48_000), &wav).unwrap();
        assert_eq!(frames, 48_000);
        let (rate, samples) = read_wav(&wav);
        assert_eq!(rate, RATE);
        let expected: Vec<i16> = (0..48_000).flat_map(signal).collect();
        assert_eq!(samples.len(), expected.len());
        assert!(samples == expected, "every frame in its place, none twice");

        // A part nothing captured is an error, never silence.
        let mut holed = Coverage::new(RATE, 2);
        holed.add(piece(temp.path(), "one", 0, 8_000, 0));
        holed.add(piece(temp.path(), "two", 16_000, 8_000, 16_000));
        let error = holed
            .stitch(0, Some(24_000), &temp.path().join("holed.wav"))
            .unwrap_err();
        assert_eq!(error, "0:01 is missing from the capture");
        assert!(
            holed
                .stitch(0, Some(30_000), &temp.path().join("x.wav"))
                .is_err()
        );

        // Before a trimmed song's start, a part nothing captured is silence, every frame after it
        // still in its place.
        let mut late = Coverage::new(RATE, 2);
        late.add(piece(temp.path(), "late", 4_000, 4_000, 4_000));
        let padded = temp.path().join("padded.wav");
        assert_eq!(
            late.stitch_padded(0, Some(8_000), 4_000, &padded).unwrap(),
            8_000
        );
        let (_, samples) = read_wav(&padded);
        assert!(samples[..8_000].iter().all(|sample| *sample == 0));
        let expected: Vec<i16> = (4_000..8_000).flat_map(signal).collect();
        assert!(samples[8_000..] == expected[..]);
        assert!(late.stitch_padded(0, Some(8_000), 2_000, &padded).is_err());
    }

    #[test]
    fn a_capture_at_twice_the_speed_plays_back_at_its_own_pace() {
        assert_eq!(restored_rate(96_000, 2.0), Some(48_000));
        assert_eq!(restored_rate(88_200, 2.0), Some(44_100));
        assert_eq!(restored_rate(48_000, 1.0), Some(48_000));
        assert_eq!(restored_rate(96_000, 1.5), Some(64_000));
        assert_eq!(restored_rate(44_100, 4.0), Some(11_025));
        assert_eq!(restored_rate(44_100, 3.0), Some(14_700));
        assert_eq!(restored_rate(48_000, 7.0), None, "not a whole rate");
        assert_eq!(restored_rate(48_000, 0.5), None);
        assert_eq!(restored_rate(0, 2.0), None);

        // One second of a 1 kHz tone, heard at 2× in a 96 kHz context: half a second of 2 kHz.
        let temp = tempfile::TempDir::new().unwrap();
        let context = 96_000u32;
        let heard: Vec<u8> = (0..context / 2)
            .flat_map(|frame| {
                let value =
                    ((f64::from(frame) / f64::from(context) * 2_000.0 * std::f64::consts::TAU)
                        .sin()
                        * 12_000.0) as i16;
                [value, value]
            })
            .flat_map(i16::to_le_bytes)
            .collect();
        let path = temp.path().join("heard.raw");
        std::fs::write(&path, heard).unwrap();
        let rate = restored_rate(context, 2.0).unwrap();
        let mut coverage = Coverage::new(rate, 2);
        coverage.add(Piece {
            start: 0,
            frames: u64::from(context / 2),
            path,
        });
        let wav = temp.path().join("restored.wav");
        let frames = coverage.stitch(0, None, &wav).unwrap();
        let (rate, samples) = read_wav(&wav);
        assert_eq!(rate, 48_000);
        assert_eq!(frames * 1000 / u64::from(rate), 1_000, "a second again");
        // Rising zero crossings of the left channel: 1,000 in a second, so 1 kHz.
        let left: Vec<i16> = samples.iter().step_by(2).copied().collect();
        let rising = left
            .windows(2)
            .filter(|pair| pair[0] < 0 && pair[1] >= 0)
            .count();
        assert!((999..=1_001).contains(&rising), "{rising} Hz");
    }

    #[test]
    fn seams_line_up_on_the_sound() {
        let tail: Vec<i16> = (100..400).flat_map(signal).collect();
        // Positions put the tail 50 frames into `head`; its sound is 43 frames in.
        let head: Vec<i16> = (57..757).flat_map(signal).collect();
        assert_eq!(best_shift(&tail, &head, 2, 50, 50), -7);
        let quiet = vec![0i16; 600];
        assert_eq!(
            best_shift(&quiet, &head, 2, 50, 50),
            0,
            "nothing to line up on"
        );
    }
}
