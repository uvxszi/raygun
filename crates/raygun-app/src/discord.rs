//! Discord presence (desktop): "Listening to <artist>", the song, its cover and a progress bar in
//! the user's Discord status while something plays; nothing while paused. Off by default.
//!
//! Discord's local IPC (a named pipe) needs an application ID, which the user creates once at
//! discord.com/developers/applications and pastes in Settings: Raygun has none of its own to
//! borrow, and using another app's would show that app's name. Phones have no such IPC.

use raygun_model::{DiscordPrefs, PlaybackState, PlaybackStatus, QueueItem};

/// What Discord is told about one moment of playback.
#[derive(Debug, Clone, PartialEq)]
pub struct Presence {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// The cover at its source (Discord fetches it; Raygun's local links don't reach it).
    pub image: Option<String>,
    /// When the song started and ends, unix ms: Discord draws the progress bar from them.
    pub start_ms: i64,
    pub end_ms: Option<i64>,
}

/// The presence for `state`, or `None` when nothing plays.
pub fn presence(
    state: &PlaybackState,
    item: Option<&QueueItem>,
    original: &dyn Fn(&str) -> String,
    now_ms: i64,
) -> Option<Presence> {
    let item = item?;
    if state.status != PlaybackStatus::Playing {
        return None;
    }
    let start_ms = now_ms - state.position_ms.max(0.0) as i64;
    Some(Presence {
        title: item.title.clone(),
        artist: item.artist.clone(),
        album: item.album.clone(),
        image: item
            .artwork_url
            .as_deref()
            .map(original)
            .filter(|url| url.starts_with("https://")),
        start_ms,
        end_ms: state
            .duration_ms
            .filter(|duration| *duration > 0.0)
            .map(|duration| start_ms + duration as i64),
    })
}

