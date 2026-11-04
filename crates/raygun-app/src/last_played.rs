//! What played when Raygun closed (M46.16): the queue, its order, the song and where it was,
//! saved device-local every few seconds and put back, paused, when Raygun opens again.

use raygun_model::{MediaSource, PlayContext, QueueItemInput, RepeatMode};
use raygun_net::ImageLinks;
use raygun_playback::PlaybackCore;
use serde::{Deserialize, Serialize};

use crate::collections::portable_url;

const KEY: &str = "playback.last";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastPlayed {
    pub items: Vec<QueueItemInput>,
    /// Indices into `items` in play order.
    pub order: Vec<usize>,
    /// The index into `items` of the song it was on.
    pub current: usize,
    pub position_ms: f64,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    /// Where the queue was played from (the player's "Playing from").
    #[serde(default)]
    pub context: Option<PlayContext>,
}

/// The core's queue as JSON to save; `{}` when it's empty (nothing to reopen). Links through this
/// run's proxy don't outlive it, so such streams go as lookups and artwork as its source address.
pub fn snapshot(core: &PlaybackCore, original: &dyn Fn(&str) -> String) -> String {
    let queue = core.queue();
    let state = core.state();
    let current = state
        .current_item_id
        .as_ref()
        .and_then(|id| queue.items.iter().position(|item| &item.id == id));
    let Some(current) = current else {
        return "{}".into();
    };
    let last = LastPlayed {
        items: queue
            .items
            .into_iter()
            .map(|item| {
                let source = match &item.source {
                    MediaSource::Url { url, .. } if !portable_url(url) => MediaSource::Lookup {
                        title: item.title.clone(),
                        artist: item.artist.clone(),
                        album: item.album.clone(),
                        duration_ms: item.duration_ms,
                    },
                    source => source.clone(),
                };
                QueueItemInput {
                    explicit: item.explicit,
                    title: item.title,
                    artist: item.artist,
                    album: item.album,
                    duration_ms: item.duration_ms,
                    artwork_url: item
                        .artwork_url
                        .as_deref()
                        .map(original)
                        .filter(|url| portable_url(url)),
                    media_kind: item.media_kind,
                    source,
                }
            })
            .collect(),
        order: queue.order.iter().map(|&index| index as usize).collect(),
        current,
        position_ms: (state.position_ms / 1000.0).floor() * 1000.0,
        shuffle: state.shuffle,
        repeat: state.repeat,
        context: queue.context.map(|context| PlayContext {
            artwork_url: context
                .artwork_url
                .as_deref()
                .map(original)
                .filter(|url| portable_url(url)),
            ..context
        }),
    };
    serde_json::to_string(&last).unwrap_or_else(|_| "{}".into())
}

pub async fn save(db: &raygun_db::Db, json: &str) {
    if let Err(error) = raygun_db::set_local_value(db, KEY, json).await {
        tracing::warn!(%error, "failed to save what plays");
    }
}

/// The saved queue, if there is one.
pub async fn load(db: &raygun_db::Db) -> Option<LastPlayed> {
    let json = raygun_db::local_value(db, KEY).await.ok().flatten()?;
    serde_json::from_str::<LastPlayed>(&json)
        .ok()
        .filter(|last| !last.items.is_empty())
}

/// Saved items with their artwork through this run's cover cache.
pub fn arrived(items: Vec<QueueItemInput>, images: &ImageLinks) -> Vec<QueueItemInput> {
    items
        .into_iter()
        .map(|item| QueueItemInput {
            artwork_url: item
                .artwork_url
                .map(|url| images.cover(&url, &Default::default())),
            ..item
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use raygun_model::{EngineEvent, EngineReport, MediaKind};

    use super::*;

    fn input(title: &str, source: MediaSource) -> QueueItemInput {
        QueueItemInput {
            explicit: false,
            title: title.into(),
            artist: Some("Artist".into()),
            album: None,
            duration_ms: Some(200_000),
            artwork_url: Some("http://127.0.0.1:5000/cover/abc".into()),
            media_kind: MediaKind::Audio,
            source,
        }
    }

    #[test]
    fn the_queue_is_saved_to_outlive_this_run() {
        let mut core = PlaybackCore::new(1);
        assert_eq!(snapshot(&core, &|url| url.to_owned()), "{}");
        core.replace(
            vec![
                input(
                    "file",
                    MediaSource::LocalFile {
                        path: "C:/Music/a.flac".into(),
                    },
                ),
                input(
                    "proxied",
                    MediaSource::Url {
                        url: "http://127.0.0.1:5000/local/b.mp3".into(),
                        headers: BTreeMap::new(),
                    },
                ),
            ],
            Some(1),
            false,
        );
        core.set_context(Some(PlayContext {
            kind: "playlist".into(),
            name: "Road trip".into(),
            subtitle: None,
            artwork_url: Some("http://127.0.0.1:5000/cover/abc".into()),
            to: "/playlist".into(),
            search: BTreeMap::from([("id".into(), "p1".into())]),
            played_at: 0.0,
        }));
        let load_id = core.state().current_item_id.clone();
        assert!(load_id.is_some());
        core.report(EngineReport {
            load_id: 1,
            event: EngineEvent::Progress {
                position_ms: 61_700.0,
                duration_ms: Some(200_000.0),
            },
        });
        let json = snapshot(&core, &|_| "https://covers.example/b.jpg".to_owned());
        let last: LastPlayed = serde_json::from_str(&json).unwrap();
        assert_eq!(last.current, 1);
        assert_eq!(last.position_ms, 61_000.0, "whole seconds");
        assert!(matches!(
            last.items[0].source,
            MediaSource::LocalFile { .. }
        ));
        assert!(matches!(last.items[1].source, MediaSource::Lookup { .. }));
        assert_eq!(
            last.items[0].artwork_url.as_deref(),
            Some("https://covers.example/b.jpg")
        );
        let context = last.context.expect("where it was played from is kept");
        assert_eq!(context.name, "Road trip");
        assert_eq!(
            context.artwork_url.as_deref(),
            Some("https://covers.example/b.jpg"),
            "artwork at its source address"
        );
        // Saves from before contexts still load.
        let old =
            r#"{"items":[],"order":[],"current":0,"positionMs":0,"shuffle":false,"repeat":"off"}"#;
        assert!(
            serde_json::from_str::<LastPlayed>(old)
                .unwrap()
                .context
                .is_none()
        );
    }
}
