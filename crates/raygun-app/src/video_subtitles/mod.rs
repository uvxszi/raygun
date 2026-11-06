//! AI subtitles in the video player: the video transcribed as it plays (`CaptionService`'s video
//! jobs, from the viewer's place and keeping ahead of them), and a subtitle track translated into
//! another language the same way, in batches that keep the source's timing. Translations are
//! kept per video, source track and language beside the AI subtitles
//! (`downloads/subtitles/ai/<video>.tr.<source>.<language>.json`, with a `.vtt` of what's done),
//! so they come back for the same lines from any address.
//!
//! Translators are this device's models (`LyricsTranslations::translate_on_device`) or a
//! translator from Settings → AI (`AiService::translate_texts`). A track's source is either the
//! cues the player shows (a subtitle file, an addon's, one added from the device) or the video's
//! AI subtitles, followed while they're being heard.
//!
//! The desktop player asks with commands and follows `VideoSubtitlesEvent`s. Android's player
//! asks with `request` reports and gets `subtitleTools` and `subtitleTrack` updates carrying the
//! cues, since it shows these tracks itself while they grow. Both players' progress reports say
//! where the viewer is.

mod entries;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::FutureExt;
use raygun_model::{
    AiProviderKind, Caption, LyricsTranslatorOption, SettingChange, SubtitleSource,
    SubtitleTranslateRequest, SubtitleTranslation, TranscriptionEngine,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use self::entries::{Entry, Limits, Next};
use crate::ai::AiService;
use crate::captions::CaptionService;
use crate::events::{CaptionsEvent, OpenRouteEvent, SettingsChangedEvent, VideoSubtitlesEvent};
use crate::lyrics_translation::{AI_PREFIX, DEVICE, LyricsTranslations, language_tag};
use crate::startup::AppDb;

/// The first batch is small, so translated lines show soon; later ones carry more context.
const FIRST_BATCH: usize = 8;
const BATCH: usize = 40;
const BATCH_CHARS: usize = 4_000;
/// Translating keeps at most this far ahead of the viewer, then waits for them.
const AHEAD_MS: u32 = 10 * 60 * 1000;
/// How often a job waiting for the viewer (or for AI subtitles to grow) looks again.
const WAIT_STEP: Duration = Duration::from_secs(3);
/// A job's file is written at most this often while it runs, and once when it ends.
const SAVE_EVERY: Duration = Duration::from_secs(10);
/// How often a model download's progress goes to the player.
const DOWNLOAD_STEP: Duration = Duration::from_secs(1);
/// The track id's source part for the video's AI subtitles.
const AI_SOURCE: &str = "ai";
const UNEXPECTED: &str = "Translating stopped unexpectedly: try again";
/// Settings → AI's transcription section, linked from the player.
const TRANSCRIPTION_ROUTE: &str = "/settings/ai#transcription";
/// The languages offered to translate into in Android's player, most spoken first.
const LANGUAGES: [&str; 28] = [
    "en", "es", "fr", "de", "it", "pt", "ru", "ja", "ko", "zh", "zh-TW", "ar", "hi", "tr", "pl",
    "nl", "sv", "uk", "id", "vi", "th", "he", "el", "cs", "ro", "hu", "da", "fi",
];

/// A translation job's file: the source's cues with their translations so far.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Kept {
    source: String,
    language: String,
    translator: String,
    by: String,
    source_label: Option<String>,
    source_language: Option<String>,
    entries: Vec<Entry>,
}

/// The video a player loaded: what plays and how to fetch it.
#[derive(Debug, Clone)]
struct Playing {
    key: String,
    url: String,
    headers: BTreeMap<String, String>,
    format: Option<String>,
}

struct Job {
    key: String,
    /// `<source>.<language>`.
    id: String,
    source: String,
    language: String,
    translator: String,
    source_label: Option<String>,
    source_language: Option<String>,
    stop: AtomicBool,
    progress: Mutex<Progress>,
}

#[derive(Default)]
struct Progress {
    entries: Vec<Entry>,
    by: String,
    running: bool,
    downloading: Option<u32>,
    error: Option<String>,
}

impl Job {
    fn progress(&self) -> std::sync::MutexGuard<'_, Progress> {
        self.progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    fn kept(&self) -> Kept {
        let progress = self.progress();
        Kept {
            source: self.source.clone(),
            language: self.language.clone(),
            translator: self.translator.clone(),
            by: progress.by.clone(),
            source_label: self.source_label.clone(),
            source_language: self.source_language.clone(),
            entries: progress.entries.clone(),
        }
    }
}

#[derive(Default)]
struct State {
    /// What the players loaded last, newest first (a few, for sessions moving between links).
    playing: Vec<Playing>,
    /// Where each viewer is, by video key, as their player reports.
    positions: HashMap<String, u32>,
    /// Videos a player shows now: a job waiting for a viewer who left ends.
    open: HashSet<String>,
    /// The video Android's player shows: it gets this module's updates.
    watching: Option<String>,
    /// AI cues already sent to Android's player, by start and end.
    sent_ai: HashSet<(u32, u32)>,
    /// Translations running or run since start, by video key and track id.
    jobs: HashMap<(String, String), Arc<Job>>,
    /// A speech model downloading from the player: its id and how much is in, in percent.
    speech_download: Option<(String, u32)>,
    /// Why setting transcription up from the player failed, for the player to say.
    setup_error: Option<String>,
}

