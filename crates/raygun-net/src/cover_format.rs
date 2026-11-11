//! Covers kept as JPEG XL: the cover cache stores each picture converted once, in the background,
//! and that file is the cache ([`crate::cover_cache`]).
//!
//! JPEGs (nearly every cover) are recompressed losslessly: the JPEG XL keeps the JPEG's own DCT
//! coefficients plus what rebuilds the original file byte for byte (its `jbrd` box), and each one
//! is rebuilt and compared before it replaces the original. Pictures from lossless formats (PNG
//! and GIF icons, lossless WebP) are stored losslessly. Other lossy ones (WebP thumbnails,
//! progressive JPEGs) are re-encoded at distance 1, visually the same, and only when that saves a
//! good share. Animations, other formats and very large pictures stay as they came.
//!
//! A webview that can't show JPEG XL gets [`displayable`]: the original JPEG rebuilt, else the
//! pixels as PNG (lossless or transparent pictures) or JPEG.
//!
//! Encoding is jxl-encoder (pure Rust, AGPL-3.0); decoding and rebuilding JPEGs is jxl-oxide.

use std::io::Cursor;

use image::{DynamicImage, ImageEncoder, ImageFormat};
use jxl_oxide::{JpegReconstructionStatus, JxlImage};

use crate::images::is_jxl;

/// The type of a stored JPEG XL, for webviews that show it.
pub const JXL_TYPE: &str = "image/jxl";

/// Pictures re-encoded from their pixels stay under this (time and memory on phones).
const MAX_PIXELS: u64 = 2_500_000;
/// JPEGs recompressed stay under this: their coefficients are held, not pixels.
const MAX_JPEG_PIXELS: u64 = 25_000_000;
/// Butteraugli distance for pictures whose source was already lossy: visually the same.
const LOSSY_DISTANCE: f32 = 1.0;
/// Effort for lossy pixel encodes: higher ones were no smaller on covers, only slower.
const LOSSY_EFFORT: u8 = 5;
/// Effort for lossless ones: icons and logos need effort 7's learned trees to beat PNG (half the
/// size where effort 5 was larger than the PNG), affordable up to [`LOSSLESS_SMALL_PIXELS`].
const LOSSLESS_EFFORT: u8 = 7;
const LOSSLESS_EFFORT_LARGE: u8 = 5;
const LOSSLESS_SMALL_PIXELS: u64 = 1 << 20;
/// A lossy re-encode adds a generation, so it's kept only when it saves at least this share.
const LOSSY_MIN_SAVING: f64 = 0.1;
/// Quality of the JPEG a lossy JPEG XL is shown as where JPEG XL can't be shown.
const SHOWN_JPEG_QUALITY: u8 = 90;

/// How a JPEG XL was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// A JPEG's own coefficients; the original file is rebuilt byte for byte.
    Recompressed,
    /// Pixels kept exactly (PNG, GIF, lossless WebP).
    Lossless,
    /// Pixels at distance 1 (lossy WebP, JPEGs that can't be recompressed).
    Lossy,
}

/// What converting a cover gave.
#[derive(Debug)]
pub enum Conversion {
    /// The JPEG XL that replaces the original.
    Jxl { bytes: Vec<u8>, method: Method },
    /// The original stays, and why.
    Original(String),
}

/// What a picture is, as far as converting goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Jpeg,
    Png,
    Gif,
    /// `lossless`: a VP8L picture.
    WebP {
        lossless: bool,
        animated: bool,
    },
}

impl Source {
    fn of(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [0xFF, 0xD8, 0xFF, ..] => Some(Self::Jpeg),
            [0x89, b'P', b'N', b'G', ..] => Some(Self::Png),
            [b'G', b'I', b'F', b'8', ..] => Some(Self::Gif),
            [
                b'R',
                b'I',
                b'F',
                b'F',
                _,
                _,
                _,
                _,
                b'W',
                b'E',
                b'B',
                b'P',
                ..,
            ] => webp(bytes),
            _ => None,
        }
    }

    fn format(self) -> ImageFormat {
        match self {
            Self::Jpeg => ImageFormat::Jpeg,
            Self::Png => ImageFormat::Png,
            Self::Gif => ImageFormat::Gif,
            Self::WebP { .. } => ImageFormat::WebP,
        }
    }

    /// Whether the source keeps pixels exactly, so the JPEG XL should too.
    fn lossless(self) -> bool {
        match self {
            Self::Png | Self::Gif => true,
            Self::WebP { lossless, .. } => lossless,
            Self::Jpeg => false,
        }
    }
}

