//! Covers for library songs that have none, found in the background. While little else happens
//! (no song loading or looking for a source, no source answering someone, no download running, no
//! battery saver) the songs in playlists (their first songs first: the covers made of them),
//! Favorites, downloads and history without a cover are looked up at the music sources a few at a
//! time ([`CoverFallbacks::search`]). What a source gave is remembered with the source's name and
//! fetched into the cover cache, kept there for saved and downloaded songs (on mobile data it's
//! fetched when shown instead). A song no source had waits a week; when the sources don't answer
//! at all the search rests half an hour; when work starts it stops until things are quiet again.

use std::sync::Arc;
use std::time::{Duration, Instant};

use raygun_db::song_memory::{self, Uncovered};
use raygun_model::{DetailTarget, PlaybackStatus, TrackQuery};
use tauri::{AppHandle, Manager};

use crate::cover_fallback::{CoverFallbacks, Looked};
use crate::startup::AppDb;

/// The first round waits this long after start.
const START_AFTER: Duration = Duration::from_secs(3 * 60);
/// Songs looked up in a round.
const PER_ROUND: u32 = 4;
/// Rests between songs and between rounds; `device_state::rest` stretches them in the
/// background and in battery saver.
const SONG_GAP: Duration = Duration::from_secs(15);
const ROUND_GAP: Duration = Duration::from_secs(3 * 60);
/// Lookups no source answered, in a row, before a longer rest.
const UNANSWERED_IN_A_ROW: u32 = 3;
const BACK_OFF: Duration = Duration::from_secs(30 * 60);
/// How long things must stay quiet before a round, and how often that's looked at.
const QUIET_FOR: Duration = Duration::from_secs(20);
const QUIET_CHECK: Duration = Duration::from_secs(10);
/// With nothing to look up, the search looks again after this long.
const IDLE: Duration = Duration::from_secs(30 * 60);
/// A song no source had a cover for is asked again after a week.
const MISS_RETRY_MS: i64 = 7 * 24 * 60 * 60 * 1000;
/// A playlist's songs its cover is made of (`collections::playlists`' first items).
const COVER_ITEMS: i64 = 12;

/// What else is happening, as the search sees it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Activity {
    pub power_save: bool,
    /// A song loads or another source is looked for.
    pub loading: bool,
    /// Calls someone waits on running at the sources (searches, stream lookups, lyrics…).
    pub source_calls: usize,
    /// Downloads run.
    pub downloading: bool,
}

impl Activity {
    /// Little else is happening: the search may ask the sources.
    pub(crate) fn quiet(&self) -> bool {
        !self.power_save && !self.loading && self.source_calls == 0 && !self.downloading
    }
}

/// How long the search rests after a lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rest {
    /// Before the round's next song.
    Song,
    /// The round is done.
    Round,
    /// The sources didn't answer several times in a row.
    BackOff,
}

impl Rest {
    fn length(self) -> Duration {
        match self {
            Self::Song => SONG_GAP,
            Self::Round => ROUND_GAP,
            Self::BackOff => BACK_OFF,
        }
    }
}

/// The search's pace: a few songs a round, a rest between songs and a longer one between rounds,
/// and a long one after the sources didn't answer several times in a row.
#[derive(Debug, Default)]
pub(crate) struct Pacer {
    in_round: u32,
    unanswered: u32,
}

impl Pacer {
    /// The rest after a lookup that went as `looked`.
    pub(crate) fn after(&mut self, looked: Looked) -> Rest {
        if looked == Looked::Unanswered {
            self.unanswered += 1;
            if self.unanswered >= UNANSWERED_IN_A_ROW {
                *self = Self::default();
                return Rest::BackOff;
            }
        } else {
            self.unanswered = 0;
        }
        self.in_round += 1;
        if self.in_round >= PER_ROUND {
            self.in_round = 0;
            Rest::Round
        } else {
            Rest::Song
        }
    }

    /// Work started: the round ends there, and the next starts afresh once it's quiet.
    pub(crate) fn interrupted(&mut self) {
        self.in_round = 0;
    }
}

pub struct CoverSearch {
    app: AppHandle,
    client: reqwest::Client,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

impl CoverSearch {
    pub fn new(app: AppHandle, client: reqwest::Client) -> Arc<Self> {
        Arc::new(Self { app, client })
    }

