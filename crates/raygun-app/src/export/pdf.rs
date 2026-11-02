//! PDF files of page images (M46.1): a page per image, sized to it at 72 dpi, written as it goes
//! (each object straight to the file, the cross-reference table last), so a whole title never
//! sits in memory. JPEG pages go in as they are (`DCTDecode`); PNG and WebP are decoded and
//! stored with Flate, their transparency as a soft mask. Other formats (GIF, AVIF, JXL) are
//! skipped and counted. Pages taller than a PDF page may be (200 inches) are split.

use std::io::{self, Cursor, Write};

/// Object numbers fixed up front: written last, once every page is known.
const CATALOG: u32 = 1;
const PAGES: u32 = 2;
/// The largest page side PDF readers accept, in points (200 inches).
const MAX_SIDE: f64 = 14_400.0;

pub struct PdfWriter<W: Write> {
    out: W,
    written: u64,
    /// Byte offset of each object (number - 1), once written.
    offsets: Vec<Option<u64>>,
    pages: Vec<u32>,
    bookmarks: Vec<(String, u32)>,
    pending_bookmark: Option<String>,
    skipped: u32,
}

/// A decoded page, ready to embed.
struct Picture {
    width: u32,
    height: u32,
    color_space: &'static str,
    filter: &'static str,
    /// Adobe's CMYK JPEGs store inverted values.
    inverted: bool,
    data: Vec<u8>,
    alpha: Option<Vec<u8>>,
}