/// Whether two presences differ enough to send again: another song, or a seek (the start moved
/// by more than a few seconds).
#[cfg_attr(any(target_os = "android", target_os = "ios"), allow(dead_code))]
fn changed(old: Option<&Presence>, new: Option<&Presence>) -> bool {
    match (old, new) {
        (None, None) => false,
        (Some(old), Some(new)) => {
            old.title != new.title
                || old.artist != new.artist
                || old.image != new.image
                || (old.start_ms - new.start_ms).abs() > 3_000
        }
        _ => true,
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod desktop {
    use std::sync::Mutex;
    use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
    use std::time::{Duration, Instant};

    use discord_rich_presence::activity::{
        Activity, ActivityType, Assets, StatusDisplayType, Timestamps,
    };
    use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

    use super::{DiscordPrefs, Presence, changed};

    /// Discord allows about five updates in 20 s.
    const MIN_GAP: Duration = Duration::from_secs(4);
    /// How long to wait before trying again when Discord isn't running.
    const RETRY: Duration = Duration::from_secs(30);

    enum Message {
        Prefs(DiscordPrefs),
        Presence(Option<Presence>),
    }

    /// Talks to Discord on a thread of its own: its IPC blocks.
    pub struct DiscordPresence {
        sender: Mutex<Sender<Message>>,
    }

    impl DiscordPresence {
        pub fn start(prefs: DiscordPrefs) -> Self {
            let (sender, receiver) = mpsc::channel();
            let _ = sender.send(Message::Prefs(prefs));
            if let Err(error) = std::thread::Builder::new()
                .name("discord-presence".into())
                .spawn(move || run(receiver))
            {
                tracing::warn!(%error, "couldn't start Discord presence");
            }
            Self {
                sender: Mutex::new(sender),
            }
        }

        pub fn set_prefs(&self, prefs: DiscordPrefs) {
            let _ = self
                .sender
                .lock()
                .expect("discord poisoned")
                .send(Message::Prefs(prefs));
        }

        pub fn show(&self, presence: Option<Presence>) {
            let _ = self
                .sender
                .lock()
                .expect("discord poisoned")
                .send(Message::Presence(presence));
        }
    }

    struct Worker {
        prefs: DiscordPrefs,
        client: Option<DiscordIpcClient>,
        /// When connecting may be tried again.
        retry_at: Option<Instant>,
        wanted: Option<Presence>,
        shown: Option<Presence>,
        sent_at: Option<Instant>,
    }

    fn run(receiver: Receiver<Message>) {
        let mut worker = Worker {
            prefs: DiscordPrefs::default(),
            client: None,
            retry_at: None,
            wanted: None,
            shown: None,
            sent_at: None,
        };
        loop {
            match receiver.recv_timeout(Duration::from_secs(1)) {
                Ok(Message::Prefs(prefs)) => {
                    if prefs != worker.prefs {
                        worker.disconnect();
                        worker.prefs = prefs;
                        worker.retry_at = None;
                    }
                }
                Ok(Message::Presence(presence)) => worker.wanted = presence,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    worker.disconnect();
                    return;
                }
            }
            worker.sync();
        }
    }

    impl Worker {
        fn active(&self) -> bool {
            self.prefs.enabled && !self.prefs.app_id.is_empty()
        }

        /// Sends what should show, if it changed and Discord allows another update.
        fn sync(&mut self) {
            if !self.active() {
                self.disconnect();
                return;
            }
            if !changed(self.shown.as_ref(), self.wanted.as_ref()) {
                return;
            }
            if self.sent_at.is_some_and(|sent| sent.elapsed() < MIN_GAP) {
                return;
            }
            if !self.connect() {
                return;
            }
            let Some(client) = self.client.as_mut() else {
                return;
            };
            let result = match &self.wanted {
                Some(presence) => client.set_activity(activity(presence)),
                None => client.clear_activity(),
            };
            self.sent_at = Some(Instant::now());
            match result {
                Ok(()) => self.shown = self.wanted.clone(),
                Err(error) => {
                    tracing::info!(%error, "Discord stopped listening");
                    self.disconnect();
                    self.retry_at = Some(Instant::now() + RETRY);
                }
            }
        }

        fn connect(&mut self) -> bool {
            if self.client.is_some() {
                return true;
            }
            if self.retry_at.is_some_and(|at| Instant::now() < at) {
                return false;
            }
            let mut client = DiscordIpcClient::new(&self.prefs.app_id);
            match client.connect() {
                Ok(()) => {
                    tracing::info!("connected to Discord");
                    self.client = Some(client);
                    self.shown = None;
                    true
                }
                Err(error) => {
                    // Discord isn't running: try again later.
                    tracing::debug!(%error, "no Discord to show presence in");
                    self.retry_at = Some(Instant::now() + RETRY);
                    false
                }
            }
        }

        fn disconnect(&mut self) {
            if let Some(mut client) = self.client.take() {
                let _ = client.clear_activity();
                let _ = client.close();
            }
            self.shown = None;
        }
    }

    fn activity(presence: &Presence) -> Activity<'_> {
        let mut timestamps = Timestamps::new().start(presence.start_ms);
        if let Some(end) = presence.end_ms {
            timestamps = timestamps.end(end);
        }
        let mut activity = Activity::new()
            .activity_type(ActivityType::Listening)
            .status_display_type(StatusDisplayType::State)
            .details(&presence.title)
            .timestamps(timestamps);
        if let Some(artist) = &presence.artist {
            activity = activity.state(artist);
        }
        if let Some(image) = &presence.image {
            let mut assets = Assets::new().large_image(image);
            if let Some(album) = &presence.album {
                assets = assets.large_text(album);
            }
            activity = activity.assets(assets);
        }
        activity
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use desktop::DiscordPresence;

/// Phones have no Discord IPC: nothing to show presence in.
#[cfg(any(target_os = "android", target_os = "ios"))]
pub struct DiscordPresence;

#[cfg(any(target_os = "android", target_os = "ios"))]
impl DiscordPresence {
    pub fn start(_prefs: DiscordPrefs) -> Self {
        Self
    }

    pub fn set_prefs(&self, _prefs: DiscordPrefs) {}

    pub fn show(&self, _presence: Option<Presence>) {}
}

#[cfg(test)]
mod tests {
    use raygun_model::{MediaKind, MediaSource, RepeatMode};

    use super::*;

    fn state(status: PlaybackStatus, position_ms: f64) -> PlaybackState {
        PlaybackState {
            status,
            current_item_id: Some("a".into()),
            position_ms,
            duration_ms: Some(200_000.0),
            volume: 1.0,
            repeat: RepeatMode::Off,
            shuffle: false,
            error: None,
            source_label: None,
            rate: 1.0,
            sleep_at: None,
            stop_after_current: false,
        }
    }

    fn item() -> QueueItem {
        QueueItem {
            id: "a".into(),
            title: "Loser".into(),
            artist: Some("Tame Impala".into()),
            album: Some("Deadbeat".into()),
            duration_ms: Some(200_000),
            artwork_url: Some("http://127.0.0.1:4000/cover/x".into()),
            media_kind: MediaKind::Audio,
            source: MediaSource::Lookup {
                title: "Loser".into(),
                artist: None,
                album: None,
                duration_ms: None,
            },
        }
    }

    #[test]
    fn presence_follows_what_plays() {
        let original = |_: &str| "https://covers.example/a.jpg".to_owned();
        let shown = presence(
            &state(PlaybackStatus::Playing, 30_000.0),
            Some(&item()),
            &original,
            1_000_000,
        )
        .unwrap();
        assert_eq!(shown.start_ms, 970_000);
        assert_eq!(shown.end_ms, Some(1_170_000));
        assert_eq!(shown.image.as_deref(), Some("https://covers.example/a.jpg"));
        assert!(
            presence(
                &state(PlaybackStatus::Paused, 30_000.0),
                Some(&item()),
                &original,
                0
            )
            .is_none(),
            "paused shows nothing"
        );

        // Playing on doesn't resend; a seek does.
        let later = presence(
            &state(PlaybackStatus::Playing, 31_000.0),
            Some(&item()),
            &original,
            1_001_000,
        );
        assert!(!changed(Some(&shown), later.as_ref()));
        let seeked = presence(
            &state(PlaybackStatus::Playing, 90_000.0),
            Some(&item()),
            &original,
            1_001_000,
        );
        assert!(changed(Some(&shown), seeked.as_ref()));
        assert!(changed(Some(&shown), None));
    }
}