pub struct VideoSubtitles {
    app: AppHandle,
    /// `downloads/subtitles/ai`.
    dir: PathBuf,
    state: Mutex<State>,
}

impl VideoSubtitles {
    pub fn new(app: AppHandle, downloads_dir: &Path) -> Arc<Self> {
        Arc::new(Self {
            app,
            dir: folder(downloads_dir),
            state: Mutex::default(),
        })
    }

    /// Follows the AI subtitles being made, for Android's player.
    pub fn start(self: &Arc<Self>) {
        let service = Arc::downgrade(self);
        CaptionsEvent::listen(&self.app, move |event| {
            let Some(service) = service.upgrade() else {
                return;
            };
            let key = event.payload.0;
            if service.state().watching.as_deref() != Some(key.as_str()) {
                return;
            }
            tauri::async_runtime::spawn(async move { service.push_ai(&key, false).await });
        });
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().0.clone()
    }

    // ---- Following the players ----

    /// A video was loaded into a player (`video::open_player`, a `load` update).
    pub fn loaded(&self, payload: &Value) {
        let text = |name: &str| {
            payload
                .get(name)
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
        };
        let (Some(key), Some(url)) = (text("id"), text("url")) else {
            return;
        };
        let headers = payload
            .get("headers")
            .cloned()
            .and_then(|headers| serde_json::from_value(headers).ok())
            .unwrap_or_default();
        let playing = Playing {
            key: key.clone(),
            url,
            headers,
            format: text("format"),
        };
        let mut state = self.state();
        state.playing.retain(|known| known.key != key);
        state.playing.insert(0, playing);
        state.playing.truncate(4);
        state.open.insert(key);
    }

    /// A player's report: where its viewer is, and whether it still shows the video.
    pub fn report(&self, raw: &Value) {
        let Some(key) = raw
            .get("id")
            .and_then(Value::as_str)
            .filter(|key| !key.is_empty())
        else {
            return;
        };
        let position = raw
            .get("positionMs")
            .and_then(Value::as_f64)
            .map(|ms| ms.clamp(0.0, f64::from(u32::MAX)) as u32);
        let report = raw.get("state").and_then(Value::as_str).unwrap_or_default();
        {
            let mut state = self.state();
            if let Some(position) = position {
                state.positions.insert(key.to_owned(), position);
            }
            match report {
                "closed" | "switched" => {
                    state.open.remove(key);
                }
                _ => {
                    state.open.insert(key.to_owned());
                }
            }
        }
        if let Some(position) = position
            && let Some(captions) = self.app.try_state::<Arc<CaptionService>>()
        {
            captions.follow(key, u64::from(position));
        }
    }

    fn position(&self, key: &str) -> u32 {
        self.state().positions.get(key).copied().unwrap_or(0)
    }

    fn is_open(&self, key: &str) -> bool {
        self.state().open.contains(key)
    }

    // ---- Translating ----

