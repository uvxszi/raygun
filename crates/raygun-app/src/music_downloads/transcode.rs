//! Songs as files (M46.10): checking a download is audio, writing it as WAV, FLAC or MP3
//! (decoded with symphonia, written with hound, flacenc and LAME), tagging saved files with
//! lofty, and turning a recording's capture into its FLAC. All blocking: call from
//! `spawn_blocking`.

use std::fs::File;
use std::path::Path;

use flacenc::component::BitRepr;
use flacenc::error::Verify;
use raygun_model::AudioFileFormat;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as DecodeError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

/// Bits per sample written to WAV and FLAC from lossy sources and recordings.
const BITS: u16 = 16;
/// Recordings: below this level a sample counts as silence when trimming the ends.
const SILENCE: f32 = 0.0015;
/// Recordings are raised to this peak (the player's volume may have been low).
const TARGET_PEAK: f32 = 0.97;

/// What a saved file's tags say.
pub struct SongTags {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// The cover image's bytes.
    pub cover: Option<Vec<u8>>,
}

struct Decoding {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    rate: u32,
    channels: usize,
}

fn open(path: &Path) -> Result<Decoding, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        hint.with_extension(extension);
    }
    let format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|error| format!("not a file Raygun can read ({error})"))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or("there's no sound in it")?;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or("its sound can't be read")?
        .clone();
    let track_id = track.id;
    let decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|error| format!("its sound can't be read ({error})"))?;
    Ok(Decoding {
        format,
        decoder,
        track_id,
        rate: params.sample_rate.unwrap_or(44_100),
        channels: params
            .channels
            .as_ref()
            .map_or(2, |channels| channels.count()),
    })
}

impl Decoding {
    /// The next decoded piece, interleaved, at 16 bits; `None` at the end.
    fn next(&mut self, samples: &mut Vec<i16>) -> Result<Option<()>, String> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => return Ok(None),
                Err(DecodeError::IoError(error))
                    if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(None);
                }
                Err(error) => return Err(error.to_string()),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(buffer) => {
                    self.rate = buffer.spec().rate();
                    self.channels = buffer.spec().channels().count();
                    samples.resize(buffer.samples_interleaved(), 0);
                    buffer.copy_to_slice_interleaved(samples.as_mut_slice());
                    return Ok(Some(()));
                }
                // A damaged packet is skipped, as players do.
                Err(DecodeError::DecodeError(_)) => continue,
                Err(error) => return Err(error.to_string()),
            }
        }
    }
}

/// Whether `path` is audio Raygun can read (decodes its first sound).
pub fn probe(path: &Path) -> Result<(), String> {
    // Opus has no decoder here, but plays in the webview: its container is enough.
    if path
        .extension()
        .is_some_and(|extension| extension == "opus" || extension == "webm")
    {
        return Ok(());
    }
    let mut decoding = open(path)?;
    let mut samples = Vec::new();
    decoding
        .next(&mut samples)?
        .ok_or_else(|| "it has no sound".to_owned())
}

/// Writes `source` to a file at `target` (its extension added) in `format`, MP3s at `mp3_kbps`;
/// returns the file and its extension.
pub fn write_file(
    source: &Path,
    format: AudioFileFormat,
    target: &Path,
    mp3_kbps: u32,
) -> Result<(std::path::PathBuf, String), String> {
    let source_extension = source
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("mp3")
        .to_ascii_lowercase();
    let extension = match format {
        AudioFileFormat::Original => source_extension.clone(),
        AudioFileFormat::Wav => "wav".to_owned(),
        AudioFileFormat::Flac => "flac".to_owned(),
        AudioFileFormat::Mp3 => "mp3".to_owned(),
    };
    let path = target.with_extension(&extension);
    if extension == source_extension {
        std::fs::copy(source, &path).map_err(|error| error.to_string())?;
        return Ok((path, extension));
    }
    let mut decoding = open(source)?;
    match format {
        AudioFileFormat::Wav => write_wav(&mut decoding, &path)?,
        AudioFileFormat::Mp3 => write_mp3(&mut decoding, &path, mp3_kbps)?,
        _ => write_flac(&mut decoding, &path)?,
    }
    Ok((path, extension))
}

