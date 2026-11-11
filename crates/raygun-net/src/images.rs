//! Fetching remote images whole: for the page cache (M27) and chapter downloads (M28).
//!
//! Chapter downloads are gentle with sources (M46.1): a page that fails with a network error,
//! HTTP 429 or a 5xx is tried again three times, waiting 2, 4 and 8 s or as long as the server's
//! `Retry-After` asks (up to a minute), and pages already saved are kept when a chapter fails or
//! is paused, so the next attempt fetches only what's missing.

use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use futures_util::{StreamExt, stream};
use reqwest::header;

use crate::page_cache::{MAX_IMAGE_BYTES, sniff_content_type};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    #[error("the server answered HTTP {0}")]
    Status(u16),
    /// HTTP 429 or 503 with a `Retry-After` in seconds.
    #[error("the server is busy (HTTP {0})")]
    Busy(u16, u64),
    #[error("the server didn't send an image")]
    NotImage,
    #[error("the image is larger than 30 MB")]
    TooLarge,
    #[error("{0}")]
    Network(String),
}

/// A fetched image: its bytes and the file extension that fits them.
pub struct Image {
    pub bytes: Vec<u8>,
    pub extension: &'static str,
}

/// Marks a page whose picture its source fetches itself (a converted Mihon source whose client
/// draws or decodes its pictures): `<address>#raygun-source=<provider id>`. The address before it
/// keeps its own fragment, which the source's code may read.
pub const SOURCE_MARK: &str = "#raygun-source=";

/// What fetches a marked page's picture: the source's provider id, the page's address and its
/// headers in; the picture's bytes out.
pub type SourceImages = Arc<
    dyn Fn(
            String,
            String,
            BTreeMap<String, String>,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, ImageError>> + Send>>
        + Send
        + Sync,
>;

static SOURCE_IMAGES: RwLock<Option<SourceImages>> = RwLock::new(None);

/// Sets what fetches the pictures of pages marked with [`SOURCE_MARK`] (the extension host).
pub fn set_source_images(fetch: SourceImages) {
    *SOURCE_IMAGES
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(fetch);
}

/// A marked page's address and its source's provider id.
pub fn source_marked(url: &str) -> Option<(&str, &str)> {
    let at = url.rfind(SOURCE_MARK)?;
    let provider = &url[at + SOURCE_MARK.len()..];
    (!provider.is_empty()).then_some((&url[..at], provider))
}

async fn fetch_through_source(
    url: &str,
    provider: &str,
    headers: &BTreeMap<String, String>,
) -> Result<Image, ImageError> {
    let fetch = SOURCE_IMAGES
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
        .ok_or_else(|| ImageError::Network("the page's source isn't loaded".into()))?;
    let bytes = fetch(provider.to_owned(), url.to_owned(), headers.clone()).await?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(ImageError::TooLarge);
    }
    let sniffed = sniff_content_type(&bytes);
    if is_jxl(&bytes) {
        let png = tokio::task::spawn_blocking(move || jxl_to_png(&bytes))
            .await
            .map_err(|error| ImageError::Network(error.to_string()))?
            .map_err(ImageError::Network)?;
        return Ok(Image {
            bytes: png,
            extension: "png",
        });
    }
    if !sniffed.starts_with("image/") {
        return Err(ImageError::NotImage);
    }
    let extension = extension_for(sniffed, "");
    Ok(Image { bytes, extension })
}

