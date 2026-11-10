//! Songs as files (M46.10): checking a download is audio and how long it is, writing it as WAV,
//! FLAC or MP3 (decoded with symphonia, written with hound, flacenc and LAME; a capture's sound
//! is encoded the same way, M46.49), and tagging saved files with lofty. All blocking: call from
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

/// Bits per sample written to WAV and FLAC from lossy sources and captures.
const BITS: u16 = 16;

/// What a saved file's tags say.
pub struct SongTags {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// The cover image's bytes.
    pub cover: Option<Vec<u8>>,
}

/// An open audio file and its decoder; captions read windows of it too (M46.32).
pub(crate) struct Decoding {
    pub(crate) format: Box<dyn FormatReader>,
    pub(crate) decoder: Box<dyn AudioDecoder>,
    pub(crate) track_id: u32,
    pub(crate) rate: u32,
    pub(crate) channels: usize,
    pub(crate) time_base: Option<symphonia::core::units::TimeBase>,
    /// The track's length, when the container says.
    pub(crate) duration_ms: Option<u64>,
}

pub(crate) fn open(path: &Path) -> Result<Decoding, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    open_source(
        Box::new(file),
        path.extension().and_then(|extension| extension.to_str()),
    )
}

/// Opens audio from any source (memory, an HTTP range reader), `extension` hinting its format.
pub(crate) fn open_source(
    source: Box<dyn symphonia::core::io::MediaSource>,
    extension: Option<&str>,
) -> Result<Decoding, String> {
    let stream = MediaSourceStream::new(source, Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = extension {
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
    let time_base = track.time_base;
    let duration_ms = time_base
        .zip(track.duration)
        .and_then(|(base, duration)| base.calc_duration(duration))
        .and_then(|time| u64::try_from(time.as_millis()).ok())
        .or_else(|| {
            let frames = track.num_frames?;
            let rate = u64::from(params.sample_rate?);
            (rate > 0).then(|| frames * 1000 / rate)
        });
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
        time_base,
        duration_ms,
    })
}

impl Decoding {
    /// The next decoded piece, interleaved, at 16 bits; `None` at the end.
    pub(crate) fn next(&mut self, samples: &mut Vec<i16>) -> Result<Option<()>, String> {
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

/// Whether Raygun only reads the container of a file with this extension (no decoder: it plays
/// in the platform's player).
fn container_only(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "opus" || extension == "webm")
}

/// How long a file says it is (ms), without decoding it; `None` when its container doesn't say.
pub fn length_ms(path: &Path) -> Option<u64> {
    let file = File::open(path).ok()?;
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
        .ok()?;
    let track = format.default_track(TrackType::Audio)?;
    let rate = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .and_then(|params| params.sample_rate);
    track
        .time_base
        .zip(track.duration)
        .and_then(|(base, duration)| base.calc_duration(duration))
        .and_then(|time| u64::try_from(time.as_millis()).ok())
        .or_else(|| {
            let frames = track.num_frames?;
            let rate = u64::from(rate?);
            (rate > 0).then(|| frames * 1000 / rate)
        })
}

/// How long a song file plays (ms), checking that it decodes (M46.49): what its container says,
/// else counted by decoding it whole (a joined stream's container doesn't say). Opus and WebM
/// only by their container. An error when it doesn't decode.
pub fn measure(path: &Path) -> Result<Option<u64>, String> {
    if container_only(path) {
        return Ok(length_ms(path));
    }
    probe(path)?;
    if let Some(ms) = length_ms(path).filter(|ms| *ms > 0) {
        return Ok(Some(ms));
    }
    let mut decoding = open(path)?;
    let mut samples = Vec::new();
    let mut ms = 0f64;
    while decoding.next(&mut samples)?.is_some() {
        let frames = samples.len() / decoding.channels.max(1);
        ms += frames as f64 * 1000.0 / f64::from(decoding.rate.max(1));
    }
    if ms <= 0.0 {
        return Err("it has no sound".into());
    }
    Ok(Some(ms.round() as u64))
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

    let mut samples = Vec::new();
    // The first piece fixes the rate and channels.
    let first = decoding.next(&mut samples)?;
    let mut encoder = Mp3Encoder::new(decoding.rate, decoding.channels, kbps)?;
    let file = File::create(path).map_err(|error| error.to_string())?;
    let mut out = std::io::BufWriter::new(file);
    if first.is_some() {
        loop {
            encoder.encode(&samples, &mut out)?;
            if decoding.next(&mut samples)?.is_none() {
                break;
            }
        }
    }
    encoder.finish(&mut out)?;
    out.flush().map_err(|error| error.to_string())
}

/// LAME at a constant bitrate, fed interleaved 16-bit samples (exported videos' sound too).
pub(crate) struct Mp3Encoder {
    encoder: mp3lame_encoder::Encoder,
    channels: usize,
    encoded: Vec<u8>,
    pair: Vec<i16>,
}

fn lame_failed(error: &dyn std::fmt::Display) -> String {
    format!("the MP3 encoder failed ({error})")
}

impl Mp3Encoder {
    pub(crate) fn new(rate: u32, channels: usize, kbps: u32) -> Result<Self, String> {
        use mp3lame_encoder::{Builder, Quality};

        let channels = channels.max(1);
        let mut builder = Builder::new().ok_or("the MP3 encoder didn't start")?;
        builder
            .set_num_channels(if channels == 1 { 1 } else { 2 })
            .map_err(|error| lame_failed(&error))?;
        builder
            .set_sample_rate(rate)
            .map_err(|error| lame_failed(&error))?;
        builder
            .set_brate(lame_bitrate(kbps))
            .map_err(|error| lame_failed(&error))?;
        // LAME's recommended quality for its speed.
        builder
            .set_quality(Quality::NearBest)
            .map_err(|error| lame_failed(&error))?;
        // The VBR/Info frame would have to be written back over the file's start.
        builder
            .set_to_write_vbr_tag(false)
            .map_err(|error| lame_failed(&error))?;
        Ok(Self {
            encoder: builder.build().map_err(|error| lame_failed(&error))?,
            channels,
            encoded: Vec::new(),
            pair: Vec::new(),
        })
    }

