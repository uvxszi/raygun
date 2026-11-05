//! Incognito (M47), as Spotify's private session and a browser's incognito window: while it's on,
//! nothing records what's played, watched, read or searched. It's one switch on this device, held
//! in memory, so it ends when Raygun closes, unless `PrivacyPrefs::keep_incognito` keeps it on for
//! the next start.
//!
//! Every history write asks [`incognito`] first and writes nothing while it's on:
//! - plays and play counts (`CollectionsService::record_play`), which are also Made for You's
//!   taste and the history other devices sync; where plays started from (`add_recent_context`);
//!   the queue kept for reopening (`PlaybackService::start_saving`);
//! - quick access: automatic pins (`quick_access::add`) and when its pages were played from
//!   (`quick_access::played`; the UI's copy never replaces them, `quick_access::keep_stamps`);
//! - Continue watching and watching progress (`video::prepare`, `video::record`, which also moves
//!   a series on to its next episode) and a title's last source (`VideoSessions::remember`);
//! - podcast and audiobook positions and played marks (`PodcastService::on_playback`);
//! - reading progress of comics and books (`Catalog::set_progress`);
//! - when lyrics were last shown and lyrics found then (`LyricsStore`), and the tags of a song
//!   looked up on its own (`DiscoveryService::track_tags`);
//! - a website's last address ([`site_address`]);
//! - Search's recent searches (`search_recent_add`);
//! - log lines that name what's played, watched, read or searched ([`log_filter`]).
//!
//! What's saved on purpose still is: favorites, playlists, library titles, downloads, follows,
//! marks made by hand, settings. The UI keeps answers that came while it's on out of the query
//! cache it saves (`shell/queryPersist.ts`).

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use raygun_db::Db;
use raygun_model::IncognitoStatus;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::catalog::now_ms;
use crate::events::IncognitoEvent;
use crate::startup::AppDb;

/// The `local_meta` key saying whether incognito was on, for `keep_incognito`. Device-local.
const STATE_KEY: &str = "privacy.incognito";
const ON: &str = "on";
const OFF: &str = "off";

/// Incognito on this device.
pub struct Privacy {
    /// When it was turned on (Unix ms); 0 while it's off.
    since_ms: AtomicI64,
}

impl Privacy {
    fn new(on: bool) -> Self {
        QUIET_LOGS.store(on, Ordering::SeqCst);
        Self {
            since_ms: AtomicI64::new(if on { now_ms().max(1) } else { 0 }),
        }
    }

    /// Incognito as Raygun starts: on only when it was on as Raygun closed and it's kept across
    /// starts. Otherwise it's off, and stored as off, so keeping it later doesn't bring back an
    /// old session.
    pub async fn load(db: &Db) -> Self {
        let keep = crate::settings::load(db)
            .await
            .map(|settings| settings.privacy.keep_incognito)
            .unwrap_or(false);
        let stored = raygun_db::local_value(db, STATE_KEY).await.ok().flatten();
        let on = starts_on(keep, stored.as_deref());
        if !on && stored.as_deref() == Some(ON) {
            store(db, false).await;
        }
        if on {
            tracing::info!("incognito kept on from the last run");
        }
        Self::new(on)
    }

    pub fn is_on(&self) -> bool {
        self.since_ms.load(Ordering::SeqCst) > 0
    }

    pub fn status(&self) -> IncognitoStatus {
        let since = self.since_ms.load(Ordering::SeqCst);
        IncognitoStatus {
            on: since > 0,
            since_ms: (since > 0).then_some(since as f64),
        }
    }