/// Fetches `url` with `headers`, accepting only a successful answer of at most 30 MB that is a
/// picture: declared `image/*`, or starting as one does (some sites send pictures as
/// `text/plain`, `application/octet-stream` or without a type), so error pages are refused.
pub async fn fetch_image(
    client: &reqwest::Client,
    url: &str,
    headers: &BTreeMap<String, String>,
) -> Result<Image, ImageError> {
    if let Some((address, provider)) = source_marked(url) {
        return fetch_through_source(address, provider, headers).await;
    }
    // Pages of a connected server's comics sign in (`server_auth`).
    let response = crate::server_auth::get(client, url, headers)
        .send()
        .await
        .map_err(|error| ImageError::Network(error.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        let retry_after = response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok());
        return Err(match (status.as_u16(), retry_after) {
            (code @ (429 | 503), Some(seconds)) => ImageError::Busy(code, seconds),
            (code, _) => ImageError::Status(code),
        });
    }
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let recipe = Unscramble::from_url(url);
    // A file that's a picture only once its bytes are put back (XORed) comes as anything.
    let bytes_recipe = matches!(recipe, Some(Unscramble::Xor(_)));
    if response
        .content_length()
        .is_some_and(|length| length > MAX_IMAGE_BYTES as u64)
    {
        return Err(ImageError::TooLarge);
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| ImageError::Network(error.to_string()))?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(ImageError::TooLarge);
    }
    // An answer not declared as a picture is one only if its bytes say so (an HTML or JSON
    // error page never does).
    if !content_type.starts_with("image/") && !bytes_recipe && !is_picture(&bytes) {
        return Err(ImageError::NotImage);
    }
    // A page whose file is XORed: the picture it hides, as itself.
    if let Some(plain) = recipe
        .as_ref()
        .and_then(|recipe| recipe.decoded_bytes(&bytes))
    {
        let extension = extension_for(sniff_content_type(&plain), &content_type);
        return Ok(Image {
            bytes: plain,
            extension,
        });
    }
    // A page whose picture comes in shuffled tiles says how to put it back together.
    if let Some(recipe) = recipe {
        let jpeg = tokio::task::spawn_blocking({
            let bytes = bytes.to_vec();
            move || recipe.apply(&bytes)
        })
        .await
        .map_err(|error| ImageError::Network(error.to_string()))?
        .map_err(ImageError::Network)?;
        return Ok(Image {
            bytes: jpeg,
            extension: "jpg",
        });
    }
    // Webviews can't show JPEG XL (M46.38): such pictures are kept and served as PNG.
    if is_jxl(&bytes) {
        let png = tokio::task::spawn_blocking({
            let bytes = bytes.to_vec();
            move || jxl_to_png(&bytes)
        })
        .await
        .map_err(|error| ImageError::Network(error.to_string()))?
        .map_err(ImageError::Network)?;
        return Ok(Image {
            bytes: png,
            extension: "png",
        });
    }
    let extension = extension_for(sniff_content_type(&bytes), &content_type);
    Ok(Image {
        bytes: bytes.to_vec(),
        extension,
    })
}

/// How a page's picture is put back together when its site shuffles it in tiles, as the Mihon
/// sources converted from Keiyoushi's ComiciViewer and GigaViewer themes mark their pages:
/// `#raygun-unscramble=comici:<16 tile indices>` or `#raygun-unscramble=giga` on the address
/// (the fragment never reaches the site).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unscramble {
    /// A 4×4 grid: tile `n` of the result is tile `tiles[n]` of the picture, counted down each
    /// column; the edges past the grid stay.
    Comici(Vec<usize>),
    /// A 4×4 grid of blocks whose sides are multiples of 8, transposed.
    Giga,
    /// The file's bytes XORed with a repeating key (Keiyoushi's `XorInterceptor`, as MANGA Plus
    /// serves its pages): `#raygun-unscramble=xor:<key in hex>`.
    Xor(Vec<u8>),
}

const UNSCRAMBLE: &str = "raygun-unscramble=";

impl Unscramble {
    pub fn from_url(url: &str) -> Option<Self> {
        let fragment = url.split_once('#')?.1;
        let recipe = fragment.strip_prefix(UNSCRAMBLE)?;
        if recipe == "giga" {
            return Some(Self::Giga);
        }
        if let Some(hex) = recipe.strip_prefix("xor:") {
            if hex.is_empty() || !hex.len().is_multiple_of(2) {
                return None;
            }
            let key: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
                .collect::<Option<_>>()?;
            return Some(Self::Xor(key));
        }
        let tiles: Vec<usize> = recipe
            .strip_prefix("comici:")?
            .split(',')
            .map(|tile| tile.trim().parse().ok())
            .collect::<Option<_>>()?;
        (tiles.len() == 16 && tiles.iter().all(|tile| *tile < 16)).then_some(Self::Comici(tiles))
    }

    /// The bytes before they're a picture (a file XORed with a key), when the recipe works on
    /// them rather than on the picture.
    pub fn decoded_bytes(&self, bytes: &[u8]) -> Option<Vec<u8>> {
        match self {
            Self::Xor(key) => Some(
                bytes
                    .iter()
                    .zip(key.iter().cycle())
                    .map(|(byte, key)| byte ^ key)
                    .collect(),
            ),
            _ => None,
        }
    }

