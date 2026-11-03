//! Captions for podcast episodes and audiobook chapters (M46.32): the feed's transcript when it
//! links one, else transcribed by AI in 30-second windows from where the listener is, ahead of
//! playback, then the rest. On this device that's Whisper (`raygun_ai::speech::Whisper`, a model
//! from the AI catalogue); online, an OpenAI-compatible provider from Settings → AI. One episode
//! is transcribed at a time; what's heard is kept (`raygun_db::captions`).

mod audio;
mod cues;
mod video_audio;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use raygun_ai::speech::{self, Whisper};
use raygun_db::captions::{self as store, CaptionProgress, CaptionRow};
use raygun_db::podcasts::{self as episodes, EpisodeRow};
use raygun_model::{Caption, CaptionSource, Captions, TranscriptionEngine, TranscriptionPrefs};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::io::AsyncWriteExt;

use self::audio::AudioReader;
use self::video_audio::VideoAudio;
use crate::events::CaptionsEvent;
use crate::startup::AppDb;

/// Online transcription costs money: it runs at most this far ahead of the listener.
const ONLINE_AHEAD_MS: u64 = 10 * 60 * 1000;
/// How often a job that's far enough ahead looks at the listener again.
const WAIT_STEP: Duration = Duration::from_secs(10);

pub struct CaptionService {
    app: AppHandle,
    client: reqwest::Client,
    downloads_dir: PathBuf,
    /// Audio fetched for transcribing episodes that aren't downloaded.
    work_dir: PathBuf,
    state: Mutex<State>,
    /// The loaded on-device model and its catalogue id.
    whisper: Arc<Mutex<Option<(String, Whisper)>>>,
}

#[derive(Default)]
struct State {
    /// The episode or video being transcribed: its key, stop flag and the listener's place
    /// (videos report it; episodes are followed through playback).
    job: Option<(String, Arc<AtomicBool>, Arc<AtomicU64>)>,
    preparing: bool,
    errors: HashMap<String, String>,
    /// Feed transcripts already fetched (or tried) since start.
    fetched: HashSet<String>,
    /// The show or book whose next episodes are transcribed when they come up.
    follow: Option<String>,
}

enum Engine {
    Local {
        model: String,
        dir: PathBuf,
    },
    Online {
        base_url: String,
        key: Option<String>,
        model: String,
    },
}

impl Engine {
    fn source(&self) -> &'static str {
        match self {
            Self::Local { .. } => "local",
            Self::Online { .. } => "online",
        }
    }
}

