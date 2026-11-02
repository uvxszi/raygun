//! Image caches and what Raygun stores (M46.2): the lasting cover cache, the reader's page cache,
//! files made for sharing, music artwork and downloads, with their sizes for Settings → Data and
//! their limits from `Settings::cache`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use raygun_db::titles;
use raygun_model::{CachePrefs, StorageArea, StorageUsage};
use raygun_net::cover_cache::Usage;
use raygun_net::proxy_image_token;
use raygun_net::{CoverCache, PageCache};
use raygun_paths::{CacheKind, Paths};
use tauri::{AppHandle, Manager};

use crate::startup::AppDb;

const MB: u64 = 1024 * 1024;
/// The sweep of covers no longer in the library waits this long after start, out of the way.
const SWEEP_DELAY: Duration = Duration::from_secs(30);

pub struct Caches {
    pub pages: PageCache,
    pub covers: CoverCache,
    share: PathBuf,
    music_art: PathBuf,
    downloads: PathBuf,
}

fn refresh_of(prefs: &CachePrefs) -> Duration {
    Duration::from_secs(u64::from(prefs.cover_refresh_days) * 24 * 60 * 60)
}

impl Caches {
    /// The caches with their limits; the page cache starts empty when the settings say so.
    pub fn new(paths: &Paths, prefs: &CachePrefs) -> Self {
        let pages_dir = paths.cache(CacheKind::Pages);
        if prefs.clear_pages_on_start {
            let _ = std::fs::remove_dir_all(&pages_dir);
        }
        Self {
            pages: PageCache::new(pages_dir, u64::from(prefs.page_cache_mb) * MB),
            covers: CoverCache::new(
                paths.cache(CacheKind::Covers),
                u64::from(prefs.cover_cache_mb) * MB,
                refresh_of(prefs),
            ),
            share: paths.cache(CacheKind::Share),
            music_art: paths.cache(CacheKind::Images),
            downloads: paths.downloads(),
        }
    }

    /// New limits from settings.
    pub fn apply(&self, prefs: &CachePrefs) {
        self.pages
            .set_max_bytes(u64::from(prefs.page_cache_mb) * MB);
        self.covers
            .set_limits(u64::from(prefs.cover_cache_mb) * MB, refresh_of(prefs));
    }

    pub async fn usage(&self, prefs: &CachePrefs) -> Vec<StorageUsage> {
        let entry = |area, usage: Usage, limit: Option<u64>, clearable| StorageUsage {
            area,
            bytes: usage.bytes as f64,
            files: usage.files.min(u64::from(u32::MAX)) as u32,
            limit_bytes: limit.map(|limit| limit as f64),
            clearable,
        };
        let (share, music, downloads) = {
            let (share, music, downloads) = (
                self.share.clone(),
                self.music_art.clone(),
                self.downloads.clone(),
            );
            tauri::async_runtime::spawn_blocking(move || {
                (
                    tree_usage(&share),
                    tree_usage(&music),
                    tree_usage(&downloads),
                )
            })
            .await
            .unwrap_or_default()
        };
        vec![
            entry(
                StorageArea::Covers,
                self.covers.usage().await,
                Some(u64::from(prefs.cover_cache_mb) * MB),
                true,
            ),
            entry(
                StorageArea::Pages,
                self.pages.usage().await,
                Some(u64::from(prefs.page_cache_mb) * MB),
                true,
            ),
            entry(StorageArea::Shared, share, None, true),
            entry(StorageArea::MusicArt, music, None, false),
            entry(StorageArea::Downloads, downloads, None, false),
        ]
    }

    pub async fn clear(&self, area: StorageArea) -> Result<(), String> {
        match area {
            StorageArea::Covers => self.covers.clear().await,
            StorageArea::Pages => self.pages.clear().await,
            StorageArea::Shared => {
                let _ = tokio::fs::remove_dir_all(&self.share).await;
            }
            StorageArea::MusicArt => {
                return Err(
                    "Music artwork comes back only with a rescan, so it isn't cleared".into(),
                );
            }
            StorageArea::Downloads => {
                return Err("Remove downloads from Downloads, not here".into());
            }
        }
        Ok(())
    }
}

/// Bytes and files in a folder and everything under it.
fn tree_usage(dir: &Path) -> Usage {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Usage::default();
    };
    let mut usage = Usage::default();
    for entry in entries.filter_map(Result::ok) {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let inner = tree_usage(&entry.path());
            usage.bytes += inner.bytes;
            usage.files += inner.files;
        } else if kind.is_file() {
            usage.bytes += entry.metadata().map_or(0, |metadata| metadata.len());
            usage.files += 1;
        }
    }
    usage
}

/// The covers that stay cached whatever the size limit: every library title's (manga, novels,
/// videos, saved Stremio titles), every followed podcast's, installed extensions' icons and
/// addons' logos.
async fn kept_tokens(app: &AppHandle) -> Result<HashSet<String>, raygun_db::DbError> {
    let db = app.state::<AppDb>().inner().0.clone();
    let mut kept = HashSet::new();
    for kind in ["manga", "novel", "video"] {
        for entry in titles::library(&db, kind).await? {
            if let Some(cover) = &entry.title.fields.cover {
                kept.insert(proxy_image_token(cover, &entry.title.fields.cover_headers));
            }
        }
    }
    for row in raygun_db::podcasts::subscriptions(&db).await? {
        if let Some(artwork) = &row.podcast.fields.artwork {
            kept.insert(proxy_image_token(artwork, &Default::default()));
        }
    }
    // Installed extensions' icons and addons' logos come as kept cover links already.
    let token_of = |link: &str| {
        link.contains("/cover/")
            .then(|| link.rsplit('/').next().unwrap_or_default().to_owned())
    };
    let extensions = app.state::<std::sync::Arc<crate::extensions::ExtensionService>>();
    for info in extensions.list().await {
        kept.extend(info.icon.as_deref().and_then(token_of));
    }
    let stremio = app.state::<std::sync::Arc<crate::stremio::StremioService>>();
    if let Ok(addons) = stremio.addons().await {
        for addon in addons {
            kept.extend(addon.logo.as_deref().and_then(token_of));
        }
    }
    Ok(kept)
}

/// After start, the library's covers are kept and covers of titles that left it are released.
pub fn sweep_later(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SWEEP_DELAY).await;
        match kept_tokens(&app).await {
            Ok(kept) => {
                let (kept_in, released) = app.state::<Caches>().covers.sync_kept(kept).await;
                if kept_in + released > 0 {
                    tracing::info!(
                        kept_in,
                        released,
                        "library covers sorted in the cover cache"
                    );
                }
            }
            Err(error) => tracing::warn!(%error, "could not read the library's covers"),
        }
    });
}