    /// The picture put back together, as JPEG.
    pub fn apply(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        if let Some(plain) = self.decoded_bytes(bytes) {
            return Ok(plain);
        }
        let source = if is_jxl(bytes) {
            let png = jxl_to_png(bytes)?;
            image::load_from_memory(&png)
        } else {
            image::load_from_memory(bytes)
        }
        .map_err(|error| format!("couldn't read the page's picture: {error}"))?
        .to_rgb8();
        let (width, height) = source.dimensions();
        let mut result = source.clone();
        let mut copy = |from: (u32, u32), to: (u32, u32), size: (u32, u32)| {
            if size.0 == 0 || size.1 == 0 {
                return;
            }
            let tile =
                image::imageops::crop_imm(&source, from.0, from.1, size.0, size.1).to_image();
            image::imageops::replace(&mut result, &tile, i64::from(to.0), i64::from(to.1));
        };
        match self {
            Self::Comici(tiles) => {
                let (tile_width, tile_height) = (width / 4, height / 4);
                for (destination, source_index) in tiles.iter().enumerate() {
                    let destination = destination as u32;
                    let source_index = *source_index as u32;
                    copy(
                        (
                            (source_index / 4) * tile_width,
                            (source_index % 4) * tile_height,
                        ),
                        (
                            (destination / 4) * tile_width,
                            (destination % 4) * tile_height,
                        ),
                        (tile_width, tile_height),
                    );
                }
            }
            // Done above, on the bytes.
            Self::Xor(_) => {}
            Self::Giga => {
                let block_width = (width / 32) * 8;
                let block_height = (height / 32) * 8;
                for index in 0..16u32 {
                    let destination = (index % 4) * 4 + index / 4;
                    copy(
                        ((index % 4) * block_width, (index / 4) * block_height),
                        (
                            (destination % 4) * block_width,
                            (destination / 4) * block_height,
                        ),
                        (block_width, block_height),
                    );
                }
            }
        }
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90)
            .encode_image(&result)
            .map_err(|error| error.to_string())?;
        Ok(jpeg)
    }
}

/// Whether `bytes` start as a JPEG, PNG, GIF, WebP, AVIF or JPEG XL picture does.
fn is_picture(bytes: &[u8]) -> bool {
    sniff_content_type(bytes).starts_with("image/") || is_jxl(bytes)
}

/// Whether `bytes` are a JPEG XL picture: a bare codestream or the ISO BMFF container.
pub fn is_jxl(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0x0A])
        || bytes.starts_with(&[
            0, 0, 0, 0x0C, b'J', b'X', b'L', b' ', 0x0D, 0x0A, 0x87, 0x0A,
        ])
}

/// A JPEG XL picture as PNG (M46.38), decoded by jxl-oxide.
pub fn jxl_to_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let decoder = jxl_oxide::integration::JxlDecoder::new(std::io::Cursor::new(bytes))
        .map_err(|error| format!("couldn't read the JPEG XL picture: {error}"))?;
    let image = image::DynamicImage::from_decoder(decoder)
        .map_err(|error| format!("couldn't read the JPEG XL picture: {error}"))?;
    let mut png = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(png.into_inner())
}

fn extension_for(sniffed: &str, declared: &str) -> &'static str {
    let mime = if sniffed.starts_with("image/") {
        sniffed
    } else {
        declared
    };
    match mime.split(';').next().unwrap_or_default().trim() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/jxl" => "jxl",
        "image/bmp" => "bmp",
        _ => "img",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DownloadError {
    /// `should_stop` asked to stop; nothing was kept.
    #[error("stopped")]
    Stopped,
    #[error("page {page}: {error}")]
    Page { page: usize, error: ImageError },
    #[error("{0}")]
    Io(String),
}

/// One image to download.
pub struct RemoteImage {
    pub url: String,
    pub headers: BTreeMap<String, String>,
}

/// Tries after a retryable failure (so up to four attempts in all).
const RETRIES: u32 = 3;
/// The first wait before trying again; it doubles each time.
#[cfg(not(test))]
const BACKOFF: Duration = Duration::from_secs(2);
#[cfg(test)]
const BACKOFF: Duration = Duration::from_millis(10);
/// The longest `Retry-After` honoured.
const MAX_RETRY_AFTER: u64 = 60;
/// Marks a partial folder with the page list it holds, so a changed list starts over.
const LIST_MARK: &str = ".pages";

