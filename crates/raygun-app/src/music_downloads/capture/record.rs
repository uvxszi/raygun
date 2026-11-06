//! Recording what Raygun plays (Android), the last resort of a song download: no source gives a
//! file of it and only YouTube's player plays it, in the webview, where neither Raygun nor its
//! capture player can read the stream. With the person's consent (RECORD_AUDIO, then Android's
//! capture question, asked once for the songs waiting and held by the plugin's
//! `RecordingService` until none waits), the song plays aloud in the webview's recording player
//! while the plugin's `PlaybackRecorder` records Raygun's own sound.
//!
//! It plays in real time: YouTube's faster speeds keep the pitch, which can't be undone the way
//! the capture player's speed is. Android copies each of Raygun's tracks to the recording before
//! it applies its volume, so it records with the media volume down or muted; only the player's
//! own volume would change it, and the recording player's stays full.
//!
//! Runs follow the player: one starts where it plays from once it plays (`recording_mark`), and
//! is held while it buffers or pauses. [`super::sound::capture`] places them, records what's
//! missing again and stitches them, as for a stream. Raygun's own player must stay quiet (the
//! recording would hold whatever it plays): a recording starts only when it is, and stops,
//! waiting to be recorded again, when it plays ([`super::CaptureError::Interrupted`]).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use raygun_model::{EngineEvent, PlaybackStatus, SongRecordingReport, SongRecordingRequest};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::{Notify, mpsc};

use super::CaptureError;
use super::sound::{PlayerState, Report, Session, send_new_sound};
use crate::playback::PlaybackService;

type NativeAudio = tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>;

/// The song download a recording is for, as the recording's card shows it.
#[derive(Debug, Clone, Copy)]
pub struct Song<'a> {
    pub download_id: &'a str,
    pub title: &'a str,
    pub artist: Option<&'a str>,
}

/// One run: the video played from `from_ms`, its pieces in `dir`.
pub(crate) struct Run<'a> {
    pub(crate) video_id: &'a str,
    pub(crate) download_id: &'a str,
    pub(crate) title: &'a str,
    pub(crate) artist: Option<&'a str>,
    pub(crate) from_ms: f64,
    pub(crate) dir: &'a Path,
}

/// The webview's recording players, by session: where their reports go.
static PLAYERS: LazyLock<Mutex<HashMap<String, mpsc::UnboundedSender<EngineEvent>>>> =
    LazyLock::new(Mutex::default);

/// Whether this device records what Raygun plays (Android: the native plugin is there).
pub fn available(app: &AppHandle) -> bool {
    app.try_state::<NativeAudio>().is_some()
}