impl<W: Write> PdfWriter<W> {
    pub fn new(out: W) -> io::Result<Self> {
        let mut writer = Self {
            out,
            written: 0,
            offsets: vec![None, None],
            pages: Vec::new(),
            bookmarks: Vec::new(),
            pending_bookmark: None,
            skipped: 0,
        };
        // The binary comment marks the file as binary for old transfer tools.
        writer.raw(b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n")?;
        Ok(writer)
    }

    fn raw(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.out.write_all(bytes)?;
        self.written += bytes.len() as u64;
        Ok(())
    }

    fn reserve(&mut self) -> u32 {
        self.offsets.push(None);
        self.offsets.len() as u32
    }

    fn begin(&mut self, id: u32) -> io::Result<()> {
        self.offsets[(id - 1) as usize] = Some(self.written);
        self.raw(format!("{id} 0 obj\n").as_bytes())
    }

    fn object(&mut self, id: u32, body: &str) -> io::Result<()> {
        self.begin(id)?;
        self.raw(body.as_bytes())?;
        self.raw(b"\nendobj\n")
    }

    fn stream(&mut self, id: u32, dict: &str, data: &[u8]) -> io::Result<()> {
        self.begin(id)?;
        self.raw(format!("<< {dict} /Length {} >>\nstream\n", data.len()).as_bytes())?;
        self.raw(data)?;
        self.raw(b"\nendstream\nendobj\n")
    }

    /// The next page added starts a bookmark named `title` (a chapter).
    pub fn bookmark(&mut self, title: &str) {
        self.pending_bookmark = Some(title.to_owned());
    }

    /// Images left out because PDF can't hold their format.
    pub fn skipped(&self) -> u32 {
        self.skipped
    }

    /// Adds an image file as one page (or more, when it's very tall). Returns false when its
    /// format can't go in a PDF.
    pub fn add_image(&mut self, bytes: &[u8]) -> io::Result<bool> {
        let Some(picture) = decode(bytes) else {
            self.skipped += 1;
            return Ok(false);
        };
        let mask_id = match &picture.alpha {
            Some(alpha) => {
                let id = self.reserve();
                let data = deflate(alpha)?;
                self.stream(
                    id,
                    &format!(
                        "/Type /XObject /Subtype /Image /Width {} /Height {} \
                         /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode",
                        picture.width, picture.height
                    ),
                    &data,
                )?;
                Some(id)
            }
            None => None,
        };
        let image_id = self.reserve();
        let mut dict = format!(
            "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace {} \
             /BitsPerComponent 8 /Filter {}",
            picture.width, picture.height, picture.color_space, picture.filter
        );
        if picture.inverted {
            dict.push_str(" /Decode [1 0 1 0 1 0 1 0]");
        }
        if let Some(mask) = mask_id {
            dict.push_str(&format!(" /SMask {mask} 0 R"));
        }
        self.stream(image_id, &dict, &picture.data)?;

        // Too wide: scale down. Too tall: split into pages that each show a slice.
        let scale = (MAX_SIDE / f64::from(picture.width)).min(1.0);
        let width = f64::from(picture.width) * scale;
        let height = f64::from(picture.height) * scale;
        let mut top = 0.0;
        while height - top > 0.01 {
            let slice = (height - top).min(MAX_SIDE);
            let bottom = slice + top - height;
            let content = format!("q {width:.2} 0 0 {height:.2} 0 {bottom:.2} cm /Im0 Do Q");
            let content_id = self.reserve();
            self.stream(content_id, "", content.as_bytes())?;
            let page_id = self.reserve();
            self.object(
                page_id,
                &format!(
                    "<< /Type /Page /Parent {PAGES} 0 R /MediaBox [0 0 {width:.2} {slice:.2}] \
                     /Resources << /XObject << /Im0 {image_id} 0 R >> >> /Contents {content_id} 0 R >>"
                ),
            )?;
            if let Some(title) = self.pending_bookmark.take() {
                self.bookmarks.push((title, page_id));
            }
            self.pages.push(page_id);
            top += slice;
        }
        Ok(true)
    }

    /// Writes the page tree, bookmarks, document info and cross-reference table.
    pub fn finish(mut self, title: &str) -> io::Result<W> {
        if self.pages.is_empty() {
            return Err(io::Error::other("None of the pages could go into a PDF"));
        }
        let kids = self
            .pages
            .iter()
            .map(|id| format!("{id} 0 R"))
            .collect::<Vec<_>>()
            .join(" ");
        let count = self.pages.len();
        self.object(
            PAGES,
            &format!("<< /Type /Pages /Kids [{kids}] /Count {count} >>"),
        )?;

        let outlines = if self.bookmarks.is_empty() {
            None
        } else {
            let root = self.reserve();
            let bookmarks = std::mem::take(&mut self.bookmarks);
            let ids: Vec<u32> = bookmarks.iter().map(|_| self.reserve()).collect();
            for (index, (name, page)) in bookmarks.iter().enumerate() {
                let mut item = format!(
                    "<< /Title {} /Parent {root} 0 R /Dest [{page} 0 R /Fit]",
                    text(name)
                );
                if index > 0 {
                    item.push_str(&format!(" /Prev {} 0 R", ids[index - 1]));
                }
                if let Some(next) = ids.get(index + 1) {
                    item.push_str(&format!(" /Next {next} 0 R"));
                }
                item.push_str(" >>");
                self.object(ids[index], &item)?;
            }
            self.object(
                root,
                &format!(
                    "<< /Type /Outlines /First {} 0 R /Last {} 0 R /Count {} >>",
                    ids[0],
                    ids[ids.len() - 1],
                    ids.len()
                ),
            )?;
            Some(root)
        };

        let info = self.reserve();
        self.object(
            info,
            &format!("<< /Title {} /Producer (Raygun) >>", text(title)),
        )?;
        let mut catalog = format!("<< /Type /Catalog /Pages {PAGES} 0 R");
        if let Some(root) = outlines {
            catalog.push_str(&format!(" /Outlines {root} 0 R /PageMode /UseOutlines"));
        }
        catalog.push_str(" >>");
        self.object(CATALOG, &catalog)?;

        let xref_at = self.written;
        let size = self.offsets.len() + 1;
        // Each entry is exactly 20 bytes, its end of line included.
        let mut table = format!("xref\n0 {size}\n0000000000 65535 f \n");
        for offset in &self.offsets {
            let offset = offset.ok_or_else(|| io::Error::other("an object was never written"))?;
            table.push_str(&format!("{offset:010} 00000 n \n"));
        }
        table.push_str(&format!(
            "trailer\n<< /Size {size} /Root {CATALOG} 0 R /Info {info} 0 R >>\nstartxref\n{xref_at}\n%%EOF\n"
        ));
        self.raw(table.as_bytes())?;
        Ok(self.out)
    }
}

/// A PDF text string: UTF-16BE with a byte order mark, in hex, so any script works.
fn text(value: &str) -> String {
    let mut hex = String::from("<FEFF");
    for unit in value.encode_utf16() {
        hex.push_str(&format!("{unit:04X}"));
    }
    hex.push('>');
    hex
}

fn deflate(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data)?;
    encoder.finish()
}

