//! Text drawn on a picture, as Android's `StaticLayout` lays it out (`lib:textinterceptor` turns a
//! web comic's notes into a page this way): lines wrapped at spaces to a width, in the system's
//! sans-serif face, with any character it lacks taken from the system's other faces.

use std::sync::OnceLock;

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use image::RgbaImage;
use raygun_net::bitmap::{Affine, argb};

/// Faces tried in order: the regular face, the bold one, then faces for other scripts.
fn candidates() -> (Vec<String>, Vec<String>, Vec<String>) {
    let strings = |list: &[&str]| {
        list.iter()
            .map(|path| (*path).to_owned())
            .collect::<Vec<_>>()
    };
    if cfg!(target_os = "android") {
        (
            strings(&[
                "/system/fonts/Roboto-Regular.ttf",
                "/system/fonts/RobotoStatic-Regular.ttf",
                "/system/fonts/DroidSans.ttf",
            ]),
            strings(&[
                "/system/fonts/RobotoStatic-Bold.ttf",
                "/system/fonts/Roboto-Bold.ttf",
                "/system/fonts/DroidSans-Bold.ttf",
            ]),
            strings(&[
                "/system/fonts/NotoSansCJK-Regular.ttc",
                "/system/fonts/NotoSansSymbols-Regular-Subsetted.ttf",
                "/system/fonts/NotoSansThai-Regular.ttf",
                "/system/fonts/NotoSansArabic-Regular.ttf",
            ]),
        )
    } else if cfg!(target_os = "windows") {
        let fonts = std::env::var("SystemRoot")
            .or_else(|_| std::env::var("WINDIR"))
            .unwrap_or_else(|_| "C:\\Windows".into())
            + "\\Fonts\\";
        let at = |names: &[&str]| names.iter().map(|name| format!("{fonts}{name}")).collect();
        (
            at(&["segoeui.ttf", "arial.ttf"]),
            at(&["segoeuib.ttf", "arialbd.ttf"]),
            at(&[
                "msyh.ttc",
                "YuGothM.ttc",
                "malgun.ttf",
                "seguisym.ttf",
                "tahoma.ttf",
            ]),
        )
    } else if cfg!(target_os = "macos") {
        (
            strings(&[
                "/System/Library/Fonts/Supplemental/Arial.ttf",
                "/Library/Fonts/Arial.ttf",
                "/System/Library/Fonts/Helvetica.ttc",
            ]),
            strings(&[
                "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
                "/Library/Fonts/Arial Bold.ttf",
            ]),
            strings(&[
                "/System/Library/Fonts/PingFang.ttc",
                "/System/Library/Fonts/Hiragino Sans GB.ttc",
            ]),
        )
    } else {
        (
            strings(&[
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
                "/usr/share/fonts/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            ]),
            strings(&[
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf",
            ]),
            strings(&[
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            ]),
        )
    }
}

fn load(path: &str) -> Option<FontVec> {
    let bytes = std::fs::read(path).ok()?;
    FontVec::try_from_vec_and_index(bytes, 0).ok()
}

struct Faces {
    regular: Option<FontVec>,
    bold: Option<FontVec>,
    others: Vec<FontVec>,
}

fn faces() -> &'static Faces {
    static FACES: OnceLock<Faces> = OnceLock::new();
    FACES.get_or_init(|| {
        let (regular, bold, others) = candidates();
        Faces {
            regular: regular.iter().find_map(|path| load(path)),
            bold: bold.iter().find_map(|path| load(path)),
            others: others.iter().filter_map(|path| load(path)).collect(),
        }
    })
}

/// One character placed: its face, its position on its line and its line.
struct Placed {
    face: &'static FontVec,
    ch: char,
    x: f32,
    line: usize,
}

/// Text laid out in lines.
pub struct Layout {
    placed: Vec<Placed>,
    lines: usize,
    line_height: f32,
    ascent: f32,
    size: f32,
}

/// The pixel scale that gives `face` an em of `size` pixels (Android's `textSize`).
fn scale_for(face: &FontVec, size: f32) -> PxScale {
    let em = face.units_per_em().unwrap_or(1000.0);
    PxScale::from(size * face.height_unscaled() / em)
}

/// Whether a line may break before or after `ch` without a space (CJK writing).
fn breaks_anywhere(ch: char) -> bool {
    matches!(u32::from(ch), 0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF)
}

impl Layout {
    pub fn new(
        text: &str,
        size: f32,
        width: f32,
        spacing: f32,
        extra: f32,
        bold: bool,
    ) -> Result<Self, String> {
        let faces = faces();
        let main = (if bold { faces.bold.as_ref() } else { None })
            .or(faces.regular.as_ref())
            .ok_or_else(|| "there's no font on this device to draw text with".to_owned())?;
        let size = if size.is_finite() && size > 0.0 {
            size
        } else {
            16.0
        };
        let width = if width.is_finite() && width > 0.0 {
            width
        } else {
            f32::MAX
        };
        let main_scaled = main.as_scaled(scale_for(main, size));
        let ascent = main_scaled.ascent();
        let line_height = (main_scaled.ascent() - main_scaled.descent()) * spacing + extra;
        let face_for = |ch: char| -> &'static FontVec {
            if ch.is_whitespace() || main.glyph_id(ch).0 != 0 {
                return main;
            }
            faces
                .others
                .iter()
                .find(|face| face.glyph_id(ch).0 != 0)
                .unwrap_or(main)
        };
        let advance = |face: &FontVec, ch: char| {
            let scaled = face.as_scaled(scale_for(face, size));
            scaled.h_advance(face.glyph_id(ch))
        };
        let mut placed = Vec::new();
        let mut line = 0usize;
        for paragraph in text.split('\n') {
            let mut x = 0.0f32;
            // Where the line may break: the index in `placed` the next line would start at.
            let mut break_at: Option<usize> = None;
            let mut line_start = placed.len();
            for ch in paragraph.chars() {
                if ch == '\r' {
                    continue;
                }
                let face = face_for(ch);
                let step = advance(face, ch);
                if breaks_anywhere(ch) {
                    break_at = Some(placed.len());
                }
                if x + step > width && !ch.is_whitespace() && placed.len() > line_start {
                    // Wrap at the last opportunity on this line, else here.
                    let wrap = break_at
                        .filter(|at| *at > line_start)
                        .unwrap_or(placed.len());
                    line += 1;
                    // Spaces that ended the line are dropped; the rest move down.
                    let moved: Vec<Placed> = placed.drain(wrap..).collect();
                    let mut moved_x = 0.0;
                    for mut item in moved.into_iter().skip_while(|item| item.ch.is_whitespace()) {
                        item.x = moved_x;
                        item.line = line;
                        moved_x += advance(item.face, item.ch);
                        placed.push(item);
                    }
                    x = moved_x;
                    line_start = wrap;
                    break_at = None;
                }
                placed.push(Placed { face, ch, x, line });
                x += step;
                if ch.is_whitespace() || breaks_anywhere(ch) {
                    break_at = Some(placed.len());
                }
            }
            line += 1;
        }
        Ok(Self {
            placed,
            lines: line,
            line_height,
            ascent,
            size,
        })
    }

    pub fn line_count(&self) -> usize {
        self.lines
    }

    /// The laid-out text's height in pixels.
    pub fn height(&self) -> f32 {
        (self.lines as f32 * self.line_height).ceil()
    }

    /// Draws the text with its top left at the origin of `matrix` (which maps the layout's
    /// coordinates into the picture's).
    pub fn draw(&self, image: &mut RgbaImage, matrix: Affine, color: u32) {
        let (width, height) = image.dimensions();
        let ink = argb(color);
        for item in &self.placed {
            if item.ch.is_whitespace() {
                continue;
            }
            let scale = scale_for(item.face, self.size);
            let baseline = item.line as f32 * self.line_height + self.ascent;
            let glyph = item
                .face
                .glyph_id(item.ch)
                .with_scale_and_position(scale, ab_glyph::point(item.x, baseline));
            let Some(outline) = item.face.outline_glyph(glyph) else {
                continue;
            };
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                let lx = f64::from(bounds.min.x) + f64::from(gx);
                let ly = f64::from(bounds.min.y) + f64::from(gy);
                let px = matrix.a * lx + matrix.c * ly + matrix.e;
                let py = matrix.b * lx + matrix.d * ly + matrix.f;
                let (px, py) = (px.floor(), py.floor());
                if px < 0.0 || py < 0.0 || px >= f64::from(width) || py >= f64::from(height) {
                    return;
                }
                let alpha = (coverage.clamp(0.0, 1.0) * f32::from(ink.0[3])) as u32;
                if alpha == 0 {
                    return;
                }
                let pixel = image.get_pixel_mut(px as u32, py as u32);
                for channel in 0..3 {
                    let below = u32::from(pixel.0[channel]);
                    let above = u32::from(ink.0[channel]);
                    pixel.0[channel] = ((above * alpha + below * (255 - alpha)) / 255) as u8;
                }
                pixel.0[3] = (alpha + u32::from(pixel.0[3]) * (255 - alpha) / 255).min(255) as u8;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_text_wraps_into_lines() {
        let Ok(short) = Layout::new("Hello", 30.0, 900.0, 1.0, 0.0, false) else {
            // No system font where the tests run.
            return;
        };
        assert_eq!(short.line_count(), 1);
        let long = Layout::new(&"word ".repeat(200), 30.0, 900.0, 1.0, 0.0, false).unwrap();
        assert!(long.line_count() > 5, "{} lines", long.line_count());
        let two = Layout::new("one\ntwo", 30.0, 900.0, 1.0, 0.0, false).unwrap();
        assert_eq!(two.line_count(), 2);
        assert!(two.height() > short.height());
        let mut image = RgbaImage::from_pixel(200, 100, image::Rgba([255, 255, 255, 255]));
        short.draw(&mut image, Affine::IDENTITY, 0xFF00_0000);
        assert!(image.pixels().any(|pixel| pixel.0[0] < 128), "some ink");
    }
}