    /// Encodes interleaved samples (as many channels as it was made with).
    pub(crate) fn encode(
        &mut self,
        samples: &[i16],
        out: &mut impl std::io::Write,
    ) -> Result<(), String> {
        use mp3lame_encoder::{InterleavedPcm, MonoPcm};

        self.encoded.clear();
        if self.channels == 1 {
            self.encoded
                .reserve(mp3lame_encoder::max_required_buffer_size(samples.len()));
            self.encoder
                .encode_to_vec(MonoPcm(samples), &mut self.encoded)
                .map_err(|error| lame_failed(&error))?;
        } else {
            let frames: &[i16] = if self.channels == 2 {
                samples
            } else {
                self.pair.clear();
                self.pair.extend(
                    samples
                        .chunks_exact(self.channels)
                        .flat_map(|frame| [frame[0], frame[1]]),
                );
                &self.pair
            };
            self.encoded
                .reserve(mp3lame_encoder::max_required_buffer_size(frames.len() / 2));
            self.encoder
                .encode_to_vec(InterleavedPcm(frames), &mut self.encoded)
                .map_err(|error| lame_failed(&error))?;
        }
        out.write_all(&self.encoded)
            .map_err(|error| error.to_string())
    }

    /// The last frames.
    pub(crate) fn finish(mut self, out: &mut impl std::io::Write) -> Result<(), String> {
        use mp3lame_encoder::FlushNoGap;

        self.encoded.clear();
        self.encoded.reserve(7200);
        self.encoder
            .flush_to_vec::<FlushNoGap>(&mut self.encoded)
            .map_err(|error| lame_failed(&error))?;
        out.write_all(&self.encoded)
            .map_err(|error| error.to_string())
    }
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
    fn songs_are_measured_by_what_they_play() {
        let temp = tempfile::TempDir::new().unwrap();
        let wav = temp.path().join("track.wav");
        tone(&wav);
        assert_eq!(measure(&wav).unwrap(), Some(3_000));
        assert_eq!(length_ms(&wav), Some(3_000));
        // An MP3 joined from pieces has no length of its own; it's counted.
        let (mp3, _) =
            write_file(&wav, AudioFileFormat::Mp3, &temp.path().join("joined"), 128).unwrap();
        let measured = measure(&mp3).unwrap().unwrap();
        assert!((2_900..3_200).contains(&measured), "{measured} ms");
        let page = temp.path().join("page.aac");
        std::fs::write(&page, b"<html>Not found</html>").unwrap();
        assert!(measure(&page).is_err());
    }
}