impl CaptionService {
    pub fn new(
        app: AppHandle,
        client: reqwest::Client,
        downloads_dir: PathBuf,
        work_dir: PathBuf,
    ) -> Arc<Self> {
        Arc::new(Self {
            app,
            client,
            downloads_dir,
            work_dir,
            state: Mutex::default(),
            whisper: Arc::default(),
        })
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().0.clone()
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn announce(&self, episode_id: &str) {
        let _ = CaptionsEvent(episode_id.to_owned()).emit(&self.app);
    }

    async fn prefs(&self) -> TranscriptionPrefs {
        crate::settings::load(&self.db())
            .await
            .map(|settings| settings.transcription)
            .unwrap_or_default()
    }

    /// An episode's captions, fetching the feed's transcript the first time; or a video's AI
    /// subtitles, by its progress key (M46.33).
    pub async fn captions(self: &Arc<Self>, episode_id: &str) -> Result<Captions, String> {
        let db = self.db();
        let episode = episodes::episode(&db, episode_id)
            .await
            .map_err(to_message)?;
        let mut progress = store::progress(&db, episode_id).await.map_err(to_message)?;
        if progress.is_none()
            && let Some(episode) = &episode
            && let Some(transcript) = episode.fields.transcript.clone()
            && self.state().fetched.insert(episode_id.to_owned())
        {
            match self.fetch_transcript(&transcript).await {
                Ok(rows) if !rows.is_empty() => {
                    store::replace(&db, episode_id, "feed", &rows)
                        .await
                        .map_err(to_message)?;
                    progress = store::progress(&db, episode_id).await.map_err(to_message)?;
                }
                Ok(_) => {}
                Err(error) => tracing::info!(%error, "couldn't read an episode's transcript"),
            }
        }
        let prefs = self.prefs().await;
        let can_transcribe = self.engine(&prefs).await.is_ok();
        // Chapters after one the listener had transcribed are transcribed as they come up.
        let (follows, failed) = {
            let state = self.state();
            (
                episode
                    .as_ref()
                    .is_some_and(|episode| state.follow.as_deref() == Some(&episode.podcast_id)),
                state.errors.contains_key(episode_id),
            )
        };
        if progress.is_none()
            && follows
            && !failed
            && can_transcribe
            && !self.is_running(episode_id)
        {
            let position = self.position_of(episode_id).unwrap_or(0);
            let _ = self.transcribe(episode_id, position).await;
        }
        let cues = store::cues(&db, episode_id).await.map_err(to_message)?;
        let state = self.state();
        let running = state
            .job
            .as_ref()
            .is_some_and(|(id, _, _)| id == episode_id);
        let progress = progress.unwrap_or_default();
        Ok(Captions {
            episode_id: episode_id.to_owned(),
            cues: cues
                .into_iter()
                .map(|cue| Caption {
                    start_ms: clamp_ms(cue.start_ms),
                    end_ms: clamp_ms(cue.end_ms),
                    text: cue.text,
                })
                .collect(),
            source: source_of(&progress),
            covered: progress
                .ranges
                .iter()
                .map(|(start, end)| (clamp_ms(*start), clamp_ms(*end)))
                .collect(),
            complete: progress.complete,
            running,
            preparing: running && state.preparing,
            error: state.errors.get(episode_id).cloned(),
            can_transcribe,
            engine: prefs.engine,
            has_feed: episode
                .as_ref()
                .is_some_and(|episode| episode.fields.transcript.is_some()),
        })
    }

    fn is_running(&self, episode_id: &str) -> bool {
        self.state()
            .job
            .as_ref()
            .is_some_and(|(id, _, _)| id == episode_id)
    }

    /// Starts transcribing an episode from `from_ms` (the listener's place), stopping any other.
    pub async fn transcribe(
        self: &Arc<Self>,
        episode_id: &str,
        from_ms: u32,
    ) -> Result<(), String> {
        let prefs = self.prefs().await;
        let engine = self.engine(&prefs).await?;
        let episode = episodes::episode(&self.db(), episode_id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "This episode was removed".to_owned())?;
        self.state().follow = Some(episode.podcast_id.clone());
        let service = self.clone();
        self.start(
            episode_id,
            u64::from(from_ms),
            move |stop, anchor| async move {
                let id = episode.id.clone();
                let (path, fetched) = service.audio_file(&episode).await?;
                let opened = path.clone();
                let reader =
                    tauri::async_runtime::spawn_blocking(move || AudioReader::open(&opened))
                        .await
                        .map_err(to_message)??;
                let duration = episode
                    .fields
                    .duration_ms
                    .and_then(|ms| u64::try_from(ms).ok())
                    .filter(|ms| *ms > 0)
                    .or(reader.duration_ms());
                let job = Job {
                    key: &id,
                    engine,
                    duration,
                    follow: Follow::Episode,
                    anchor: &anchor,
                    stop: &stop,
                };
                let result = service.run(job, Sound::Episode(reader)).await;
                if fetched {
                    let _ = tokio::fs::remove_file(&path).await;
                }
                result
            },
        )
    }

    /// Makes AI subtitles for a video (M46.33) from what plays at `url` (the player's address:
    /// the proxy's stream link or a download's local link). `from_ms` is where the viewer is;
    /// `all` hears the whole video (a download, offline) rather than staying ahead of them.
    pub async fn transcribe_video(
        self: &Arc<Self>,
        key: &str,
        url: &str,
        format: Option<String>,
        from_ms: u32,
        all: bool,
    ) -> Result<(), String> {
        let prefs = self.prefs().await;
        let engine = self.engine(&prefs).await?;
        // Already running: only the viewer's place moves.
        if let Some((_, _, anchor)) = self.state().job.as_ref().filter(|(id, _, _)| id == key) {
            anchor.store(u64::from(from_ms), Ordering::Relaxed);
            return Ok(());
        }
        let service = self.clone();
        let client = self.client.clone();
        let url = url.to_owned();
        let video_key = key.to_owned();
        self.start(key, u64::from(from_ms), move |stop, anchor| async move {
            let sound = tauri::async_runtime::spawn_blocking(move || {
                VideoAudio::open(client, &url, format.as_deref())
            })
            .await
            .map_err(to_message)??;
            let duration = sound.duration_ms();
            let job = Job {
                key: &video_key,
                engine,
                duration,
                follow: if all { Follow::Whole } else { Follow::Viewer },
                anchor: &anchor,
                stop: &stop,
            };
            let result = service.run(job, Sound::Video(Box::new(sound))).await;
            service.write_track(&video_key).await;
            result
        })
    }

    /// Hears a song's words (M46.36): from its downloaded copy, else a stream one of the sources
    /// has (YouTube's player gives none). The lines become the song's synced lyrics, to edit like
    /// any others. The job's key is [`song_key`].
    pub async fn transcribe_song(
        self: &Arc<Self>,
        query: raygun_model::TrackQuery,
    ) -> Result<(), String> {
        let prefs = self.prefs().await;
        let engine = self.engine(&prefs).await?;
        let key = song_key(&query);
        let service = self.clone();
        let job_key = key.clone();
        self.start(&key, 0, move |stop, anchor| async move {
            // Earlier words of the song are heard again.
            let _ = store::clear(&service.db(), &job_key).await;
            let sound = service.song_sound(&query).await?;
            let duration = sound.duration_ms();
            let job = Job {
                key: &job_key,
                engine,
                duration,
                follow: Follow::Whole,
                anchor: &anchor,
                stop: &stop,
            };
            let result = service.run(job, Sound::Video(Box::new(sound))).await;
            service.save_song_lyrics(&job_key, &query).await?;
            result
        })
    }

    /// What a song sounds like, for hearing its words: its downloaded copy, else the first stream
    /// the resolver finds that names it (not YouTube's).
    async fn song_sound(&self, query: &raygun_model::TrackQuery) -> Result<VideoAudio, String> {
        let songs = self
            .app
            .state::<Arc<crate::music_downloads::SongDownloadService>>()
            .inner()
            .clone();
        if let Some(path) = songs.offline_file(&raygun_resolver::track_key(query)) {
            let reader = tauri::async_runtime::spawn_blocking(move || AudioReader::open(&path))
                .await
                .map_err(to_message)??;
            return Ok(VideoAudio::File(reader));
        }
        let resolver = self
            .app
            .state::<Arc<crate::resolver::ResolverService>>()
            .inner()
            .clone();
        let media = self.app.state::<raygun_net::MediaLinks>().inner().clone();
        let mut exclusions = crate::resolution::Exclusions::default();
        let mut last_error = None;
        for _ in 0..SONG_TRIES {
            let pick = match resolver.resolve(query, &exclusions).await {
                Ok(pick) => pick,
                Err(error) => {
                    last_error = Some(error);
                    break;
                }
            };
            exclusions.tried.push(pick.key.clone());
            let raygun_model::MediaSource::Url { url, headers } = &pick.source else {
                continue;
            };
            let Some(link) = media.link(url, headers) else {
                continue;
            };
            let client = self.client.clone();
            match tauri::async_runtime::spawn_blocking(move || {
                VideoAudio::open(client, &link, None)
            })
            .await
            .map_err(to_message)?
            {
                Ok(sound) => return Ok(sound),
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| {
            "Only YouTube has this song, and its player gives no sound to hear: download it first"
                .into()
        }))
    }

    /// Keeps what was heard of a song as its own synced lyrics (M46.36).
    async fn save_song_lyrics(
        &self,
        key: &str,
        query: &raygun_model::TrackQuery,
    ) -> Result<(), String> {
        let cues = store::cues(&self.db(), key).await.map_err(to_message)?;
        if cues.is_empty() {
            return Err("No words were heard in this song".into());
        }
        let synced = cues
            .into_iter()
            .map(|cue| raygun_model::LyricLine {
                time_ms: clamp_ms(cue.start_ms),
                text: cue.text,
            })
            .collect();
        let sound = self.app.state::<Arc<crate::sound::SoundService>>();
        let mut edit = sound.edit(&crate::sound::key(query));
        edit.lyrics = Some(raygun_model::EditedLyrics {
            synced,
            plain: None,
        });
        sound.set(query, edit).await.map(|_| ())
    }

    /// Goes back to an episode's own transcript (M46.36: the listener picks the captions'
    /// source). What AI heard is kept aside (M46.45), and the feed's transcript comes back from
    /// where it was kept, else is read again.
    pub async fn use_feed(self: &Arc<Self>, episode_id: &str) -> Result<(), String> {
        self.switch(episode_id, AI_SHELF, FEED_SHELF).await?;
        if !self.has_captions(episode_id).await {
            self.state().fetched.remove(episode_id);
        }
        self.announce(episode_id);
        Ok(())
    }

    /// Captions heard by AI in place of the feed's transcript (M46.45): the feed's is kept
    /// aside; what AI heard before comes back at once, else transcribing starts at `from_ms`.
    pub async fn use_ai(self: &Arc<Self>, episode_id: &str, from_ms: u32) -> Result<(), String> {
        self.switch(episode_id, FEED_SHELF, AI_SHELF).await?;
        if self.has_captions(episode_id).await {
            self.announce(episode_id);
            Ok(())
        } else {
            self.transcribe(episode_id, from_ms).await
        }
    }

    /// Keeps an episode's captions on `away`'s shelf and brings back `back`'s, if any.
    async fn switch(&self, episode_id: &str, away: &str, back: &str) -> Result<(), String> {
        self.stop(episode_id);
        let db = self.db();
        if self.has_captions(episode_id).await {
            store::move_to(&db, episode_id, &format!("{episode_id}{away}"))
                .await
                .map_err(to_message)?;
        }
        let shelf = format!("{episode_id}{back}");
        if store::progress(&db, &shelf)
            .await
            .map_err(to_message)?
            .is_some()
        {
            store::move_to(&db, &shelf, episode_id)
                .await
                .map_err(to_message)?;
        }
        self.state().errors.remove(episode_id);
        Ok(())
    }

    async fn has_captions(&self, episode_id: &str) -> bool {
        store::progress(&self.db(), episode_id)
            .await
            .ok()
            .flatten()
            .is_some()
    }

    /// Hears several songs' words, one after another in the order given (M46.46: songs
    /// selected in a list). Fails at once when transcription isn't set up.
    pub async fn transcribe_songs(
        self: &Arc<Self>,
        queries: Vec<raygun_model::TrackQuery>,
    ) -> Result<(), String> {
        let prefs = self.prefs().await;
        self.engine(&prefs).await?;
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            // One list at a time; a second waits for the first.
            let _turn = SONG_BATCH.lock().await;
            for query in queries {
                let key = song_key(&query);
                if let Err(error) = service.transcribe_song(query).await {
                    tracing::warn!(%error, song = %key, "couldn't transcribe a song");
                    continue;
                }
                while service.is_running(&key) {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        });
        Ok(())
    }

    /// The words a transcription heard in a song, kept from its last run (M46.45): offered with
    /// the lyrics sources' versions, so switching back costs nothing.
    pub async fn song_transcript(
        &self,
        query: &raygun_model::TrackQuery,
    ) -> Option<raygun_model::Lyrics> {
        let cues = store::cues(&self.db(), &song_key(query)).await.ok()?;
        if cues.is_empty() {
            return None;
        }
        Some(raygun_model::Lyrics {
            provider_name: TRANSCRIBED.to_owned(),
            synced: cues
                .into_iter()
                .map(|cue| raygun_model::LyricLine {
                    time_ms: clamp_ms(cue.start_ms),
                    text: cue.text,
                })
                .collect(),
            plain: None,
            offset_ms: 0,
            edited: false,
        })
    }

    /// Runs a job for `key`, stopping any other; its outcome is announced and kept.
    fn start<F, Fut>(self: &Arc<Self>, key: &str, from_ms: u64, work: F) -> Result<(), String>
    where
        F: FnOnce(Arc<AtomicBool>, Arc<AtomicU64>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
    {
        let stop = Arc::new(AtomicBool::new(false));
        let anchor = Arc::new(AtomicU64::new(from_ms));
        {
            let mut state = self.state();
            state.errors.remove(key);
            if state.job.as_ref().is_some_and(|(id, _, _)| id == key) {
                return Ok(());
            }
            if let Some((_, previous, _)) = state.job.take() {
                previous.store(true, Ordering::Relaxed);
            }
            state.job = Some((key.to_owned(), stop.clone(), anchor.clone()));
            state.preparing = true;
        }
        self.announce(key);
        let service = self.clone();
        let id = key.to_owned();
        tauri::async_runtime::spawn(async move {
            let result = work(stop, anchor).await;
            {
                let mut state = service.state();
                if state.job.as_ref().is_some_and(|(job, _, _)| *job == id) {
                    state.job = None;
                    state.preparing = false;
                }
                if let Err(error) = &result {
                    tracing::info!(%error, "transcribing stopped");
                    state.errors.insert(id.clone(), error.clone());
                }
                // The model holds 150 to 300 MB: let it go until it's needed again.
                if state.job.is_none() {
                    *service.whisper.lock().unwrap_or_else(|p| p.into_inner()) = None;
                }
            }
            service.announce(&id);
        });
        Ok(())
    }

    /// Stops transcribing an episode; what was heard is kept. Its show isn't followed anymore.
    pub fn stop(&self, episode_id: &str) {
        let mut state = self.state();
        if state
            .job
            .as_ref()
            .is_some_and(|(id, _, _)| id == episode_id)
            && let Some((_, stop, _)) = state.job.take()
        {
            stop.store(true, Ordering::Relaxed);
        }
        state.follow = None;
        drop(state);
        self.announce(episode_id);
    }

    /// Forgets an episode's captions (to transcribe it again).
    pub async fn clear(&self, episode_id: &str) -> Result<(), String> {
        self.stop(episode_id);
        store::clear(&self.db(), episode_id)
            .await
            .map_err(to_message)?;
        self.state().errors.remove(episode_id);
        self.announce(episode_id);
        Ok(())
    }

    /// What transcribes, as set up in Settings; an error says what's missing.
    async fn engine(&self, prefs: &TranscriptionPrefs) -> Result<Engine, String> {
        let ai = self.app.state::<crate::ai::AiService>();
        match prefs.engine {
            TranscriptionEngine::Off => Err("Transcription is off in Settings → AI".into()),
            TranscriptionEngine::Local => ai
                .installed_dir(&prefs.local_model)
                .map(|dir| Engine::Local {
                    model: prefs.local_model.clone(),
                    dir,
                })
                .ok_or_else(|| "Download the speech model in Settings → AI first".into()),
            TranscriptionEngine::Online => {
                let id = prefs
                    .provider_id
                    .as_deref()
                    .ok_or("Pick a provider for transcription in Settings → AI")?;
                let (base_url, key) = ai.transcriber(id).await?;
                Ok(Engine::Online {
                    base_url,
                    key,
                    model: prefs.online_model.clone(),
                })
            }
        }
    }

    /// The listener's place in the episode, when it's the one playing.
    fn position_of(&self, episode_id: &str) -> Option<u32> {
        let podcasts = self
            .app
            .try_state::<Arc<crate::podcasts::PodcastService>>()?;
        if podcasts.playing_episode().as_deref() != Some(episode_id) {
            return None;
        }
        let playback = self.app.try_state::<crate::playback::PlaybackService>()?;
        Some(playback.state().position_ms.max(0.0) as u32)
    }

    async fn run(&self, job: Job<'_>, mut sound: Sound) -> Result<(), String> {
        let db = self.db();
        let id = job.key;
        let engine = job.engine;
        let duration = job.duration;
        if let Engine::Local { model, dir } = &engine {
            self.load_whisper(model, dir).await?;
        }
        self.state().preparing = false;
        self.announce(id);

        let mut language: Option<String> = None;
        loop {
            if job.stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            let progress = store::progress(&db, id)
                .await
                .map_err(to_message)?
                .unwrap_or_default();
            if progress.complete {
                break;
            }
            let listening = match job.follow {
                Follow::Episode => self.position_of(id).map(u64::from),
                Follow::Viewer => Some(job.anchor.load(Ordering::Relaxed)),
                Follow::Whole => None,
            };
            let anchor = listening.unwrap_or_else(|| job.anchor.load(Ordering::Relaxed));
            let mut at = progress.next_gap(anchor as i64).max(0) as u64;
            if let Some(duration) = duration
                && at >= duration
            {
                // The rest is done: fill what came before the listener.
                at = progress.next_gap(0).max(0) as u64;
                if at >= duration {
                    break;
                }
            }
            let limited = matches!(engine, Engine::Online { .. }) && job.follow != Follow::Whole;
            if limited && at > anchor + ONLINE_AHEAD_MS {
                if listening.is_none() {
                    return Ok(());
                }
                tokio::time::sleep(WAIT_STEP).await;
                continue;
            }
            let (back, samples) = tauri::async_runtime::spawn_blocking(move || {
                let samples = sound.window(at, u64::from(speech::WINDOW_MS));
                (sound, samples)
            })
            .await
            .map_err(to_message)?;
            sound = back;
            let samples = samples?;
            if samples.is_empty() {
                let end = duration.unwrap_or(at + 1).max(at + 1);
                store::add(
                    &db,
                    id,
                    engine.source(),
                    &[],
                    (at as i64, end as i64),
                    Some(at as i64),
                )
                .await
                .map_err(to_message)?;
                break;
            }
            let window_ms = (samples.len() as u64 * 1000 / u64::from(speech::SAMPLE_RATE)) as u32;
            let (segments, consumed) = match &engine {
                // Whisper makes words up for silence and scraps ("you"): nothing to hear there.
                _ if is_quiet(&samples) || window_ms < MIN_WINDOW_MS => (Vec::new(), window_ms),
                Engine::Local { .. } => {
                    let whisper = self.whisper.clone();
                    let wanted = language.clone();
                    let heard = tauri::async_runtime::spawn_blocking(move || {
                        let mut loaded = whisper.lock().unwrap_or_else(|p| p.into_inner());
                        let (_, model) = loaded.as_mut().ok_or("The speech model isn't loaded")?;
                        model
                            .transcribe(&samples, wanted.as_deref())
                            .map_err(|error| error.to_string())
                    })
                    .await
                    .map_err(to_message)??;
                    // The language is kept once a window had real speech in it: music or a
                    // quiet opening can be mistaken for any language.
                    if language.is_none() && heard.segments.len() >= 2 {
                        language = heard.language.clone();
                    }
                    (heard.segments, heard.consumed_ms)
                }
                Engine::Online {
                    base_url,
                    key,
                    model,
                } => {
                    let segments = speech::transcribe_online(
                        &self.client,
                        base_url,
                        key.as_deref(),
                        model,
                        &samples,
                        None,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                    (segments, window_ms)
                }
            };
            let consumed = u64::from(consumed.clamp(1000.min(window_ms), window_ms.max(1)));
            let rows: Vec<CaptionRow> = segments
                .into_iter()
                .map(|segment| CaptionRow {
                    start_ms: (at + u64::from(segment.start_ms)) as i64,
                    end_ms: (at + u64::from(segment.end_ms)) as i64,
                    text: segment.text,
                })
                .collect();
            let covered = (at as i64, (at + consumed) as i64);
            store::add(
                &db,
                id,
                engine.source(),
                &rows,
                covered,
                duration.map(|ms| ms as i64),
            )
            .await
            .map_err(to_message)?;
            self.announce(id);
        }
        Ok(())
    }

    /// A video's AI subtitles as a WebVTT file the players load like any other track, when
    /// there are any (M46.33). Written after each job; also on request.
    pub async fn video_track(&self, key: &str) -> Option<raygun_model::VideoSubtitle> {
        let relative = self.write_track(key).await?;
        let url = self.app.state::<raygun_net::ImageLinks>().local(&relative);
        Some(raygun_model::VideoSubtitle {
            url,
            lang: None,
            label: Some("AI subtitles".into()),
            format: Some("vtt".into()),
        })
    }

    /// Writes `downloads/subtitles/ai/<key>.vtt` from the stored cues; returns its path under
    /// the downloads folder.
    async fn write_track(&self, key: &str) -> Option<String> {
        let cues = store::cues(&self.db(), key).await.ok()?;
        if cues.is_empty() {
            return None;
        }
        let relative = format!("subtitles/ai/{}.vtt", safe_name(key));
        let path = self.downloads_dir.join(&relative);
        tokio::fs::create_dir_all(path.parent()?).await.ok()?;
        tokio::fs::write(&path, cues::to_vtt(&cues)).await.ok()?;
        Some(relative)
    }

    async fn load_whisper(&self, model: &str, dir: &Path) -> Result<(), String> {
        let loaded = self
            .whisper
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .is_some_and(|(id, _)| id == model);
        if loaded {
            return Ok(());
        }
        let dir = dir.to_owned();
        let whisper = tauri::async_runtime::spawn_blocking(move || Whisper::load(&dir))
            .await
            .map_err(to_message)?
            .map_err(|error| error.to_string())?;
        *self.whisper.lock().unwrap_or_else(|p| p.into_inner()) = Some((model.to_owned(), whisper));
        Ok(())
    }

    /// The episode's audio as a file: its download, else fetched into the work folder (removed
    /// once the episode is done). The flag says it was fetched.
    async fn audio_file(&self, episode: &EpisodeRow) -> Result<(PathBuf, bool), String> {
        if let Some(download) = episodes::download(&self.db(), &episode.id)
            .await
            .map_err(to_message)?
            && download.state == "done"
            && let Some(relative) = download.path
        {
            let path = self.downloads_dir.join(relative);
            if path.is_file() {
                return Ok((path, false));
            }
        }
        let extension = audio_extension(
            &episode.fields.audio_url,
            episode.fields.audio_type.as_deref(),
        );
        let path = self
            .work_dir
            .join(format!("{}.{extension}", safe_name(&episode.id)));
        if path.is_file() {
            return Ok((path, true));
        }
        tokio::fs::create_dir_all(&self.work_dir)
            .await
            .map_err(to_message)?;
        let partial = path.with_extension("part");
        let response = self
            .client
            .get(&episode.fields.audio_url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|error| format!("Couldn't get the audio: {error}"))?;
        let mut file = tokio::fs::File::create(&partial)
            .await
            .map_err(to_message)?;
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|error| format!("Couldn't get the audio: {error}"))?;
            file.write_all(&chunk).await.map_err(to_message)?;
        }
        file.flush().await.map_err(to_message)?;
        drop(file);
        tokio::fs::rename(&partial, &path)
            .await
            .map_err(to_message)?;
        Ok((path, true))
    }

    async fn fetch_transcript(&self, transcript: &str) -> Result<Vec<CaptionRow>, String> {
        let (kind, url) = transcript
            .split_once('|')
            .ok_or_else(|| "a transcript link without a type".to_owned())?;
        let text = self
            .client
            .get(url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(to_message)?
            .text()
            .await
            .map_err(to_message)?;
        cues::parse(kind, &text).ok_or_else(|| format!("{kind} transcripts aren't timed"))
    }
}

/// Below this level (RMS) a window is taken as silence.
const QUIET: f32 = 0.002;
/// Shorter windows (an episode's last scrap) aren't sent.
const MIN_WINDOW_MS: u32 = 1_000;

fn is_quiet(samples: &[f32]) -> bool {
    let energy: f32 = samples.iter().map(|sample| sample * sample).sum();
    (energy / samples.len().max(1) as f32).sqrt() < QUIET
}

/// Streams tried when looking for a song's sound to hear (M46.36).
const SONG_TRIES: usize = 4;

/// The captions key of a song's transcription (M46.36): `song:` and its track key.
/// Songs heard one list at a time (M46.46).
static SONG_BATCH: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Where a source's captions wait while the listener uses the other (M46.45).
const AI_SHELF: &str = "#ai";
const FEED_SHELF: &str = "#feed";
/// The lyrics source name of a song's transcription; the UI names it.
pub const TRANSCRIBED: &str = "transcribed";

pub fn song_key(query: &raygun_model::TrackQuery) -> String {
    format!("song:{}", raygun_resolver::track_key(query))
}

/// Who the job keeps up with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Follow {
    /// The playing episode, through playback.
    Episode,
    /// A video's viewer, who reports their place (`Job::anchor`).
    Viewer,
    /// Nobody: the whole recording, from the start (offline subtitles).
    Whole,
}

struct Job<'a> {
    key: &'a str,
    engine: Engine,
    duration: Option<u64>,
    follow: Follow,
    anchor: &'a AtomicU64,
    stop: &'a AtomicBool,
}