/// Runs a call into the native plugin off the async threads (it blocks until answered).
async fn native<T: Send + 'static>(
    app: &AppHandle,
    call: impl FnOnce(&NativeAudio) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let app = app.clone();
    tokio::task::spawn_blocking(move || match app.try_state::<NativeAudio>() {
        Some(native) => call(native.inner()),
        None => Err("This device doesn't record what Raygun plays".into()),
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Whether the person's consent to record is held now.
pub async fn ready(app: &AppHandle) -> bool {
    available(app)
        && native(app, NativeAudio::recording_ready)
            .await
            .unwrap_or(false)
}

/// Asks the person to allow recording (Android's questions; Raygun's window must show). Returns
/// whether they did.
pub async fn consent(app: &AppHandle) -> Result<bool, String> {
    let answer = native(app, NativeAudio::recording_consent).await?;
    if let Some(reason) = answer.reason.as_deref().filter(|_| !answer.granted) {
        tracing::info!(%reason, "recording wasn't allowed");
    }
    Ok(answer.granted)
}

/// No song waits to be recorded: the consent goes back, and Android's notice of it.
pub async fn end(app: &AppHandle) {
    if available(app) {
        let _ = native(app, NativeAudio::recording_end).await;
    }
}

/// Whether Raygun's own player plays (or is about to): a recording would hold its sound.
pub(crate) fn player_busy(app: &AppHandle) -> bool {
    app.try_state::<PlaybackService>().is_some_and(|playback| {
        matches!(
            playback.state().status,
            PlaybackStatus::Playing | PlaybackStatus::Loading | PlaybackStatus::Buffering
        )
    })
}

/// Whether Raygun's window is open, its page listening: the recording's player plays in it
/// (Android: the window goes with Raygun's activity, which can close while downloads carry on).
pub(crate) fn window_open(app: &AppHandle) -> bool {
    app.try_state::<PlaybackService>()
        .is_none_or(|playback| playback.webview_present())
}

/// A recording starts only while Raygun's window is open and its player quiet; else it waits to
/// be recorded again.
pub(crate) async fn wait_until_quiet(app: &AppHandle, stop: &Notify) -> Result<(), CaptureError> {
    if player_busy(app) || !window_open(app) {
        return Err(CaptureError::Interrupted);
    }
    // The consent can go away between songs (Android's own Stop on its notice).
    let held = tokio::select! {
        held = ready(app) => held,
        () = stop.notified() => return Err(CaptureError::Stopped),
    };
    if held {
        Ok(())
    } else {
        Err(CaptureError::Interrupted)
    }
}

/// What the recording's player in the webview did. An error tells it the session is over.
pub fn report(report: SongRecordingReport) -> Result<(), String> {
    let sender = PLAYERS
        .lock()
        .expect("recording players poisoned")
        .get(&report.session)
        .cloned()
        .ok_or("That recording was stopped")?;
    sender
        .send(report.event)
        .map_err(|_| "That recording was stopped".to_owned())
}

/// Starts one run: the recorder, then the webview's player at `run.from_ms`.
pub(crate) async fn start_run(app: &AppHandle, run: Run<'_>) -> Result<Session, String> {
    let session = uuid::Uuid::now_v7().simple().to_string();
    let embed_url = app
        .try_state::<PlaybackService>()
        .map(|playback| playback.youtube_embed_url())
        .ok_or("Raygun's player isn't ready")?;
    let (events, player) = mpsc::unbounded_channel();
    PLAYERS
        .lock()
        .expect("recording players poisoned")
        .insert(session.clone(), events);
    let (id, dir) = (session.clone(), run.dir.to_string_lossy().into_owned());
    if let Err(error) = native(app, move |native| native.recording_start(&id, &dir)).await {
        PLAYERS
            .lock()
            .expect("recording players poisoned")
            .remove(&session);
        return Err(error);
    }
    let request = SongRecordingRequest {
        session: session.clone(),
        download_id: run.download_id.to_owned(),
        title: run.title.to_owned(),
        artist: run.artist.map(str::to_owned),
        video_id: Some(run.video_id.to_owned()),
        start_ms: run.from_ms,
        embed_url,
    };
    if let Err(error) = crate::events::SongRecordingEvent(request).emit(app) {
        end_run(app, &session).await;
        return Err(error.to_string());
    }
    let (sender, reports) = mpsc::unbounded_channel();
    let poll = tokio::spawn(follow_run(app.clone(), session.clone(), player, sender));
    Ok(Session {
        id: session,
        reports,
        poll: Some(poll),
        recording: true,
    })
}

/// Ends a run: the webview's player stops, and the recorder.
pub(crate) async fn end_run(app: &AppHandle, session: &str) {
    PLAYERS
        .lock()
        .expect("recording players poisoned")
        .remove(session);
    let _ = crate::events::SongRecordingEvent(SongRecordingRequest {
        session: session.to_owned(),
        download_id: String::new(),
        title: String::new(),
        artist: None,
        video_id: None,
        start_ms: 0.0,
        embed_url: String::new(),
    })
    .emit(app);
    let id = session.to_owned();
    let _ = native(app, move |native| native.recording_stop(&id)).await;
}

/// What a run does about its player's news.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    /// The song plays from here now (a run starts), or doesn't (`None`: held).
    Mark(Option<f64>),
    Length(f64),
    Ended,
    Failed(String),
}

/// The recording's view of its player: whether it plays, and whether a run is open.
#[derive(Debug, Default)]
struct Follow {
    playing: bool,
    marked: bool,
    length_told: bool,
}

impl Follow {
    /// The steps one of the player's reports calls for. A run opens at the first position the
    /// player gives once it plays; it's held when it buffers, pauses, ends or fails.
    fn on(&mut self, event: EngineEvent) -> Vec<Step> {
        let mut steps = Vec::new();
        match event {
            EngineEvent::Playing => self.playing = true,
            EngineEvent::Progress {
                position_ms,
                duration_ms,
            } => {
                if !self.length_told
                    && let Some(length) = duration_ms.filter(|ms| *ms > 0.0)
                {
                    self.length_told = true;
                    steps.push(Step::Length(length));
                }
                if self.playing && !self.marked {
                    self.marked = true;
                    steps.push(Step::Mark(Some(position_ms.max(0.0))));
                }
            }
            EngineEvent::Paused | EngineEvent::Buffering => {
                self.playing = false;
                steps.extend(self.hold());
            }
            EngineEvent::Ended => {
                steps.extend(self.hold());
                steps.push(Step::Ended);
            }
            EngineEvent::Error { message } => {
                steps.extend(self.hold());
                steps.push(Step::Failed(message));
            }
            EngineEvent::Loudness { .. } => {}
        }
        steps
    }

    fn hold(&mut self) -> Option<Step> {
        std::mem::take(&mut self.marked).then_some(Step::Mark(None))
    }
}

/// Follows a run: the player's reports and the recorder's pieces (four times a second), as the
/// capture's [`Report`]s; stops when Raygun's own player plays, or Raygun's window (with the
/// recording's player) goes.
async fn follow_run(
    app: AppHandle,
    session: String,
    mut player: mpsc::UnboundedReceiver<EngineEvent>,
    sender: mpsc::UnboundedSender<Report>,
) {
    let mut follow = Follow::default();
    let mut read = HashMap::new();
    let mut every = tokio::time::interval(Duration::from_millis(250));
    every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            event = player.recv() => {
                let Some(event) = event else {
                    let _ = sender.send(Report::Failed("The recording's player stopped".into()));
                    return;
                };
                for step in follow.on(event) {
                    match step {
                        Step::Mark(at) => {
                            let id = session.clone();
                            if let Err(error) =
                                native(&app, move |native| native.recording_mark(&id, at)).await
                            {
                                let _ = sender.send(Report::Failed(error));
                                return;
                            }
                        }
                        Step::Length(length) => {
                            let _ = sender.send(Report::Length(length));
                        }
                        Step::Ended => {
                            // What the recorder wrote up to the end, then the end.
                            let _ = take_sound(&app, &session, &mut read, &sender).await;
                            let _ = sender.send(Report::Ended);
                            return;
                        }
                        Step::Failed(error) => {
                            let _ = sender.send(Report::Failed(error));
                            return;
                        }
                    }
                }
            }
            _ = every.tick() => {
                if let Err(error) = take_sound(&app, &session, &mut read, &sender).await {
                    let _ = sender.send(error);
                    return;
                }
                if player_busy(&app) || !window_open(&app) {
                    let _ = sender.send(Report::Interrupted);
                    return;
                }
            }
        }
    }
}