    /// Starts translating a video's subtitles into `request.language` from where the viewer is,
    /// or goes on with the translation made before. With this device's translator, missing models
    /// are only downloaded once `request.download` says so: until then the answer names their
    /// size (`download_bytes`) and nothing starts.
    pub async fn translate(
        self: &Arc<Self>,
        request: SubtitleTranslateRequest,
    ) -> Result<SubtitleTranslation, String> {
        language_tag(&request.language)?;
        let key = request.key.clone();
        {
            let mut state = self.state();
            state.positions.insert(key.clone(), request.from_ms);
            state.open.insert(key.clone());
        }
        let (source, source_label, source_language, cues) = match &request.source {
            SubtitleSource::Ai => (AI_SOURCE.to_owned(), None, None, self.ai_cues(&key).await?),
            SubtitleSource::Cues {
                label,
                language,
                cues,
            } => (
                entries::source_id(cues),
                label.clone(),
                language.clone(),
                cues.clone(),
            ),
        };
        if let Some(from) = &source_language
            && raygun_ai::marian::same_language(from, &request.language)
        {
            return Err("These subtitles are in that language already".into());
        }
        let ai_source = source == AI_SOURCE;
        let texts: Vec<&str> = cues.iter().map(|cue| cue.text.as_str()).collect();
        if ai_source && cues.is_empty() && !self.transcribing(&key) {
            return Err("This video has no AI subtitles yet: transcribe it first".into());
        }
        if !ai_source
            && !texts
                .iter()
                .any(|text| crate::lyrics_translation::has_words(text))
        {
            return Err("These subtitles have no words to translate".into());
        }
        let id = format!("{source}.{}", request.language);

        // A job already on this track: it carries on from the viewer's place.
        if let Some(job) = self.job(&key, &id) {
            let running = job.progress().running;
            if running && job.translator == request.translator {
                return Ok(self.view(&job, 0.0));
            }
            job.stop.store(true, Ordering::Relaxed);
        }

        let mut missing: Vec<String> = Vec::new();
        let mut by = String::new();
        if request.translator == DEVICE {
            let options = self
                .app
                .state::<Arc<LyricsTranslations>>()
                .translators(&texts, &request.language)
                .await?;
            let device = options
                .into_iter()
                .find(|option| option.id == DEVICE)
                .filter(|option| option.supported)
                .ok_or_else(|| {
                    "No model on this device translates into this language".to_owned()
                })?;
            let ai = self.app.state::<AiService>();
            missing = device
                .models
                .iter()
                .filter(|model| ai.installed_dir(model).is_none())
                .cloned()
                .collect();
            if !missing.is_empty() && !request.download {
                let kept = self.read_kept(&key, &source, &request.language).await;
                let mut view = self.kept_view(
                    &key,
                    kept.unwrap_or_else(|| Kept {
                        source: source.clone(),
                        language: request.language.clone(),
                        translator: request.translator.clone(),
                        source_label: source_label.clone(),
                        source_language: source_language.clone(),
                        entries: entries::entries(&cues, &[]),
                        ..Kept::default()
                    }),
                );
                view.download_bytes = device.download_bytes.max(1.0);
                return Ok(view);
            }
        } else if let Some(provider_id) = request.translator.strip_prefix(AI_PREFIX) {
            let provider = self
                .app
                .state::<AiService>()
                .providers()
                .await?
                .into_iter()
                .find(|provider| provider.id == provider_id)
                .ok_or_else(|| "This translator was removed in Settings → AI".to_owned())?;
            if !provider.has_key && provider.kind != AiProviderKind::OpenAiCompatible {
                return Err(format!(
                    "{} needs its API key in Settings → AI",
                    provider.name
                ));
            }
            by = provider.name;
        } else {
            return Err("Pick a translator".into());
        }

        // What was translated before with the same translator goes on from there.
        let kept = self
            .read_kept(&key, &source, &request.language)
            .await
            .filter(|kept| kept.translator == request.translator)
            .map(|kept| kept.entries)
            .unwrap_or_default();
        let job = Arc::new(Job {
            key: key.clone(),
            id: id.clone(),
            source,
            language: request.language.clone(),
            translator: request.translator.clone(),
            source_label,
            source_language,
            stop: AtomicBool::new(false),
            progress: Mutex::new(Progress {
                entries: entries::entries(&cues, &kept),
                by,
                running: true,
                ..Progress::default()
            }),
        });
        self.state()
            .jobs
            .insert((key.clone(), id.clone()), job.clone());
        tracing::info!(video = %key, track = %id, translator = %request.translator, "translating subtitles");
        let service = self.clone();
        let running = job.clone();
        tauri::async_runtime::spawn(async move { service.run(running, missing).await });
        self.changed(&job, None);
        Ok(self.view(&job, 0.0))
    }

    /// Stops translating a track; what's translated stays.
    pub fn stop(&self, key: &str, id: &str) {
        if let Some(job) = self.job(key, id) {
            job.stop.store(true, Ordering::Relaxed);
        }
    }

