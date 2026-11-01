//! A disk cache for remote images (manga pages and covers, M27). Files are named by the image
//! proxy's token, written atomically, and the least recently used ones are removed once the folder
//! grows past its cap.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

/// Images larger than this aren't cached (nor served through the cache path).
pub const MAX_IMAGE_BYTES: usize = 30 * 1024 * 1024;

#[derive(Clone)]
pub struct PageCache {
    dir: PathBuf,
    max_bytes: u64,
    /// Bytes written since the folder was last measured; a scan runs once this is large enough.
    written: Arc<AtomicU64>,
}

impl PageCache {
    pub fn new(dir: PathBuf, max_bytes: u64) -> Self {
        Self {
            dir,
            max_bytes,
            // Start "full" so the first write measures the folder.
            written: Arc::new(AtomicU64::new(u64::MAX / 2)),
        }
    }

    fn path(&self, token: &str) -> Option<PathBuf> {
        // Tokens are hex; anything else never touches the disk.
        token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
            .then(|| self.dir.join(token))
    }

    /// The cached image, marking it recently used.
    pub async fn read(&self, token: &str) -> Option<Vec<u8>> {
        let path = self.path(token)?;
        let bytes = tokio::fs::read(&path).await.ok()?;
        let _ = tokio::task::spawn_blocking(move || touch(&path)).await;
        Some(bytes)
    }

    /// Stores an image: written to a temporary file first, so a failure never leaves half a file.
    pub async fn write(&self, token: &str, bytes: &[u8]) {
        let Some(path) = self.path(token) else { return };
        let temporary = self.dir.join(format!("{token}.part"));
        let result = async {
            tokio::fs::create_dir_all(&self.dir).await?;
            tokio::fs::write(&temporary, bytes).await?;
            tokio::fs::rename(&temporary, &path).await
        }
        .await;
        if let Err(error) = result {
            tracing::debug!(%error, "page cache write failed");
            let _ = tokio::fs::remove_file(&temporary).await;
            return;
        }
        let written = self
            .written
            .fetch_add(bytes.len() as u64, Ordering::Relaxed);
        if written + bytes.len() as u64 > self.max_bytes / 10 {
            self.written.store(0, Ordering::Relaxed);
            let (dir, max) = (self.dir.clone(), self.max_bytes);
            let _ = tokio::task::spawn_blocking(move || evict(&dir, max)).await;
        }
    }
}

fn touch(path: &Path) {
    if let Ok(file) = std::fs::File::options().write(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

/// Removes the least recently used files until the folder is under 90% of `max_bytes`.
fn evict(dir: &Path, max_bytes: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            metadata.is_file().then(|| {
                (
                    metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    metadata.len(),
                    entry.path(),
                )
            })
        })
        .collect();
    let mut total: u64 = files.iter().map(|(_, size, _)| size).sum();
    if total <= max_bytes {
        return;
    }
    files.sort_by_key(|(modified, _, _)| *modified);
    let target = max_bytes / 10 * 9;
    let mut removed = 0;
    for (_, size, path) in files {
        if total <= target {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= size;
            removed += 1;
        }
    }
    tracing::info!(removed, remaining_bytes = total, "page cache trimmed");
}

/// The image type of `bytes` from its first bytes, for serving cached files.
pub fn sniff_content_type(bytes: &[u8]) -> &'static str {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
        [b'G', b'I', b'F', b'8', ..] => "image/gif",
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => "image/webp",
        [
            _,
            _,
            _,
            _,
            b'f',
            b't',
            b'y',
            b'p',
            b'a',
            b'v',
            b'i',
            b'f',
            ..,
        ] => "image/avif",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn writes_atomically_and_reads_back() {
        let temp = tempfile::TempDir::new().unwrap();
        let cache = PageCache::new(temp.path().join("pages"), 1 << 20);
        assert_eq!(cache.read("abc123").await, None);
        cache.write("abc123", b"\x89PNG data").await;
        assert_eq!(
            cache.read("abc123").await.as_deref(),
            Some(&b"\x89PNG data"[..])
        );
        assert!(!temp.path().join("pages/abc123.part").exists());
        cache.write("../escape", b"x").await;
        assert!(
            !temp.path().join("escape").exists(),
            "only hex tokens reach the disk"
        );
    }

    #[tokio::test]
    async fn trims_the_least_recently_used_files() {
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path().join("pages");
        // 1000 bytes allowed; each file is 300.
        let cache = PageCache::new(dir.clone(), 1000);
        for token in ["a1", "b2", "c3"] {
            cache.write(token, &[0u8; 300]).await;
            std::thread::sleep(Duration::from_millis(20));
        }
        // Reading a1 makes it recent, so b2 is the oldest.
        assert!(cache.read("a1").await.is_some());
        std::thread::sleep(Duration::from_millis(20));
        cache.write("d4", &[0u8; 300]).await;
        assert!(!dir.join("b2").exists(), "least recently used goes first");
        assert!(dir.join("a1").exists());
        assert!(dir.join("d4").exists());
    }

    #[test]
    fn sniffs_common_image_types() {
        assert_eq!(sniff_content_type(b"\x89PNG\r\n"), "image/png");
        assert_eq!(sniff_content_type(&[0xFF, 0xD8, 0xFF, 0xE0]), "image/jpeg");
        assert_eq!(sniff_content_type(b"RIFF1234WEBPVP8"), "image/webp");
        assert_eq!(sniff_content_type(b"hello"), "application/octet-stream");
    }
}