/// The LAME bitrate nearest at or above `kbps`, up to 320.
fn lame_bitrate(kbps: u32) -> mp3lame_encoder::Bitrate {
    use mp3lame_encoder::Bitrate;
    match kbps {
        0..=64 => Bitrate::Kbps64,
        65..=96 => Bitrate::Kbps96,
        97..=128 => Bitrate::Kbps128,
        129..=160 => Bitrate::Kbps160,
        161..=192 => Bitrate::Kbps192,
        193..=256 => Bitrate::Kbps256,
        _ => Bitrate::Kbps320,
    }
}

/// Encodes to a constant-bitrate MP3 (M46.18). Mono stays mono; more than two channels keep the
/// front pair.
fn write_mp3(decoding: &mut Decoding, path: &Path, kbps: u32) -> Result<(), String> {
    use std::io::Write;

    use mp3lame_encoder::{Builder, FlushNoGap, InterleavedPcm, MonoPcm, Quality};

    let failed = |error: &dyn std::fmt::Display| format!("the MP3 encoder failed ({error})");
    let mut samples = Vec::new();
    // The first piece fixes the rate and channels.
    let first = decoding.next(&mut samples)?;
    let channels = decoding.channels.max(1);
    let mono = channels == 1;
    let mut builder = Builder::new().ok_or("the MP3 encoder didn't start")?;
    builder
        .set_num_channels(if mono { 1 } else { 2 })
        .map_err(|error| failed(&error))?;
    builder
        .set_sample_rate(decoding.rate)
        .map_err(|error| failed(&error))?;
    builder
        .set_brate(lame_bitrate(kbps))
        .map_err(|error| failed(&error))?;
    // LAME's recommended quality for its speed.
    builder
        .set_quality(Quality::NearBest)
        .map_err(|error| failed(&error))?;
    // The VBR/Info frame would have to be written back over the file's start.
    builder
        .set_to_write_vbr_tag(false)
        .map_err(|error| failed(&error))?;
    let mut encoder = builder.build().map_err(|error| failed(&error))?;
    let file = File::create(path).map_err(|error| error.to_string())?;
    let mut out = std::io::BufWriter::new(file);
    let mut encoded = Vec::new();
    let mut pair = Vec::new();
    if first.is_some() {
        loop {
            encoded.clear();
            if mono {
                encoded.reserve(mp3lame_encoder::max_required_buffer_size(samples.len()));
                encoder
                    .encode_to_vec(MonoPcm(samples.as_slice()), &mut encoded)
                    .map_err(|error| failed(&error))?;
            } else {
                let frames: &[i16] = if channels == 2 {
                    &samples
                } else {
                    pair.clear();
                    pair.extend(
                        samples
                            .chunks_exact(channels)
                            .flat_map(|frame| [frame[0], frame[1]]),
                    );
                    &pair
                };
                encoded.reserve(mp3lame_encoder::max_required_buffer_size(frames.len() / 2));
                encoder
                    .encode_to_vec(InterleavedPcm(frames), &mut encoded)
                    .map_err(|error| failed(&error))?;
            }
            out.write_all(&encoded).map_err(|error| error.to_string())?;
            if decoding.next(&mut samples)?.is_none() {
                break;
            }
        }
    }
    encoded.clear();
    encoded.reserve(7200);
    encoder
        .flush_to_vec::<FlushNoGap>(&mut encoded)
        .map_err(|error| failed(&error))?;
    out.write_all(&encoded).map_err(|error| error.to_string())?;
    out.flush().map_err(|error| error.to_string())
}