/// Downloads `images` into `dir` as `001.<ext>`, `002.<ext>`, ... (more digits past 999 pages)
/// with `concurrency` requests in flight. Files go to `<dir>.part` first, which is renamed to
/// `dir` once every image is there, so `dir` only ever holds a complete set; a failed or stopped
/// download keeps what it saved there, and the next call fetches only the rest. `on_page` gets the
/// count finished so far; `should_stop` is checked before each image and while waiting to retry.
pub async fn download_images(
    client: &reqwest::Client,
    images: &[RemoteImage],
    dir: &Path,
    concurrency: usize,
    on_page: impl Fn(usize),
    should_stop: impl Fn() -> bool,
) -> Result<(), DownloadError> {
    let partial = partial_dir(dir);
    download_into(
        client,
        images,
        &partial,
        concurrency,
        &on_page,
        &should_stop,
    )
    .await?;
    let io = |error: std::io::Error| DownloadError::Io(error.to_string());
    let _ = tokio::fs::remove_file(partial.join(LIST_MARK)).await;
    if tokio::fs::try_exists(dir).await.unwrap_or(false) {
        tokio::fs::remove_dir_all(dir).await.map_err(io)?;
    }
    tokio::fs::rename(&partial, dir).await.map_err(io)
}

/// The partial folder of a chapter download, for removing it with the chapter.
pub fn partial_folder(dir: &Path) -> PathBuf {
    partial_dir(dir)
}

