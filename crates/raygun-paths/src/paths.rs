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
    /// Files handed to the share sheet or clipboard (M46.1).
    Share,
    /// Covers, posters, thumbnails and icons (M46.2).
    Covers,
    /// Episodes' audio fetched for transcribing (M46.32).
    Captions,
    /// Artists' pages as last answered: popular songs, facts, releases, events.
    Artists,
    /// Video exports being converted, until they're copied where they go.
    Convert,
    /// Pieces of torrents being streamed, removed once the video closes.
    Torrents,
    /// What dictionaries answered for words looked up in the book reader.
    Dictionary,
    /// SponsorBlock's parts of YouTube videos, kept a day and swept after a month unused.
    SponsorBlock,
}

impl CacheKind {
    fn dir_name(self) -> &'static str {
        match self {
            Self::Images => "images",
            Self::Http => "http",
            Self::Streams => "streams",
            Self::Pages => "pages",
            Self::Enhanced => "enhanced",
            Self::Share => "share",
            Self::Covers => "covers",
            Self::Captions => "captions",
            Self::Artists => "artists",
            Self::Convert => "convert",
            Self::Torrents => "torrents",
            Self::Dictionary => "dictionary",
            Self::SponsorBlock => "sponsorblock",
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

    /// Optional device-local bootstrap overrides, read before the database opens. User settings live in the database.
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

    /// Saved and exported files where the platform has no shared Downloads/Pictures (M46.1).
    pub fn exports(&self) -> PathBuf {
        self.root.join("exports")
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