fn write_wav(decoding: &mut Decoding, path: &Path) -> Result<(), String> {
    let mut samples = Vec::new();
    // The first piece fixes the rate and channels.
    let first = decoding.next(&mut samples)?;
    let spec = hound::WavSpec {
        channels: decoding.channels as u16,
        sample_rate: decoding.rate,
        bits_per_sample: BITS,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(|error| error.to_string())?;
    if first.is_some() {
        loop {
            for sample in &samples {
                writer
                    .write_sample(*sample)
                    .map_err(|error| error.to_string())?;
            }
            if decoding.next(&mut samples)?.is_none() {
                break;
            }
        }
    }
    writer.finalize().map_err(|error| error.to_string())
}

/// Decoded samples handed to flacenc a block at a time.
struct DecodedSource<'a> {
    decoding: &'a mut Decoding,
    pending: Vec<i32>,
    piece: Vec<i16>,
    ended: bool,
}

impl flacenc::source::Source for DecodedSource<'_> {
    fn channels(&self) -> usize {
        self.decoding.channels
    }

    fn bits_per_sample(&self) -> usize {
        usize::from(BITS)
    }

    fn sample_rate(&self) -> usize {
        self.decoding.rate as usize
    }

    fn read_samples<F: flacenc::source::Fill>(
        &mut self,
        block_size: usize,
        dest: &mut F,
    ) -> Result<usize, flacenc::error::SourceError> {
        let wanted = block_size * self.decoding.channels;
        while self.pending.len() < wanted && !self.ended {
            match self.decoding.next(&mut self.piece) {
                Ok(Some(())) => self
                    .pending
                    .extend(self.piece.iter().map(|sample| i32::from(*sample))),
                Ok(None) => self.ended = true,
                Err(error) => {
                    return Err(flacenc::error::SourceError::from_io_error(
                        std::io::Error::other(error),
                    ));
                }
            }
        }
        let take = wanted.min(self.pending.len());
        let block: Vec<i32> = self.pending.drain(..take).collect();
        dest.fill_interleaved(&block)?;
        Ok(take / self.decoding.channels.max(1))
    }
}

fn write_flac(decoding: &mut Decoding, path: &Path) -> Result<(), String> {
    // Reading one piece first fixes the rate and channels the stream header carries.
    let mut piece = Vec::new();
    let mut pending = Vec::new();
    let mut ended = false;
    match decoding.next(&mut piece)? {
        Some(()) => pending.extend(piece.iter().map(|sample| i32::from(*sample))),
        None => ended = true,
    }
    let source = DecodedSource {
        decoding,
        pending,
        piece,
        ended,
    };
    encode_flac(source, path)
}

fn encode_flac<S: flacenc::source::Source>(source: S, path: &Path) -> Result<(), String> {
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|(_, error)| format!("{error:?}"))?;
    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|error| format!("{error:?}"))?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|error| format!("{error:?}"))?;
    let mut bytes = sink.as_slice().to_vec();
    fix_min_block_size(&mut bytes);
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}

/// flacenc writes the short last block's size as the stream's minimum block size; the FLAC
/// format leaves the last block out of it, and readers (symphonia) take a minimum below the
/// maximum to mean variable-sized blocks, then misread the frames. Every block but the last is
/// the same size here, so the minimum is the maximum.
fn fix_min_block_size(bytes: &mut [u8]) {
    // "fLaC", then STREAMINFO's 4-byte header (type 0); min and max block sizes come first.
    if bytes.len() >= 12 && &bytes[..4] == b"fLaC" && bytes[4] & 0x7f == 0 {
        let max = [bytes[10], bytes[11]];
        bytes[8..10].copy_from_slice(&max);
    }
}

