//! Saving a cover image (M46.26): the source's largest version, as PNG, JPEG or WebP, into
//! `Pictures/Raygun` or the picked folder.

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
    } else {
        None
    }
}

fn extension(format: ImageFileFormat) -> &'static str {
    match format {
        ImageFileFormat::Png => "png",
        ImageFileFormat::Jpeg => "jpg",
        ImageFileFormat::Webp => "webp",
    }
}

fn mime(format: ImageFileFormat) -> &'static str {
    match format {
        ImageFileFormat::Png => "image/png",
        ImageFileFormat::Jpeg => "image/jpeg",
        ImageFileFormat::Webp => "image/webp",
    }
}

/// The image in `format`: as it came when it's already that, else decoded and encoded again.
pub fn convert(bytes: Vec<u8>, format: ImageFileFormat) -> Result<Vec<u8>, String> {
    if format_of(&bytes) == Some(format) {
        return Ok(bytes);
    }
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
    }
    Ok(out.into_inner())
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
    let original = app.state::<raygun_net::ImageLinks>().original(url);
    let client = raygun_net::http_client().map_err(|error| error.to_string())?;
    let mut bytes = None;
    for candidate in larger_versions(&original) {
        let Ok(response) = client.get(&candidate).send().await else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        if let Ok(body) = response.bytes().await
            && body.len() <= MAX_BYTES
            && image::guess_format(&body).is_ok()
        {
            bytes = Some(body.to_vec());
            break;
        }
    }
    let bytes = bytes.ok_or("Couldn't download the image")?;
    let converted = tauri::async_runtime::spawn_blocking(move || convert(bytes, format))
        .await
        .map_err(|error| error.to_string())??;

    let service: Arc<ExportService> = app.state::<Arc<ExportService>>().inner().clone();
    let place = match destination {
        ImageDestination::Pictures => Place::Pictures,
        ImageDestination::Folder => {
            let prefs = service.save_prefs().await;
            super::place_for(&prefs, raygun_model::ExportDestination::Folder)?
        }
    };
    let stem = if title.trim().is_empty() {
        "Cover"
    } else {
        title
    };
    let name = names::file_name(stem, extension(format));
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
        let jpeg = convert(png.clone(), ImageFileFormat::Jpeg).unwrap();
        assert_eq!(format_of(&jpeg), Some(ImageFileFormat::Jpeg));
        let webp = convert(png, ImageFileFormat::Webp).unwrap();
        assert_eq!(format_of(&webp), Some(ImageFileFormat::Webp));
    }
}
