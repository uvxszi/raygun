//! Saving a cover image (M46.26): the source's largest version, as PNG, JPEG or WebP, into
//! `Pictures/Raygun` or the picked folder; shown large, shared or copied as it came.

use std::sync::Arc;

use raygun_model::{ImageDestination, ImageFileFormat};
use tauri::{AppHandle, Manager};

use super::output::Place;
use super::{ExportService, names};

/// Covers larger than this aren't saved (no cover is).
const MAX_BYTES: usize = 40 * 1024 * 1024;

/// Larger versions of a cover address that well-known image hosts serve, largest first; the
/// address itself comes last.
pub fn larger_versions(url: &str) -> Vec<String> {
    let mut versions = Vec::new();
    // Apple: `…/600x600bb.jpg` (any size) → 3000 px.
    if url.contains("mzstatic.com") {
        let sized = regex_like_size(url, "bb.");
        if let Some(bigger) = sized {
            versions.push(bigger);
        }
    }
    // Deezer: `…/cover/<id>/250x250-000000-80-0-0.jpg` → 1000 px (its `cover_xl`).
    if (url.contains("dzcdn.net") || url.contains("cdn-images.dzcdn"))
        && let Some((head, tail)) = url.rsplit_once('/')
        && let Some((_, rest)) = tail.split_once('-')
    {
        versions.push(format!("{head}/1000x1000-{rest}"));
    }
    // YouTube: any thumbnail → `maxresdefault`.
    if (url.contains("i.ytimg.com/vi/") || url.contains("img.youtube.com/vi/"))
        && let Some((head, _)) = url.rsplit_once('/')
    {
        versions.push(format!("{head}/maxresdefault.jpg"));
        versions.push(format!("{head}/hqdefault.jpg"));
    }
    // TMDB (Stremio posters and backgrounds): `/t/p/w500/<file>` → `/t/p/original/<file>`.
    if url.contains("image.tmdb.org/t/p/")
        && let Some((head, rest)) = url.split_once("/t/p/")
        && let Some((size, file)) = rest.split_once('/')
        && size != "original"
    {
        versions.push(format!("{head}/t/p/original/{file}"));
        if size.starts_with('w') {
            versions.push(format!("{head}/t/p/w1280/{file}"));
        }
    }
    // Metahub (Cinemeta): `/poster/small/<imdb id>/img` → `large`, then `medium`.
    if url.contains("metahub.space/") {
        for kind in ["/poster/", "/background/"] {
            if let Some((head, rest)) = url.split_once(kind)
                && let Some((size, tail)) = rest.split_once('/')
                && size != "large"
            {
                versions.push(format!("{head}{kind}large/{tail}"));
                if size != "medium" {
                    versions.push(format!("{head}{kind}medium/{tail}"));
                }
            }
        }
    }
    // IMDb posters on Amazon's CDN: `…._V1_SX300.jpg` → `…._V1_.jpg` (the full picture).
    if url.contains("media-amazon.com/images/")
        && let Some(at) = url.find("._V1_")
        && let Some(dot) = url.rfind('.')
        && dot > at + 4
    {
        let bigger = format!("{}._V1_{}", &url[..at], &url[dot..]);
        if bigger != url {
            versions.push(bigger);
        }
    }
    // MangaDex: `<file>.256.jpg` / `<file>.512.jpg` thumbnails → the file itself.
    if url.contains("uploads.mangadex.org/covers/") {
        for thumb in [".256.jpg", ".512.jpg"] {
            if let Some(file) = url.strip_suffix(thumb) {
                versions.push(file.to_owned());
            }
        }
    }
    // Last.fm and Audius-style sizes in the path.
    if url.contains("lastfm") && url.contains("/i/u/") {
        versions.push(
            url.replace("/i/u/300x300/", "/i/u/")
                .replace("/i/u/174s/", "/i/u/"),
        );
    }
    if url.contains("150x150.jpg") {
        versions.push(url.replace("150x150.jpg", "1000x1000.jpg"));
    }
    versions.push(url.to_owned());
    versions.dedup();
    versions
}

