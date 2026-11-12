//! Answers looked up online, kept on this device under `cache/answers/`: a question asked again
//! answers at once and works offline. A found answer is asked again after a week, a question
//! nothing answered after a day; offline, or when the engines fail, any kept answer is used
//! however old. Pictures are kept as their source addresses, linked through the proxy when shown.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use raygun_model::{Answer, AnswerSuggestion};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::dictionary::cache::{now_ms, trim};

/// How long a kept answer counts as fresh.
const FOUND_FRESH_MS: u64 = 7 * 24 * 60 * 60 * 1000;
const MISSING_FRESH_MS: u64 = 24 * 60 * 60 * 1000;
/// Answers kept at most; the least recently written go first.
const MAX_FILES: usize = 2000;
/// The folder is trimmed after this many writes.
const TRIM_EVERY: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Stored {
    key: String,
    saved_ms: u64,
    answers: Vec<Answer>,
    #[serde(default)]
    related: Vec<AnswerSuggestion>,
}

/// A kept answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub answers: Vec<Answer>,
    pub related: Vec<AnswerSuggestion>,
    pub saved_ms: u64,
}

impl Entry {
    pub fn fresh(&self, now_ms: u64) -> bool {
        let lasts = if self.answers.is_empty() {
            MISSING_FRESH_MS
        } else {
            FOUND_FRESH_MS
        };
        now_ms.saturating_sub(self.saved_ms) < lasts
    }
}

pub struct Kept {
    dir: PathBuf,
    writes: AtomicUsize,
}

impl Kept {
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

    pub async fn get(&self, key: &str) -> Option<Entry> {
        let bytes = tokio::fs::read(self.file(key)).await.ok()?;
        let stored: Stored = serde_json::from_slice(&bytes).ok()?;
        (stored.key == key).then_some(Entry {
            answers: stored.answers,
            related: stored.related,
            saved_ms: stored.saved_ms,
        })
    }

    pub async fn put(&self, key: &str, answers: &[Answer], related: &[AnswerSuggestion]) {
        let stored = Stored {
            key: key.to_owned(),
            saved_ms: now_ms(),
            answers: answers.to_vec(),
            related: related.to_vec(),
        };
        let Ok(json) = serde_json::to_vec(&stored) else {
            return;
        };
        let file = self.file(key);
        let partial = file.with_extension("part");
        let written = async {
            tokio::fs::create_dir_all(&self.dir).await?;
            tokio::fs::write(&partial, &json).await?;
            tokio::fs::rename(&partial, &file).await
        };
        if let Err(error) = written.await {
            tracing::debug!(%error, "an answer wasn't kept");
            return;
        }
        if self.writes.fetch_add(1, Ordering::Relaxed) % TRIM_EVERY == TRIM_EVERY - 1 {
            let dir = self.dir.clone();
            tauri::async_runtime::spawn_blocking(move || trim(&dir, MAX_FILES));
        }
    }

    /// Forgets every kept answer (and the exchange rates).
    pub async fn clear(&self) -> Result<(), String> {
        match tokio::fs::remove_dir_all(&self.dir).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }

    /// Kept answers and the bytes they take.
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

/// The key a question's answer is kept by: the engine, the app's language, the words.
pub fn key(engine: &str, language: &str, words: &str) -> String {
    format!("{engine}\u{1f}{language}\u{1f}{}", words.to_lowercase())
}
