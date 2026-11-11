//! Captions for podcast episodes and audiobook chapters (M46.32): the feed's transcript when it
//! links one, else transcribed by AI in 30-second windows from where the listener is, ahead of
//! playback, then the rest. On this device that's Whisper (`raygun_ai::speech::Whisper`, a model
//! from the AI catalogue); online, an OpenAI-compatible provider from Settings → AI. One episode
//! is transcribed at a time; what's heard is kept (`raygun_db::captions`).
//!
//! Songs (M46.36) have a lane of their own: a queue heard one song at a time from the start,
//! beside whatever episode or video is being transcribed (both share the one loaded model). A
//! song stopped partway goes on from where it stopped; the queue is kept across restarts
//! (`local_value` [`SONG_QUEUE_VALUE`]); a song heard to the end becomes its own synced lyrics.

mod audio;
pub(crate) mod cues;
pub(crate) mod video_audio;

use std::collections::{HashMap, HashSet, VecDeque};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{FutureExt, StreamExt};
use raygun_ai::speech::{self, SpeechError, Whisper};
use raygun_db::captions::{self as store, CaptionProgress, CaptionRow};
use raygun_db::podcasts::{self as episodes, EpisodeRow};
use raygun_model::{
    Caption, CaptionSource, Captions, TrackQuery, TranscriptionEngine, TranscriptionPrefs,
};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::io::AsyncWriteExt;

use self::audio::AudioReader;
use self::video_audio::VideoAudio;
use crate::events::{CaptionsEvent, LyricsTranscribedEvent};
use crate::startup::AppDb;

/// Online transcription costs money: it runs at most this far ahead of the listener.
const ONLINE_AHEAD_MS: u64 = 10 * 60 * 1000;
/// How often a job that's far enough ahead looks at the listener again.
const WAIT_STEP: Duration = Duration::from_secs(10);
/// A stream that stops sending fails the job instead of leaving it hearing forever.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Songs waiting to be heard, kept across restarts: a JSON list of track queries.
pub const SONG_QUEUE_VALUE: &str = "captions.songQueue";
/// A queue kept from the last run starts again this long after start, out of the way.
const RESUME_AFTER: Duration = Duration::from_secs(20);

pub struct CaptionService {
    app: AppHandle,
    client: reqwest::Client,
    downloads_dir: PathBuf,
    /// Audio fetched for transcribing episodes that aren't downloaded.
    work_dir: PathBuf,
    state: Mutex<State>,
    /// The loaded on-device model and its catalogue id.
    whisper: Arc<Mutex<Option<(String, Whisper)>>>,
    /// Which model `whisper` holds, readable without waiting for a window to finish.
    loaded: Mutex<Option<String>>,
    /// Held while the model loads, so the two lanes never load it twice.
    loading: tokio::sync::Mutex<()>,
    /// The song queue's saves are numbered; an older one never overwrites a newer one.
    queue_version: AtomicU64,
    saved_version: Arc<tokio::sync::Mutex<u64>>,
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
    /// How long each recording being heard is, when known.
    durations: HashMap<String, u64>,
    songs: SongLane,
}

/// Songs whose words are heard, one at a time.
#[derive(Default)]
struct SongLane {
    queue: VecDeque<(String, TrackQuery)>,
    current: Option<SongJob>,
    /// The worker that goes through the queue is running.
    working: bool,
}

impl SongLane {
    /// Adds songs at the back, or at the front in the order given when `first`. A song already
    /// waiting moves up when `first` and stays put otherwise; the one being heard isn't added.
    fn add(&mut self, items: Vec<(String, TrackQuery)>, first: bool) {
        let mut added: Vec<(String, TrackQuery)> = Vec::new();
        for (key, query) in items {
            if self.current.as_ref().is_some_and(|job| job.key == key)
                || added.iter().any(|(other, _)| *other == key)
            {
                continue;
            }
            match self.queue.iter().position(|(queued, _)| *queued == key) {
                Some(index) if first => {
                    self.queue.remove(index);
                }
                Some(_) => continue,
                None => {}
            }
            added.push((key, query));
        }
        if first {
            for item in added.into_iter().rev() {
                self.queue.push_front(item);
            }
        } else {
            self.queue.extend(added);
        }
    }
}

