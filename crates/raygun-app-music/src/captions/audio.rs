//! Windows of an episode's audio for transcription (M46.32): decoded with symphonia, mixed to
//! mono and brought to Whisper's 16 kHz. Blocking: call from `spawn_blocking`. Videos' sound
//! (M46.33) comes through the same reader, from any source.

use std::path::Path;

use symphonia::core::errors::Error as DecodeError;
use symphonia::core::formats::{SeekMode, SeekTo};
use symphonia::core::units::{Time, Timestamp};

use crate::music_downloads::transcode::{Decoding, open, open_source};

/// Opens the audio again from its start (a source that can't seek is read anew).
pub type Opener = Box<dyn Fn() -> Result<Decoding, String> + Send>;

/// Skipping further ahead than this seeks instead of decoding through.
const SEEK_AFTER_MS: u64 = 5_000;
const TARGET_RATE: u64 = raygun_ai::speech::SAMPLE_RATE as u64;

pub struct AudioReader {
    opener: Opener,
    decoding: Decoding,
    /// Mono samples at the file's rate, the first one at source frame `start_frame`.
    pending: Vec<f32>,
    start_frame: u64,
    ended: bool,
}

impl AudioReader {
    pub fn open(path: &Path) -> Result<Self, String> {
        let path = path.to_owned();
        Self::from_opener(Box::new(move || open(&path)))
    }

    pub fn from_opener(opener: Opener) -> Result<Self, String> {
        Ok(Self {
            decoding: opener()?,
            opener,
            pending: Vec::new(),
            start_frame: 0,
            ended: false,
        })
    }

    pub fn duration_ms(&self) -> Option<u64> {
        self.decoding.duration_ms
    }

    /// Up to `len_ms` of 16 kHz mono from `from_ms`; empty past the end.
    pub fn window(&mut self, from_ms: u64, len_ms: u64) -> Result<Vec<f32>, String> {
        let rate = u64::from(self.decoding.rate.max(1));
        let target = from_ms * rate / 1000;
        let end_pending = self.start_frame + self.pending.len() as u64;
        if target < self.start_frame || target > end_pending + SEEK_AFTER_MS * rate / 1000 {
            self.seek(from_ms)?;
        }
        let wanted = len_ms * rate / 1000;
        while !self.ended && self.start_frame + (self.pending.len() as u64) < target + wanted {
            self.decode_more()?;
            // Nothing before the window is needed.
            if self.start_frame < target {
                let drop = ((target - self.start_frame) as usize).min(self.pending.len());
                self.pending.drain(..drop);
                self.start_frame += drop as u64;
            }
        }
        let from = target.saturating_sub(self.start_frame) as usize;
        let to =
            ((target + wanted).saturating_sub(self.start_frame) as usize).min(self.pending.len());
        if from >= to {
            return Ok(Vec::new());
        }
        Ok(downsample(&self.pending[from..to], rate))
    }

    fn seek(&mut self, from_ms: u64) -> Result<(), String> {
        self.pending.clear();
        self.ended = false;
        let to = SeekTo::Time {
            time: Time::from_millis_u64(from_ms),
            track_id: Some(self.decoding.track_id),
        };
        match self.decoding.format.seek(SeekMode::Accurate, to) {
            Ok(seeked) => {
                self.decoding.decoder.reset();
                self.start_frame = self.frame_of(seeked.actual_ts).unwrap_or(0);
            }
            // Not seekable: read again from the start.
            Err(_) => {
                self.decoding = (self.opener)()?;
                self.start_frame = 0;
            }
        }
        Ok(())
    }

    /// Decodes one more packet onto `pending`.
    fn decode_more(&mut self) -> Result<(), String> {
        let packet = loop {
            match self.decoding.format.next_packet() {
                Ok(Some(packet)) if packet.track_id == self.decoding.track_id => break packet,
                Ok(Some(_)) => continue,
                Ok(None) => {
                    self.ended = true;
                    return Ok(());
                }
                Err(DecodeError::IoError(error))
                    if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    self.ended = true;
                    return Ok(());
                }
                Err(error) => return Err(error.to_string()),
            }
        };
        let packet_frame = self.frame_of(packet.pts);
        let buffer = match self.decoding.decoder.decode(&packet) {
            Ok(buffer) => buffer,
            // A damaged packet is skipped, as players do.
            Err(DecodeError::DecodeError(_)) => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        let channels = buffer.spec().channels().count().max(1);
        self.decoding.rate = buffer.spec().rate();
        let mut interleaved: Vec<f32> = vec![0.0; buffer.samples_interleaved()];
        buffer.copy_to_slice_interleaved(interleaved.as_mut_slice());
        if self.pending.is_empty()
            && let Some(frame) = packet_frame
        {
            self.start_frame = frame;
        }
        self.pending.extend(
            interleaved
                .chunks(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32),
        );
        Ok(())
    }

