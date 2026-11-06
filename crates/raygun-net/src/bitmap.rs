//! Pictures held for a script that draws them: Android's `Bitmap`, `BitmapFactory` and `Canvas`
//! as the Mihon sources converted to Raygun's runtime use them to put a page's shuffled pieces
//! back together (`raygun-ext-host`'s runtime `mihon` holds one [`Bitmaps`] per extension).
//!
//! Pixels are RGBA, as an `ARGB_8888` bitmap's are. A picture is drawn into another through an
//! affine matrix (a canvas's translation, scale, rotation and mirroring), sampled at each target
//! pixel's centre, so straight copies stay exact.

use std::collections::{HashMap, VecDeque};
use std::io::Cursor;

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

/// The most pixels all held pictures may have together (about 256 MB); the oldest go first.
const MAX_PIXELS: u64 = 64 * 1024 * 1024;
/// The largest side a picture may have.
const MAX_SIDE: u32 = 32_768;

/// A script's pictures, by number.
#[derive(Default)]
pub struct Bitmaps {
    images: HashMap<u32, RgbaImage>,
    order: VecDeque<u32>,
    next: u32,
}

/// An affine map from one picture's pixel space into another's: `x' = a·x + c·y + e`,
/// `y' = b·x + d·y + f` (Android's `Matrix` values in that order).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Affine {
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    fn inverse(&self) -> Option<Self> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < 1e-12 || !det.is_finite() {
            return None;
        }
        Some(Self {
            a: self.d / det,
            b: -self.b / det,
            c: -self.c / det,
            d: self.a / det,
            e: (self.c * self.f - self.d * self.e) / det,
            f: (self.b * self.e - self.a * self.f) / det,
        })
    }
}

/// An `ARGB` colour (Android's `Color` int) as RGBA.
pub fn argb(color: u32) -> Rgba<u8> {
    Rgba([
        (color >> 16) as u8,
        (color >> 8) as u8,
        color as u8,
        (color >> 24) as u8,
    ])
}

fn to_argb(pixel: &Rgba<u8>) -> u32 {
    let [r, g, b, a] = pixel.0;
    (u32::from(a) << 24) | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

/// Source over: `top` drawn on `bottom`.
fn blend(bottom: &mut Rgba<u8>, top: Rgba<u8>) {
    let alpha = u32::from(top.0[3]);
    if alpha == 255 {
        *bottom = top;
        return;
    }
    if alpha == 0 {
        return;
    }
    let below = u32::from(bottom.0[3]);
    let out_alpha = alpha + below * (255 - alpha) / 255;
    if out_alpha == 0 {
        *bottom = Rgba([0, 0, 0, 0]);
        return;
    }
    let mut out = [0u8; 4];
    for (channel, value) in out.iter_mut().enumerate().take(3) {
        let mixed = (u32::from(top.0[channel]) * alpha
            + u32::from(bottom.0[channel]) * below * (255 - alpha) / 255)
            / out_alpha;
        *value = mixed.min(255) as u8;
    }
    out[3] = out_alpha.min(255) as u8;
    *bottom = Rgba(out);
}

/// Reads a picture: PNG, JPEG, WebP, GIF (its first frame) or JPEG XL.
pub fn decode(bytes: &[u8]) -> Result<RgbaImage, String> {
    let image = if crate::images::is_jxl(bytes) {
        let png = crate::images::jxl_to_png(bytes)?;
        image::load_from_memory_with_format(&png, ImageFormat::Png)
    } else {
        image::load_from_memory(bytes)
    }
    .map_err(|error| format!("couldn't read the picture: {error}"))?;
    Ok(image.to_rgba8())
}

/// Writes a picture as Android's `Bitmap.compress` would: `format` is `JPEG`, `PNG` or a WebP
/// kind (lossy WebP is written as JPEG, lossless as PNG: what reads it sniffs the bytes).
pub fn encode(image: &RgbaImage, format: &str, quality: u8) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    let lossless = matches!(format, "PNG" | "WEBP_LOSSLESS");
    if lossless {
        image
            .write_to(&mut out, ImageFormat::Png)
            .map_err(|error| error.to_string())?;
    } else {
        // JPEG has no alpha: transparent pixels come out as their colour, black when unset.
        let rgb = DynamicImage::ImageRgba8(image.clone()).to_rgb8();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100))
            .encode_image(&rgb)
            .map_err(|error| error.to_string())?;
    }
    Ok(out.into_inner())
}