/// What's heard: an episode's file or a video's stream.
enum Sound {
    Episode(AudioReader),
    Video(Box<VideoAudio>),
}

impl Sound {
    fn window(&mut self, from_ms: u64, len_ms: u64) -> Result<Vec<f32>, String> {
        match self {
            Self::Episode(reader) => reader.window(from_ms, len_ms),
            Self::Video(video) => video.window(from_ms, len_ms),
        }
    }
}

fn source_of(progress: &CaptionProgress) -> Option<CaptionSource> {
    match progress.source.as_str() {
        "feed" => Some(CaptionSource::Feed),
        "local" => Some(CaptionSource::Local),
        "online" => Some(CaptionSource::Online),
        _ => None,
    }
}

fn clamp_ms(ms: i64) -> u32 {
    ms.clamp(0, i64::from(u32::MAX)) as u32
}

/// A file extension symphonia can go by.
fn audio_extension(url: &str, mime: Option<&str>) -> &'static str {
    const KNOWN: [&str; 7] = ["mp3", "m4a", "m4b", "aac", "ogg", "flac", "wav"];
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let extension = path
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, extension)| extension.to_ascii_lowercase());
    if let Some(known) = extension
        .as_deref()
        .and_then(|extension| KNOWN.iter().find(|known| **known == extension))
    {
        return known;
    }
    match mime.unwrap_or_default() {
        "audio/mp4" | "audio/x-m4a" | "audio/m4a" | "audio/x-m4b" => "m4a",
        "audio/aac" => "aac",
        "audio/ogg" | "audio/vorbis" => "ogg",
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/wav" | "audio/x-wav" => "wav",
        _ => "mp3",
    }
}

