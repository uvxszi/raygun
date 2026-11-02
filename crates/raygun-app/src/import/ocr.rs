//! Reading the text lines of a picture (M46.21: songs from screenshots). Windows has an OCR engine
//! of its own (`Windows.Media.Ocr`), which is used there; elsewhere (Android) it's ocrs, an open
//! source engine whose two models (12 MB) are downloaded once through the model catalogue
//! (`ocrs-latin`, SHA-256-checked) and kept in `models/`.

use std::path::Path;

use super::parse::TextLine;

/// The catalogue's ocrs models.
pub const OCRS_MODEL: &str = "ocrs-latin";

/// Whether this device reads pictures without downloading anything first.
pub fn built_in() -> bool {
    cfg!(windows)
}

/// Pictures narrower than this are doubled first: small screenshot text reads far better larger.
const SMALL_WIDTH: u32 = 1600;
/// Below this average brightness (0-255) a picture is dark-themed and is inverted first.
const DARK: u64 = 110;

/// Readies a picture for reading: grey, dark themes turned light (both engines read dark text on a
/// light background best), and small pictures doubled.
fn prepare(bytes: &[u8]) -> Result<image::GrayImage, String> {
    let mut grey = image::load_from_memory(bytes)
        .map_err(|_| "That isn't a picture Raygun can open".to_owned())?
        .into_luma8();
    let pixels = u64::from(grey.width()) * u64::from(grey.height());
    if pixels == 0 {
        return Err("The picture is empty".into());
    }
    let mean = grey
        .pixels()
        .map(|pixel| u64::from(pixel.0[0]))
        .sum::<u64>()
        / pixels;
    if mean < DARK {
        image::imageops::invert(&mut grey);
    }
    if grey.width() < SMALL_WIDTH && grey.height() * 2 <= 10_000 {
        grey = image::imageops::resize(
            &grey,
            grey.width() * 2,
            grey.height() * 2,
            image::imageops::FilterType::CatmullRom,
        );
    }
    Ok(grey)
}

/// The text lines of an image (PNG, JPEG, WebP or GIF bytes), in reading order. `models` is the
/// installed `ocrs-latin` folder where Windows' engine isn't used.
pub fn read(bytes: &[u8], models: Option<&Path>) -> Result<Vec<TextLine>, String> {
    let picture = prepare(bytes)?;
    #[cfg(windows)]
    {
        let _ = models;
        windows_ocr(&picture)
    }
    #[cfg(not(windows))]
    {
        let models = models.ok_or("The text reader isn't downloaded yet")?;
        ocrs_read(&picture, models)
    }
}

#[cfg(windows)]
fn windows_ocr(picture: &image::GrayImage) -> Result<Vec<TextLine>, String> {
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

    // Windows decodes the picture itself, from PNG bytes.
    let mut png = Vec::new();
    image::DynamicImage::ImageLuma8(picture.clone())
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    let bytes = png.as_slice();

    let failed =
        |error: windows::core::Error| format!("Windows couldn't read the picture ({error})");
    let stream = InMemoryRandomAccessStream::new().map_err(failed)?;
    let writer = DataWriter::CreateDataWriter(&stream).map_err(failed)?;
    writer.WriteBytes(bytes).map_err(failed)?;
    writer
        .StoreAsync()
        .map_err(failed)?
        .join()
        .map_err(failed)?;
    writer
        .FlushAsync()
        .map_err(failed)?
        .join()
        .map_err(failed)?;
    writer.DetachStream().map_err(failed)?;
    stream.Seek(0).map_err(failed)?;
    let decoder = BitmapDecoder::CreateAsync(&stream)
        .map_err(failed)?
        .join()
        .map_err(|_| "That isn't a picture Windows can open".to_owned())?;
    let bitmap = decoder
        .GetSoftwareBitmapAsync()
        .map_err(failed)?
        .join()
        .map_err(failed)?;
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|_| "Windows has no text recognizer for your languages".to_owned())?;
    let result = engine
        .RecognizeAsync(&bitmap)
        .map_err(failed)?
        .join()
        .map_err(failed)?;
    let mut lines = Vec::new();
    for line in result.Lines().map_err(failed)? {
        let words = line.Words().map_err(failed)?;
        let Ok(first) = words.GetAt(0) else {
            continue;
        };
        let rect = first.BoundingRect().map_err(failed)?;
        lines.push(TextLine {
            x: rect.X,
            y: rect.Y,
            height: rect.Height,
            text: line.Text().map_err(failed)?.to_string(),
        });
    }
    lines.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    Ok(lines)
}

#[cfg(not(windows))]
fn ocrs_read(picture: &image::GrayImage, models: &Path) -> Result<Vec<TextLine>, String> {
    use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
    use rten::Model;
    use rten_imageproc::BoundingRect;

    let image = image::DynamicImage::ImageLuma8(picture.clone()).into_rgb8();
    let detection = Model::load_file(models.join("text-detection.onnx"))
        .map_err(|error| format!("The text reader couldn't start ({error})"))?;
    let recognition = Model::load_file(models.join("text-recognition.onnx"))
        .map_err(|error| format!("The text reader couldn't start ({error})"))?;
    let engine = OcrEngine::new(OcrEngineParams {
        detection_model: Some(detection),
        recognition_model: Some(recognition),
        ..Default::default()
    })
    .map_err(|error| error.to_string())?;
    let source = ImageSource::from_bytes(image.as_raw(), image.dimensions())
        .map_err(|error| error.to_string())?;
    let input = engine
        .prepare_input(source)
        .map_err(|error| error.to_string())?;
    let words = engine
        .detect_words(&input)
        .map_err(|error| error.to_string())?;
    let rects = engine.find_text_lines(&input, &words);
    let texts = engine
        .recognize_text(&input, &rects)
        .map_err(|error| error.to_string())?;
    let mut lines: Vec<TextLine> = rects
        .iter()
        .zip(texts)
        .filter_map(|(rect, text)| {
            let text = text?.to_string();
            let first = rect.first()?.bounding_rect();
            let height = rect
                .iter()
                .map(|word| word.bounding_rect().height())
                .fold(0.0_f32, f32::max);
            Some(TextLine {
                x: first.left(),
                y: first.top(),
                height,
                text,
            })
        })
        .collect();
    lines.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    Ok(lines)
}