    pub fn start(self: &Arc<Self>) {
        let search = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(START_AFTER).await;
            search.run().await;
        });
    }

    /// What else is happening now.
    fn activity(&self) -> Activity {
        let loading = self
            .app
            .try_state::<crate::playback::PlaybackService>()
            .is_some_and(|playback| {
                let state = playback.state();
                state.looking_for_source
                    || matches!(
                        state.status,
                        PlaybackStatus::Loading | PlaybackStatus::Buffering
                    )
            });
        let downloading = self
            .app
            .try_state::<Arc<crate::notifications::DownloadNotices>>()
            .is_some_and(|notices| notices.running());
        Activity {
            power_save: crate::device_state::current().power_save,
            loading,
            source_calls: raygun_ext_host::interactive_calls(),
            downloading,
        }
    }

    /// Returns once nothing else has happened for a little while.
    async fn wait_until_quiet(&self) {
        let mut since: Option<Instant> = None;
        loop {
            if self.activity().quiet() {
                let start = *since.get_or_insert_with(Instant::now);
                if start.elapsed() >= QUIET_FOR {
                    return;
                }
            } else {
                since = None;
            }
            crate::device_state::rest(QUIET_CHECK).await;
        }
    }

    async fn run(&self) {
        let mut pacer = Pacer::default();
        loop {
            // Offline Mode, or Data Saver's part for background lookups: none until they end.
            crate::network_mode::wait_for_lookups().await;
            self.wait_until_quiet().await;
            let db = self.app.state::<AppDb>().0.clone();
            let waiting = song_memory::uncovered(
                &db,
                now_ms() - MISS_RETRY_MS,
                COVER_ITEMS,
                i64::from(PER_ROUND),
            )
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "couldn't list songs without covers");
                Vec::new()
            });
            if waiting.is_empty() {
                crate::device_state::rest(IDLE).await;
                continue;
            }
            for song in waiting {
                // Work started, or lookups rest now: the round stops here.
                if !self.activity().quiet() || crate::network_mode::lookups_rest() {
                    pacer.interrupted();
                    break;
                }
                let looked = self.look_up(&song).await;
                let rest = pacer.after(looked);
                crate::device_state::rest(rest.length()).await;
                if rest != Rest::Song {
                    break;
                }
            }
        }
    }

    /// Looks for one song's cover and caches the picture found.
    async fn look_up(&self, song: &Uncovered) -> Looked {
        let target = DetailTarget::Song {
            query: TrackQuery {
                title: song.title.clone(),
                artist: song.artist.clone(),
                album: song.album.clone(),
                duration_ms: song.duration_ms.and_then(|ms| u32::try_from(ms).ok()),
            },
        };
        // On mobile data the picture loads when it's shown.
        let prefetch = !crate::network::on_mobile_data(&self.app);
        let fetch = |url: String| self.fetch(url, song.saved, prefetch);
        // Under the key the song is stored by, which says it was looked for.
        let key = crate::cover_fallback::song_key(&song.track_key);
        let looked = self
            .app
            .state::<Arc<CoverFallbacks>>()
            .search(&key, &target, fetch)
            .await;
        tracing::debug!(song = %song.track_key, ?looked, "looked for a song's cover");
        looked
    }

    /// Fetches a picture into the cover cache (kept there for saved songs); whether it came.
    async fn fetch(&self, url: String, kept: bool, prefetch: bool) -> bool {
        if !prefetch {
            return true;
        }
        let images = self.app.state::<raygun_net::ImageLinks>();
        let headers = Default::default();
        // The proxy knows the address from now on, as when a list shows it.
        if kept {
            images.kept_cover(&url, &headers);
        } else {
            images.cover(&url, &headers);
        }
        let token = raygun_net::proxy_image_token(&url, &headers);
        let caches = self.app.state::<crate::caches::Caches>();
        match caches
            .covers
            .get(&self.client, &token, &url, &headers, kept)
            .await
        {
            Ok(_) => true,
            Err(error) => {
                tracing::debug!(%error, "a cover found elsewhere didn't download");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_search_waits_while_anything_else_happens() {
        assert!(Activity::default().quiet());
        for busy in [
            Activity {
                power_save: true,
                ..Activity::default()
            },
            Activity {
                loading: true,
                ..Activity::default()
            },
            Activity {
                source_calls: 1,
                ..Activity::default()
            },
            Activity {
                downloading: true,
                ..Activity::default()
            },
        ] {
            assert!(!busy.quiet(), "{busy:?}");
        }
    }

    #[test]
    fn a_few_songs_a_round_and_a_long_rest_when_sources_dont_answer() {
        let mut pacer = Pacer::default();
        // A round: a short rest after each song, a longer one after the last.
        let round: Vec<Rest> = [Looked::Found, Looked::Missing, Looked::Found, Looked::Found]
            .into_iter()
            .map(|looked| pacer.after(looked))
            .collect();
        assert_eq!(round, [Rest::Song, Rest::Song, Rest::Song, Rest::Round]);
        assert_eq!(PER_ROUND as usize, round.len());

        // Work starting ends the round: the next one is a whole round again.
        assert_eq!(pacer.after(Looked::Found), Rest::Song);
        pacer.interrupted();
        let next: Vec<Rest> = (0..PER_ROUND).map(|_| pacer.after(Looked::Found)).collect();
        assert_eq!(next.last(), Some(&Rest::Round));
        assert!(
            next[..next.len() - 1]
                .iter()
                .all(|rest| *rest == Rest::Song)
        );

        // Sources not answering a few times in a row: a long rest, then afresh.
        assert_eq!(pacer.after(Looked::Unanswered), Rest::Song);
        assert_eq!(pacer.after(Looked::Unanswered), Rest::Song);
        assert_eq!(pacer.after(Looked::Unanswered), Rest::BackOff);
        assert_eq!(pacer.after(Looked::Found), Rest::Song);
        // An answer in between starts the count again.
        pacer.after(Looked::Unanswered);
        pacer.after(Looked::Missing);
        pacer.after(Looked::Unanswered);
        assert_ne!(pacer.after(Looked::Unanswered), Rest::BackOff);
        assert!(Rest::BackOff.length() > Rest::Round.length());
        assert!(Rest::Round.length() > Rest::Song.length());
    }
}