impl Bitmaps {
    fn keep(&mut self, image: RgbaImage) -> Result<u32, String> {
        let (width, height) = image.dimensions();
        if width > MAX_SIDE || height > MAX_SIDE {
            return Err(format!("a {width}×{height} picture is too large"));
        }
        // The oldest pictures make room.
        let wanted = u64::from(width) * u64::from(height);
        while self.pixels() + wanted > MAX_PIXELS {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.images.remove(&oldest);
        }
        self.next = self.next.wrapping_add(1).max(1);
        let id = self.next;
        self.images.insert(id, image);
        self.order.push_back(id);
        Ok(id)
    }

    fn pixels(&self) -> u64 {
        self.images
            .values()
            .map(|image| u64::from(image.width()) * u64::from(image.height()))
            .sum()
    }

    pub fn get(&self, id: u32) -> Result<&RgbaImage, String> {
        self.images
            .get(&id)
            .ok_or_else(|| "the picture was recycled".to_owned())
    }

    fn get_mut(&mut self, id: u32) -> Result<&mut RgbaImage, String> {
        self.images
            .get_mut(&id)
            .ok_or_else(|| "the picture was recycled".to_owned())
    }

    /// The size of picture `id`.
    pub fn size(&self, id: u32) -> Result<(u32, u32), String> {
        Ok(self.get(id)?.dimensions())
    }

    /// `BitmapFactory.decode…`: a picture read from its file's bytes.
    pub fn decode(&mut self, bytes: &[u8]) -> Result<u32, String> {
        self.keep(decode(bytes)?)
    }

    /// `Bitmap.createBitmap(width, height, config)`: a picture of one colour (transparent).
    pub fn create(&mut self, width: u32, height: u32, color: u32) -> Result<u32, String> {
        if width == 0 || height == 0 {
            return Err("width and height must be > 0".into());
        }
        if width > MAX_SIDE || height > MAX_SIDE {
            return Err(format!("a {width}×{height} picture is too large"));
        }
        self.keep(RgbaImage::from_pixel(width, height, argb(color)))
    }

