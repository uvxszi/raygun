//! A video's sound as MP3: MP3 frames are copied as they are; AAC is decoded (symphonia) and
//! encoded with LAME, as songs saved as MP3 are.

use std::io::Write;
use std::sync::atomic::AtomicBool;

use symphonia::core::codecs::audio::well_known::CODEC_ID_AAC;
use symphonia::core::codecs::audio::{AudioCodecParameters, AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as DecodeError;
use symphonia::core::packet::Packet as Coded;
use symphonia::core::units::{Duration, Timestamp};

use super::plan::Picker;
use super::{Codec, Error, Result, Source, check_cancel, codec};
use crate::music_downloads::transcode::Mp3Encoder;

/// Decodes AAC frames and encodes what they play.
struct Reencode {
    decoder: Box<dyn AudioDecoder>,
    encoder: Option<Mp3Encoder>,
    kbps: u32,
    samples: Vec<i16>,
}

impl Reencode {
    fn new(config: &[u8], kbps: u32) -> Result<Self> {
        let (_, rate, channels) = codec::parse_audio_config(config)
            .ok_or_else(|| Error::Unsupported("Its sound's settings are unreadable".into()))?;
        let mut params = AudioCodecParameters::new();
        params.for_codec(CODEC_ID_AAC);
        params.with_sample_rate(rate);
        if channels > 0 {
            params.with_channels(symphonia::core::audio::Channels::Discrete(u16::from(
                channels,
            )));
        }
        params.with_extra_data(config.to_vec().into_boxed_slice());
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &AudioDecoderOptions::default())
            .map_err(|error| Error::Unsupported(format!("Its sound can't be read ({error})")))?;
        Ok(Self {
            decoder,
            encoder: None,
            kbps,
            samples: Vec::new(),
        })
    }

    fn push(&mut self, data: &[u8], out: &mut dyn Write) -> Result<()> {
        let packet = Coded::new(0, Timestamp::new(0), Duration::new(1024), data.to_vec());
        let buffer = match self.decoder.decode(&packet) {
            Ok(buffer) => buffer,
            // A damaged frame is skipped, as players do.
            Err(DecodeError::DecodeError(_)) => return Ok(()),
            Err(error) => return Err(Error::Invalid(format!("Its sound can't be read ({error})"))),
        };
        let rate = buffer.spec().rate();
        let channels = buffer.spec().channels().count();
        self.samples.resize(buffer.samples_interleaved(), 0);
        buffer.copy_to_slice_interleaved(self.samples.as_mut_slice());
        let encoder = match &mut self.encoder {
            Some(encoder) => encoder,
            None => self
                .encoder
                .insert(Mp3Encoder::new(rate, channels, self.kbps).map_err(Error::Unsupported)?),
        };
        encoder
            .encode(&self.samples, &mut Adapter(out))
            .map_err(|error| Error::Io(std::io::Error::other(error)))
    }

    fn finish(self, out: &mut dyn Write) -> Result<()> {
        match self.encoder {
            Some(encoder) => encoder
                .finish(&mut Adapter(out))
                .map_err(|error| Error::Io(std::io::Error::other(error))),
            None => Ok(()),
        }
    }
}

/// A `dyn Write` where an `impl Write` is wanted.
struct Adapter<'a>(&'a mut dyn Write);

impl Write for Adapter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

/// Writes the source's first sound stream as MP3 (re-encoded at `kbps` unless it's MP3).
pub fn write(source: &Source, kbps: u32, out: &mut dyn Write, cancel: &AtomicBool) -> Result<()> {
    let mut demux = source.open()?;
    let mut picker = Picker::new(true);
    let mut reencode: Option<Reencode> = None;
    let mut written = false;
    let mut count = 0usize;
    while let Some(packet) = demux.next()? {
        count += 1;
        check_cancel(cancel, count)?;
        if picker.pick(demux.tracks(), packet.track).is_none() {
            continue;
        }
        match &demux.tracks()[packet.track].codec {
            Codec::MpegAudio { layer: 3, .. } => out.write_all(&packet.data)?,
            Codec::Aac(config) => {
                let reencode = match &mut reencode {
                    Some(reencode) => reencode,
                    None => reencode.insert(Reencode::new(config, kbps)?),
                };
                reencode.push(&packet.data, out)?;
            }
            other => {
                return Err(Error::Unsupported(format!(
                    "Its sound is {}, which Raygun can't turn into MP3",
                    other.name()
                )));
            }
        }
        written = true;
    }
    if let Some(reencode) = reencode {
        reencode.finish(out)?;
    }
    if !written {
        return Err(Error::Unsupported("This video has no sound to save".into()));
    }
    Ok(())
}