    /// A video's translated subtitles: those translating now and those kept from before.
    pub async fn translations(&self, key: &str) -> Result<Vec<SubtitleTranslation>, String> {
        let mut out: Vec<SubtitleTranslation> = Vec::new();
        let jobs: Vec<Arc<Job>> = {
            let state = self.state();
            state
                .jobs
                .iter()
                .filter(|((job_key, _), _)| job_key == key)
                .map(|(_, job)| job.clone())
                .collect()
        };
        for job in &jobs {
            out.push(self.view(job, 0.0));
        }
        for kept in self.all_kept(key).await {
            let id = format!("{}.{}", kept.source, kept.language);
            if out.iter().any(|known| known.id == id) {
                continue;
            }
            out.push(self.kept_view(key, kept));
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// Who can translate into `language`: this device (its models, with what it would download
    /// for any lines) and the translators from Settings → AI.
    pub async fn translators(&self, language: &str) -> Result<Vec<LyricsTranslatorOption>, String> {
        self.app
            .state::<Arc<LyricsTranslations>>()
            .translators(&[], language)
            .await
    }

    fn job(&self, key: &str, id: &str) -> Option<Arc<Job>> {
        self.state()
            .jobs
            .get(&(key.to_owned(), id.to_owned()))
            .cloned()
    }

    async fn run(self: Arc<Self>, job: Arc<Job>, missing: Vec<String>) {
        let result = AssertUnwindSafe(self.work(&job, missing))
            .catch_unwind()
            .await
            .unwrap_or_else(|_| Err(UNEXPECTED.into()));
        {
            let mut progress = job.progress();
            progress.running = false;
            progress.downloading = None;
            if let Err(error) = &result {
                tracing::info!(%error, video = %job.key, "translating subtitles stopped");
                progress.error = Some(error.clone());
            }
        }
        // A job replaced by another for the same track (another translator) leaves its file
        // and the players to the new one.
        let current = self
            .job(&job.key, &job.id)
            .is_some_and(|known| Arc::ptr_eq(&known, &job));
        if current {
            self.save(&job).await;
            self.changed(&job, None);
        }
    }

    async fn work(&self, job: &Job, missing: Vec<String>) -> Result<(), String> {
        for model in &missing {
            self.download_model(job, model).await?;
        }
        let mut first = true;
        let mut saved = Instant::now();
        // Lines translated since the file was last written.
        let mut unsaved = false;
        loop {
            if job.stopped() {
                return Ok(());
            }
            let growing = job.source == AI_SOURCE && self.transcribing(&job.key);
            if job.source == AI_SOURCE {
                let cues = self.ai_cues(&job.key).await?;
                let mut progress = job.progress();
                progress.entries = entries::entries(&cues, &progress.entries);
            }
            let anchor = self.position(&job.key);
            let limits = Limits {
                max_cues: if first { FIRST_BATCH } else { BATCH },
                max_chars: BATCH_CHARS,
                ahead_ms: AHEAD_MS,
            };
            let next = entries::next_batch(&job.progress().entries, anchor, &limits);
            match next {
                Next::Batch(indices) => {
                    let texts: Vec<String> = {
                        let progress = job.progress();
                        indices
                            .iter()
                            .map(|index| progress.entries[*index].text.clone())
                            .collect()
                    };
                    let at = job.progress().entries[indices[0]].start_ms;
                    let translated = self.translate_batch(job, &texts).await.map_err(|error| {
                        format!("Couldn't translate the subtitles at {}\n{error}", clock(at))
                    })?;
                    if job.stopped() {
                        return Ok(());
                    }
                    let added: Vec<Caption> = {
                        let mut progress = job.progress();
                        // The entries only change in this loop, so the batch's places still hold.
                        for (index, text) in indices.iter().zip(translated) {
                            if let Some(entry) = progress.entries.get_mut(*index) {
                                entry.translation = Some(text);
                            }
                        }
                        let batch: Vec<Entry> = indices
                            .iter()
                            .filter_map(|index| progress.entries.get(*index).cloned())
                            .collect();
                        entries::translated(&batch)
                    };
                    first = false;
                    unsaved = true;
                    if saved.elapsed() >= SAVE_EVERY {
                        saved = Instant::now();
                        unsaved = false;
                        self.save(job).await;
                    }
                    self.changed(job, Some(added));
                }
                // AI subtitles still being heard, or a viewer still watching, are waited for;
                // what's done is written first.
                waiting => {
                    let wait = growing || (waiting == Next::Wait && self.is_open(&job.key));
                    if !wait {
                        return Ok(());
                    }
                    if unsaved {
                        saved = Instant::now();
                        unsaved = false;
                        self.save(job).await;
                    }
                    tokio::time::sleep(WAIT_STEP).await;
                }
            }
        }
    }

    /// Translates a batch of cue texts, each different text once.
    async fn translate_batch(&self, job: &Job, texts: &[String]) -> Result<Vec<String>, String> {
        if job.translator == DEVICE {
            let pieces: Vec<Vec<String>> = texts
                .iter()
                .map(|text| entries::device_pieces(text))
                .collect();
            let flat: Vec<String> = pieces.iter().flatten().cloned().collect();
            let (unique, places) = entries::distinct(&flat);
            let done = self
                .app
                .state::<Arc<LyricsTranslations>>()
                .translate_on_device(&job.language, &unique, |_, _| {})
                .await?;
            let mut at = 0;
            Ok(pieces
                .iter()
                .map(|cue| {
                    let lines: Vec<String> = places[at..at + cue.len()]
                        .iter()
                        .map(|place| done.get(*place).map_or("", |line| line.trim()).to_owned())
                        .collect();
                    at += cue.len();
                    lines.join("\n")
                })
                .collect())
        } else {
            let provider = job
                .translator
                .strip_prefix(AI_PREFIX)
                .ok_or_else(|| "Pick a translator".to_owned())?;
            let (unique, places) = entries::distinct(texts);
            let (name, done) = self
                .app
                .state::<AiService>()
                .translate_texts(
                    provider,
                    &unique,
                    &job.language,
                    raygun_ai::translate::Content::Subtitles,
                )
                .await?;
            job.progress().by = name;
            Ok(places
                .iter()
                .map(|place| done.get(*place).map_or("", |text| text.trim()).to_owned())
                .collect())
        }
    }

    /// Downloads one of this device's models before translating, saying how far it got.
    async fn download_model(&self, job: &Job, model: &str) -> Result<(), String> {
        let ai = self.app.state::<AiService>();
        if ai.installed_dir(model).is_some() {
            return Ok(());
        }
        let size = raygun_ai::models::find(model)
            .map(|info| info.size())
            .unwrap_or(0);
        let download = ai.download_model(model);
        tokio::pin!(download);
        loop {
            tokio::select! {
                result = &mut download => return result,
                () = tokio::time::sleep(DOWNLOAD_STEP) => {
                    let received = ai.received(model).unwrap_or(0);
                    job.progress().downloading = Some(percent(received, size));
                    self.changed(job, None);
                }
            }
        }
    }

    /// What a job's track looks like now. `download_bytes` asks before downloading models.
    fn view(&self, job: &Job, download_bytes: f64) -> SubtitleTranslation {
        let anchor = self.position(&job.key);
        let progress = job.progress();
        let (done, total) = entries::counts(&progress.entries);
        let ahead = entries::ahead_ms(&progress.entries, anchor);
        SubtitleTranslation {
            key: job.key.clone(),
            id: job.id.clone(),
            language: job.language.clone(),
            translator: job.translator.clone(),
            by: progress.by.clone(),
            source_label: job.source_label.clone(),
            source_language: job.source_language.clone(),
            cues: entries::translated(&progress.entries),
            done,
            total,
            ahead_ms: ahead,
            ready_until_ms: ahead.map(|ahead| anchor.saturating_add(ahead)),
            complete: done == total && !(job.source == AI_SOURCE && self.transcribing(&job.key)),
            running: progress.running,
            downloading: progress.downloading,
            error: progress.error.clone(),
            download_bytes,
        }
    }

    /// A kept translation nothing works on now.
    fn kept_view(&self, key: &str, kept: Kept) -> SubtitleTranslation {
        let anchor = self.position(key);
        let (done, total) = entries::counts(&kept.entries);
        SubtitleTranslation {
            key: key.to_owned(),
            id: format!("{}.{}", kept.source, kept.language),
            ahead_ms: entries::ahead_ms(&kept.entries, anchor),
            ready_until_ms: entries::ahead_ms(&kept.entries, anchor)
                .map(|ahead| anchor.saturating_add(ahead)),
            cues: entries::translated(&kept.entries),
            language: kept.language,
            translator: kept.translator,
            by: kept.by,
            source_label: kept.source_label,
            source_language: kept.source_language,
            done,
            total,
            complete: done == total,
            running: false,
            downloading: None,
            error: None,
            download_bytes: 0.0,
        }
    }

    /// Tells the desktop player a track changed, and sends Android's player what's new: the
    /// cues `added`, or every cue when `None`.
    fn changed(&self, job: &Job, added: Option<Vec<Caption>>) {
        let _ = VideoSubtitlesEvent(job.key.clone()).emit(&self.app);
        if self.state().watching.as_deref() != Some(job.key.as_str()) {
            return;
        }
        let view = self.view(job, 0.0);
        let replace = added.is_none();
        let cues = added.unwrap_or_else(|| view.cues.clone());
        push(&self.app, &translation_update(&view, cues, replace));
    }

    // ---- AI subtitles ----

    /// The video's AI subtitles so far.
    async fn ai_cues(&self, key: &str) -> Result<Vec<Caption>, String> {
        let rows = raygun_db::captions::cues(&self.db(), key)
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows
            .into_iter()
            .map(|row| Caption {
                start_ms: clamp_ms(row.start_ms),
                end_ms: clamp_ms(row.end_ms),
                text: row.text,
            })
            .collect())
    }

    /// The video's AI subtitles are being made now.
    fn transcribing(&self, key: &str) -> bool {
        self.app
            .try_state::<Arc<CaptionService>>()
            .is_some_and(|captions| captions.anchor(key).is_some())
    }

    /// Sends Android's player the video's AI subtitles: what's new since the last time, or all of
    /// them when `all` (or nothing went yet), with how far ahead of the viewer they reach.
    async fn push_ai(&self, key: &str, all: bool) {
        let Some(captions) = self.app.try_state::<Arc<CaptionService>>() else {
            return;
        };
        let Ok(heard) = captions.inner().captions(key).await else {
            return;
        };
        let anchor = captions
            .anchor(key)
            .map_or_else(|| self.position(key), |anchor| anchor as u32);
        let (cues, replace) = {
            let mut state = self.state();
            if state.watching.as_deref() != Some(key) {
                return;
            }
            let replace = all || state.sent_ai.is_empty();
            if replace {
                state.sent_ai.clear();
            }
            let fresh: Vec<Caption> = heard
                .cues
                .iter()
                .filter(|cue| replace || !state.sent_ai.contains(&(cue.start_ms, cue.end_ms)))
                .cloned()
                .collect();
            state
                .sent_ai
                .extend(fresh.iter().map(|cue| (cue.start_ms, cue.end_ms)));
            (fresh, replace)
        };
        let ahead = entries::covered_ahead(&heard.covered, anchor);
        push(
            &self.app,
            &json!({
                "type": "subtitleTrack",
                "id": key,
                "track": AI_SOURCE,
                "kind": "ai",
                "cues": cues,
                "replace": replace,
                "status": {
                    "running": heard.running,
                    "preparing": heard.preparing,
                    "aheadMs": ahead,
                    "readyUntilMs": anchor.saturating_add(ahead),
                    "complete": heard.complete,
                    "error": heard.error,
                },
            }),
        );
    }

    // ---- Android's player ----

    /// A request from Android's player about AI subtitles.
    pub async fn on_request(self: &Arc<Self>, action: &str, raw: &Value) {
        let key = raw
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if key.is_empty() {
            return;
        }
        let position = raw
            .get("positionMs")
            .and_then(Value::as_f64)
            .map_or(0, |ms| ms.clamp(0.0, f64::from(u32::MAX)) as u32);
        match action {
            // The player loaded a video and shows its AI subtitles from now on.
            "subtitleTools" => {
                {
                    let mut state = self.state();
                    state.watching = Some(key.clone());
                    state.sent_ai.clear();
                    state.positions.insert(key.clone(), position);
                }
                self.push_tools(&key).await;
                if self.transcribing(&key) {
                    self.push_ai(&key, true).await;
                }
                if let Ok(translations) = self.translations(&key).await {
                    for view in translations {
                        let cues = view.cues.clone();
                        push(&self.app, &translation_update(&view, cues, true));
                    }
                }
            }
            "transcribe" => {
                if raw.get("confirmed").and_then(Value::as_bool) == Some(true) {
                    self.confirm_online().await;
                }
                if let Err(error) = self.transcribe(&key, position).await {
                    push_notice(&self.app, &key, &error);
                }
            }
            "transcribeStop" => {
                if let Some(captions) = self.app.try_state::<Arc<CaptionService>>() {
                    captions.stop(&key);
                }
            }
            "transcribeSetup" => {
                let service = self.clone();
                tauri::async_runtime::spawn(async move {
                    service.set_up_transcription(&key, position).await;
                });
            }
            "translateSubtitles" => {
                let Some(source) = raw
                    .get("source")
                    .cloned()
                    .and_then(|source| serde_json::from_value::<SubtitleSource>(source).ok())
                else {
                    push_notice(&self.app, &key, "Those subtitles couldn't be read");
                    return;
                };
                let request = SubtitleTranslateRequest {
                    key: key.clone(),
                    source,
                    language: raw
                        .get("language")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    translator: raw
                        .get("translator")
                        .and_then(Value::as_str)
                        .unwrap_or(DEVICE)
                        .to_owned(),
                    from_ms: position,
                    download: raw.get("download").and_then(Value::as_bool) == Some(true),
                };
                let language = request.language.clone();
                let translator = request.translator.clone();
                match self.translate(request).await {
                    Ok(view) if view.download_bytes > 0.0 => push(
                        &self.app,
                        &json!({
                            "type": "subtitleAsk",
                            "id": key,
                            "language": language,
                            "translator": translator,
                            "downloadBytes": view.download_bytes,
                        }),
                    ),
                    Ok(_) => {}
                    Err(error) => push_notice(&self.app, &key, &error),
                }
            }
            "translateStop" => {
                if let Some(track) = raw.get("track").and_then(Value::as_str) {
                    self.stop(&key, track);
                }
            }
            // Settings → AI's transcription section, while the video waits in its small window.
            "openAiSettings" => {
                let _ = OpenRouteEvent(TRANSCRIPTION_ROUTE.into()).emit(&self.app);
            }
            _ => {}
        }
    }

    /// Transcribes the video Android's player shows, from `position`, through the stream
    /// proxy (which sends the source's headers).
    async fn transcribe(&self, key: &str, position: u32) -> Result<(), String> {
        let playing = self
            .state()
            .playing
            .iter()
            .find(|playing| playing.key == key)
            .cloned()
            .ok_or_else(|| "This video can't be transcribed: play it again".to_owned())?;
        let media = self
            .app
            .try_state::<raygun_net::MediaLinks>()
            .ok_or_else(|| "The stream proxy isn't running".to_owned())?;
        let url = if media.is_local(&playing.url) {
            playing.url.clone()
        } else {
            media
                .link(&playing.url, &playing.headers)
                .ok_or_else(|| "Only web addresses can be transcribed".to_owned())?
        };
        let captions = self.app.state::<Arc<CaptionService>>();
        captions
            .inner()
            .transcribe_video(key, &url, playing.format.clone(), position, false)
            .await?;
        self.state().sent_ai.clear();
        self.push_ai(key, true).await;
        Ok(())
    }

    /// The player's "Download the speech model": the model set in Settings (Whisper tiny at
    /// first) is downloaded, transcription is set to use it, and the video is transcribed.
    async fn set_up_transcription(self: &Arc<Self>, key: &str, position: u32) {
        let db = self.db();
        let mut prefs = crate::settings::load(&db)
            .await
            .map(|settings| settings.transcription)
            .unwrap_or_default();
        if prefs.local_model.is_empty() {
            prefs.local_model = raygun_model::TranscriptionPrefs::default().local_model;
        }
        let model = prefs.local_model.clone();
        let ai = self.app.state::<AiService>();
        if ai.installed_dir(&model).is_none() {
            let size = raygun_ai::models::find(&model)
                .map(|info| info.size())
                .unwrap_or(0);
            {
                let mut state = self.state();
                state.setup_error = None;
                state.speech_download = Some((model.clone(), 0));
            }
            self.push_tools(key).await;
            let download = ai.download_model(&model);
            tokio::pin!(download);
            let result = loop {
                tokio::select! {
                    result = &mut download => break result,
                    () = tokio::time::sleep(DOWNLOAD_STEP) => {
                        let received = ai.received(&model).unwrap_or(0);
                        self.state().speech_download = Some((model.clone(), percent(received, size)));
                        self.push_tools(key).await;
                    }
                }
            };
            self.state().speech_download = None;
            if let Err(error) = result {
                self.state().setup_error = Some(error);
                self.push_tools(key).await;
                return;
            }
        }
        prefs.engine = TranscriptionEngine::Local;
        match crate::settings::update(&db, SettingChange::Transcription(prefs)).await {
            Ok(_) => {
                let _ = SettingsChangedEvent("transcription".into()).emit(&self.app);
            }
            Err(error) => {
                self.state().setup_error = Some(error.to_string());
                self.push_tools(key).await;
                return;
            }
        }
        self.push_tools(key).await;
        if let Err(error) = self.transcribe(key, self.position(key).max(position)).await {
            push_notice(&self.app, key, &error);
        }
    }

    /// Online transcription was confirmed once in the player (it may cost money).
    async fn confirm_online(&self) {
        let db = self.db();
        let Ok(settings) = crate::settings::load(&db).await else {
            return;
        };
        if settings.transcription.online_confirmed {
            return;
        }
        let mut prefs = settings.transcription;
        prefs.online_confirmed = true;
        if crate::settings::update(&db, SettingChange::Transcription(prefs))
            .await
            .is_ok()
        {
            let _ = SettingsChangedEvent("transcription".into()).emit(&self.app);
        }
    }

    /// What Android's player offers for AI subtitles: whether transcription is set up (and the
    /// speech model to download in place when it isn't), and who can translate into what.
    async fn push_tools(&self, key: &str) {
        let db = self.db();
        let settings = crate::settings::load(&db).await.unwrap_or_default();
        let prefs = settings.transcription;
        let problem = match self.app.try_state::<Arc<CaptionService>>() {
            Some(captions) => captions.engine_problem().await,
            None => Some("Transcription isn't available".into()),
        };
        let ai = self.app.state::<AiService>();
        let model = if prefs.local_model.is_empty() {
            raygun_model::TranscriptionPrefs::default().local_model
        } else {
            prefs.local_model.clone()
        };
        let installable = problem.is_some()
            && prefs.engine != TranscriptionEngine::Online
            && ai.installed_dir(&model).is_none();
        let model_info = raygun_ai::models::find(&model).ok().filter(|_| installable);
        let catalog = raygun_ai::models::catalog();
        let preferred = Some(settings.lyrics_translation.local_model.as_str())
            .filter(|model| !model.is_empty());
        let device: Vec<&str> = LANGUAGES
            .iter()
            .copied()
            .filter(|tag| raygun_ai::marian::plan(&catalog, tag, preferred).is_some())
            .collect();
        let translators: Vec<Value> = ai
            .providers()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|provider| {
                json!({
                    "id": format!("{AI_PREFIX}{}", provider.id),
                    "name": provider.name,
                    "supported": provider.has_key || provider.kind == AiProviderKind::OpenAiCompatible,
                })
            })
            .collect();
        let (downloading, setup_error) = {
            let state = self.state();
            (
                state
                    .speech_download
                    .as_ref()
                    .filter(|(id, _)| *id == model)
                    .map(|(_, percent)| *percent),
                state.setup_error.clone(),
            )
        };
        push(
            &self.app,
            &json!({
                "type": "subtitleTools",
                "id": key,
                "transcription": {
                    "ready": problem.is_none(),
                    "problem": problem,
                    "engine": prefs.engine,
                    "confirmed": prefs.online_confirmed,
                    "onlineModel": prefs.online_model,
                    "model": model_info.map(|info| json!({ "id": info.id, "name": info.name, "bytes": info.size() })),
                    "downloading": downloading,
                    "error": setup_error,
                },
                "translation": {
                    "languages": LANGUAGES,
                    "device": device,
                    "translators": translators,
                },
            }),
        );
    }