/// A WebP's kind from its chunks: lossy (`VP8 `), lossless (`VP8L`), animated (`VP8X` flag).
fn webp(bytes: &[u8]) -> Option<Source> {
    const ANIMATED: Source = Source::WebP {
        lossless: false,
        animated: true,
    };
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let kind = &bytes[at..at + 4];
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().ok()?) as usize;
        match kind {
            b"VP8X" if bytes.get(at + 8).is_some_and(|flags| flags & 0x02 != 0) => {
                return Some(ANIMATED);
            }
            b"ANMF" => return Some(ANIMATED),
            b"VP8 " => {
                return Some(Source::WebP {
                    lossless: false,
                    animated: false,
                });
            }
            b"VP8L" => {
                return Some(Source::WebP {
                    lossless: true,
                    animated: false,
                });
            }
            _ => {}
        }
        // Chunks are padded to an even size.
        at = at.checked_add(8)?.checked_add(size + (size & 1))?;
    }
    None
}

/// Whether a PNG has an animation (APNG's `acTL` before the image data).
fn png_animated(bytes: &[u8]) -> bool {
    let mut at = 8;
    while at + 8 <= bytes.len() {
        let size = u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
        match &bytes[at + 4..at + 8] {
            b"acTL" => return true,
            b"IDAT" => return false,
            _ => {}
        }
        at = at.saturating_add(12).saturating_add(size as usize);
    }
    false
}

fn gif_animated(bytes: &[u8]) -> bool {
    use image::AnimationDecoder;
    image::codecs::gif::GifDecoder::new(Cursor::new(bytes))
        .map(|decoder| decoder.into_frames().take(2).count() > 1)
        .unwrap_or(false)
}

fn animated(bytes: &[u8], source: Source) -> bool {
    match source {
        Source::Png => png_animated(bytes),
        Source::Gif => gif_animated(bytes),
        Source::WebP { animated, .. } => animated,
        Source::Jpeg => false,
    }
}

/// Converts a cover for storing; never fails, only keeps the original with a reason.
pub fn convert(original: &[u8]) -> Conversion {
    let keep = |reason: &str| Conversion::Original(reason.to_owned());
    if is_jxl(original) {
        return keep("already JPEG XL");
    }
    let Some(source) = Source::of(original) else {
        return keep("not a format that's converted");
    };
    if animated(original, source) {
        return keep("animated");
    }
    let dimensions =
        image::ImageReader::with_format(Cursor::new(original), source.format()).into_dimensions();
    let Ok((width, height)) = dimensions else {
        return keep("not a readable picture");
    };
    let pixels = u64::from(width) * u64::from(height);
    if source == Source::Jpeg && pixels <= MAX_JPEG_PIXELS {
        match recompress(original) {
            Ok(bytes) if bytes.len() < original.len() => {
                return Conversion::Jxl {
                    bytes,
                    method: Method::Recompressed,
                };
            }
            Ok(_) => return keep("the JPEG XL wasn't smaller"),
            // Progressive, CMYK or unusual JPEGs: from their pixels below.
            Err(error) => tracing::debug!(%error, "JPEG not recompressed"),
        }
    }
    if pixels > MAX_PIXELS {
        return keep("too large to re-encode");
    }
    let effort = match source.lossless() {
        true if pixels <= LOSSLESS_SMALL_PIXELS => LOSSLESS_EFFORT,
        true => LOSSLESS_EFFORT_LARGE,
        false => LOSSY_EFFORT,
    };
    match encode_pixels(original, source, effort) {
        Ok((bytes, method)) => {
            let enough = match method {
                Method::Lossy => {
                    (bytes.len() as f64) <= original.len() as f64 * (1.0 - LOSSY_MIN_SAVING)
                }
                Method::Lossless | Method::Recompressed => bytes.len() < original.len(),
            };
            if enough {
                Conversion::Jxl { bytes, method }
            } else {
                keep("the JPEG XL wasn't smaller")
            }
        }
        Err(error) => Conversion::Original(error),
    }
}

/// A JPEG recompressed losslessly, kept only when the original comes back byte for byte.
fn recompress(jpeg: &[u8]) -> Result<Vec<u8>, String> {
    let data = jxl_encoder::jpeg::read_jpeg(jpeg).map_err(|error| error.to_string())?;
    let jxl = jxl_encoder::jpeg::encode_jpeg_to_jxl_container(&data)
        .map_err(|error| error.to_string())?;
    match rebuilt_jpeg(&jxl)? {
        Some(back) if back == jpeg => Ok(jxl),
        Some(_) => Err("the JPEG didn't come back byte for byte".into()),
        None => Err("the JPEG XL can't rebuild the JPEG".into()),
    }
}

fn read_jxl(jxl: &[u8]) -> Result<JxlImage, String> {
    JxlImage::builder()
        .read(Cursor::new(jxl))
        .map_err(|error| format!("couldn't read the JPEG XL picture: {error}"))
}

/// The JPEG a recompressed JPEG XL holds, when it holds one.
fn rebuilt_jpeg(jxl: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let image = read_jxl(jxl)?;
    rebuild_from(&image)
}