    /// Turns it on (keeping when it started if it already was) or off; true when it changed.
    fn switch(&self, on: bool) -> bool {
        let changed = if on {
            self.since_ms
                .compare_exchange(0, now_ms().max(1), Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        } else {
            self.since_ms.swap(0, Ordering::SeqCst) > 0
        };
        QUIET_LOGS.store(self.is_on(), Ordering::SeqCst);
        changed
    }
}

/// Incognito as the log files see it: they start before the app's state does.
static QUIET_LOGS: AtomicBool = AtomicBool::new(false);

/// Log fields that name what's played, watched, read or searched; `row.song.track_key` counts by
/// its last part.
const NAMING_FIELDS: [&str; 17] = [
    "track",
    "track_key",
    "song",
    "title",
    "book",
    "episode",
    "chapter",
    "series",
    "key",
    "url",
    "query",
    "offered",
    "candidate",
    "file",
    "rel",
    "json",
    "raw",
];

fn names_content(field: &str) -> bool {
    let last = field.rsplit('.').next().unwrap_or(field);
    NAMING_FIELDS.contains(&last)
}

/// The log files' filter: while incognito is on, lines that name what's played, watched, read or
/// searched aren't written. Every other line, errors among them, still is.
pub fn log_filter<S>() -> impl tracing_subscriber::layer::Filter<S> {
    quiet_while(&QUIET_LOGS)
}

fn quiet_while<S>(quiet: &'static AtomicBool) -> impl tracing_subscriber::layer::Filter<S> {
    tracing_subscriber::filter::DynFilterFn::new(
        move |metadata: &tracing::Metadata<'_>, _: &tracing_subscriber::layer::Context<'_, S>| {
            !(quiet.load(Ordering::Relaxed)
                && metadata
                    .fields()
                    .iter()
                    .any(|field| names_content(field.name())))
        },
    )
    // Asked for every line, since incognito comes and goes.
    .with_callsite_filter(|_| tracing::subscriber::Interest::sometimes())
}

/// Incognito starts on only when it was left on and is kept across starts.
fn starts_on(keep: bool, stored: Option<&str>) -> bool {
    keep && stored == Some(ON)
}

async fn store(db: &Db, on: bool) {
    if let Err(error) = raygun_db::set_local_value(db, STATE_KEY, if on { ON } else { OFF }).await {
        tracing::warn!(%error, "couldn't keep whether incognito is on");
    }
}

/// Whether incognito is on: history writes check this and write nothing while it is.
pub fn incognito(app: &AppHandle) -> bool {
    app.try_state::<Privacy>()
        .is_some_and(|privacy| privacy.is_on())
}

/// One write of the stored state at a time.
static STORING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Turns incognito on or off; every window hears of it (`IncognitoEvent`).
pub async fn set_incognito(app: &AppHandle, on: bool) -> Result<IncognitoStatus, String> {
    let privacy = app
        .try_state::<Privacy>()
        .ok_or("Incognito isn't ready yet")?;
    let changed = privacy.switch(on);
    let status = privacy.status();
    if changed {
        tracing::info!(on, "incognito");
        if let Err(error) = IncognitoEvent(status).emit(app) {
            tracing::warn!(%error, "failed to emit the incognito change");
        }
        // The state as it is when written, so turning it on and off quickly keeps the last.
        let _turn = STORING.lock().await;
        store(&app.state::<AppDb>().0, privacy.is_on()).await;
    }
    Ok(status)
}

/// The address to keep for a website (M46.3) after a visit: where it was, unless incognito is on,
/// when it stays where it was left before.
pub async fn site_address(
    app: &AppHandle,
    db: &Db,
    id: &str,
    visited: Option<String>,
) -> Option<String> {
    if !incognito(app) {
        return visited;
    }
    left_at(db, id).await
}

async fn left_at(db: &Db, id: &str) -> Option<String> {
    raygun_db::websites::get(db, id)
        .await
        .ok()
        .flatten()
        .and_then(|site| site.last_url)
}

#[cfg(test)]
mod tests {
    use raygun_model::{PrivacyPrefs, SettingChange};
    use tempfile::TempDir;

    use super::*;

    async fn open(temp: &TempDir) -> Db {
        Db::open(&temp.path().join("raygun.db")).await.unwrap()
    }

    #[test]
    fn incognito_ends_with_the_app_unless_kept() {
        assert!(!starts_on(false, None));
        assert!(
            !starts_on(false, Some(ON)),
            "not kept: a new start is out of it"
        );
        assert!(!starts_on(true, None));
        assert!(!starts_on(true, Some(OFF)));
        assert!(starts_on(true, Some(ON)));
    }