/// Turns a recording's float WAV into FLAC: silence trimmed from both ends, raised to a steady
/// peak, 16 bits. Returns how long the kept sound is, in ms.
pub fn finish_recording(wav: &Path, flac: &Path) -> Result<u64, String> {
    let mut reader = hound::WavReader::open(wav).map_err(|error| error.to_string())?;
    let spec = reader.spec();
    let channels = usize::from(spec.channels.max(1));
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|error| error.to_string())?,
        hound::SampleFormat::Int => reader
            .samples::<i32>()
            .map(|sample| {
                sample.map(|value| value as f32 / (1u32 << (spec.bits_per_sample - 1)) as f32)
            })
            .collect::<Result<_, _>>()
            .map_err(|error| error.to_string())?,
    };
    let kept = trim_silence(&samples, channels);
    if kept.is_empty() {
        return Err("Nothing was heard while the song played".into());
    }
    let peak = kept
        .iter()
        .fold(0f32, |peak, sample| peak.max(sample.abs()));
    let gain = if peak > 0.0 { TARGET_PEAK / peak } else { 1.0 };
    let scale = f32::from(i16::MAX);
    let pcm: Vec<i32> = kept
        .iter()
        .map(|sample| ((sample * gain).clamp(-1.0, 1.0) * scale).round() as i32)
        .collect();
    let frames = pcm.len() / channels;
    let source = flacenc::source::MemSource::from_samples(
        &pcm,
        channels,
        usize::from(BITS),
        spec.sample_rate as usize,
    );
    encode_flac(source, flac)?;
    Ok(frames as u64 * 1000 / u64::from(spec.sample_rate.max(1)))
}

/// The samples between the first and last sound, whole frames only.
fn trim_silence(samples: &[f32], channels: usize) -> &[f32] {
    let loud = |sample: &f32| sample.abs() > SILENCE;
    let Some(first) = samples.iter().position(loud) else {
        return &[];
    };
    let last = samples.iter().rposition(loud).unwrap_or(first);
    let start = first - first % channels;
    let end = ((last / channels) + 1) * channels;
    &samples[start..end.min(samples.len())]
}

/// Writes title, artist, album and cover into a saved file, in the tags its format uses.
pub fn tag(path: &Path, tags: &SongTags) -> Result<(), String> {
    use lofty::config::WriteOptions;
    use lofty::picture::{MimeType, Picture, PictureType};
    use lofty::prelude::*;
    use lofty::tag::Tag;

    let mut file = lofty::read_from_path(path).map_err(|error| error.to_string())?;
    if file.primary_tag().is_none() {
        let kind = file.primary_tag_type();
        file.insert_tag(Tag::new(kind));
    }
    let tag = file.primary_tag_mut().ok_or("the file takes no tags")?;
    tag.set_title(tags.title.clone());
    if let Some(artist) = &tags.artist {
        tag.set_artist(artist.clone());
    }
    if let Some(album) = &tags.album {
        tag.set_album(album.clone());
    }
    if let Some(cover) = &tags.cover {
        let mime = if cover.starts_with(&[0x89, b'P', b'N', b'G']) {
            MimeType::Png
        } else {
            MimeType::Jpeg
        };
        tag.push_picture(
            Picture::unchecked(cover.clone())
                .pic_type(PictureType::CoverFront)
                .mime_type(mime)
                .build(),
        );
    }
    file.save_to_path(path, WriteOptions::default())
        .map_err(|error| error.to_string())
}

