//! What the dictionaries answered, kept on this device under `cache/dictionary/`: a word looked
//! up again opens at once and works offline. A word a source knew is asked again after a month,
//! one it didn't know after three days; while offline, or when the source fails, any kept answer
//! is used however old.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use raygun_model::DictionaryEntry;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// How long a kept answer counts as fresh.
const FOUND_FRESH_MS: u64 = 30 * 24 * 60 * 60 * 1000;
const MISSING_FRESH_MS: u64 = 3 * 24 * 60 * 60 * 1000;
/// Answers kept at most; the least recently written go first.
const MAX_FILES: usize = 4000;
/// The folder is trimmed after this many writes.
const TRIM_EVERY: usize = 100;

#[derive(Serialize, Deserialize)]
struct Stored {
    /// The key as asked, so two keys never share a file.
    key: String,
    saved_ms: u64,
    entry: Option<DictionaryEntry>,
}

/// A kept answer: the entry (none when the source didn't know the word) and its age.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub entry: Option<DictionaryEntry>,
    pub saved_ms: u64,
}

impl Kept {
    pub fn fresh(&self, now_ms: u64) -> bool {
        let lasts = if self.entry.is_some() {
            FOUND_FRESH_MS
        } else {
            MISSING_FRESH_MS
        };
        now_ms.saturating_sub(self.saved_ms) < lasts
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

/// The key one answer is kept by: the source, the words' language, the language translations
/// were read into, and the word.
pub fn key(source: &str, language: &str, target: &str, word: &str) -> String {
    format!("{source}\u{1f}{language}\u{1f}{target}\u{1f}{word}")
}

pub struct Answers {
    dir: PathBuf,
    writes: AtomicUsize,
}

impl Answers {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            writes: AtomicUsize::new(0),
        }
    }

    fn file(&self, key: &str) -> PathBuf {
        let digest = Sha256::digest(key.as_bytes());
        let hex: String = digest[..16]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self.dir.join(format!("{hex}.json"))
    }

    pub async fn get(&self, key: &str) -> Option<Kept> {
        let bytes = tokio::fs::read(self.file(key)).await.ok()?;
        let stored: Stored = serde_json::from_slice(&bytes).ok()?;
        (stored.key == key).then_some(Kept {
            entry: stored.entry,
            saved_ms: stored.saved_ms,
        })
    }

    pub async fn put(&self, key: &str, entry: Option<&DictionaryEntry>) {
        self.put_at(key, entry, now_ms()).await;
    }

    pub async fn put_at(&self, key: &str, entry: Option<&DictionaryEntry>, saved_ms: u64) {
        let stored = Stored {
            key: key.to_owned(),
            saved_ms,
            entry: entry.cloned(),
        };
        let Ok(json) = serde_json::to_vec(&stored) else {
            return;
        };
        let file = self.file(key);
        // Written whole, then put in place: a reader never sees half a file.
        let partial = file.with_extension("part");
        let written = async {
            tokio::fs::create_dir_all(&self.dir).await?;
            tokio::fs::write(&partial, &json).await?;
            tokio::fs::rename(&partial, &file).await
        };
        if let Err(error) = written.await {
            tracing::debug!(%error, "a dictionary's answer wasn't kept");
            return;
        }
        if self.writes.fetch_add(1, Ordering::Relaxed) % TRIM_EVERY == TRIM_EVERY - 1 {
            let dir = self.dir.clone();
            tauri::async_runtime::spawn_blocking(move || trim(&dir, MAX_FILES));
        }
    }

    /// Forgets every kept answer.
    pub async fn clear(&self) -> Result<(), String> {
        match tokio::fs::remove_dir_all(&self.dir).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }

    /// Answers kept and the bytes they take.
    pub fn usage(&self) -> (u32, u64) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return (0, 0);
        };
        entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.metadata().ok())
            .filter(std::fs::Metadata::is_file)
            .fold((0, 0), |(files, bytes), meta| {
                (files + 1, bytes + meta.len())
            })
    }
}

/// Keeps `dir` at most `max` files, dropping the least recently written.
pub fn trim(dir: &Path, max: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    if files.len() <= max {
        return;
    }
    files.sort_by_key(|(modified, _)| *modified);
    let extra = files.len() - max;
    for (_, path) in files.into_iter().take(extra) {
        let _ = std::fs::remove_file(path);
    }
}