/// Sends what the recorder wrote since last time; a recorder that failed (Android's notice
/// stopped, the consent gone) is the run's end, waiting to be recorded again.
async fn take_sound(
    app: &AppHandle,
    session: &str,
    read: &mut HashMap<String, u64>,
    sender: &mpsc::UnboundedSender<Report>,
) -> Result<(), Report> {
    let id = session.to_owned();
    let state: PlayerState = native(app, move |native| native.recording_status(&id))
        .await
        .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()))
        .map_err(Report::Failed)?;
    send_new_sound(&state.pieces, read, sender);
    if state.state == "failed" {
        tracing::info!(error = ?state.error, "the recording stopped");
        // The consent gone (Android's Stop) waits to be allowed again; anything else failed.
        return Err(if ready(app).await {
            Report::Failed(
                state
                    .error
                    .unwrap_or_else(|| "The recording stopped".into()),
            )
        } else {
            Report::Interrupted
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress(position_ms: f64) -> EngineEvent {
        EngineEvent::Progress {
            position_ms,
            duration_ms: Some(200_000.0),
        }
    }

    #[test]
    fn runs_open_where_the_player_plays_and_hold_while_it_doesnt() {
        let mut follow = Follow::default();
        // Loading: it buffers, then says where it is before it plays.
        assert_eq!(follow.on(EngineEvent::Buffering), []);
        assert_eq!(follow.on(progress(0.0)), [Step::Length(200_000.0)]);
        // It plays: the run starts at the first position it gives.
        assert_eq!(follow.on(EngineEvent::Playing), []);
        assert_eq!(follow.on(progress(40.0)), [Step::Mark(Some(40.0))]);
        assert_eq!(follow.on(progress(540.0)), []);
        // It buffers midway: held, and a new run where it plays again.
        assert_eq!(follow.on(EngineEvent::Buffering), [Step::Mark(None)]);
        assert_eq!(follow.on(progress(61_000.0)), []);
        assert_eq!(follow.on(EngineEvent::Playing), []);
        assert_eq!(follow.on(progress(61_000.0)), [Step::Mark(Some(61_000.0))]);
        // Paused (a call took the sound): held.
        assert_eq!(follow.on(EngineEvent::Paused), [Step::Mark(None)]);
        assert_eq!(follow.on(EngineEvent::Paused), []);
        assert_eq!(follow.on(EngineEvent::Playing), []);
        assert_eq!(follow.on(progress(90_000.0)), [Step::Mark(Some(90_000.0))]);
        // The end: held, then the end.
        assert_eq!(
            follow.on(EngineEvent::Ended),
            [Step::Mark(None), Step::Ended]
        );
    }

    #[test]
    fn a_player_that_fails_ends_the_run() {
        let mut follow = Follow::default();
        follow.on(EngineEvent::Playing);
        follow.on(progress(0.0));
        assert_eq!(
            follow.on(EngineEvent::Error {
                message: "The video was removed or is private".into()
            }),
            [
                Step::Mark(None),
                Step::Failed("The video was removed or is private".into())
            ]
        );
        // One that never played has no run to hold.
        let mut follow = Follow::default();
        assert_eq!(
            follow.on(EngineEvent::Error {
                message: "YouTube refused the embedded player".into()
            }),
            [Step::Failed("YouTube refused the embedded player".into())]
        );
    }

    #[test]
    fn reports_for_a_stopped_recording_are_refused() {
        let refused = report(SongRecordingReport {
            session: "gone".into(),
            event: EngineEvent::Playing,
        });
        assert_eq!(refused.unwrap_err(), "That recording was stopped");
    }
}