    #[test]
    fn turning_it_on_keeps_when_it_started() {
        let privacy = Privacy::new(false);
        assert_eq!(
            privacy.status(),
            IncognitoStatus {
                on: false,
                since_ms: None
            }
        );
        assert!(privacy.switch(true));
        let since = privacy.status().since_ms.expect("on since now");
        assert!(privacy.is_on());
        assert!(!privacy.switch(true), "already on");
        assert_eq!(privacy.status().since_ms, Some(since));
        assert!(privacy.switch(false));
        assert!(!privacy.is_on());
        assert!(!privacy.switch(false), "already off");
        assert_eq!(privacy.status().since_ms, None);
    }

    #[tokio::test]
    async fn a_kept_session_comes_back_and_an_old_one_doesnt() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        assert!(!Privacy::load(&db).await.is_on(), "off on a first start");

        // Left on, not kept: the next start is out of it, and stays out once kept later.
        store(&db, true).await;
        assert!(!Privacy::load(&db).await.is_on());
        assert_eq!(
            raygun_db::local_value(&db, STATE_KEY)
                .await
                .unwrap()
                .as_deref(),
            Some(OFF)
        );
        crate::settings::update(
            &db,
            SettingChange::Privacy(PrivacyPrefs {
                keep_incognito: true,
                ..PrivacyPrefs::default()
            }),
        )
        .await
        .unwrap();
        assert!(!Privacy::load(&db).await.is_on());

        // Left on while kept: it's still on, since this start.
        store(&db, true).await;
        let privacy = Privacy::load(&db).await;
        assert!(privacy.is_on());
        assert!(privacy.status().since_ms.is_some());
    }

    #[test]
    fn log_lines_naming_what_plays_are_left_out_while_incognito() {
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::Layer;
        use tracing_subscriber::layer::SubscriberExt;

        /// The fields of each line written.
        struct Lines(Arc<Mutex<Vec<String>>>);
        impl<S: tracing::Subscriber> Layer<S> for Lines {
            fn on_event(
                &self,
                event: &tracing::Event<'_>,
                _: tracing_subscriber::layer::Context<'_, S>,
            ) {
                let mut fields: Vec<&str> = event
                    .metadata()
                    .fields()
                    .iter()
                    .map(|field| field.name())
                    .collect();
                fields.sort_unstable();
                self.0.lock().unwrap().push(fields.join(","));
            }
        }

        static QUIET: AtomicBool = AtomicBool::new(false);
        let lines = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry()
            .with(Lines(lines.clone()).with_filter(quiet_while(&QUIET)));
        let log = || {
            tracing::info!(
                track = "nina simone|feeling good",
                "stream candidates ranked"
            );
            tracing::info!(
                row.song.track_key = "nina simone|feeling good",
                "song downloaded"
            );
            tracing::info!(on = true, "incognito");
            tracing::warn!(error = "timed out", "a source failed");
        };
        tracing::subscriber::with_default(subscriber, || {
            log();
            QUIET.store(true, Ordering::SeqCst);
            log();
            QUIET.store(false, Ordering::SeqCst);
            tracing::info!(track = "back out of it", "playing cached stream");
        });
        assert_eq!(
            *lines.lock().unwrap(),
            [
                "message,track",
                "message,row.song.track_key",
                "message,on",
                "error,message",
                "message,on",
                "error,message",
                "message,track",
            ]
        );
    }

    #[tokio::test]
    async fn a_website_stays_where_it_was_left() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        let id = raygun_db::websites::add(&db, "Archive", "https://archive.org/")
            .await
            .unwrap();
        assert_eq!(left_at(&db, &id).await, None);
        raygun_db::websites::set_state(&db, &id, Some("https://archive.org/details/a"), true)
            .await
            .unwrap();
        assert_eq!(
            left_at(&db, &id).await.as_deref(),
            Some("https://archive.org/details/a")
        );
        assert_eq!(left_at(&db, "gone").await, None);
    }
}