fn rebuild_from(image: &JxlImage) -> Result<Option<Vec<u8>>, String> {
    if image.jpeg_reconstruction_status() != JpegReconstructionStatus::Available {
        return Ok(None);
    }
    let mut jpeg = Vec::new();
    image
        .reconstruct_jpeg(&mut jpeg)
        .map_err(|error| format!("couldn't rebuild the JPEG: {error}"))?;
    Ok(Some(jpeg))
}

fn decode_jxl(jxl: &[u8]) -> Result<DynamicImage, String> {
    let decoder = jxl_oxide::integration::JxlDecoder::new(Cursor::new(jxl))
        .map_err(|error| format!("couldn't read the JPEG XL picture: {error}"))?;
    DynamicImage::from_decoder(decoder)
        .map_err(|error| format!("couldn't read the JPEG XL picture: {error}"))
}

/// The pixels the encoder gets: 8 bits (what's shown; a 16-bit PNG keeps its 8-bit look), gray or
/// color, alpha only when something is see-through.
fn pixels_of(picture: &DynamicImage) -> (Vec<u8>, jxl_encoder::PixelLayout) {
    use jxl_encoder::PixelLayout;
    let gray = !picture.color().has_color();
    let see_through = picture.color().has_alpha()
        && picture
            .to_rgba8()
            .pixels()
            .any(|pixel| pixel.0[3] != u8::MAX);
    match (gray, see_through) {
        (true, false) => (picture.to_luma8().into_raw(), PixelLayout::Gray8),
        (true, true) => (picture.to_luma_alpha8().into_raw(), PixelLayout::GrayAlpha8),
        (false, false) => (picture.to_rgb8().into_raw(), PixelLayout::Rgb8),
        (false, true) => (picture.to_rgba8().into_raw(), PixelLayout::Rgba8),
    }
}

/// A picture encoded from its pixels: losslessly when its source was, else at distance 1. The
/// result is decoded once to check it (one icon in the survey came out unreadable).
fn encode_pixels(original: &[u8], source: Source, effort: u8) -> Result<(Vec<u8>, Method), String> {
    let picture = image::load_from_memory_with_format(original, source.format())
        .map_err(|error| format!("couldn't read the picture: {error}"))?;
    let (width, height) = (picture.width(), picture.height());
    let (pixels, layout) = pixels_of(&picture);
    let (encoded, method) = if source.lossless() {
        let encoded = jxl_encoder::LosslessConfig::new()
            .with_effort(effort)
            .encode(&pixels, width, height, layout);
        (encoded, Method::Lossless)
    } else {
        let encoded = jxl_encoder::LossyConfig::new(LOSSY_DISTANCE)
            .with_effort(effort)
            .encode(&pixels, width, height, layout);
        (encoded, Method::Lossy)
    };
    let jxl = encoded.map_err(|error| format!("couldn't encode the JPEG XL: {error}"))?;
    let back = decode_jxl(&jxl)?;
    if (back.width(), back.height()) != (width, height) {
        return Err("the JPEG XL decoded at another size".into());
    }
    if method == Method::Lossless && pixels_of(&back).0 != pixels {
        return Err("the JPEG XL didn't decode to the same pixels".into());
    }
    Ok((jxl, method))
}

/// A stored JPEG XL as something every webview shows.
#[derive(Debug, Clone)]
pub struct Shown {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}

/// A stored JPEG XL for a webview that can't show it: the JPEG it was made from, rebuilt, or its
/// pixels as PNG (lossless or see-through pictures) or JPEG.
pub fn displayable(jxl: &[u8]) -> Result<Shown, String> {
    let image = read_jxl(jxl)?;
    if let Ok(Some(jpeg)) = rebuild_from(&image) {
        return Ok(Shown {
            bytes: jpeg,
            content_type: "image/jpeg",
        });
    }
    let lossless = !image.image_header().metadata.xyb_encoded;
    drop(image);
    let picture = decode_jxl(jxl)?;
    let mut out = Vec::new();
    if lossless || picture.color().has_alpha() {
        let (pixels, color) = if picture.color().has_alpha() {
            (
                picture.to_rgba8().into_raw(),
                image::ExtendedColorType::Rgba8,
            )
        } else {
            (picture.to_rgb8().into_raw(), image::ExtendedColorType::Rgb8)
        };
        image::codecs::png::PngEncoder::new_with_quality(
            &mut out,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::Adaptive,
        )
        .write_image(&pixels, picture.width(), picture.height(), color)
        .map_err(|error| error.to_string())?;
        return Ok(Shown {
            bytes: out,
            content_type: "image/png",
        });
    }
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, SHOWN_JPEG_QUALITY)
        .encode_image(&picture.to_rgb8())
        .map_err(|error| error.to_string())?;
    Ok(Shown {
        bytes: out,
        content_type: "image/jpeg",
    })
}

#[cfg(test)]
pub(crate) mod tests;