    fn frame_of(&self, ts: Timestamp) -> Option<u64> {
        let rate = u64::from(self.decoding.rate);
        match self.decoding.time_base {
            Some(base) => {
                let millis = base.calc_time(ts)?.as_millis();
                u64::try_from(millis)
                    .ok()
                    .map(|millis| millis * rate / 1000)
            }
            None => u64::try_from(ts.get()).ok(),
        }
    }
}

/// Decodes a whole source (an HLS segment) to 16 kHz mono.
pub fn decode_all(
    source: Box<dyn symphonia::core::io::MediaSource>,
    extension: Option<&str>,
) -> Result<Vec<f32>, String> {
    let decoding = open_source(source, extension)?;
    let mut reader = AudioReader {
        opener: Box::new(|| Err("a segment can't be read again".to_owned())),
        decoding,
        pending: Vec::new(),
        start_frame: 0,
        ended: false,
    };
    while !reader.ended {
        reader.decode_more()?;
    }
    let rate = u64::from(reader.decoding.rate.max(1));
    Ok(downsample(&reader.pending, rate))
}

/// Brings mono samples at `rate` to 16 kHz: each output sample is the mean of the input
/// samples it stands for, which also keeps higher frequencies from folding back.
pub fn downsample(samples: &[f32], rate: u64) -> Vec<f32> {
    if rate == TARGET_RATE {
        return samples.to_vec();
    }
    let out_len = (samples.len() as u64 * TARGET_RATE / rate) as usize;
    let step = rate as f64 / TARGET_RATE as f64;
    (0..out_len)
        .map(|index| {
            let from = (index as f64 * step) as usize;
            let to = (((index + 1) as f64 * step) as usize)
                .max(from + 1)
                .min(samples.len());
            if from >= to {
                return 0.0;
            }
            samples[from..to].iter().sum::<f32>() / (to - from) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downsampling_keeps_length_and_level() {
        let tone: Vec<f32> = (0..48_000).map(|_| 0.5).collect();
        let out = downsample(&tone, 48_000);
        assert_eq!(out.len(), 16_000);
        assert!(out.iter().all(|sample| (sample - 0.5).abs() < 1e-6));
        assert_eq!(downsample(&tone[..441], 44_100).len(), 160);
    }

    #[test]
    fn windows_come_from_where_they_were_asked() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("ramp.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 32_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        // Ten seconds; each second louder than the one before.
        for second in 0..10 {
            for _ in 0..32_000 {
                let level = (second * 3000) as i16;
                writer.write_sample(level).unwrap();
                writer.write_sample(level).unwrap();
            }
        }
        writer.finalize().unwrap();

        let mut reader = AudioReader::open(&path).unwrap();
        assert_eq!(reader.duration_ms(), Some(10_000));
        let level = |samples: &[f32]| samples[samples.len() / 2] * 32_768.0 / 3000.0;
        let first = reader.window(0, 2_000).unwrap();
        assert_eq!(first.len(), 32_000);
        assert!((level(&first[..16_000]) - 0.0).abs() < 0.01);
        let later = reader.window(7_000, 2_000).unwrap();
        assert!(
            (level(&later[..16_000]) - 7.0).abs() < 0.01,
            "{}",
            level(&later[..16_000])
        );
        // Back to the start seeks.
        let again = reader.window(1_000, 1_000).unwrap();
        assert!((level(&again) - 1.0).abs() < 0.01);
        // The last window is short, and past the end is empty.
        assert_eq!(reader.window(9_500, 2_000).unwrap().len(), 8_000);
        assert!(reader.window(10_000, 2_000).unwrap().is_empty());
    }
}