fn safe_name(id: &str) -> String {
    id.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_files_get_an_extension_symphonia_knows() {
        assert_eq!(
            audio_extension("https://cdn.example/a/ch01.MP3?x=1", None),
            "mp3"
        );
        assert_eq!(
            audio_extension("https://cdn.example/stream", Some("audio/x-m4a")),
            "m4a"
        );
        assert_eq!(audio_extension("https://cdn.example/ep.php", None), "mp3");
        assert_eq!(safe_name("0193-ab/cd"), "0193-ab_cd");
    }

    #[test]
    fn silence_is_not_sent() {
        assert!(is_quiet(&[0.0; 16_000]));
        assert!(is_quiet(&[0.001, -0.001, 0.0005]));
        let tone: Vec<f32> = (0..16_000)
            .map(|index| (index as f32 * 0.05).sin() * 0.2)
            .collect();
        assert!(!is_quiet(&tone));
    }

    /// Transcribes a file with a downloaded Whisper model, window by window as the service does:
    /// `WHISPER_DIR=<model folder> SPEECH_FILE=<audio> cargo test -p raygun-app -- --ignored
    /// whisper_live --nocapture`.
    #[test]
    #[ignore = "needs a Whisper model and a spoken file"]
    fn whisper_live() {
        let dir = PathBuf::from(std::env::var("WHISPER_DIR").expect("WHISPER_DIR"));
        let file = PathBuf::from(std::env::var("SPEECH_FILE").expect("SPEECH_FILE"));
        let started = std::time::Instant::now();
        let mut whisper = Whisper::load(&dir).unwrap();
        println!("loaded in {:?}", started.elapsed());
        let mut reader = AudioReader::open(&file).unwrap();
        let mut at = 0u64;
        let mut heard = Vec::new();
        loop {
            let samples = reader.window(at, u64::from(speech::WINDOW_MS)).unwrap();
            if samples.is_empty() {
                break;
            }
            if is_quiet(&samples) || samples.len() < 16_000 {
                break;
            }
            let started = std::time::Instant::now();
            let window = whisper.transcribe(&samples, None).unwrap();
            println!(
                "window at {at} ms: {} segments, used {} ms, {:?}, in {:?}",
                window.segments.len(),
                window.consumed_ms,
                window.language,
                started.elapsed()
            );
            for segment in &window.segments {
                println!(
                    "  {:>6} – {:>6}  {}",
                    at + u64::from(segment.start_ms),
                    at + u64::from(segment.end_ms),
                    segment.text
                );
                heard.push(segment.text.clone());
            }
            at += u64::from(window.consumed_ms.max(1000));
        }
        let text = heard.join(" ").to_lowercase();
        if let Ok(expected) = std::env::var("SPEECH_EXPECT") {
            for word in expected.split(',') {
                assert!(text.contains(word), "{word} wasn't heard");
            }
        }
    }
}
