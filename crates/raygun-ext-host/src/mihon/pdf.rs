//! PDF pages drawn into the pictures converted sources hold (Android's `PdfRenderer`, for sources
//! whose chapters are PDF files): documents read once and kept by number until closed, each
//! page rasterized by `hayro` (pure Rust) to the picture's size and laid over it.

use std::collections::HashMap;

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::TRANSPARENT;
use hayro::vello_cpu::peniko::ImageAlphaType;
use hayro::{PixmapSettings, RenderCache, RenderSettings};
use raygun_net::bitmap::Bitmaps;
use serde_json::json;

/// Most a document may hold: a chapter as one PDF of its pages.
const MAX_PDF_BYTES: usize = 200 * 1024 * 1024;

/// Documents open in one runtime, by number.
#[derive(Default)]
pub(crate) struct Pdfs {
    open: HashMap<u32, Pdf>,
    next: u32,
}

impl Pdfs {
    /// Reads a document: its number and each page's size in points.
    pub(crate) fn open(&mut self, bytes: Vec<u8>) -> Result<serde_json::Value, String> {
        if bytes.len() > MAX_PDF_BYTES {
            return Err("the PDF is too large to draw".to_owned());
        }
        let pdf =
            Pdf::new(bytes).map_err(|error| format!("the PDF couldn't be read ({error:?})"))?;
        let pages: Vec<[f32; 2]> = pdf
            .pages()
            .iter()
            .map(|page| {
                let (width, height) = page.render_dimensions();
                [width, height]
            })
            .collect();
        self.next += 1;
        let id = self.next;
        self.open.insert(id, pdf);
        Ok(json!({ "id": id, "pages": pages }))
    }

    pub(crate) fn close(&mut self, id: u32) {
        self.open.remove(&id);
    }

    /// Draws page `index` of document `id` over picture `bitmap`, scaled to fill it (as
    /// `page.render(bitmap, null, null, mode)` does).
    pub(crate) fn render(
        &self,
        id: u32,
        index: usize,
        bitmaps: &mut Bitmaps,
        bitmap: u32,
    ) -> Result<(), String> {
        let pdf = self
            .open
            .get(&id)
            .ok_or_else(|| "the PDF was closed".to_owned())?;
        let page = pdf
            .pages()
            .get(index)
            .ok_or_else(|| format!("the PDF has no page {}", index + 1))?;
        let (page_width, page_height) = page.render_dimensions();
        let target = bitmaps.image_mut(bitmap)?;
        let (width, height) = target.dimensions();
        if page_width <= 0.0 || page_height <= 0.0 || width == 0 || height == 0 {
            return Ok(());
        }
        let settings = PixmapSettings {
            x_scale: width as f32 / page_width,
            y_scale: height as f32 / page_height,
            bg_color: TRANSPARENT,
        };
        let cache = RenderCache::new();
        let pixmap = hayro::render(
            page,
            &cache,
            &InterpreterSettings::default(),
            &RenderSettings::default(),
            &settings,
        );
        let (drawn_width, drawn_height) = (u32::from(pixmap.width()), u32::from(pixmap.height()));
        let rgba = pixmap.take_rgba8(ImageAlphaType::Alpha);
        // Over what the picture holds (sources fill it with white first).
        for y in 0..drawn_height.min(height) {
            for x in 0..drawn_width.min(width) {
                let at = ((y * drawn_width + x) * 4) as usize;
                let [red, green, blue, alpha] =
                    [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]];
                if alpha == 0 {
                    continue;
                }
                let pixel = target.get_pixel_mut(x, y);
                if alpha == 255 {
                    pixel.0 = [red, green, blue, 255];
                    continue;
                }
                let over = f32::from(alpha) / 255.0;
                let under = f32::from(pixel.0[3]) / 255.0 * (1.0 - over);
                let out = over + under;
                let mix = |top: u8, bottom: u8| {
                    ((f32::from(top) * over + f32::from(bottom) * under) / out).round() as u8
                };
                pixel.0 = [
                    mix(red, pixel.0[0]),
                    mix(green, pixel.0[1]),
                    mix(blue, pixel.0[2]),
                    (out * 255.0).round() as u8,
                ];
            }
        }
        Ok(())
    }
}
