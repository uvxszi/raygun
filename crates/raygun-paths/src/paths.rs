use std::fs;
use std::path::{Path, PathBuf};

use crate::StorageMode;

/// The resolved data root and every well-known location inside it (ARCHITECTURE §3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    root: PathBuf,
    mode: StorageMode,
}

/// Subfolders of `cache/`. Everything in a cache folder may be deleted at any time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheKind {
    Images,
    Http,
    Streams,
    Pages,
    Enhanced,
}

impl CacheKind {
    fn dir_name(self) -> &'static str {
        match self {
            Self::Images => "images",
            Self::Http => "http",
            Self::Streams => "streams",
            Self::Pages => "pages",
            Self::Enhanced => "enhanced",
        }
    }
}

impl Paths {
    pub fn new(root: PathBuf, mode: StorageMode) -> Self {
        Self { root, mode }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn mode(&self) -> StorageMode {
        self.mode
    }

    /// SQLite database holding all structured state.
    pub fn db(&self) -> PathBuf {
        self.root.join("raygun.db")
    }

    /// Human-editable settings.
    pub fn settings(&self) -> PathBuf {
        self.root.join("settings.toml")
    }

    /// Encrypted API keys and tokens.
    pub fn secrets(&self) -> PathBuf {
        self.root.join("secrets.db")
    }

    pub fn extensions(&self) -> PathBuf {
        self.root.join("extensions")
    }

    /// AI models (upscalers, OCR, ONNX).
    pub fn models(&self) -> PathBuf {
        self.root.join("models")
    }

    /// Helper executables such as yt-dlp and ffmpeg.
    pub fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }

    pub fn cache(&self, kind: CacheKind) -> PathBuf {
        self.root.join("cache").join(kind.dir_name())
    }

    /// Media the user downloaded.
    pub fn downloads(&self) -> PathBuf {
        self.root.join("downloads")
    }

    /// Webview profile: cookies, localStorage, IndexedDB.
    pub fn webview(&self) -> PathBuf {
        self.root.join("webview")
    }

    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// Creates the folders needed before anything else starts. Other folders are created by their owners.
    pub fn ensure_base_dirs(&self) -> std::io::Result<()> {
        fs::create_dir_all(self.logs())?;
        fs::create_dir_all(self.webview())
    }
}
