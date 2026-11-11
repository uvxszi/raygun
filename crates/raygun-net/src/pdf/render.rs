//! A page drawn as a picture: rasterized by `hayro` on white at the width asked for, then written
//! as PNG (grey when the page has no colour, a third of the size), which keeps text sharp.

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::page::Page;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::vello_cpu::peniko::ImageAlphaType;
use hayro::{PixmapSettings, RenderCache, RenderSettings};
use image::ImageEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};

/// The widest and tallest a page is drawn: the rasterizer counts pixels in 16 bits, and a page
/// this size is already sharper than any screen shows it.
const MAX_WIDTH: f32 = 4096.0;
const MAX_HEIGHT: f32 = 8192.0;

pub(super) fn png(page: &Page<'_>, width: u32) -> Result<Vec<u8>, String> {
    let (page_width, page_height) = page.render_dimensions();
    if !(page_width > 0.0 && page_height > 0.0) {
        return Err("The page is empty".into());
    }
    let wanted = (width.max(1) as f32) / page_width;
    let scale = wanted
        .min(MAX_WIDTH / page_width)
        .min(MAX_HEIGHT / page_height)
        .max(0.01);
    let settings = PixmapSettings {
        x_scale: scale,
        y_scale: scale,
        bg_color: WHITE,
    };
    let cache = RenderCache::new();
    let pixmap = hayro::render(
        page,
        &cache,
        &InterpreterSettings::default(),
        &RenderSettings::default(),
        &settings,
    );
    let (pixel_width, pixel_height) = (u32::from(pixmap.width()), u32::from(pixmap.height()));
    if pixel_width == 0 || pixel_height == 0 {
        return Err("The page is empty".into());
    }
    let rgba = pixmap.take_rgba8(ImageAlphaType::Alpha);
    encode(&rgba, pixel_width, pixel_height)
}

/// RGBA pixels as PNG, over white; grey pixels only make a one-channel picture.
fn encode(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let over_white = |value: u8, alpha: u8| -> u8 {
        if alpha == 255 {
            value
        } else {
            let alpha = u16::from(alpha);
            ((u16::from(value) * alpha + 255 * (255 - alpha) + 127) / 255) as u8
        }
    };
    let (pixels, _) = rgba.as_chunks::<4>();
    let grey = pixels
        .iter()
        .all(|pixel| pixel[0] == pixel[1] && pixel[1] == pixel[2]);
    let mut out = Vec::new();
    let encoder =
        PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::Adaptive);
    let written = if grey {
        let luma: Vec<u8> = pixels
            .iter()
            .map(|pixel| over_white(pixel[0], pixel[3]))
            .collect();
        encoder.write_image(&luma, width, height, image::ExtendedColorType::L8)
    } else {
        let rgb: Vec<u8> = pixels
            .iter()
            .flat_map(|pixel| {
                [
                    over_white(pixel[0], pixel[3]),
                    over_white(pixel[1], pixel[3]),
                    over_white(pixel[2], pixel[3]),
                ]
            })
            .collect();
        encoder.write_image(&rgb, width, height, image::ExtendedColorType::Rgb8)
    };
    written.map_err(|error| format!("The page couldn't be drawn ({error})"))?;
    Ok(out)
}