fn decode(bytes: &[u8]) -> Option<Picture> {
    if bytes.starts_with(&[0xFF, 0xD8]) {
        return jpeg(bytes);
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return png(bytes);
    }
    if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return webp(bytes);
    }
    None
}

/// A JPEG's size and colour components, from its frame header; the file goes in unchanged.
fn jpeg(bytes: &[u8]) -> Option<Picture> {
    let mut adobe = false;
    let mut at = 2;
    while at + 4 <= bytes.len() {
        if bytes[at] != 0xFF {
            return None;
        }
        let marker = bytes[at + 1];
        if marker == 0xFF {
            at += 1;
            continue;
        }
        if marker == 0x01 || (0xD0..=0xD8).contains(&marker) {
            at += 2;
            continue;
        }
        let length = usize::from(u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]));
        if marker == 0xEE && bytes.get(at + 4..at + 9) == Some(b"Adobe".as_slice()) {
            adobe = true;
        }
        let frame = (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker);
        if frame {
            let header = bytes.get(at + 4..at + 10)?;
            let height = u32::from(u16::from_be_bytes([header[1], header[2]]));
            let width = u32::from(u16::from_be_bytes([header[3], header[4]]));
            let (color_space, inverted) = match header[5] {
                1 => ("/DeviceGray", false),
                3 => ("/DeviceRGB", false),
                4 => ("/DeviceCMYK", adobe),
                _ => return None,
            };
            if width == 0 || height == 0 {
                return None;
            }
            return Some(Picture {
                width,
                height,
                color_space,
                filter: "/DCTDecode",
                inverted,
                data: bytes.to_vec(),
                alpha: None,
            });
        }
        at += 2 + length;
    }
    None
}

/// Splits interleaved pixels into colour channels and an alpha plane.
fn split_alpha(pixels: &[u8], channels: usize) -> (Vec<u8>, Vec<u8>) {
    let colour = channels - 1;
    let mut data = Vec::with_capacity(pixels.len() / channels * colour);
    let mut alpha = Vec::with_capacity(pixels.len() / channels);
    for pixel in pixels.chunks_exact(channels) {
        data.extend_from_slice(&pixel[..colour]);
        alpha.push(pixel[colour]);
    }
    (data, alpha)
}

/// Flate-compressed pixels, with alpha only when some pixel isn't opaque.
fn flate_picture(
    width: u32,
    height: u32,
    pixels: &[u8],
    channels: usize,
    has_alpha: bool,
) -> Option<Picture> {
    let (raw, alpha) = if has_alpha {
        let (data, alpha) = split_alpha(pixels, channels);
        let opaque = alpha.iter().all(|value| *value == 255);
        (data, (!opaque).then_some(alpha))
    } else {
        (pixels.to_vec(), None)
    };
    let colour = if has_alpha { channels - 1 } else { channels };
    Some(Picture {
        width,
        height,
        color_space: if colour == 1 {
            "/DeviceGray"
        } else {
            "/DeviceRGB"
        },
        filter: "/FlateDecode",
        inverted: false,
        data: deflate(&raw).ok()?,
        alpha,
    })
}

fn png(bytes: &[u8]) -> Option<Picture> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buffer).ok()?;
    let pixels = &buffer[..info.buffer_size()];
    let (channels, has_alpha) = match info.color_type {
        png::ColorType::Grayscale => (1, false),
        png::ColorType::GrayscaleAlpha => (2, true),
        png::ColorType::Rgb => (3, false),
        png::ColorType::Rgba => (4, true),
        png::ColorType::Indexed => return None,
    };
    flate_picture(info.width, info.height, pixels, channels, has_alpha)
}

fn webp(bytes: &[u8]) -> Option<Picture> {
    let mut decoder = image_webp::WebPDecoder::new(Cursor::new(bytes)).ok()?;
    let (width, height) = decoder.dimensions();
    let has_alpha = decoder.has_alpha();
    let mut buffer = vec![0; decoder.output_buffer_size()?];
    decoder.read_image(&mut buffer).ok()?;
    flate_picture(
        width,
        height,
        &buffer,
        if has_alpha { 4 } else { 3 },
        has_alpha,
    )
}