/// The media type of a song file by its extension.
pub fn mime(extension: &str) -> &'static str {
    match extension {
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "flac" => "audio/flac",
        "ogg" => "audio/ogg",
        "opus" => "audio/opus",
        "wav" => "audio/wav",
        "webm" => "audio/webm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A second of a 440 Hz tone with silence around it, as a 16-bit WAV.
    fn tone(path: &Path) {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for frame in 0..132_300u32 {
            let sound = (22_050..110_250).contains(&frame);
            let value = if sound {
                ((frame as f32 / 44_100.0 * 440.0 * std::f32::consts::TAU).sin() * 8_000.0) as i16
            } else {
                0
            };
            writer.write_sample(value).unwrap();
            writer.write_sample(value).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn songs_become_wav_and_flac_with_tags() {
        let temp = tempfile::TempDir::new().unwrap();
        let source = temp.path().join("track.wav");
        tone(&source);
        probe(&source).unwrap();

        let (flac, extension) = write_file(
            &source,
            AudioFileFormat::Flac,
            &temp.path().join("out"),
            320,
        )
        .unwrap();
        assert_eq!(extension, "flac");
        probe(&flac).unwrap();
        tag(
            &flac,
            &SongTags {
                title: "Tone".into(),
                artist: Some("Raygun".into()),
                album: None,
                cover: None,
            },
        )
        .unwrap();
        use lofty::prelude::*;
        let read = lofty::read_from_path(&flac).unwrap();
        assert_eq!(read.primary_tag().unwrap().title().as_deref(), Some("Tone"));

        // FLAC back to WAV keeps the length.
        let (wav, _) =
            write_file(&flac, AudioFileFormat::Wav, &temp.path().join("back"), 320).unwrap();
        let reader = hound::WavReader::open(&wav).unwrap();
        assert_eq!(reader.duration(), 132_300);

        // MP3 (M46.18): decodable, tagged, about as long, and smaller at a lower bitrate.
        let (mp3, extension) =
            write_file(&flac, AudioFileFormat::Mp3, &temp.path().join("mp3"), 320).unwrap();
        assert_eq!(extension, "mp3");
        probe(&mp3).unwrap();
        tag(
            &mp3,
            &SongTags {
                title: "Tone".into(),
                artist: None,
                album: None,
                cover: None,
            },
        )
        .unwrap();
        let read = lofty::read_from_path(&mp3).unwrap();
        assert_eq!(read.primary_tag().unwrap().title().as_deref(), Some("Tone"));
        let seconds = read.properties().duration().as_secs_f64();
        assert!((2.9..3.2).contains(&seconds), "{seconds} s");
        let (small, _) =
            write_file(&flac, AudioFileFormat::Mp3, &temp.path().join("small"), 64).unwrap();
        assert!(std::fs::metadata(&small).unwrap().len() < std::fs::metadata(&mp3).unwrap().len());

        // The same format is a copy.
        let (copy, extension) = write_file(
            &source,
            AudioFileFormat::Original,
            &temp.path().join("copy"),
            320,
        )
        .unwrap();
        assert_eq!(extension, "wav");
        assert_eq!(
            std::fs::read(copy).unwrap(),
            std::fs::read(&source).unwrap()
        );

        assert!(probe(&temp.path().join("missing.mp3")).is_err());
        let page = temp.path().join("page.mp3");
        std::fs::write(&page, b"<html>Not found</html>").unwrap();
        assert!(probe(&page).is_err(), "an error page isn't a song");
    }

    #[test]
    fn recordings_lose_their_silent_ends_and_get_louder() {
        let temp = tempfile::TempDir::new().unwrap();
        let wav = temp.path().join("capture.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 8_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&wav, spec).unwrap();
        for frame in 0..24_000u32 {
            let value = if (8_000..16_000).contains(&frame) {
                (frame as f32 / 8_000.0 * 440.0 * std::f32::consts::TAU).sin() * 0.2
            } else {
                0.0
            };
            writer.write_sample(value).unwrap();
            writer.write_sample(value).unwrap();
        }
        writer.finalize().unwrap();
        let flac = temp.path().join("track.flac");
        let kept = finish_recording(&wav, &flac).unwrap();
        assert!(
            (990..=1000).contains(&kept),
            "about the 1 s of sound: {kept}"
        );
        let (back, _) =
            write_file(&flac, AudioFileFormat::Wav, &temp.path().join("back"), 320).unwrap();
        let samples: Vec<i16> = hound::WavReader::open(back)
            .unwrap()
            .samples::<i16>()
            .map(Result::unwrap)
            .collect();
        let peak = samples
            .iter()
            .map(|sample| sample.unsigned_abs())
            .max()
            .unwrap();
        assert!(peak > 30_000, "raised from 0.2 to near full scale: {peak}");
        assert!(trim_silence(&[0.0; 8], 2).is_empty());
    }
}