fn list_signature(images: &[RemoteImage]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    images.len().hash(&mut hasher);
    for image in images {
        image.url.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

/// Page indexes (0-based) already saved in a partial folder.
fn saved_pages(partial: &Path) -> HashSet<usize> {
    let Ok(entries) = std::fs::read_dir(partial) else {
        return HashSet::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.ends_with(".tmp") && !name.starts_with('.'))
        .filter_map(|name| {
            let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<usize>().ok()?.checked_sub(1)
        })
        .collect()
}

async fn download_into(
    client: &reqwest::Client,
    images: &[RemoteImage],
    partial: &Path,
    concurrency: usize,
    on_page: &impl Fn(usize),
    should_stop: &impl Fn() -> bool,
) -> Result<(), DownloadError> {
    let io = |error: std::io::Error| DownloadError::Io(error.to_string());
    let signature = list_signature(images);
    let mark = partial.join(LIST_MARK);
    let same_list = tokio::fs::read_to_string(&mark)
        .await
        .is_ok_and(|saved| saved == signature);
    if !same_list && tokio::fs::try_exists(partial).await.unwrap_or(false) {
        tokio::fs::remove_dir_all(partial).await.map_err(io)?;
    }
    tokio::fs::create_dir_all(partial).await.map_err(io)?;
    tokio::fs::write(&mark, &signature).await.map_err(io)?;
    let saved = {
        let partial = partial.to_owned();
        tokio::task::spawn_blocking(move || saved_pages(&partial))
            .await
            .unwrap_or_default()
    };
    let width = images.len().to_string().len().max(3);
    // A named async fn (not a closure) keeps the stream's future `Send` for spawned callers.
    let fetches: Vec<_> = images
        .iter()
        .enumerate()
        .filter(|(index, _)| !saved.contains(index))
        .map(|(index, image)| save_page(client, index, image, partial, width, should_stop))
        .collect();
    let mut done = images.len() - fetches.len();
    if done > 0 {
        on_page(done);
    }
    let mut pages = stream::iter(fetches).buffer_unordered(concurrency.max(1));
    while let Some(result) = pages.next().await {
        result?;
        done += 1;
        on_page(done);
    }
    Ok(())
}

fn retryable(error: &ImageError) -> bool {
    match error {
        ImageError::Network(_) | ImageError::Busy(..) => true,
        ImageError::Status(code) => *code == 429 || (500..600).contains(code),
        ImageError::NotImage | ImageError::TooLarge => false,
    }
}

async fn save_page(
    client: &reqwest::Client,
    index: usize,
    image: &RemoteImage,
    partial: &Path,
    width: usize,
    should_stop: &impl Fn() -> bool,
) -> Result<(), DownloadError> {
    let mut attempt = 0;
    let fetched = loop {
        if should_stop() {
            return Err(DownloadError::Stopped);
        }
        match fetch_image(client, &image.url, &image.headers).await {
            Ok(fetched) => break fetched,
            Err(error) if attempt < RETRIES && retryable(&error) => {
                let wait = match error {
                    ImageError::Busy(_, seconds) => {
                        Duration::from_secs(seconds.min(MAX_RETRY_AFTER))
                    }
                    _ => BACKOFF * 2u32.pow(attempt),
                };
                attempt += 1;
                tokio::time::sleep(wait).await;
            }
            Err(error) => {
                return Err(DownloadError::Page {
                    page: index + 1,
                    error,
                });
            }
        }
    };
    let name = format!("{:0width$}.{}", index + 1, fetched.extension);
    // Written beside its name first, so a page cut off mid-write is never taken as saved.
    let temporary = partial.join(format!("{name}.tmp"));
    let io = |error: std::io::Error| DownloadError::Io(error.to_string());
    tokio::fs::write(&temporary, &fetched.bytes)
        .await
        .map_err(io)?;
    tokio::fs::rename(&temporary, partial.join(name))
        .await
        .map_err(io)
}

fn partial_dir(dir: &Path) -> PathBuf {
    let mut name = dir.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dir.with_file_name(name)
}

/// Image files of a finished download, in page order.
pub fn downloaded_pages(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort();
    Ok(names)
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::Router;
    use axum::http::HeaderMap;
    use axum::response::{IntoResponse, Response};
    use axum::routing::get;

    use super::*;

    /// A 64×64 picture of 16 flat tiles, each its own shade, counted as the recipe counts them.
    fn tiles(order: impl Fn(u32, u32) -> u8) -> Vec<u8> {
        let picture = image::RgbImage::from_fn(64, 64, |x, y| {
            let shade = order(x / 16, y / 16) * 15;
            image::Rgb([shade, shade, shade])
        });
        let mut png = std::io::Cursor::new(Vec::new());
        picture.write_to(&mut png, image::ImageFormat::Png).unwrap();
        png.into_inner()
    }

    fn shade_at(jpeg: &[u8], x: u32, y: u32) -> u8 {
        let picture = image::load_from_memory(jpeg).unwrap().to_rgb8();
        picture.get_pixel(x, y)[0]
    }

    #[test]
    fn xored_files_are_put_back() {
        let recipe = Unscramble::from_url("https://x/1.jpg#raygun-unscramble=xor:0aff").unwrap();
        assert_eq!(recipe, Unscramble::Xor(vec![0x0a, 0xff]));
        let plain = [0x89, b'P', b'N', b'G', 1, 2];
        let hidden: Vec<u8> = plain
            .iter()
            .zip([0x0a, 0xff].iter().cycle())
            .map(|(byte, key)| byte ^ key)
            .collect();
        assert_eq!(recipe.apply(&hidden).unwrap(), plain);
        assert_eq!(
            Unscramble::from_url("https://x/1.jpg#raygun-unscramble=xor:0"),
            None
        );
    }

    #[test]
    fn shuffled_pages_are_put_back_together() {
        assert_eq!(
            Unscramble::from_url("https://x/1.jpg#raygun-unscramble=giga"),
            Some(Unscramble::Giga)
        );
        assert_eq!(Unscramble::from_url("https://x/1.jpg#scramble"), None);
        assert_eq!(
            Unscramble::from_url("https://x/1.jpg#raygun-unscramble=comici:1,2"),
            None
        );
        // Comici: result tile n (down each column) is source tile tiles[n]; the reverse order
        // here, so the shades come out reversed.
        let reversed: Vec<usize> = (0..16).rev().collect();
        let source = tiles(|column, row| (column * 4 + row) as u8);
        let fixed = Unscramble::Comici(reversed).apply(&source).unwrap();
        assert!(
            shade_at(&fixed, 8, 8).abs_diff(15 * 15) <= 3,
            "the first tile is the last one"
        );
        assert!(
            shade_at(&fixed, 56, 56).abs_diff(0) <= 3,
            "the last tile is the first one"
        );
        // Giga: blocks are transposed (here 16×16 blocks: 64 / 32 * 8).
        let source = tiles(|column, row| (row * 4 + column) as u8);
        let fixed = Unscramble::Giga.apply(&source).unwrap();
        assert!(
            shade_at(&fixed, 8 + 16, 8).abs_diff(4 * 15) <= 3,
            "column 1, row 0 was row 1, column 0"
        );
    }

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest";
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 1, 2];
    const GIF: &[u8] = b"GIF89a\x01\x00\x01\x00";

    static COUNTED: AtomicUsize = AtomicUsize::new(0);
    static FLAKY: AtomicUsize = AtomicUsize::new(0);
    static LATE: AtomicUsize = AtomicUsize::new(0);

    async fn origin() -> SocketAddr {
        async fn png(headers: HeaderMap) -> Response {
            if headers.get("referer").is_none() {
                return axum::http::StatusCode::FORBIDDEN.into_response();
            }
            ([(header::CONTENT_TYPE, "image/png")], PNG).into_response()
        }
        // Declared as WebP, but the bytes say JPEG: the bytes win.
        async fn jpeg() -> Response {
            ([(header::CONTENT_TYPE, "image/webp")], JPEG).into_response()
        }
        async fn html() -> Response {
            ([(header::CONTENT_TYPE, "text/html")], "<p>blocked</p>").into_response()
        }
        // Pictures some sites send under other types, and error pages that aren't pictures.
        async fn as_text() -> Response {
            ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], JPEG).into_response()
        }
        async fn as_bytes() -> Response {
            ([(header::CONTENT_TYPE, "application/octet-stream")], PNG).into_response()
        }
        async fn untyped() -> Response {
            Response::new(axum::body::Body::from(GIF))
        }
        async fn json() -> Response {
            (
                [(header::CONTENT_TYPE, "application/json")],
                r#"{"error":"gone"}"#,
            )
                .into_response()
        }
        async fn text() -> Response {
            ([(header::CONTENT_TYPE, "text/plain")], "Not found").into_response()
        }
        async fn counted() -> Response {
            COUNTED.fetch_add(1, Ordering::SeqCst);
            ([(header::CONTENT_TYPE, "image/png")], PNG).into_response()
        }
        // Busy once (with Retry-After), failing once, then fine.
        async fn flaky() -> Response {
            match FLAKY.fetch_add(1, Ordering::SeqCst) {
                0 => (
                    axum::http::StatusCode::TOO_MANY_REQUESTS,
                    [(header::RETRY_AFTER, "0")],
                )
                    .into_response(),
                1 => axum::http::StatusCode::BAD_GATEWAY.into_response(),
                _ => ([(header::CONTENT_TYPE, "image/png")], PNG).into_response(),
            }
        }
        // Not an image the first time (a failure no retry fixes), an image afterwards.
        async fn late() -> Response {
            if LATE.fetch_add(1, Ordering::SeqCst) == 0 {
                ([(header::CONTENT_TYPE, "text/html")], "<p>wait</p>").into_response()
            } else {
                ([(header::CONTENT_TYPE, "image/jpeg")], JPEG).into_response()
            }
        }
        async fn always_busy() -> Response {
            (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                [(header::RETRY_AFTER, "0")],
            )
                .into_response()
        }
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let router = Router::new()
            .route("/1.png", get(png))
            .route("/2", get(jpeg))
            .route("/html", get(html))
            .route("/as-text", get(as_text))
            .route("/as-bytes", get(as_bytes))
            .route("/untyped", get(untyped))
            .route("/json", get(json))
            .route("/text", get(text))
            .route("/counted.png", get(counted))
            .route("/flaky", get(flaky))
            .route("/busy", get(always_busy))
            .route("/late", get(late));
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        address
    }

    fn image(origin: SocketAddr, path: &str) -> RemoteImage {
        RemoteImage {
            url: format!("http://{origin}{path}"),
            headers: [("Referer".to_owned(), "https://site/".to_owned())].into(),
        }
    }

    #[tokio::test]
    async fn answers_not_declared_as_pictures_are_judged_by_their_bytes() {
        let origin = origin().await;
        let client = crate::http_client().unwrap();
        let fetch = |path: &str| {
            let url = format!("http://{origin}{path}");
            let client = client.clone();
            async move { fetch_image(&client, &url, &BTreeMap::new()).await }
        };
        let picture = fetch("/as-text").await.unwrap();
        assert_eq!((picture.bytes.as_slice(), picture.extension), (JPEG, "jpg"));
        let picture = fetch("/as-bytes").await.unwrap();
        assert_eq!((picture.bytes.as_slice(), picture.extension), (PNG, "png"));
        let picture = fetch("/untyped").await.unwrap();
        assert_eq!((picture.bytes.as_slice(), picture.extension), (GIF, "gif"));
        for page in ["/html", "/json", "/text"] {
            assert_eq!(
                fetch(page).await.err(),
                Some(ImageError::NotImage),
                "{page} isn't a picture"
            );
        }
    }

    #[tokio::test]
    async fn downloads_pages_with_sniffed_extensions() {
        let origin = origin().await;
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path().join("t1").join("c1");
        let progress = AtomicUsize::new(0);
        download_images(
            &crate::http_client().unwrap(),
            &[image(origin, "/1.png"), image(origin, "/2")],
            &dir,
            2,
            |done| progress.store(done, Ordering::SeqCst),
            || false,
        )
        .await
        .unwrap();
        assert_eq!(downloaded_pages(&dir).unwrap(), ["001.png", "002.jpg"]);
        assert_eq!(std::fs::read(dir.join("001.png")).unwrap(), PNG);
        assert_eq!(progress.load(Ordering::SeqCst), 2);
        assert!(!temp.path().join("t1/c1.part").exists());
    }

    #[tokio::test]
    async fn a_bad_page_fails_the_chapter_and_the_next_try_fetches_only_the_rest() {
        let origin = origin().await;
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path().join("c1");
        let client = crate::http_client().unwrap();
        let error = download_images(
            &client,
            &[image(origin, "/counted.png"), image(origin, "/late")],
            &dir,
            1,
            |_| {},
            || false,
        )
        .await
        .unwrap_err();
        assert_eq!(
            error,
            DownloadError::Page {
                page: 2,
                error: ImageError::NotImage
            }
        );
        assert!(!dir.exists());
        // The page it got is kept for the next attempt.
        assert!(temp.path().join("c1.part/001.png").exists());
        let before = COUNTED.load(Ordering::SeqCst);
        let progress = AtomicUsize::new(0);
        download_images(
            &client,
            &[image(origin, "/counted.png"), image(origin, "/late")],
            &dir,
            1,
            |done| progress.store(done, Ordering::SeqCst),
            || false,
        )
        .await
        .unwrap();
        assert_eq!(
            COUNTED.load(Ordering::SeqCst),
            before,
            "page 1 wasn't fetched again"
        );
        assert_eq!(downloaded_pages(&dir).unwrap(), ["001.png", "002.jpg"]);
        assert_eq!(progress.load(Ordering::SeqCst), 2);
        assert!(!temp.path().join("c1.part").exists());
        assert!(!dir.join(".pages").exists());
        std::fs::remove_dir_all(&dir).unwrap();

        let missing = RemoteImage {
            url: format!("http://{origin}/1.png"),
            headers: BTreeMap::new(),
        };
        let error = download_images(&client, &[missing], &dir, 1, |_| {}, || false)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            DownloadError::Page {
                error: ImageError::Status(403),
                ..
            }
        ));

        let stopped = download_images(
            &client,
            &[image(origin, "/1.png")],
            &dir,
            1,
            |_| {},
            || true,
        )
        .await
        .unwrap_err();
        assert_eq!(stopped, DownloadError::Stopped);
    }

    #[tokio::test]
    async fn busy_servers_are_retried_after_a_wait() {
        let origin = origin().await;
        let temp = tempfile::TempDir::new().unwrap();
        let client = crate::http_client().unwrap();
        download_images(
            &client,
            &[image(origin, "/flaky")],
            &temp.path().join("c1"),
            1,
            |_| {},
            || false,
        )
        .await
        .unwrap();
        assert_eq!(FLAKY.load(Ordering::SeqCst), 3);
        let error = download_images(
            &client,
            &[image(origin, "/busy")],
            &temp.path().join("c2"),
            1,
            |_| {},
            || false,
        )
        .await
        .unwrap_err();
        assert_eq!(
            error,
            DownloadError::Page {
                page: 1,
                error: ImageError::Busy(503, 0)
            }
        );
    }
}