/// Apple's `<w>x<h>bb.` size → `3000x3000bb.`.
fn regex_like_size(url: &str, marker: &str) -> Option<String> {
    let at = url.rfind(marker)?;
    let head = &url[..at];
    let start = head.rfind('/')? + 1;
    let size = &head[start..];
    let (width, height) = size.split_once('x')?;
    if width.parse::<u32>().is_err() || height.parse::<u32>().is_err() {
        return None;
    }
    Some(format!("{}3000x3000{}", &url[..start], &url[at..]))
}

fn format_of(bytes: &[u8]) -> Option<ImageFileFormat> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some(ImageFileFormat::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(ImageFileFormat::Jpeg)
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(ImageFileFormat::Webp)
    } else if raygun_net::images::is_jxl(bytes) {
        Some(ImageFileFormat::Jxl)
    } else {
        None
    }
}

fn extension(format: ImageFileFormat) -> &'static str {
    match format {
        ImageFileFormat::Png => "png",
        ImageFileFormat::Jpeg => "jpg",
        ImageFileFormat::Webp => "webp",
        ImageFileFormat::Jxl => "jxl",
    }
}

fn mime(format: ImageFileFormat) -> &'static str {
    match format {
        ImageFileFormat::Png => "image/png",
        ImageFileFormat::Jpeg => "image/jpeg",
        ImageFileFormat::Webp => "image/webp",
        ImageFileFormat::Jxl => "image/jxl",
    }
}

/// The image in `format`: as it came when it's already that, else decoded and encoded again.
pub fn convert(bytes: Vec<u8>, format: ImageFileFormat) -> Result<Vec<u8>, String> {
    if format_of(&bytes) == Some(format) {
        return Ok(bytes);
    }
    // A JPEG XL picture is read through PNG; the image crate can't read it itself.
    let bytes = if format_of(&bytes) == Some(ImageFileFormat::Jxl) {
        raygun_net::images::jxl_to_png(&bytes)?
    } else {
        bytes
    };
    let image = image::load_from_memory(&bytes)
        .map_err(|error| format!("couldn't read the image: {error}"))?;
    let mut out = std::io::Cursor::new(Vec::new());
    match format {
        ImageFileFormat::Png => image
            .write_to(&mut out, image::ImageFormat::Png)
            .map_err(|error| error.to_string())?,
        ImageFileFormat::Jpeg => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92);
            image
                .to_rgb8()
                .write_with_encoder(encoder)
                .map_err(|error| error.to_string())?;
        }
        ImageFileFormat::Webp => {
            let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut out);
            image
                .to_rgba8()
                .write_with_encoder(encoder)
                .map_err(|error| error.to_string())?;
        }
        ImageFileFormat::Jxl => return encode_jxl(&image),
    }
    Ok(out.into_inner())
}

/// A lossless JPEG XL of `image` (M46.38), by zune-jpegxl.
fn encode_jxl(image: &image::DynamicImage) -> Result<Vec<u8>, String> {
    use zune_core::bit_depth::BitDepth;
    use zune_core::colorspace::ColorSpace;
    use zune_core::options::EncoderOptions;
    let rgba = image.to_rgba8();
    let options = EncoderOptions::new(
        rgba.width() as usize,
        rgba.height() as usize,
        ColorSpace::RGBA,
        BitDepth::Eight,
    );
    let mut out = Vec::new();
    zune_jpegxl::JxlSimpleEncoder::new(rgba.as_raw(), options)
        .encode(&mut out)
        .map_err(|error| format!("couldn't write JPEG XL: {error:?}"))?;
    Ok(out)
}