    // ---- Kept translations ----

    fn kept_path(&self, key: &str, source: &str, language: &str) -> PathBuf {
        self.dir.join(kept_file_name(key, source, language))
    }

    async fn read_kept(&self, key: &str, source: &str, language: &str) -> Option<Kept> {
        let text = tokio::fs::read_to_string(self.kept_path(key, source, language))
            .await
            .ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Every kept translation of a video.
    async fn all_kept(&self, key: &str) -> Vec<Kept> {
        let prefix = kept_prefix(key);
        let Ok(mut folder) = tokio::fs::read_dir(&self.dir).await else {
            return Vec::new();
        };
        let mut out = Vec::new();
        while let Ok(Some(file)) = folder.next_entry().await {
            let name = file.file_name().to_string_lossy().into_owned();
            if !name.starts_with(&prefix) || !name.ends_with(".json") {
                continue;
            }
            if let Ok(text) = tokio::fs::read_to_string(file.path()).await
                && let Ok(kept) = serde_json::from_str::<Kept>(&text)
                && !kept.source.is_empty()
            {
                out.push(kept);
            }
        }
        out
    }

    /// Writes a job's file and its track (`.vtt`); nothing is kept while incognito is on.
    async fn save(&self, job: &Job) {
        if crate::privacy::incognito(&self.app) {
            return;
        }
        let kept = job.kept();
        let path = self.kept_path(&job.key, &kept.source, &kept.language);
        let Ok(text) = serde_json::to_string(&kept) else {
            return;
        };
        let rows: Vec<raygun_db::captions::CaptionRow> = entries::translated(&kept.entries)
            .into_iter()
            .map(|cue| raygun_db::captions::CaptionRow {
                start_ms: i64::from(cue.start_ms),
                end_ms: i64::from(cue.end_ms),
                text: cue.text,
            })
            .collect();
        let written = async {
            tokio::fs::create_dir_all(&self.dir).await?;
            tokio::fs::write(&path, text).await?;
            tokio::fs::write(
                path.with_extension("vtt"),
                crate::captions::cues::to_vtt(&rows),
            )
            .await
        };
        if let Err(error) = written.await {
            tracing::warn!(%error, "couldn't keep translated subtitles");
        }
    }
}

/// A translated track as Android's player takes it.
fn translation_update(view: &SubtitleTranslation, cues: Vec<Caption>, replace: bool) -> Value {
    json!({
        "type": "subtitleTrack",
        "id": view.key,
        "track": view.id,
        "kind": "translation",
        "language": view.language,
        "sourceLabel": view.source_label,
        "cues": cues,
        "replace": replace,
        "status": {
            "running": view.running,
            "aheadMs": view.ahead_ms,
            "readyUntilMs": view.ready_until_ms,
            "done": view.done,
            "total": view.total,
            "complete": view.complete,
            "downloading": view.downloading,
            "error": view.error,
            "by": view.by,
        },
    })
}

/// Sends an update to Android's player, which shows these tracks itself; the desktop player
/// follows commands and events instead.
#[cfg(target_os = "android")]
fn push(app: &AppHandle, update: &Value) {
    if let Err(error) = crate::video::update_player(app, update) {
        tracing::debug!(%error, "the player didn't take an update");
    }
}

#[cfg(not(target_os = "android"))]
fn push(_app: &AppHandle, _update: &Value) {}

/// Something the player says briefly (why a request didn't start).
fn push_notice(app: &AppHandle, key: &str, text: &str) {
    push(
        app,
        &json!({ "type": "subtitleNotice", "id": key, "text": text }),
    );
}

fn percent(received: u64, total: u64) -> u32 {
    if total == 0 {
        return 0;
    }
    ((received.min(total) * 100) / total) as u32
}

fn clamp_ms(ms: i64) -> u32 {
    ms.clamp(0, i64::from(u32::MAX)) as u32
}

/// `m:ss`, for naming where something failed.
fn clock(ms: u32) -> String {
    let seconds = ms / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// A video's kept translation of a track: `<video>.tr.<source>.<language>.json`.
fn kept_file_name(key: &str, source: &str, language: &str) -> String {
    format!("{}{source}.{language}.json", kept_prefix(key))
}

/// What every kept translation of a video's file name starts with. Video names have no dots, so
/// another video's files never start the same way.
fn kept_prefix(key: &str) -> String {
    format!("{}.tr.", safe_name(key))
}

/// Where videos' AI subtitles (`captions` writes `<video>.vtt`) and their translations are kept:
/// `downloads/subtitles/ai`. Settings → Data counts and clears it with the caches.
pub fn folder(downloads_dir: &Path) -> PathBuf {
    downloads_dir.join("subtitles").join("ai")
}

/// The name a video's files start with.
pub fn file_name_of(key: &str) -> String {
    safe_name(key)
}

/// A video key as a file name (as `captions` names AI subtitle files).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kept_files_are_named_by_video_source_and_language() {
        assert_eq!(safe_name("stremio:movie:tt123"), "stremio_movie_tt123");
        assert_eq!(
            kept_file_name("stremio:movie:tt1", "ai", "pt-BR"),
            "stremio_movie_tt1.tr.ai.pt-BR.json"
        );
        // Another video's translations don't count as this one's.
        let other = kept_file_name("source:a.b:1", "0123456789abcdef", "de");
        assert!(other.starts_with(&kept_prefix("source:a.b:1")));
        assert!(!other.starts_with(&kept_prefix("source:a")));
        assert_eq!(clock(754_000), "12:34");
        assert_eq!(percent(50, 200), 25);
        assert_eq!(percent(500, 200), 100);
        assert_eq!(percent(5, 0), 0);
    }

