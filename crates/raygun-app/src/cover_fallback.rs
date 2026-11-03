//! Covers from other sources (M46.40): when a song's, an album's or an artist's own cover is
//! missing or fails to load, the music sources are asked for theirs (`ExtensionService::
//! cover_options`) and the nearest is used, remembered on this device so it's asked once.
//! Lookups run two at a time, so a list full of missing covers doesn't flood the sources.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use raygun_model::DetailTarget;
use tauri::{AppHandle, Manager};
use tokio::sync::Semaphore;

use crate::extensions::ExtensionService;
use crate::startup::AppDb;

/// The `local_meta` key of the remembered covers (JSON `{ key: [url or null, unix ms] }`).
const STORE: &str = "covers.fallback";
/// A lookup that found nothing is tried again after a day.
const RETRY_MS: i64 = 24 * 60 * 60 * 1000;
/// Remembered covers kept at most; the oldest go first.
const KEPT: usize = 5000;
/// Lookups at once.
const AT_ONCE: usize = 2;

type Remembered = HashMap<String, (Option<String>, i64)>;

pub struct CoverFallbacks {
    app: AppHandle,
    remembered: Mutex<Option<Remembered>>,
    lookups: Semaphore,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

/// What a cover is remembered by: the kind and its normalized names.
fn key(target: &DetailTarget) -> String {
    match target {
        DetailTarget::Song { query } => format!("song:{}", raygun_resolver::track_key(query)),
        DetailTarget::Album { artist, album } => {
            format!(
                "album:{}",
                crate::details::album_key(artist.as_deref(), album)
            )
        }
        DetailTarget::Artist { name } => format!("artist:{}", raygun_resolver::normalize(name)),
        DetailTarget::Playlist { id } => format!("playlist:{id}"),
    }
}

impl CoverFallbacks {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app,
            remembered: Mutex::new(None),
            lookups: Semaphore::new(AT_ONCE),
        })
    }

    async fn load(&self) {
        if self
            .remembered
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some()
        {
            return;
        }
        let db = self.app.state::<AppDb>().0.clone();
        let stored = raygun_db::local_value(&db, STORE)
            .await
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str::<Remembered>(&json).ok())
            .unwrap_or_default();
        let mut slot = self.remembered.lock().unwrap_or_else(|p| p.into_inner());
        slot.get_or_insert(stored);
    }

    fn remembered(&self, key: &str) -> Option<(Option<String>, i64)> {
        self.remembered
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .and_then(|map| map.get(key).cloned())
    }

    async fn remember(&self, key: String, url: Option<String>) {
        let json = {
            let mut slot = self.remembered.lock().unwrap_or_else(|p| p.into_inner());
            let map = slot.get_or_insert_with(HashMap::new);
            map.insert(key, (url, now_ms()));
            if map.len() > KEPT {
                let mut by_age: Vec<(String, i64)> = map
                    .iter()
                    .map(|(key, (_, at))| (key.clone(), *at))
                    .collect();
                by_age.sort_by_key(|(_, at)| *at);
                for (old, _) in by_age.into_iter().take(map.len() - KEPT) {
                    map.remove(&old);
                }
            }
            serde_json::to_string(map).unwrap_or_default()
        };
        let db = self.app.state::<AppDb>().0.clone();
        if let Err(error) = raygun_db::set_local_value(&db, STORE, &json).await {
            tracing::warn!(%error, "could not keep a fallback cover");
        }
    }

    /// Another source's cover for `target`, or `None` when no source has one.
    pub async fn cover(&self, target: &DetailTarget) -> Option<String> {
        // A playlist's cover is its songs' (or the user's).
        if matches!(target, DetailTarget::Playlist { .. }) {
            return None;
        }
        self.load().await;
        let key = key(target);
        if let Some((url, at)) = self.remembered(&key)
            && (url.is_some() || now_ms() - at < RETRY_MS)
        {
            return url;
        }
        let _turn = self.lookups.acquire().await.ok()?;
        // Another lookup may have found it while this one waited.
        if let Some((Some(url), _)) = self.remembered(&key) {
            return Some(url);
        }
        let extensions = self.app.state::<Arc<ExtensionService>>().inner().clone();
        let url = extensions
            .cover_options(target)
            .await
            .into_iter()
            .next()
            .map(|option| option.url);
        self.remember(key, url.clone()).await;
        url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_name_the_kind_and_its_names() {
        let song = DetailTarget::Song {
            query: raygun_model::TrackQuery {
                title: "Get Lucky (Radio Edit)".into(),
                artist: Some("Daft Punk".into()),
                album: None,
                duration_ms: None,
            },
        };
        assert_eq!(key(&song), "song:daft punk|get lucky");
        let artist = DetailTarget::Artist {
            name: "Daft Punk".into(),
        };
        assert_eq!(key(&artist), "artist:daft punk");
    }
}
