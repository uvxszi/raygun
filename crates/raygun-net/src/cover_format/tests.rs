use std::time::Instant;

use image::{ImageBuffer, Rgb, Rgba};

use super::*;

/// A photo-like picture: smooth gradients with some grain, as covers are.
pub(crate) fn photo(width: u32, height: u32) -> image::RgbImage {
    ImageBuffer::from_fn(width, height, |x, y| {
        let grain = ((x * 7919 + y * 104_729) % 23) as u8;
        Rgb([
            (x * 255 / width) as u8 ^ grain,
            (y * 255 / height) as u8,
            ((x + y) * 127 / (width + height)) as u8 + grain,
        ])
    })
}

pub(crate) fn jpeg_of(picture: &image::RgbImage) -> Vec<u8> {
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 85)
        .encode_image(picture)
        .unwrap();
    jpeg
}

fn png_of(picture: &DynamicImage) -> Vec<u8> {
    let mut png = Cursor::new(Vec::new());
    picture.write_to(&mut png, ImageFormat::Png).unwrap();
    png.into_inner()
}

/// An icon: flat colors on a see-through background.
fn icon() -> DynamicImage {
    DynamicImage::ImageRgba8(ImageBuffer::from_fn(96, 96, |x, y| {
        let inside = (x as i32 - 48).pow(2) + (y as i32 - 48).pow(2) < 40 * 40;
        if inside {
            Rgba([200, (x * 2) as u8 & 0xF0, 40, 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    }))
}

#[test]
fn jpegs_are_recompressed_and_come_back_byte_for_byte() {
    let jpeg = jpeg_of(&photo(320, 240));
    let Conversion::Jxl { bytes, method } = convert(&jpeg) else {
        panic!("a baseline JPEG is recompressed");
    };
    assert_eq!(method, Method::Recompressed);
    assert!(is_jxl(&bytes));
    assert!(bytes.len() < jpeg.len(), "{} ≥ {}", bytes.len(), jpeg.len());
    let shown = displayable(&bytes).unwrap();
    assert_eq!(shown.content_type, "image/jpeg");
    assert_eq!(shown.bytes, jpeg, "the original JPEG comes back");
}

#[test]
fn icons_are_kept_losslessly_and_shown_as_png() {
    let icon = icon();
    let png = png_of(&icon);
    let Conversion::Jxl { bytes, method } = convert(&png) else {
        panic!("a PNG icon is converted");
    };
    assert_eq!(method, Method::Lossless);
    let shown = displayable(&bytes).unwrap();
    assert_eq!(shown.content_type, "image/png");
    let back = image::load_from_memory(&shown.bytes).unwrap();
    assert_eq!(back.to_rgba8(), icon.to_rgba8(), "the same pixels");
}

#[test]
fn lossy_pictures_are_shown_as_jpeg() {
    let picture = DynamicImage::ImageRgb8(photo(200, 150));
    let (pixels, layout) = pixels_of(&picture);
    let jxl = jxl_encoder::LossyConfig::new(LOSSY_DISTANCE)
        .encode(&pixels, 200, 150, layout)
        .unwrap();
    let shown = displayable(&jxl).unwrap();
    assert_eq!(shown.content_type, "image/jpeg");
    let back = image::load_from_memory(&shown.bytes).unwrap();
    assert_eq!((back.width(), back.height()), (200, 150));
}

#[test]
fn what_isnt_converted_stays_as_it_came() {
    let jxl = match convert(&jpeg_of(&photo(320, 240))) {
        Conversion::Jxl { bytes, .. } => bytes,
        Conversion::Original(reason) => panic!("{reason}"),
    };
    for (bytes, why) in [
        (jxl, "already JPEG XL"),
        (b"<svg xmlns='http://www.w3.org/2000/svg'/>".to_vec(), "SVG"),
        (b"\xFF\xD8\xFF\xE0 not really".to_vec(), "a broken JPEG"),
    ] {
        assert!(
            matches!(convert(&bytes), Conversion::Original(_)),
            "{why} stays"
        );
    }
    assert!(displayable(b"\xFF\x0Anot a picture").is_err());
}

#[test]
fn animations_stay_as_they_came() {
    // An APNG's acTL comes before its image data; a WebP's VP8X flags say it's animated.
    let mut apng = png_of(&icon());
    let at = 8 + 25; // after the signature and IHDR
    let actl = [
        0, 0, 0, 8, b'a', b'c', b'T', b'L', 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    apng.splice(at..at, actl);
    assert!(png_animated(&apng));
    assert!(matches!(convert(&apng), Conversion::Original(_)));
    let mut webp = b"RIFF\0\0\0\0WEBPVP8X\x0a\0\0\0\x02\0\0\0\0\0\0\0\0\0".to_vec();
    webp.extend_from_slice(b"ANIM\x06\0\0\0\0\0\0\0\0\0");
    assert_eq!(
        Source::of(&webp),
        Some(Source::WebP {
            lossless: false,
            animated: true
        })
    );
    assert!(matches!(convert(&webp), Conversion::Original(_)));
}

/// Sizes and times over a folder of real covers (Cover Art Archive, YouTube thumbnails, store
/// artwork, icons): `COVERS_DIR=<folder> cargo test -p raygun-net --release -- --ignored
/// cover_jxl_survey --nocapture`.
#[test]
#[ignore = "reads a folder of covers"]
fn cover_jxl_survey() {
    let dir = std::env::var("COVERS_DIR").expect("COVERS_DIR names a folder of covers");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    files.sort();
    let (mut before, mut after, mut convert_ms, mut shown_ms, mut decode_ms) =
        (0usize, 0usize, 0f64, 0f64, 0f64);
    let mut methods = std::collections::BTreeMap::<String, (usize, usize, usize)>::new();
    println!(
        "{:<36} {:>9} {:>9} {:>6} {:>9} {:>8} {:>8} {:>8}  how",
        "file", "size", "stored", "saved", "pixels", "conv ms", "show ms", "dec ms"
    );
    for path in files {
        let original = std::fs::read(&path).unwrap();
        let pixels = image::load_from_memory(&original)
            .map(|picture| u64::from(picture.width()) * u64::from(picture.height()))
            .unwrap_or(0);
        let started = Instant::now();
        let conversion = convert(&original);
        let took = started.elapsed().as_secs_f64() * 1000.0;
        convert_ms += took;
        let (stored, how, shown, decoded) = match &conversion {
            Conversion::Jxl { bytes, method } => {
                let started = Instant::now();
                let shown = displayable(bytes).unwrap();
                let shown_took = started.elapsed().as_secs_f64() * 1000.0;
                let started = Instant::now();
                decode_jxl(bytes).unwrap();
                let decode_took = started.elapsed().as_secs_f64() * 1000.0;
                if *method == Method::Recompressed {
                    assert_eq!(shown.bytes, original, "{}", path.display());
                }
                if let Ok(out) = std::env::var("COVERS_OUT") {
                    let name = path.file_name().unwrap().to_string_lossy().into_owned();
                    std::fs::create_dir_all(&out).unwrap();
                    std::fs::write(std::path::Path::new(&out).join(name + ".jxl"), bytes).unwrap();
                }
                (
                    bytes.len(),
                    format!("{method:?} → shown as {}", shown.content_type),
                    shown_took,
                    decode_took,
                )
            }
            Conversion::Original(reason) => (original.len(), format!("kept: {reason}"), 0.0, 0.0),
        };
        shown_ms += shown;
        decode_ms += decoded;
        before += original.len();
        after += stored;
        let key = how.split(' ').next().unwrap_or_default().to_owned();
        let entry = methods.entry(key).or_default();
        *entry = (entry.0 + 1, entry.1 + original.len(), entry.2 + stored);
        println!(
            "{:<36} {:>9} {:>9} {:>5.1}% {:>9} {:>8.1} {:>8.1} {:>8.1}  {how}",
            path.file_name().unwrap().to_string_lossy(),
            original.len(),
            stored,
            100.0 * (1.0 - stored as f64 / original.len() as f64),
            pixels,
            took,
            shown,
            decoded,
        );
    }
    println!(
        "total {before} → {after} bytes ({:.1}% saved); convert {convert_ms:.0} ms, shown \
         {shown_ms:.0} ms, full decodes {decode_ms:.0} ms",
        100.0 * (1.0 - after as f64 / before as f64)
    );
    for (how, (count, original, stored)) in methods {
        println!(
            "  {how}: {count} files, {original} → {stored} ({:.1}% saved)",
            100.0 * (1.0 - stored as f64 / original as f64)
        );
    }
    if std::env::var("COVERS_EFFORTS").is_ok() {
        efforts(&std::env::var("COVERS_DIR").unwrap());
    }
}

/// PSNR between two pictures of one size, over RGB.
fn psnr(first: &DynamicImage, second: &DynamicImage) -> f64 {
    let (first, second) = (first.to_rgb8(), second.to_rgb8());
    let squared: f64 = first
        .as_raw()
        .iter()
        .zip(second.as_raw())
        .map(|(one, other)| (f64::from(*one) - f64::from(*other)).powi(2))
        .sum();
    let mean = squared / first.as_raw().len() as f64;
    if mean == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (255.0f64.powi(2) / mean).log10()
    }
}

/// Pixel encodes at other efforts, with their size, time and PSNR against the source's pixels.
fn efforts(dir: &str) {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    files.sort();
    println!("\npixel encodes by effort: size / ms / PSNR dB");
    for path in files {
        let original = std::fs::read(&path).unwrap();
        let Some(source) = Source::of(&original) else {
            continue;
        };
        // Recompressed JPEGs never get here.
        if source == Source::Jpeg && recompress(&original).is_ok() {
            continue;
        }
        let picture = image::load_from_memory(&original).unwrap();
        let mut line = format!(
            "{:<36} {:>8}",
            path.file_name().unwrap().to_string_lossy(),
            original.len()
        );
        for effort in [3, 5, 7] {
            let started = Instant::now();
            let result = encode_pixels(&original, source, effort);
            let took = started.elapsed().as_secs_f64() * 1000.0;
            match result {
                Ok((jxl, _)) => {
                    let back = decode_jxl(&jxl).unwrap();
                    line += &format!(
                        " | e{effort} {:>7} {:>6.1} {:>5.1}",
                        jxl.len(),
                        took,
                        psnr(&picture, &back)
                    );
                }
                Err(error) => line += &format!(" | e{effort} failed: {error:.40}"),
            }
        }
        println!("{line}");
    }
}