    /// `Bitmap.createBitmap(source, x, y, width, height)`: a part of a picture, as its own.
    pub fn crop(
        &mut self,
        id: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<u32, String> {
        let source = self.get(id)?;
        let (source_width, source_height) = source.dimensions();
        if width == 0
            || height == 0
            || x.saturating_add(width) > source_width
            || y.saturating_add(height) > source_height
        {
            return Err(format!(
                "x + width must be <= bitmap.width() ({x} + {width} > {source_width}) and y + height <= bitmap.height() ({y} + {height} > {source_height})"
            ));
        }
        let part = image::imageops::crop_imm(source, x, y, width, height).to_image();
        self.keep(part)
    }

    /// `Bitmap.createScaledBitmap(source, width, height, filter)`.
    pub fn scaled(
        &mut self,
        id: u32,
        width: u32,
        height: u32,
        filter: bool,
    ) -> Result<u32, String> {
        if width == 0 || height == 0 {
            return Err("width and height must be > 0".into());
        }
        let source = self.get(id)?;
        let kind = if filter {
            image::imageops::FilterType::Triangle
        } else {
            image::imageops::FilterType::Nearest
        };
        let scaled = image::imageops::resize(source, width, height, kind);
        self.keep(scaled)
    }

    /// `bitmap.copy(config, mutable)`.
    pub fn copy(&mut self, id: u32) -> Result<u32, String> {
        let copy = self.get(id)?.clone();
        self.keep(copy)
    }

    /// `bitmap.recycle()`.
    pub fn free(&mut self, id: u32) {
        self.images.remove(&id);
        self.order.retain(|kept| *kept != id);
    }

    /// Draws the part `(left, top, right, bottom)` of picture `source` into `target` through
    /// `matrix` (from the source's pixels to the target's), clipped to `clip` in the target when
    /// given. Each target pixel takes the source pixel its centre falls on.
    pub fn draw(
        &mut self,
        target: u32,
        source: u32,
        part: (i64, i64, i64, i64),
        matrix: Affine,
        clip: Option<(i64, i64, i64, i64)>,
    ) -> Result<(), String> {
        let inverse = matrix
            .inverse()
            .ok_or_else(|| "the canvas's matrix can't be drawn through".to_owned())?;
        let (source_width, source_height) = self.size(source)?;
        let left = part.0.max(0);
        let top = part.1.max(0);
        let right = part.2.min(i64::from(source_width));
        let bottom = part.3.min(i64::from(source_height));
        if left >= right || top >= bottom {
            return Ok(());
        }
        // Drawing a picture into itself reads the picture as it was.
        let source_image = if source == target {
            self.get(source)?.clone()
        } else {
            // Taken out while drawing so both can be borrowed.
            self.images
                .remove(&source)
                .ok_or_else(|| "the picture was recycled".to_owned())?
        };
        let result = (|| {
            let image = self.get_mut(target)?;
            let (width, height) = image.dimensions();
            let corners = [
                matrix.apply(left as f64, top as f64),
                matrix.apply(right as f64, top as f64),
                matrix.apply(left as f64, bottom as f64),
                matrix.apply(right as f64, bottom as f64),
            ];
            let min_x = corners.iter().map(|c| c.0).fold(f64::INFINITY, f64::min);
            let max_x = corners
                .iter()
                .map(|c| c.0)
                .fold(f64::NEG_INFINITY, f64::max);
            let min_y = corners.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
            let max_y = corners
                .iter()
                .map(|c| c.1)
                .fold(f64::NEG_INFINITY, f64::max);
            let (clip_left, clip_top, clip_right, clip_bottom) =
                clip.unwrap_or((0, 0, i64::from(width), i64::from(height)));
            let x0 = (min_x.floor() as i64).max(0).max(clip_left);
            let y0 = (min_y.floor() as i64).max(0).max(clip_top);
            let x1 = (max_x.ceil() as i64).min(i64::from(width)).min(clip_right);
            let y1 = (max_y.ceil() as i64)
                .min(i64::from(height))
                .min(clip_bottom);
            for y in y0..y1 {
                for x in x0..x1 {
                    let (sx, sy) = inverse.apply(x as f64 + 0.5, y as f64 + 0.5);
                    let (sx, sy) = (sx.floor() as i64, sy.floor() as i64);
                    if sx < left || sx >= right || sy < top || sy >= bottom {
                        continue;
                    }
                    let pixel = *source_image.get_pixel(sx as u32, sy as u32);
                    blend(image.get_pixel_mut(x as u32, y as u32), pixel);
                }
            }
            Ok(())
        })();
        if source != target {
            self.images.insert(source, source_image);
        }
        result
    }

    /// Fills `rect` of a picture (all of it without one) through `matrix` with `color`: drawn
    /// over (`blend`) as `drawRect` does, or replacing as `eraseColor` does.
    pub fn fill(
        &mut self,
        id: u32,
        rect: Option<(f64, f64, f64, f64)>,
        matrix: Affine,
        color: u32,
        replace: bool,
    ) -> Result<(), String> {
        let image = self.get_mut(id)?;
        let (width, height) = image.dimensions();
        let rgba = argb(color);
        let Some((left, top, right, bottom)) = rect else {
            for pixel in image.pixels_mut() {
                if replace {
                    *pixel = rgba;
                } else {
                    blend(pixel, rgba);
                }
            }
            return Ok(());
        };
        let inverse = matrix
            .inverse()
            .ok_or_else(|| "the canvas's matrix can't be drawn through".to_owned())?;
        let corners = [
            matrix.apply(left, top),
            matrix.apply(right, top),
            matrix.apply(left, bottom),
            matrix.apply(right, bottom),
        ];
        let x0 = corners
            .iter()
            .map(|c| c.0)
            .fold(f64::INFINITY, f64::min)
            .floor()
            .max(0.0) as u32;
        let y0 = corners
            .iter()
            .map(|c| c.1)
            .fold(f64::INFINITY, f64::min)
            .floor()
            .max(0.0) as u32;
        let x1 = (corners
            .iter()
            .map(|c| c.0)
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil() as i64)
            .clamp(0, i64::from(width)) as u32;
        let y1 = (corners
            .iter()
            .map(|c| c.1)
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil() as i64)
            .clamp(0, i64::from(height)) as u32;
        for y in y0..y1 {
            for x in x0..x1 {
                let (sx, sy) = inverse.apply(f64::from(x) + 0.5, f64::from(y) + 0.5);
                if sx < left || sx >= right || sy < top || sy >= bottom {
                    continue;
                }
                let pixel = image.get_pixel_mut(x, y);
                if replace {
                    *pixel = rgba;
                } else {
                    blend(pixel, rgba);
                }
            }
        }
        Ok(())
    }

    /// `bitmap.getPixels(…)`: a region's colours as `ARGB` ints, row by row.
    pub fn pixels_of(
        &self,
        id: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<Vec<u32>, String> {
        let image = self.get(id)?;
        if x.saturating_add(width) > image.width() || y.saturating_add(height) > image.height() {
            return Err("the region is outside the picture".into());
        }
        let mut out = Vec::with_capacity(width as usize * height as usize);
        for row in y..y + height {
            for column in x..x + width {
                out.push(to_argb(image.get_pixel(column, row)));
            }
        }
        Ok(out)
    }

    /// `bitmap.setPixels(…)`: a region's colours from `ARGB` ints, row by row.
    pub fn set_pixels(
        &mut self,
        id: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        colors: &[u32],
    ) -> Result<(), String> {
        let image = self.get_mut(id)?;
        if x.saturating_add(width) > image.width() || y.saturating_add(height) > image.height() {
            return Err("the region is outside the picture".into());
        }
        for (index, color) in colors
            .iter()
            .enumerate()
            .take(width as usize * height as usize)
        {
            let column = x + (index as u32 % width);
            let row = y + (index as u32 / width);
            image.put_pixel(column, row, argb(*color));
        }
        Ok(())
    }

    /// `bitmap.compress(format, quality, stream)`.
    pub fn encode(&self, id: u32, format: &str, quality: u8) -> Result<Vec<u8>, String> {
        encode(self.get(id)?, format, quality)
    }

    /// Puts a picture made elsewhere (text drawn by the runtime) among these.
    pub fn insert(&mut self, image: RgbaImage) -> Result<u32, String> {
        self.keep(image)
    }

    /// Picture `id` to draw on.
    pub fn image_mut(&mut self, id: u32) -> Result<&mut RgbaImage, String> {
        self.get_mut(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiles() -> RgbaImage {
        // Four 2×2 tiles: red, green, blue, white.
        RgbaImage::from_fn(4, 4, |x, y| match (x / 2, y / 2) {
            (0, 0) => Rgba([255, 0, 0, 255]),
            (1, 0) => Rgba([0, 255, 0, 255]),
            (0, 1) => Rgba([0, 0, 255, 255]),
            _ => Rgba([255, 255, 255, 255]),
        })
    }

    #[test]
    fn a_tile_is_copied_where_the_matrix_puts_it() {
        let mut bitmaps = Bitmaps::default();
        let source = bitmaps.keep(tiles()).unwrap();
        let target = bitmaps.create(4, 4, 0).unwrap();
        // The green tile (2,0)-(4,2) to the top left.
        let matrix = Affine {
            e: -2.0,
            ..Affine::IDENTITY
        };
        bitmaps
            .draw(target, source, (2, 0, 4, 2), matrix, None)
            .unwrap();
        let image = bitmaps.get(target).unwrap();
        assert_eq!(image.get_pixel(0, 0), &Rgba([0, 255, 0, 255]));
        assert_eq!(image.get_pixel(1, 1), &Rgba([0, 255, 0, 255]));
        assert_eq!(image.get_pixel(2, 0), &Rgba([0, 0, 0, 0]), "the rest stays");
    }

    #[test]
    fn rotations_and_mirrors_map_each_pixel() {
        let mut bitmaps = Bitmaps::default();
        let source = bitmaps.keep(tiles()).unwrap();
        let target = bitmaps.create(4, 4, 0).unwrap();
        // Mirrored left to right: x' = 4 - x.
        let mirror = Affine {
            a: -1.0,
            e: 4.0,
            ..Affine::IDENTITY
        };
        bitmaps
            .draw(target, source, (0, 0, 4, 4), mirror, None)
            .unwrap();
        let image = bitmaps.get(target).unwrap();
        assert_eq!(image.get_pixel(0, 0), &Rgba([0, 255, 0, 255]));
        assert_eq!(image.get_pixel(3, 3), &Rgba([0, 0, 255, 255]));
        // A quarter turn clockwise: x' = 4 - y, y' = x.
        let target = bitmaps.create(4, 4, 0).unwrap();
        let turn = Affine {
            a: 0.0,
            b: 1.0,
            c: -1.0,
            d: 0.0,
            e: 4.0,
            f: 0.0,
        };
        bitmaps
            .draw(target, source, (0, 0, 4, 4), turn, None)
            .unwrap();
        let image = bitmaps.get(target).unwrap();
        // Red (top left) goes to the top right, blue (bottom left) to the top left.
        assert_eq!(image.get_pixel(3, 0), &Rgba([255, 0, 0, 255]));
        assert_eq!(image.get_pixel(0, 0), &Rgba([0, 0, 255, 255]));
    }

    #[test]
    fn pictures_round_trip_and_crop() {
        let mut bitmaps = Bitmaps::default();
        let source = bitmaps.keep(tiles()).unwrap();
        let png = bitmaps.encode(source, "PNG", 100).unwrap();
        let back = bitmaps.decode(&png).unwrap();
        assert_eq!(bitmaps.size(back).unwrap(), (4, 4));
        let part = bitmaps.crop(back, 2, 2, 2, 2).unwrap();
        assert_eq!(
            bitmaps.pixels_of(part, 0, 0, 1, 1).unwrap(),
            vec![0xFFFF_FFFF]
        );
        assert!(bitmaps.crop(back, 3, 3, 2, 2).is_err());
        let jpeg = bitmaps.encode(source, "JPEG", 90).unwrap();
        assert!(jpeg.starts_with(&[0xFF, 0xD8]));
        bitmaps.free(back);
        assert!(bitmaps.size(back).is_err());
    }

    #[test]
    fn fills_replace_or_draw_over() {
        let mut bitmaps = Bitmaps::default();
        let id = bitmaps.create(2, 2, 0).unwrap();
        bitmaps
            .fill(
                id,
                Some((0.0, 0.0, 1.0, 2.0)),
                Affine::IDENTITY,
                0xFF00_00FF,
                false,
            )
            .unwrap();
        assert_eq!(
            bitmaps.pixels_of(id, 0, 0, 2, 1).unwrap(),
            vec![0xFF00_00FF, 0]
        );
        bitmaps
            .fill(id, None, Affine::IDENTITY, 0xFFFF_FFFF, true)
            .unwrap();
        assert_eq!(
            bitmaps.pixels_of(id, 1, 1, 1, 1).unwrap(),
            vec![0xFFFF_FFFF]
        );
    }
}