struct SongJob {
    key: String,
    query: TrackQuery,
    stop: Arc<AtomicBool>,
    /// Finding the song's sound and loading the model, before the first window.
    preparing: bool,
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
        let client = raygun_net::client_builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
            .unwrap_or(client);
        Arc::new(Self {
            app,
            client,
            downloads_dir,
            work_dir,
            state: Mutex::default(),
            whisper: Arc::default(),
            loaded: Mutex::default(),
            loading: tokio::sync::Mutex::const_new(()),
            queue_version: AtomicU64::new(0),
            saved_version: Arc::default(),
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
        let episode_job = state
            .job
            .as_ref()
            .is_some_and(|(id, _, _)| id == episode_id);
        let song_job = state
            .songs
            .current
            .as_ref()
            .filter(|job| job.key == episode_id);
        let running = episode_job || song_job.is_some();
        let preparing =
            (episode_job && state.preparing) || song_job.is_some_and(|job| job.preparing);
        let queued = state
            .songs
            .queue
            .iter()
            .position(|(key, _)| key == episode_id)
            .map(|index| index as u32 + 1);
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
            preparing,
            error: state.errors.get(episode_id).cloned(),
            can_transcribe,
            engine: prefs.engine,
            has_feed: episode
                .as_ref()
                .is_some_and(|episode| episode.fields.transcript.is_some()),
            queued,
            duration_ms: state
                .durations
                .get(episode_id)
                .map(|ms| clamp_ms(*ms as i64)),
        })
    }

    fn is_running(&self, episode_id: &str) -> bool {
        self.state()
            .job
            .as_ref()
            .is_some_and(|(id, _, _)| id == episode_id)
    }

    /// A video's viewer moved on (the player's reports): its subtitles keep ahead of there.
    pub fn follow(&self, key: &str, position_ms: u64) {
        if let Some((_, _, anchor)) = self.state().job.as_ref().filter(|(id, _, _)| id == key) {
            anchor.store(position_ms, Ordering::Relaxed);
        }
    }

    /// Where the viewer of the video being transcribed is, as last reported.
    pub fn anchor(&self, key: &str) -> Option<u64> {
        self.state()
            .job
            .as_ref()
            .filter(|(id, _, _)| id == key)
            .map(|(_, _, anchor)| anchor.load(Ordering::Relaxed))
    }

    /// Why nothing can be transcribed now (off, no model, no provider); `None` when it can.
    pub async fn engine_problem(&self) -> Option<String> {
        let prefs = self.prefs().await;
        self.engine(&prefs).await.err()
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
                    stop: stop.clone(),
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
                stop: stop.clone(),
            };
            let result = service.run(job, Sound::Video(Box::new(sound))).await;
            service.write_track(&video_key).await;
            result
        })
    }

    /// Hears a song's words (M46.36): from its downloaded copy, else a stream one of the sources
    /// has (a YouTube video's own audio file too, M46.51). The lines become the song's synced
    /// lyrics, to edit like any others. The job's key is [`song_key`]. The song goes first in the
    /// song queue; one heard to the end before is heard anew, one stopped partway goes on from
    /// there.
    pub async fn transcribe_song(self: &Arc<Self>, query: TrackQuery) -> Result<(), String> {
        let prefs = self.prefs().await;
        self.engine(&prefs).await?;
        let key = song_key(&query);
        if !self.song_waiting(&key) && self.heard_whole(&key).await {
            store::clear(&self.db(), &key).await.map_err(to_message)?;
        }
        self.enqueue(vec![(key, query)], true);
        Ok(())
    }

    /// Hears several songs' words, one after another in the order given (M46.46: songs
    /// selected in a list), after those already waiting. Songs heard to the end before are left
    /// as they are. Fails at once when transcription isn't set up.
    pub async fn transcribe_songs(
        self: &Arc<Self>,
        queries: Vec<TrackQuery>,
    ) -> Result<(), String> {
        let prefs = self.prefs().await;
        self.engine(&prefs).await?;
        let mut items = Vec::new();
        for query in queries {
            let key = song_key(&query);
            if items.iter().any(|(added, _)| *added == key) || self.heard_whole(&key).await {
                continue;
            }
            items.push((key, query));
        }
        self.enqueue(items, false);
        Ok(())
    }

    /// Puts the songs kept from the last run back in line, a little after start, when
    /// transcription is still set up.
    pub fn resume_songs(self: &Arc<Self>) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(RESUME_AFTER).await;
            let kept = raygun_db::local_value(&service.db(), SONG_QUEUE_VALUE)
                .await
                .ok()
                .flatten()
                .and_then(|json| serde_json::from_str::<Vec<TrackQuery>>(&json).ok())
                .unwrap_or_default();
            if kept.is_empty() {
                return;
            }
            let prefs = service.prefs().await;
            if service.engine(&prefs).await.is_err() {
                return;
            }
            let items = kept
                .into_iter()
                .map(|query| (song_key(&query), query))
                .collect();
            service.enqueue(items, false);
        });
    }

    /// Whether a song is being heard or waits to be.
    fn song_waiting(&self, key: &str) -> bool {
        let state = self.state();
        state
            .songs
            .current
            .as_ref()
            .is_some_and(|job| job.key == key)
            || state.songs.queue.iter().any(|(queued, _)| queued == key)
    }

    async fn heard_whole(&self, key: &str) -> bool {
        store::progress(&self.db(), key)
            .await
            .ok()
            .flatten()
            .is_some_and(|progress| progress.complete)
    }

    /// Adds songs to the queue (at its front, in the order given, when `first`), and starts
    /// the worker that hears them if it isn't running. A song already waiting moves up when
    /// `first`; the one being heard stays.
    fn enqueue(self: &Arc<Self>, items: Vec<(String, TrackQuery)>, first: bool) {
        let (start_worker, keys) = {
            let mut state = self.state();
            let lane = &mut state.songs;
            lane.add(items, first);
            let start_worker = !lane.working && !lane.queue.is_empty();
            if start_worker {
                lane.working = true;
            }
            let keys: Vec<String> = lane.queue.iter().map(|(key, _)| key.clone()).collect();
            for key in &keys {
                state.errors.remove(key);
            }
            (start_worker, keys)
        };
        self.save_queue();
        for key in &keys {
            self.announce(key);
        }
        if start_worker {
            let service = self.clone();
            tauri::async_runtime::spawn(async move { service.hear_songs().await });
        }
    }

    /// The song queue's worker: one song at a time until none wait.
    async fn hear_songs(self: Arc<Self>) {
        loop {
            let next = {
                let mut state = self.state();
                let lane = &mut state.songs;
                match lane.queue.pop_front() {
                    Some((key, query)) => {
                        let stop = Arc::new(AtomicBool::new(false));
                        lane.current = Some(SongJob {
                            key: key.clone(),
                            query: query.clone(),
                            stop: stop.clone(),
                            preparing: true,
                        });
                        Some((key, query, stop))
                    }
                    None => {
                        lane.current = None;
                        lane.working = false;
                        None
                    }
                }
            };
            let Some((key, query, stop)) = next else {
                break;
            };
            // Everyone waiting moved up one.
            self.announce_songs();
            let outcome = AssertUnwindSafe(self.hear_song(&key, &query, stop))
                .catch_unwind()
                .await
                .unwrap_or_else(|_| Err(UNEXPECTED.into()));
            {
                let mut state = self.state();
                state.songs.current = None;
                state.durations.remove(&key);
                if let Err(error) = &outcome {
                    tracing::info!(%error, song = %key, "couldn't transcribe a song");
                    state.errors.insert(key.clone(), error.clone());
                }
            }
            self.save_queue();
            self.announce(&key);
        }
        self.release_model_if_idle();
        self.save_queue();
    }

    /// Hears one song from where it stopped (the start, the first time); heard to the end, its
    /// words become its lyrics. Stopped, it answers `Ok` and keeps what was heard.
    async fn hear_song(
        &self,
        key: &str,
        query: &TrackQuery,
        stop: Arc<AtomicBool>,
    ) -> Result<(), String> {
        let prefs = self.prefs().await;
        let engine = self.engine(&prefs).await?;
        let (sound, captured) = self.song_sound(query, &stop).await?;
        let outcome = self.hear_sound(key, query, &stop, engine, sound).await;
        // A capture of YouTube's player is kept while the song may go on from where it stopped.
        if let Some(dir) = captured
            && !stop.load(Ordering::Relaxed)
        {
            let _ = tokio::fs::remove_dir_all(dir).await;
        }
        outcome
    }

    /// Hears `sound` as the song's words, from where it stopped.
    async fn hear_sound(
        &self,
        key: &str,
        query: &TrackQuery,
        stop: &Arc<AtomicBool>,
        engine: Engine,
        sound: VideoAudio,
    ) -> Result<(), String> {
        let stop = stop.clone();
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let duration = sound.duration_ms();
        let anchor = AtomicU64::new(0);
        let job = Job {
            key,
            engine,
            duration,
            follow: Follow::Song,
            anchor: &anchor,
            stop: stop.clone(),
        };
        if let Err(error) = self.run(job, Sound::Video(Box::new(sound))).await {
            let heard = store::progress(&self.db(), key)
                .await
                .ok()
                .flatten()
                .is_some_and(|progress| !progress.ranges.is_empty());
            return Err(song_failure(&error, heard));
        }
        // Stopped, or what was heard was deleted meanwhile (Delete saved lyrics): nothing is kept.
        if stop.load(Ordering::Relaxed) || !self.heard_whole(key).await {
            return Ok(());
        }
        self.save_song_lyrics(key, query).await?;
        let _ = LyricsTranscribedEvent(key.to_owned()).emit(&self.app);
        Ok(())
    }

    /// Tells every song waiting or being heard that the queue changed.
    fn announce_songs(&self) {
        let keys: Vec<String> = {
            let state = self.state();
            state
                .songs
                .current
                .iter()
                .map(|job| job.key.clone())
                .chain(state.songs.queue.iter().map(|(key, _)| key.clone()))
                .collect()
        };
        for key in keys {
            self.announce(&key);
        }
    }

    /// Keeps the song queue (the song being heard first) for the next start.
    fn save_queue(&self) {
        let (version, json) = {
            let state = self.state();
            let queries: Vec<&TrackQuery> = state
                .songs
                .current
                .iter()
                .map(|job| &job.query)
                .chain(state.songs.queue.iter().map(|(_, query)| query))
                .collect();
            (
                self.queue_version.fetch_add(1, Ordering::Relaxed) + 1,
                serde_json::to_string(&queries).unwrap_or_else(|_| "[]".into()),
            )
        };
        let Some(db) = self.app.try_state::<AppDb>().map(|db| db.0.clone()) else {
            return;
        };
        let saved = self.saved_version.clone();
        tauri::async_runtime::spawn(async move {
            let mut last = saved.lock().await;
            if *last > version {
                return;
            }
            *last = version;
            if let Err(error) = raygun_db::set_local_value(&db, SONG_QUEUE_VALUE, &json).await {
                tracing::warn!(%error, "couldn't keep the songs waiting to be transcribed");
            }
        });
    }

    /// What a song sounds like, for hearing its words: its downloaded copy, else the first stream
    /// the resolver finds that names it, a YouTube video's own audio file included (M46.51: M4A,
    /// which decodes here), else, for a song YouTube's player plays, its sound captured from a
    /// hidden player of its own (M46.52; with the folder that capture is in). Errors say what was
    /// tried, in words, with the details on the lines after.
    async fn song_sound(
        &self,
        query: &TrackQuery,
        stop: &Arc<AtomicBool>,
    ) -> Result<(VideoAudio, Option<PathBuf>), String> {
        let mut failures: Vec<(String, String)> = Vec::new();
        let mut heard_videos: Vec<String> = Vec::new();
        let mut unresolved = None;
        if let Some(sound) = self
            .stream_sound(query, &mut failures, &mut heard_videos, &mut unresolved)
            .await?
        {
            return Ok((sound, None));
        }
        // YouTube's player plays it: its sound, as the player gives it.
        if let Some(video_id) = heard_videos.first()
            && !stop.load(Ordering::Relaxed)
        {
            let dir = self.work_dir.join("youtube").join(safe_name(video_id));
            match self.youtube_sound(video_id, &dir, query, stop).await {
                Ok(sound) => return Ok((sound, Some(dir))),
                Err(error) => failures.push(("YouTube's player".into(), error)),
            }
        }
        Err(no_sound(&failures, unresolved.as_deref()))
    }

    /// The song's downloaded copy, else the first stream that names it and opens here; what was
    /// tried goes in `failures`, and the YouTube videos met in `heard_videos`.
    async fn stream_sound(
        &self,
        query: &TrackQuery,
        failures: &mut Vec<(String, String)>,
        heard_videos: &mut Vec<String>,
        unresolved: &mut Option<String>,
    ) -> Result<Option<VideoAudio>, String> {
        let songs = self
            .app
            .state::<Arc<crate::music_downloads::SongDownloadService>>()
            .inner()
            .clone();
        if let Some(path) = songs.offline_file(&raygun_resolver::track_key(query)) {
            match tauri::async_runtime::spawn_blocking(move || AudioReader::open(&path))
                .await
                .map_err(to_message)?
            {
                Ok(reader) => return Ok(Some(VideoAudio::File(reader))),
                Err(error) => failures.push(("the downloaded copy".into(), error)),
            }
        }
        let resolver = self
            .app
            .state::<Arc<crate::resolver::ResolverService>>()
            .inner()
            .clone();
        let media = self.app.state::<raygun_net::MediaLinks>().inner().clone();
        // Streams read here: sources with several formats give one that decodes (M46.51).
        let mut exclusions = crate::resolution::Exclusions {
            decodable: true,
            ..crate::resolution::Exclusions::default()
        };
        let using = resolver.stream_use(None, true).await;
        for _ in 0..SONG_TRIES {
            let pick = match resolver.resolve(query, &exclusions).await {
                Ok(pick) => pick,
                Err(error) => {
                    *unresolved = Some(error);
                    break;
                }
            };
            exclusions.tried.push(pick.key.clone());
            exclusions.failed_sources.push(pick.source.clone());
            // YouTube's embed has no sound to read: the video's own audio file has (M46.51).
            let source = match &pick.source {
                raygun_model::MediaSource::YouTube { video_id } => {
                    if heard_videos.contains(video_id) {
                        continue;
                    }
                    heard_videos.push(video_id.clone());
                    match resolver.youtube_audio(video_id, using).await {
                        Ok(stream) => stream.source,
                        Err(error) => {
                            failures.push((format!("{}'s stream", pick.provider_name), error));
                            continue;
                        }
                    }
                }
                source => source.clone(),
            };
            if let Some(video_id) = crate::resolver::youtube_video(&pick.key) {
                heard_videos.push(video_id.to_owned());
            }
            let opened = match &source {
                raygun_model::MediaSource::Url { url, headers } => {
                    let Some(link) = media.link(url, headers) else {
                        continue;
                    };
                    let client = self.client.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        VideoAudio::open(client, &link, None)
                    })
                    .await
                    .map_err(to_message)?
                }
                raygun_model::MediaSource::LocalFile { path } => {
                    let path = PathBuf::from(path);
                    tauri::async_runtime::spawn_blocking(move || AudioReader::open(&path))
                        .await
                        .map_err(to_message)?
                        .map(VideoAudio::File)
                }
                raygun_model::MediaSource::YouTube { .. }
                | raygun_model::MediaSource::Lookup { .. } => continue,
            };
            match opened {
                Ok(sound) => return Ok(Some(sound)),
                Err(error) => failures.push((format!("{}'s stream", pick.provider_name), error)),
            }
        }
        Ok(None)
    }

    /// A YouTube video's sound for hearing its words (M46.52): captured from a hidden YouTube
    /// player of its own, silent and beside whatever plays, at twice its pace where that keeps
    /// what speech needs, into the work folder (kept until it's whole, so a stopped song goes on
    /// from there).
    async fn youtube_sound(
        &self,
        video_id: &str,
        dir: &Path,
        query: &TrackQuery,
        stop: &Arc<AtomicBool>,
    ) -> Result<VideoAudio, String> {
        use crate::music_downloads::capture::{self, CaptureError, youtube};

        // Captured whole before the song was stopped: heard from there.
        let (wav, whole) = (dir.join("capture.wav"), dir.join(WHOLE));
        if tokio::fs::try_exists(&whole).await.unwrap_or(false) {
            let path = wav.clone();
            if let Ok(Ok(reader)) =
                tauri::async_runtime::spawn_blocking(move || AudioReader::open(&path)).await
            {
                return Ok(VideoAudio::File(reader));
            }
        }
        let stopped = Arc::new(tokio::sync::Notify::new());
        let watcher = {
            let (stop, stopped) = (stop.clone(), stopped.clone());
            tauri::async_runtime::spawn(async move {
                while !stop.load(Ordering::Relaxed) {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                stopped.notify_one();
            })
        };
        let expected = capture::Expected {
            duration_ms: query.duration_ms.map(u64::from),
            start_ms: 0,
            end_ms: None,
        };
        let captured = capture::youtube_sound(
            &self.app,
            video_id,
            dir,
            &expected,
            youtube::Purpose::Words,
            &|_| {},
            &stopped,
        )
        .await;
        watcher.abort();
        let stitched = captured.map_err(|error| match error {
            CaptureError::Stopped => "Stopped".to_owned(),
            CaptureError::Interrupted => {
                "YouTube's player plays in Raygun's window: open Raygun to hear this song"
                    .to_owned()
            }
            CaptureError::Device(error) | CaptureError::Failed(error) => error,
            CaptureError::Partial { why, .. } => {
                format!("Only part of it could be heard ({why}); try again to hear the rest")
            }
        })?;
        let _ = tokio::fs::write(&whole, b"").await;
        let path = stitched.wav;
        tauri::async_runtime::spawn_blocking(move || AudioReader::open(&path))
            .await
            .map_err(to_message)?
            .map(VideoAudio::File)
    }

    /// Keeps what was heard of a song as its own synced lyrics (M46.36).
    async fn save_song_lyrics(&self, key: &str, query: &TrackQuery) -> Result<(), String> {
        let cues = store::cues(&self.db(), key).await.map_err(to_message)?;
        if cues.is_empty() {
            return Err(NO_WORDS.into());
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
        sound
            .set(query, edit)
            .await
            .map(|_| ())
            .map_err(|error| format!("Couldn't keep the words as the song's lyrics\n{error}"))
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

    /// The words a transcription heard in a song, kept from its last run (M46.45): offered with
    /// the lyrics sources' versions, so switching back costs nothing.
    pub async fn song_transcript(&self, query: &TrackQuery) -> Option<raygun_model::Lyrics> {
        // Only a whole song: part of one would read as lyrics that stop.
        let key = song_key(query);
        if !self.heard_whole(&key).await {
            return None;
        }
        let cues = store::cues(&self.db(), &key).await.ok()?;
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
            translations: Vec::new(),
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
            // A panic must not leave the job marked as running forever.
            let result = AssertUnwindSafe(work(stop, anchor))
                .catch_unwind()
                .await
                .unwrap_or_else(|_| Err(UNEXPECTED.into()));
            {
                let mut state = service.state();
                if state.job.as_ref().is_some_and(|(job, _, _)| *job == id) {
                    state.job = None;
                    state.preparing = false;
                    state.durations.remove(&id);
                }
                if let Err(error) = &result {
                    tracing::info!(%error, "transcribing stopped");
                    state.errors.insert(id.clone(), error.clone());
                }
            }
            service.release_model_if_idle();
            service.announce(&id);
        });
        Ok(())
    }

    /// The model holds 150 to 300 MB: it's let go while nothing is being heard.
    fn release_model_if_idle(&self) {
        let idle = {
            let state = self.state();
            state.job.is_none() && state.songs.current.is_none() && state.songs.queue.is_empty()
        };
        if idle {
            *self.loaded.lock().unwrap_or_else(|p| p.into_inner()) = None;
            *self.whisper.lock().unwrap_or_else(|p| p.into_inner()) = None;
        }
    }

    /// Stops transcribing an episode, a video or a song (taken out of the song queue too); what
    /// was heard is kept. A stopped episode's show isn't followed anymore.
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
        if let Some(job) = state
            .songs
            .current
            .as_ref()
            .filter(|job| job.key == episode_id)
        {
            job.stop.store(true, Ordering::Relaxed);
        }
        let waiting = state.songs.queue.len();
        state.songs.queue.retain(|(key, _)| key != episode_id);
        let dequeued = state.songs.queue.len() != waiting;
        if !episode_id.starts_with(SONG_PREFIX) {
            state.follow = None;
        }
        drop(state);
        if dequeued {
            self.save_queue();
            self.announce_songs();
        }
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

    /// The listener's place in the song with captions key `key`, when it's the one playing.
    fn song_position(&self, key: &str) -> Option<u32> {
        let playback = self.app.try_state::<crate::playback::PlaybackService>()?;
        let state = playback.state();
        let id = state.current_item_id.as_deref()?;
        let queue = playback.queue();
        let item = queue.items.iter().find(|item| item.id == id)?;
        (song_key(&TrackQuery::for_item(item)) == key).then(|| state.position_ms.max(0.0) as u32)
    }

    /// Forgets everything heard of a song (the lyrics menu's Delete saved lyrics): its hearing
    /// stops and leaves the queue, and its captions go, kept shelves included. Waits a moment for
    /// a window being heard to end, so it can't add its lines back after.
    pub async fn forget_song(&self, query: &TrackQuery) -> Result<(), String> {
        let key = song_key(query);
        self.stop(&key);
        for _ in 0..50 {
            let busy = self
                .state()
                .songs
                .current
                .as_ref()
                .is_some_and(|job| job.key == key);
            if !busy {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        store::clear_with_shelves(&self.db(), &key)
            .await
            .map_err(to_message)?;
        {
            let mut state = self.state();
            state.errors.remove(&key);
            state.durations.remove(&key);
        }
        self.save_queue();
        self.announce(&key);
        Ok(())
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
        {
            let mut state = self.state();
            if state.job.as_ref().is_some_and(|(job, _, _)| job == id) {
                state.preparing = false;
            }
            if let Some(job) = state.songs.current.as_mut().filter(|job| job.key == id) {
                job.preparing = false;
            }
            if let Some(duration) = duration {
                state.durations.insert(id.to_owned(), duration);
            }
        }
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
                Follow::Song => self.song_position(id).map(u64::from),
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
            let limited = matches!(engine, Engine::Online { .. })
                && matches!(job.follow, Follow::Episode | Follow::Viewer);
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
            let samples = samples
                .map_err(|error| format!("Couldn't read the sound at {}\n{error}", clock(at)))?;
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
                Engine::Local { model, dir } => {
                    // The other lane may have let the model go between two windows.
                    self.load_whisper(model, dir).await?;
                    let whisper = self.whisper.clone();
                    let wanted = language.clone();
                    let stop = job.stop.clone();
                    let heard = tauri::async_runtime::spawn_blocking(move || {
                        let mut loaded = whisper.lock().unwrap_or_else(|p| p.into_inner());
                        let (_, model) = loaded
                            .as_mut()
                            .ok_or_else(|| SpeechError::Model("it isn't loaded".into()))?;
                        model.transcribe_until(&samples, wanted.as_deref(), &|| {
                            stop.load(Ordering::Relaxed)
                        })
                    })
                    .await
                    .map_err(to_message)?;
                    let heard = match heard {
                        Ok(heard) => heard,
                        Err(SpeechError::Cancelled) if job.stop.load(Ordering::Relaxed) => {
                            return Ok(());
                        }
                        Err(error) => {
                            return Err(format!(
                                "The speech model failed at {}\n{error}",
                                clock(at)
                            ));
                        }
                    };
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
                    .map_err(|error| {
                        format!("The transcription provider didn't answer\n{error}")
                    })?;
                    (segments, window_ms)
                }
            };
            if job.stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            let consumed = u64::from(consumed.clamp(1000.min(window_ms), window_ms.max(1)));
            let mut rows: Vec<CaptionRow> = segments
                .into_iter()
                .map(|segment| CaptionRow {
                    start_ms: (at + u64::from(segment.start_ms)) as i64,
                    end_ms: (at + u64::from(segment.end_ms)) as i64,
                    text: segment.text,
                })
                .collect();
            let before = store::cue_before(&db, id, at as i64)
                .await
                .map_err(to_message)?;
            drop_repeated_start(before.as_ref(), &mut rows);
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
            .map_err(|error| format!("Couldn't keep what was heard\n{error}"))?;
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

    /// Loads the on-device model unless it's the one loaded. The model's own lock is held for a
    /// whole window, so it's only taken on blocking threads; `loaded` says which model is in.
    async fn load_whisper(&self, model: &str, dir: &Path) -> Result<(), String> {
        let is_loaded = || self.loaded_model().as_deref() == Some(model);
        if is_loaded() {
            return Ok(());
        }
        // One load at a time: the other lane waits for it rather than loading a second copy.
        let _loading = self.loading.lock().await;
        if is_loaded() {
            return Ok(());
        }
        let whisper = self.whisper.clone();
        let id = model.to_owned();
        let dir = dir.to_owned();
        tauri::async_runtime::spawn_blocking(move || {
            let mut slot = whisper.lock().unwrap_or_else(|p| p.into_inner());
            // The old model goes first, so two are never in memory at once.
            *slot = None;
            *slot = Some((id, Whisper::load(&dir)?));
            Ok::<_, SpeechError>(())
        })
        .await
        .map_err(to_message)?
        .map_err(|error| {
            format!("The speech model couldn't load: download it again in Settings → AI\n{error}")
        })?;
        *self.loaded.lock().unwrap_or_else(|p| p.into_inner()) = Some(model.to_owned());
        Ok(())
    }

    fn loaded_model(&self) -> Option<String> {
        self.loaded
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
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

/// Marks a capture of YouTube's player that holds the whole song (M46.52), next to it.
const WHOLE: &str = "whole";

/// Where a source's captions wait while the listener uses the other (M46.45).
const AI_SHELF: &str = "#ai";
const FEED_SHELF: &str = "#feed";
/// The lyrics source name of a song's transcription; the UI names it.
pub const TRANSCRIBED: &str = "transcribed";
/// Song keys start with this.
const SONG_PREFIX: &str = "song:";
const UNEXPECTED: &str = "Transcribing stopped unexpectedly: try again";
const NO_WORDS: &str = "No words were heard in this song";

/// The captions key of a song's transcription (M46.36): `song:` and its track key.
pub fn song_key(query: &TrackQuery) -> String {
    format!("{SONG_PREFIX}{}", raygun_resolver::track_key(query))
}

/// Why no sound was found for a song: the first line says it in words, the rest are details.
fn no_sound(failures: &[(String, String)], unresolved: Option<&str>) -> String {
    let details: Vec<String> = failures
        .iter()
        .map(|(what, error)| format!("{what}: {error}"))
        .chain(unresolved.map(str::to_owned))
        .collect();
    let headline = match failures.first() {
        Some((what, _)) => format!("Couldn't read {what} of this song"),
        None => "No source has this song to hear".to_owned(),
    };
    std::iter::once(headline)
        .chain(details)
        .collect::<Vec<_>>()
        .join("\n")
}

/// A song's job failing partway: what was heard is kept and the next try goes on from there.
/// Errors that already say what failed keep their words, with that added.
fn song_failure(error: &str, heard: bool) -> String {
    let (headline, details) = error.split_once('\n').unwrap_or(("", error));
    let headline = if headline.is_empty() {
        "Couldn't hear this song's words"
    } else {
        headline
    };
    if heard {
        format!("{headline}. What was heard is kept; trying again goes on from there\n{details}")
    } else {
        format!("{headline}\n{details}")
    }
}

/// A window may start by hearing the line the window before ended on again.
fn drop_repeated_start(before: Option<&CaptionRow>, rows: &mut Vec<CaptionRow>) {
    let (Some(before), Some(first)) = (before, rows.first()) else {
        return;
    };
    let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
    if same(&before.text, &first.text) && first.start_ms <= before.end_ms + REPEAT_GAP_MS {
        rows.remove(0);
    }
}

/// How close a line said again must start to the end of the same line to count as heard twice.
const REPEAT_GAP_MS: i64 = 1_500;

/// `m:ss`, for naming where something failed.
fn clock(ms: u64) -> String {
    let seconds = ms / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
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
    /// A song, all of it: from the listener's place while it's the one playing, so the lines
    /// sung now come first, then the rest.
    Song,
}

struct Job<'a> {
    key: &'a str,
    engine: Engine,
    duration: Option<u64>,
    follow: Follow,
    anchor: &'a AtomicU64,
    stop: Arc<AtomicBool>,
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

    fn song(title: &str) -> (String, TrackQuery) {
        let query = TrackQuery {
            title: title.into(),
            artist: Some("Band".into()),
            album: None,
            duration_ms: None,
        };
        (song_key(&query), query)
    }

    fn order(lane: &SongLane) -> Vec<String> {
        lane.queue
            .iter()
            .map(|(_, query)| query.title.clone())
            .collect()
    }

    #[test]
    fn songs_wait_in_line_and_a_tap_goes_first() {
        let mut lane = SongLane::default();
        lane.add(vec![song("A"), song("B"), song("A"), song("C")], false);
        assert_eq!(order(&lane), ["A", "B", "C"]);
        // A list added later waits behind; songs already waiting keep their place.
        lane.add(vec![song("B"), song("D")], false);
        assert_eq!(order(&lane), ["A", "B", "C", "D"]);
        // One asked for on its own goes first, also when it was waiting.
        lane.add(vec![song("C")], true);
        assert_eq!(order(&lane), ["C", "A", "B", "D"]);
        // The song being heard isn't queued again.
        let (key, query) = song("E");
        lane.current = Some(SongJob {
            key,
            query,
            stop: Arc::default(),
            preparing: false,
        });
        lane.add(vec![song("E")], true);
        assert_eq!(order(&lane), ["C", "A", "B", "D"]);
    }

    #[test]
    fn failures_say_what_was_tried_first() {
        let none = no_sound(&[], Some("no stream matched"));
        assert_eq!(none, "No source has this song to hear\nno stream matched");
        let unreadable = no_sound(
            &[("Indie Tracks's stream".into(), "unsupported codec".into())],
            None,
        );
        assert_eq!(
            unreadable,
            "Couldn't read Indie Tracks's stream of this song\nIndie Tracks's stream: unsupported codec"
        );
        // YouTube's own audio file that wasn't given is one more source that was tried (M46.51).
        let youtube = no_sound(
            &[(
                "YouTube's stream".into(),
                "YouTube didn't give a stream for it".into(),
            )],
            None,
        );
        assert_eq!(
            youtube,
            "Couldn't read YouTube's stream of this song\nYouTube's stream: YouTube didn't give \
             a stream for it"
        );
        assert!(!youtube.to_lowercase().contains("only youtube"));
        assert_eq!(
            song_failure("Couldn't read the sound at 1:30\nend of stream", true),
            "Couldn't read the sound at 1:30. What was heard is kept; trying again goes on from \
             there\nend of stream"
        );
        assert_eq!(
            song_failure("database is locked", false),
            "Couldn't hear this song's words\ndatabase is locked"
        );
        assert_eq!(clock(95_400), "1:35");
    }

    #[test]
    fn a_line_heard_again_at_a_window_start_is_dropped() {
        let row = |start_ms, end_ms, text: &str| CaptionRow {
            start_ms,
            end_ms,
            text: text.into(),
        };
        let before = row(27_000, 30_000, "Who you gonna call?");
        let mut rows = vec![
            row(30_200, 32_000, "who you gonna call? "),
            row(33_000, 35_000, "Ghostbusters!"),
        ];
        drop_repeated_start(Some(&before), &mut rows);
        assert_eq!(rows.len(), 1);
        // The same line much later is the chorus again, not a repeat.
        let mut chorus = vec![row(60_000, 62_000, "Who you gonna call?")];
        drop_repeated_start(Some(&before), &mut chorus);
        assert_eq!(chorus.len(), 1);
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
