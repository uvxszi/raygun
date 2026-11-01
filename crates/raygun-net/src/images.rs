//! Fetching remote images whole: for the page cache (M27) and chapter downloads (M28).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use futures_util::{StreamExt, stream};
use reqwest::header;

use crate::page_cache::{MAX_IMAGE_BYTES, sniff_content_type};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    #[error("the server answered HTTP {0}")]
    Status(u16),
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

/// Fetches `url` with `headers`, accepting only a successful `image/*` answer of at most 30 MB.
pub async fn fetch_image(
    client: &reqwest::Client,
    url: &str,
    headers: &BTreeMap<String, String>,
) -> Result<Image, ImageError> {
    let mut request = client.get(url);
    for (name, value) in headers {
        request = request.header(name, value);
    }
    let response = request
        .send()
        .await
        .map_err(|error| ImageError::Network(error.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(ImageError::Status(status.as_u16()));
    }
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    if !content_type.starts_with("image/") {
        return Err(ImageError::NotImage);
    }
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
    let extension = extension_for(sniff_content_type(&bytes), &content_type);
    Ok(Image {
        bytes: bytes.to_vec(),
        extension,
    })
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

/// Downloads `images` into `dir` as `001.<ext>`, `002.<ext>`, ... with `concurrency` requests in
/// flight. Files go to `<dir>.part` first, which is renamed to `dir` once every image is there, so
/// `dir` only ever holds a complete set. `on_page` gets the count finished so far; `should_stop`
/// is checked before each image.
pub async fn download_images(
    client: &reqwest::Client,
    images: &[RemoteImage],
    dir: &Path,
    concurrency: usize,
    on_page: impl Fn(usize),
    should_stop: impl Fn() -> bool,
) -> Result<(), DownloadError> {
    let partial = partial_dir(dir);
    let result = download_into(
        client,
        images,
        &partial,
        concurrency,
        &on_page,
        &should_stop,
    )
    .await;
    if let Err(error) = result {
        let _ = tokio::fs::remove_dir_all(&partial).await;
        return Err(error);
    }
    let io = |error: std::io::Error| DownloadError::Io(error.to_string());
    if tokio::fs::try_exists(dir).await.unwrap_or(false) {
        tokio::fs::remove_dir_all(dir).await.map_err(io)?;
    }
    tokio::fs::rename(&partial, dir).await.map_err(io)
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
    if tokio::fs::try_exists(partial).await.unwrap_or(false) {
        tokio::fs::remove_dir_all(partial).await.map_err(io)?;
    }
    tokio::fs::create_dir_all(partial).await.map_err(io)?;
    // A named async fn (not a closure) keeps the stream's future `Send` for spawned callers.
    let fetches: Vec<_> = images
        .iter()
        .enumerate()
        .map(|(index, image)| save_page(client, index, image, partial, should_stop))
        .collect();
    let mut pages = stream::iter(fetches).buffer_unordered(concurrency.max(1));
    let mut done = 0;
    while let Some(result) = pages.next().await {
        result?;
        done += 1;
        on_page(done);
    }
    Ok(())
}

async fn save_page(
    client: &reqwest::Client,
    index: usize,
    image: &RemoteImage,
    partial: &Path,
    should_stop: &impl Fn() -> bool,
) -> Result<(), DownloadError> {
    if should_stop() {
        return Err(DownloadError::Stopped);
    }
    let fetched = fetch_image(client, &image.url, &image.headers)
        .await
        .map_err(|error| DownloadError::Page {
            page: index + 1,
            error,
        })?;
    let path = partial.join(format!("{:03}.{}", index + 1, fetched.extension));
    tokio::fs::write(&path, &fetched.bytes)
        .await
        .map_err(|error| DownloadError::Io(error.to_string()))
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

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest";
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 1, 2];

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
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let router = Router::new()
            .route("/1.png", get(png))
            .route("/2", get(jpeg))
            .route("/html", get(html));
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
    async fn a_bad_page_fails_the_chapter_and_keeps_nothing() {
        let origin = origin().await;
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path().join("c1");
        let client = crate::http_client().unwrap();
        let error = download_images(
            &client,
            &[image(origin, "/1.png"), image(origin, "/html")],
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
        assert!(!temp.path().join("c1.part").exists());

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
        assert!(!temp.path().join("c1.part").exists());
    }
}