    #[test]
    fn a_kept_file_reads_back() {
        let kept = Kept {
            source: "ai".into(),
            language: "de".into(),
            translator: DEVICE.into(),
            entries: vec![Entry {
                start_ms: 1_000,
                end_ms: 2_000,
                text: "Hello".into(),
                translation: Some("Hallo".into()),
            }],
            ..Kept::default()
        };
        let text = serde_json::to_string(&kept).unwrap();
        assert!(text.contains("\"startMs\":1000") && text.contains("\"translation\":\"Hallo\""));
        let back: Kept = serde_json::from_str(&text).unwrap();
        assert_eq!(back.entries, kept.entries);
        // Older or partial files still read.
        let partial: Kept = serde_json::from_str(r#"{"source":"ab","language":"fr"}"#).unwrap();
        assert!(partial.entries.is_empty() && partial.translator.is_empty());
        let update = translation_update(
            &SubtitleTranslation {
                key: "k".into(),
                id: "ai.de".into(),
                language: "de".into(),
                translator: DEVICE.into(),
                by: String::new(),
                source_label: None,
                source_language: None,
                cues: Vec::new(),
                done: 1,
                total: 2,
                ahead_ms: Some(3_000),
                ready_until_ms: Some(64_000),
                complete: false,
                running: true,
                downloading: None,
                error: None,
                download_bytes: 0.0,
            },
            vec![Caption {
                start_ms: 1_000,
                end_ms: 2_000,
                text: "Hallo".into(),
            }],
            false,
        );
        assert_eq!(update["track"], "ai.de");
        assert_eq!(update["cues"][0]["startMs"], 1_000);
        assert_eq!(update["status"]["aheadMs"], 3_000);
        assert_eq!(update["status"]["readyUntilMs"], 64_000);
        assert_eq!(update["replace"], false);
    }
}