/// Proxy links to the larger versions of the cover at `url` (a source address or Raygun's link to
/// one), largest first, each fetched with the cover's own headers; the cover itself comes last.
pub fn shown_versions(app: &AppHandle, url: &str) -> Vec<String> {
    let links = app.state::<raygun_net::ImageLinks>();
    let (original, headers) = links.source(url);
    let mut shown: Vec<String> = larger_versions(&original)
        .into_iter()
        .map(|candidate| {
            if links.is_remote(&candidate) {
                links.cover(&candidate, &headers)
            } else {
                candidate
            }
        })
        .collect();
    if !shown.iter().any(|each| each == url) {
        shown.push(url.to_owned());
    }
    shown.dedup();
    shown
}

/// The largest version of the cover at `url` that its host gives, through the cover cache (so the
/// one the viewer showed isn't fetched twice).
async fn fetch_largest(
    app: &AppHandle,
    service: &ExportService,
    url: &str,
) -> Result<Vec<u8>, String> {
    for candidate in shown_versions(app, url) {
        let Ok(response) = service.client().get(&candidate).send().await else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        if let Ok(body) = response.bytes().await
            && body.len() <= MAX_BYTES
            && format_of(&body).is_some()
        {
            return Ok(body.to_vec());
        }
    }
    Err("The image couldn't be downloaded from its source".into())
}

fn stem(title: &str) -> &str {
    if title.trim().is_empty() {
        "Cover"
    } else {
        title
    }
}

/// Saves the cover at `url` (a source address or Raygun's link to one) as `format`. Returns
/// where it went.
pub async fn save(
    app: &AppHandle,
    url: &str,
    title: &str,
    format: ImageFileFormat,
    destination: ImageDestination,
) -> Result<String, String> {
    let service: Arc<ExportService> = app.state::<Arc<ExportService>>().inner().clone();
    let bytes = fetch_largest(app, &service, url).await?;
    let converted = tauri::async_runtime::spawn_blocking(move || convert(bytes, format))
        .await
        .map_err(|error| error.to_string())??;

    let place = match destination {
        ImageDestination::Pictures => Place::Pictures,
        ImageDestination::Folder => {
            let prefs = service.save_prefs().await;
            super::place_for(&prefs, raygun_model::ExportDestination::Folder)?
        }
    };
    let name = names::file_name(stem(title), extension(format));
    let outputs = service.outputs().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut output = outputs.create(&place, &[], &name, mime(format))?;
        let written = std::io::Write::write_all(&mut output.file, &converted);
        let ok = written.is_ok();
        let saved = outputs.finish(output, ok)?;
        written.map_err(|error| error.to_string())?;
        Ok::<_, String>(saved.shown)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Opens the share sheet for the cover at `url` (its largest version, as it came), or puts it on
/// the clipboard when `copy`. Desktop has no share sheet: sharing saves to Pictures and shows the
/// file in its folder, as reader pages do (M46.6).
pub async fn share(app: &AppHandle, url: &str, title: &str, copy: bool) -> Result<(), String> {
    let service: Arc<ExportService> = app.state::<Arc<ExportService>>().inner().clone();
    let bytes = fetch_largest(app, &service, url).await?;
    // Other apps and the clipboard don't read JPEG XL: it goes as PNG.
    let (bytes, format) = match format_of(&bytes) {
        Some(ImageFileFormat::Jxl) | None => {
            let png =
                tauri::async_runtime::spawn_blocking(move || convert(bytes, ImageFileFormat::Png))
                    .await
                    .map_err(|error| error.to_string())??;
            (png, ImageFileFormat::Png)
        }
        Some(format) => (bytes, format),
    };
    let name = names::file_name(stem(title), extension(format));
    let place = if copy || cfg!(target_os = "android") {
        Place::Share
    } else {
        Place::Pictures
    };
    let outputs = service.outputs().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if place == Place::Share {
            outputs.clear_shared();
        }
        let mut output = outputs.create(&place, &[], &name, mime(format))?;
        let written = std::io::Write::write_all(&mut output.file, &bytes);
        let ok = written.is_ok();
        let saved = outputs.finish(output, ok)?;
        written.map_err(|error| error.to_string())?;
        let path = if place == Place::Share {
            saved.path
        } else {
            saved.local
        }
        .ok_or("The image has no file to hand over")?;
        if copy {
            outputs.copy_image(&path, &name)
        } else {
            outputs.share(&path, mime(format), &name)
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asks_hosts_for_their_largest_cover() {
        assert_eq!(
            larger_versions("https://is1-ssl.mzstatic.com/image/thumb/Music/ab/cd/600x600bb.jpg")
                [0],
            "https://is1-ssl.mzstatic.com/image/thumb/Music/ab/cd/3000x3000bb.jpg"
        );
        assert_eq!(
            larger_versions(
                "https://e-cdns-images.dzcdn.net/images/cover/abc/250x250-000000-80-0-0.jpg"
            )[0],
            "https://e-cdns-images.dzcdn.net/images/cover/abc/1000x1000-000000-80-0-0.jpg"
        );
        assert_eq!(
            larger_versions("https://i.ytimg.com/vi/xyz/mqdefault.jpg")[0],
            "https://i.ytimg.com/vi/xyz/maxresdefault.jpg"
        );
        assert_eq!(
            larger_versions("https://image.tmdb.org/t/p/w500/abc.jpg")[0],
            "https://image.tmdb.org/t/p/original/abc.jpg"
        );
        assert_eq!(
            larger_versions("https://image.tmdb.org/t/p/original/abc.jpg"),
            vec!["https://image.tmdb.org/t/p/original/abc.jpg".to_owned()]
        );
        assert_eq!(
            larger_versions("https://images.metahub.space/poster/small/tt0111161/img"),
            vec![
                "https://images.metahub.space/poster/large/tt0111161/img".to_owned(),
                "https://images.metahub.space/poster/medium/tt0111161/img".to_owned(),
                "https://images.metahub.space/poster/small/tt0111161/img".to_owned(),
            ]
        );
        assert_eq!(
            larger_versions("https://images.metahub.space/background/medium/tt0111161/img")[0],
            "https://images.metahub.space/background/large/tt0111161/img"
        );
        assert_eq!(
            larger_versions("https://m.media-amazon.com/images/M/MV5Babc._V1_SX300.jpg")[0],
            "https://m.media-amazon.com/images/M/MV5Babc._V1_.jpg"
        );
        assert_eq!(
            larger_versions("https://uploads.mangadex.org/covers/id/file.jpg.256.jpg")[0],
            "https://uploads.mangadex.org/covers/id/file.jpg"
        );
        let plain = larger_versions("https://example.com/a.png");
        assert_eq!(plain, vec!["https://example.com/a.png".to_owned()]);
    }

    #[test]
    fn converts_between_formats_and_keeps_one_already_right() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_pixel(4, 4, image::Rgb([200, 30, 30]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let png = png.into_inner();
        assert_eq!(convert(png.clone(), ImageFileFormat::Png).unwrap(), png);
        // JPEG XL out, and read back in (M46.38).
        let jxl = convert(png.clone(), ImageFileFormat::Jxl).unwrap();
        assert_eq!(format_of(&jxl), Some(ImageFileFormat::Jxl));
        let back = convert(jxl, ImageFileFormat::Png).unwrap();
        assert_eq!(format_of(&back), Some(ImageFileFormat::Png));
        let jpeg = convert(png.clone(), ImageFileFormat::Jpeg).unwrap();
        assert_eq!(format_of(&jpeg), Some(ImageFileFormat::Jpeg));
        let webp = convert(png, ImageFileFormat::Webp).unwrap();
        assert_eq!(format_of(&webp), Some(ImageFileFormat::Webp));
    }
}
