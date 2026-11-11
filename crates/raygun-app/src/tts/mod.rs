//! Reading books aloud (M33): sentence by sentence, with the device's own voices (Android's
//! text-to-speech engine through the `raygun-audio` plugin, the webview's `speechSynthesis` on
//! desktop) or voices downloaded to this device (Piper, Kokoro: `voices.rs` synthesizes ahead of
//! what plays, and the plugin's `SpeechClips` or the webview plays it). Rust keeps the session:
//! the sentence being read and the word reached, pausing and resuming there, stepping by
//! paragraphs, the notification's controls (Android's `ReadAloudService`), and the place each
//! chapter was left, so reading it aloud again goes on from there. Progress reaches the UI as
//! `TtsEvent`s.
//!
//! A book is read in the language chosen for it in the reader, else the one it says it's in (its
//! EPUB's, its source's), else the one its chapter's text is in; long passages clearly in another
//! language get that language's voice only when settings ask. Roman numerals in titles are read
//! as numbers (`raygun_ai::tts::numerals`). Speed, pitch, volume and voice changes apply to the
//! reading at once ([`settings_changed`]), from the sentence (or word) being read.

mod session;
mod voices;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use raygun_ai::tts::{language, numerals};
use raygun_model::{
    ReadAloudPrefs, TtsLanguage, TtsNotice, TtsParagraph, TtsRequest, TtsState, TtsUpdate,
    TtsVoice, TtsVoices,
};
#[cfg(not(target_os = "android"))]
use raygun_model::{SpeechCommand, SpeechLine};
use tauri::{AppHandle, Manager};
use tauri_plugin_raygun_audio::SpeechItem;
use tauri_specta::Event;
use tokio::sync::mpsc;

use self::session::{Control, Mark, Report, Utterance};
use self::voices::{ModelVoice, VoiceWorker};
use crate::startup::AppDb;

/// `local_meta` key of the places chapters were left aloud.
const MARKS: &str = "reader.readAloud.marks";
/// How many chapters' places are kept.
const MARKS_KEPT: usize = 300;
/// `local_meta` key of the language each book is read in, where one was chosen.
const LANGUAGES: &str = "reader.readAloud.languages";
/// How many books' languages are kept.
const LANGUAGES_KEPT: usize = 1000;
/// How long the notification waits for the next chapter after one ends, ms.
const NEXT_CHAPTER_WAIT_MS: u64 = 15_000;

/// The read-aloud service, managed as Tauri state.
pub struct ReadAloud {
    app: AppHandle,
    state: Mutex<State>,
    /// Engine reports and controls, handled in the order they came.
    reports: mpsc::UnboundedSender<Incoming>,
    /// Whether the webview's speech engine can speak (desktop).
    #[cfg_attr(target_os = "android", allow(dead_code))]
    webview_speech: AtomicBool,
    voices: voices::Voices,
}

/// What the service handles in order.
enum Incoming {
    /// An engine's report or a control, as JSON.
    Report(String),
    /// Run `run`'s reader changed (the device's voices couldn't read): it starts again from the
    /// sentence it's at.
    Restart(u64),
}

#[derive(Default)]
struct State {
    /// Increases with every start, pause, resume and stop: reports of older runs are stale.
    run: u64,
    session: Option<Session>,
    marks: Option<BTreeMap<String, Mark>>,
    languages: Option<BTreeMap<String, String>>,
}

/// What reads the session.
#[derive(Clone)]
enum Reader {
    /// The device's own voices.
    System,
    /// A downloaded voice.
    Model(Box<ModelVoice>),
}

/// What a session was made with: when settings change, what differs decides whether reading
/// starts again from where it is.
#[derive(Debug, Clone, PartialEq)]
struct Made {
    prefs: ReadAloudPrefs,
    /// Speed in tenths.
    rate: u32,
    /// The language chosen for the book.
    chosen: Option<String>,
}

impl Made {
    fn of(settings: &raygun_model::Settings, chosen: Option<String>) -> Self {
        Self {
            prefs: settings.read_aloud.clone(),
            rate: settings.novel_reader.tts_rate,
            chosen,
        }
    }

    /// The same, but for the volume (which the clips' player changes as it plays).
    fn but_volume(&self) -> Self {
        let mut made = self.clone();
        made.prefs.volume = 0;
        made
    }

    fn rate(&self) -> f64 {
        f64::from(self.rate.clamp(5, 20)) / 10.0
    }

    fn pitch(&self) -> f64 {
        f64::from(self.prefs.pitch.clamp(5, 20)) / 10.0
    }

    fn volume(&self) -> f64 {
        f64::from(self.prefs.volume.clamp(10, 100)) / 100.0
    }
}

/// What a chapter is read as and with, from the settings and the book's language.
struct Prepared {
    items: Vec<Utterance>,
    language: Option<String>,
    reader: Reader,
    /// Downloaded voices for passages in other languages, by language.
    passages: BTreeMap<String, Box<ModelVoice>>,
    notice: Option<TtsNotice>,
    made: Made,
}

struct Session {
    title_id: String,
    chapter_id: String,
    title: String,
    chapter: String,
    /// The chapter as it came, to read it again when settings change.
    paragraphs: Vec<TtsParagraph>,
    items: Vec<Utterance>,
    /// The sentence being read (or paused on).
    at: usize,
    /// The word reached in it (UTF-16 in what's spoken), as the device's voices report words.
    word: u32,
    /// The sentence the run now playing started at, and the word its text was cut at.
    cut: (usize, u32),
    paused: bool,
    reader: Reader,
    passages: BTreeMap<String, Box<ModelVoice>>,
    language: Option<String>,
    made: Made,
    notice: Option<TtsNotice>,
    /// A downloaded voice's synthesis, for the run now playing.
    worker: Option<VoiceWorker>,
    /// A downloaded voice's player still holds the paused run: Resume plays on from it.
    held: bool,
}

impl Session {
    fn update(&self, state: TtsState) -> TtsUpdate {
        let item = self.items.get(self.at);
        TtsUpdate {
            state,
            chapter_id: self.chapter_id.clone(),
            paragraph: item.map_or(0, |item| item.paragraph),
            start: item.map_or(0, |item| item.start),
            end: item.map_or(0, |item| item.end),
            notice: self.notice.clone(),
            language: self.language.clone(),
        }
    }

    fn mark(&self) -> Option<Mark> {
        let item = self.items.get(self.at)?;
        Some(Mark {
            paragraph: item.paragraph,
            start: item.start,
            word: self.word,
            at: chrono_now(),
        })
    }

    /// Takes what `prepared` reads with, keeping the place: the same sentence, and the word in
    /// it while it reads the same.
    fn take(&mut self, prepared: Prepared) {
        let place = self
            .items
            .get(self.at)
            .map(|item| (item.paragraph, item.start, item.spoken.clone()));
        let at = place
            .as_ref()
            .and_then(|(paragraph, start, _)| {
                prepared
                    .items
                    .iter()
                    .position(|item| item.paragraph == *paragraph && item.start == *start)
            })
            .unwrap_or(self.at)
            .min(prepared.items.len().saturating_sub(1));
        let same_words = place.as_ref().map(|(_, _, spoken)| spoken.as_str())
            == prepared.items.get(at).map(|item| item.spoken.as_str());
        self.word = if same_words { self.word } else { 0 };
        self.at = at;
        self.cut = (at, self.word);
        self.items = prepared.items;
        self.reader = prepared.reader;
        self.passages = prepared.passages;
        self.language = prepared.language;
        self.notice = prepared.notice;
        self.made = prepared.made;
    }
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}

/// The service, once installed.
fn service(app: &AppHandle) -> Option<Arc<ReadAloud>> {
    app.try_state::<Arc<ReadAloud>>()
        .map(|state| state.inner().clone())
}

/// Starts the service: engine reports (Android's over JNI, the webview's through `tts_report`)
/// are handled in order.
pub fn install(app: &AppHandle) {
    let (sender, mut receiver) = mpsc::unbounded_channel::<Incoming>();
    let service = Arc::new(ReadAloud {
        app: app.clone(),
        state: Mutex::new(State::default()),
        reports: sender,
        webview_speech: AtomicBool::new(false),
        voices: voices::Voices::default(),
    });
    app.manage(service.clone());
    tauri::async_runtime::spawn(async move {
        while let Some(incoming) = receiver.recv().await {
            match incoming {
                Incoming::Report(json) => {
                    if let Some(report) = session::parse_report(&json) {
                        service.handle(report).await;
                    }
                }
                Incoming::Restart(run) => service.restart(run).await,
            }
        }
    });
    #[cfg(target_os = "android")]
    {
        let app = app.clone();
        tauri_plugin_raygun_audio::set_tts_handler(move |json| report(&app, &json));
    }
}

/// A report from an engine, or a control from the notification.
pub fn report(app: &AppHandle, json: &str) {
    if let Some(service) = service(app) {
        let _ = service.reports.send(Incoming::Report(json.to_owned()));
    }
}

/// What the webview's engine says when it starts: whether `speechSynthesis` is there.
pub fn set_available(app: &AppHandle, available: bool) {
    if let Some(service) = service(app) {
        service.webview_speech.store(available, Ordering::Relaxed);
    }
}

/// Whether books can be read aloud here: by the device's voices, or a downloaded one.
pub async fn available(app: &AppHandle) -> bool {
    voices::usable(app).await || system_available(app).await
}

/// Whether the device's own voices can read.
async fn system_available(app: &AppHandle) -> bool {
    #[cfg(target_os = "android")]
    {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            plugin(&app).is_some_and(|engine| engine.tts_available())
        })
        .await
        .unwrap_or(false)
    }
    #[cfg(not(target_os = "android"))]
    {
        service(app).is_some_and(|service| service.webview_speech.load(Ordering::Relaxed))
    }
}

pub async fn speak(app: &AppHandle, request: TtsRequest) -> Result<(), String> {
    let service = service(app).ok_or("Reading aloud isn't ready")?;
    service.speak(request).await
}

pub async fn pause(app: &AppHandle) -> Result<(), String> {
    if let Some(service) = service(app) {
        service.pause().await;
    }
    Ok(())
}

pub async fn resume(app: &AppHandle) -> Result<(), String> {
    if let Some(service) = service(app) {
        service.resume().await;
    }
    Ok(())
}

pub async fn step(app: &AppHandle, step: i32) -> Result<(), String> {
    if let Some(service) = service(app) {
        service.step(step).await;
    }
    Ok(())
}

pub async fn stop(app: &AppHandle) -> Result<(), String> {
    if let Some(service) = service(app) {
        service.stop(false).await;
    }
    Ok(())
}

/// Settings changed: what's being read goes on with them, from where it is.
pub async fn settings_changed(app: &AppHandle) {
    if let Some(service) = service(app) {
        service.refresh().await;
    }
}

/// The language `title_id` is read in: the one chosen for it, and the one it says it's in.
pub async fn language_of(app: &AppHandle, title_id: &str) -> TtsLanguage {
    let Some(service) = service(app) else {
        return TtsLanguage {
            chosen: None,
            book: None,
        };
    };
    TtsLanguage {
        chosen: service.chosen_language(title_id).await,
        book: service.stated_language(title_id).await,
    }
}

/// Reads `title_id` in `language` from now on (none: the book's own); a reading of it goes on
/// in it at once.
pub async fn set_language(
    app: &AppHandle,
    title_id: &str,
    language: Option<String>,
) -> Result<(), String> {
    let language = language.filter(|tag| !tag.is_empty());
    if let Some(tag) = &language {
        let well_formed = (2..=35).contains(&tag.len())
            && tag
                .split('-')
                .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric()));
        if !well_formed {
            return Err(format!("\"{tag}\" is not a language tag"));
        }
    }
    let service = service(app).ok_or("Reading aloud isn't ready")?;
    service.keep_language(title_id, language).await;
    service.refresh().await;
    Ok(())
}

/// Where reading aloud is now, when it is.
pub fn now(app: &AppHandle) -> Option<TtsUpdate> {
    let service = service(app)?;
    let state = service.state.lock().unwrap_or_else(|p| p.into_inner());
    let session = state.session.as_ref()?;
    Some(session.update(if session.paused {
        TtsState::Paused
    } else {
        TtsState::Speaking
    }))
}

/// The voices this device can download and read with.
pub fn voice_list() -> TtsVoices {
    let runtime_id =
        raygun_ai::models::runtime_for(raygun_ai::tts::RUNTIME_ENGINE).map(|model| model.id);
    let voices = raygun_ai::models::catalog()
        .iter()
        .filter(|model| model.kind == "tts")
        .flat_map(|model| {
            let engine = voices::engine_of(model);
            model
                .tts
                .iter()
                .flat_map(|setup| &setup.voices)
                .map(move |voice| TtsVoice {
                    key: raygun_ai::tts::select::voice_key(&model.id, &voice.id),
                    model_id: model.id.clone(),
                    engine,
                    name: voice.name.clone(),
                    language: voice.language.clone(),
                    gender: voice.gender.clone(),
                })
        })
        .collect();
    TtsVoices { runtime_id, voices }
}

/// Reads a sample sentence with `key`'s voice (none: the device's own), in `language`.
pub async fn preview(
    app: &AppHandle,
    key: Option<String>,
    language: Option<String>,
) -> Result<(), String> {
    let service = service(app).ok_or("Reading aloud isn't ready")?;
    service.preview(key, language).await
}

impl ReadAloud {
    fn emit(&self, update: TtsUpdate) {
        let _ = crate::events::TtsEvent(update).emit(&self.app);
    }

    async fn settings(&self) -> raygun_model::Settings {
        let db = self.app.state::<AppDb>().0.clone();
        crate::settings::load(&db).await.unwrap_or_default()
    }

    async fn marks(&self) -> BTreeMap<String, Mark> {
        if let Some(marks) = &self.state.lock().unwrap_or_else(|p| p.into_inner()).marks {
            return marks.clone();
        }
        let db = self.app.state::<AppDb>().0.clone();
        let loaded: BTreeMap<String, Mark> = raygun_db::local_value(&db, MARKS)
            .await
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.marks.get_or_insert(loaded).clone()
    }

    /// Keeps (or with `None`, forgets) where `chapter` was left aloud.
    fn keep_mark(&self, chapter: &str, mark: Option<Mark>) {
        let text = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let Some(marks) = state.marks.as_mut() else {
                return;
            };
            match mark {
                Some(mark) => {
                    marks.insert(chapter.to_owned(), mark);
                }
                None => {
                    marks.remove(chapter);
                }
            }
            if marks.len() > MARKS_KEPT {
                *marks =
                    session::trimmed_marks(std::mem::take(marks).into_iter().collect(), MARKS_KEPT);
            }
            serde_json::to_string(marks).unwrap_or_default()
        };
        let db = self.app.state::<AppDb>().0.clone();
        tauri::async_runtime::spawn(async move {
            let _ = raygun_db::set_local_value(&db, MARKS, &text).await;
        });
    }

    /// The languages chosen for books, by title.
    async fn languages(&self) -> BTreeMap<String, String> {
        if let Some(languages) = &self
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .languages
        {
            return languages.clone();
        }
        let db = self.app.state::<AppDb>().0.clone();
        let loaded: BTreeMap<String, String> = raygun_db::local_value(&db, LANGUAGES)
            .await
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.languages.get_or_insert(loaded).clone()
    }

    async fn chosen_language(&self, title_id: &str) -> Option<String> {
        if title_id.is_empty() {
            return None;
        }
        self.languages().await.get(title_id).cloned()
    }

    async fn keep_language(&self, title_id: &str, language: Option<String>) {
        self.languages().await;
        let text = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let languages = state.languages.get_or_insert_with(BTreeMap::new);
            match language {
                Some(language) => {
                    languages.insert(title_id.to_owned(), language);
                }
                None => {
                    languages.remove(title_id);
                }
            }
            // Old choices go first when there are too many (titles' ids sort by when they were
            // made, UUIDv7).
            while languages.len() > LANGUAGES_KEPT {
                languages.pop_first();
            }
            serde_json::to_string(languages).unwrap_or_default()
        };
        let db = self.app.state::<AppDb>().0.clone();
        let _ = raygun_db::set_local_value(&db, LANGUAGES, &text).await;
    }

    /// The language the book says it's in: its EPUB's, or its source's.
    async fn stated_language(&self, title_id: &str) -> Option<String> {
        if title_id.is_empty() {
            return None;
        }
        let dictionary = self
            .app
            .try_state::<crate::dictionary::DictionaryService>()?;
        dictionary.book_language(title_id).await
    }

    /// What `paragraphs` of `title_id` are read as and with, by the settings now.
    async fn prepare(&self, title_id: &str, paragraphs: &[TtsParagraph]) -> Prepared {
        let settings = self.settings().await;
        let chosen = self.chosen_language(title_id).await;
        let made = Made::of(&settings, chosen.clone());
        let language = match chosen {
            Some(chosen) => Some(chosen),
            None => match self.stated_language(title_id).await {
                Some(stated) => Some(stated),
                None => language::guess(&session::language_sample(paragraphs)).map(str::to_owned),
            },
        };
        let prefs = &made.prefs;
        // Everywhere and ordinals go with titles (the reader's options turn them off together).
        let rules = numerals::Rules {
            titles: prefs.roman_titles,
            everywhere: prefs.roman_titles && prefs.roman_everywhere,
            ordinals: prefs.roman_ordinals,
        };
        let items = session::utterances(
            paragraphs,
            &session::Reading {
                language: language.as_deref(),
                detect_languages: prefs.detect_languages,
                rules,
            },
        );
        let system = system_available(&self.app).await;
        let (reader, mut notice) = voices::choose(&self.app, prefs, language.as_deref(), system);
        let mut passages = BTreeMap::new();
        if let Reader::Model(voice) = &reader {
            let others: BTreeSet<&String> = items
                .iter()
                .filter_map(|item| item.language.as_ref())
                .collect();
            for other in others {
                match voices::passage_voice(&self.app, prefs, voice, other) {
                    Some(passage) => {
                        passages.insert(other.clone(), Box::new(passage));
                    }
                    // The book's voice reads it.
                    None if notice.is_none() => {
                        notice = Some(TtsNotice::OtherVoice {
                            language: Some(other.clone()),
                        });
                    }
                    None => {}
                }
            }
        }
        Prepared {
            items,
            language,
            reader,
            passages,
            notice,
            made,
        }
    }

    async fn speak(self: &Arc<Self>, request: TtsRequest) -> Result<(), String> {
        let prepared = self.prepare(&request.title_id, &request.paragraphs).await;
        if prepared.items.is_empty() {
            return Err("There is nothing to read".into());
        }
        // The chapter being read now (opened again, say) goes on from the sentence it's at; else
        // from where it was left aloud.
        let live = {
            let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state
                .session
                .as_ref()
                .filter(|session| session.chapter_id == request.chapter_id)
                .and_then(Session::mark)
        };
        let marks = self.marks().await;
        let mark = live.or_else(|| marks.get(&request.chapter_id).copied());
        let (at, word) = session::start_point(
            &prepared.items,
            request.from,
            request.offset,
            request.resume,
            mark.as_ref(),
        )
        .ok_or("There is nothing to read")?;
        crate::playback::other_media_started(&self.app).await;
        // Whatever read before stops, whichever voices read it.
        system_stop(&self.app).await;
        clips(&self.app, ClipCommand::Stop).await;
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            // Reports of what read before are stale from here on.
            state.run += 1;
            if let Some(old) = state.session.as_mut()
                && let Some(worker) = old.worker.take()
            {
                worker.cancel();
            }
            state.session = Some(Session {
                title_id: request.title_id,
                chapter_id: request.chapter_id,
                title: request.title,
                chapter: request.chapter,
                paragraphs: request.paragraphs,
                items: prepared.items,
                at,
                word,
                cut: (at, word),
                paused: false,
                reader: prepared.reader,
                passages: prepared.passages,
                language: prepared.language,
                made: prepared.made,
                notice: prepared.notice,
                worker: None,
                held: false,
            });
        }
        self.start_run(at, word).await;
        Ok(())
    }

    /// Settings (or the book's language) changed: reading goes on with them from where it is.
    /// The volume alone turns the downloaded voices' player at once.
    async fn refresh(self: &Arc<Self>) {
        let (title_id, chapter_id, paragraphs, made) = {
            let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let Some(session) = state.session.as_ref() else {
                return;
            };
            (
                session.title_id.clone(),
                session.chapter_id.clone(),
                session.paragraphs.clone(),
                session.made.clone(),
            )
        };
        let settings = self.settings().await;
        let now = Made::of(&settings, self.chosen_language(&title_id).await);
        if now == made {
            return;
        }
        if now.but_volume() == made.but_volume() {
            let model = {
                let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
                let Some(session) = state.session.as_mut() else {
                    return;
                };
                let model = matches!(session.reader, Reader::Model(_));
                if model {
                    session.made = now.clone();
                }
                model
            };
            if model {
                clips(&self.app, ClipCommand::Volume(now.volume())).await;
                return;
            }
        }
        let prepared = self.prepare(&title_id, &paragraphs).await;
        if prepared.items.is_empty() {
            return;
        }
        let restart = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let fresh = state
                .session
                .as_ref()
                .is_some_and(|session| session.chapter_id == chapter_id);
            if !fresh {
                return;
            }
            state.run += 1;
            let Some(session) = state.session.as_mut() else {
                return;
            };
            if let Some(worker) = session.worker.take() {
                worker.cancel();
            }
            let old = session.reader.clone();
            session.take(prepared);
            session.held = false;
            (old, (!session.paused).then_some((session.at, session.word)))
        };
        let (old, start) = restart;
        self.stop_engines(&old).await;
        match start {
            Some((at, word)) => self.start_run(at, word).await,
            None => self.emit_now(TtsState::Paused),
        }
    }

    /// Plays the session from sentence `at` (the word `word` in it) as a new run.
    async fn start_run(self: &Arc<Self>, at: usize, word: u32) {
        let Some((run, plan, worker)) = self.begin_run(at, word) else {
            return;
        };
        self.emit_now(TtsState::Speaking);
        self.notify(&plan.title, &plan.chapter, true).await;
        match (&plan.reader, worker) {
            (Reader::Model(voice), Some(worker)) => {
                clips(&self.app, ClipCommand::Volume(plan.volume)).await;
                let (service, voice) = (self.clone(), voice.clone());
                tauri::async_runtime::spawn(async move {
                    service.synthesize(run, voice, plan, worker).await;
                });
            }
            _ => self.speak_system(run, plan).await,
        }
    }

    /// Has the device's voices read run `run`; when they can't, a downloaded voice reads on.
    async fn speak_system(self: &Arc<Self>, run: u64, plan: Plan) {
        let speech: Vec<SpeechItem> = plan
            .items
            .iter()
            .enumerate()
            .map(|(offset, item)| {
                let index = plan.first + offset;
                SpeechItem {
                    id: session::item_id(run, index, index == plan.last),
                    text: if offset == 0 {
                        session::from_word(&item.spoken, plan.word).0
                    } else {
                        item.spoken.clone()
                    },
                    language: item.language.clone(),
                }
            })
            .collect();
        let spoken = system_speak(
            &self.app,
            speech,
            plan.rate,
            plan.pitch,
            plan.volume,
            plan.language,
        )
        .await;
        if let Err(error) = spoken {
            tracing::warn!(%error, "the device's voices couldn't read aloud");
            if !self.fall_forward(run).await {
                self.fail(run, TtsNotice::EngineFailed).await;
            }
        }
    }

    /// The device's voices couldn't read run `run`: a downloaded voice reads on from the same
    /// sentence, when there is one (and none failed already). Whether one does.
    async fn fall_forward(self: &Arc<Self>, run: u64) -> bool {
        let prefs = self.settings().await.read_aloud;
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.run != run {
            return true;
        }
        let Some(session) = state.session.as_mut() else {
            return true;
        };
        if matches!(session.notice, Some(TtsNotice::VoiceFailed)) {
            return false;
        }
        let Some(voice) = voices::any_voice(&self.app, &prefs, session.language.as_deref()) else {
            return false;
        };
        let speaks = voice.speaks(session.language.as_deref());
        session.reader = Reader::Model(Box::new(voice));
        session.notice = (!speaks).then(|| TtsNotice::OtherVoice {
            language: session.language.clone(),
        });
        // Started from the queue: this run's own call can't start the next.
        let _ = self.reports.send(Incoming::Restart(run));
        true
    }

    /// Starts run `run`'s session again from the sentence it's at, when nothing newer started.
    async fn restart(self: &Arc<Self>, run: u64) {
        let at = {
            let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.run != run {
                return;
            }
            match state.session.as_ref() {
                Some(session) if !session.paused => session.at,
                _ => return,
            }
        };
        self.start_run(at, 0).await;
    }

    /// A new run from sentence `at` (the word `word` in it): what it plays, and for a downloaded
    /// voice, its synthesis's handle.
    fn begin_run(&self, at: usize, word: u32) -> Option<(u64, Plan, Option<voices::WorkerHandle>)> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.run += 1;
        let run = state.run;
        let session = state.session.as_mut()?;
        if let Some(worker) = session.worker.take() {
            worker.cancel();
        }
        let at = at.min(session.items.len().saturating_sub(1));
        let item = session.items.get(at)?;
        // The device's voices start at the word's start; downloaded ones at the sentence's.
        let word = match session.reader {
            Reader::System => session::from_word(&item.spoken, word).1,
            Reader::Model(_) => 0,
        };
        session.at = at;
        session.word = word;
        session.cut = (at, word);
        session.paused = false;
        session.held = false;
        let handle = match session.reader {
            Reader::Model(_) => {
                let worker = VoiceWorker::new(at);
                let handle = worker.handle();
                session.worker = Some(worker);
                Some(handle)
            }
            Reader::System => None,
        };
        let plan = Plan {
            reader: session.reader.clone(),
            passages: session.passages.clone(),
            items: session.items[at..].to_vec(),
            first: at,
            last: session.items.len() - 1,
            word,
            rate: session.made.rate(),
            pitch: session.made.pitch(),
            volume: session.made.volume(),
            language: session.language.clone(),
            title: session.title.clone(),
            chapter: session.chapter.clone(),
        };
        Some((run, plan, handle))
    }

    fn emit_now(&self, state: TtsState) {
        let update = {
            let guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
            guard.session.as_ref().map(|session| session.update(state))
        };
        if let Some(update) = update {
            self.emit(update);
        }
    }

    async fn pause(self: &Arc<Self>) {
        let (reader, mark, chapter, title, name) = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let Some(session) = state.session.as_mut() else {
                return;
            };
            if session.paused {
                return;
            }
            session.paused = true;
            let model = matches!(session.reader, Reader::Model(_));
            session.held = model;
            let (reader, mark) = (session.reader.clone(), session.mark());
            let (chapter, title, name) = (
                session.chapter_id.clone(),
                session.title.clone(),
                session.chapter.clone(),
            );
            // The device's voices are stopped: their late reports are stale.
            if !model {
                state.run += 1;
            }
            (reader, mark, chapter, title, name)
        };
        match reader {
            Reader::System => system_stop(&self.app).await,
            Reader::Model(_) => {
                clips(&self.app, ClipCommand::Pause).await;
            }
        }
        self.keep_mark(&chapter, mark);
        self.emit_now(TtsState::Paused);
        self.notify(&title, &name, false).await;
    }

    async fn resume(self: &Arc<Self>) {
        let (held, at, word, title, name) = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let Some(session) = state.session.as_mut() else {
                return;
            };
            if !session.paused {
                return;
            }
            session.paused = false;
            (
                session.held,
                session.at,
                session.word,
                session.title.clone(),
                session.chapter.clone(),
            )
        };
        if held && clips(&self.app, ClipCommand::Resume).await {
            self.emit_now(TtsState::Speaking);
            self.notify(&title, &name, true).await;
            return;
        }
        self.start_run(at, word).await;
    }

    /// Reads from the paragraph `step` places from the one being read (0: its start again).
    async fn step(self: &Arc<Self>, step: i32) {
        let found = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            // What plays now is stale from here on.
            state.run += 1;
            let Some(session) = state.session.as_mut() else {
                return;
            };
            if let Some(worker) = session.worker.take() {
                worker.cancel();
            }
            session::step_from(&session.items, session.at, step)
                .map(|target| (target, session.reader.clone()))
        };
        if let Some((target, reader)) = found {
            self.stop_engines(&reader).await;
            self.start_run(target, 0).await;
        }
    }

    /// Stops reading, keeping the place. `told` when the notification or a headset stopped it
    /// (the reader hears it then).
    async fn stop(self: &Arc<Self>, told: bool) {
        let ended = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.run += 1;
            state.session.take()
        };
        let Some(mut session) = ended else {
            return;
        };
        if let Some(worker) = session.worker.take() {
            worker.cancel();
        }
        self.stop_engines(&session.reader).await;
        self.keep_mark(&session.chapter_id, session.mark());
        if told {
            self.emit(session.update(TtsState::Stopped));
        }
        end_notice(&self.app).await;
        self.voices.release_later();
    }

    async fn stop_engines(&self, reader: &Reader) {
        match reader {
            Reader::System => system_stop(&self.app).await,
            Reader::Model(_) => {
                clips(&self.app, ClipCommand::Stop).await;
            }
        }
    }

    /// The engine gave up on run `run`: reading stops and the reader says why.
    async fn fail(self: &Arc<Self>, run: u64, notice: TtsNotice) {
        let ended = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.run != run {
                return;
            }
            state.run += 1;
            state.session.take()
        };
        if let Some(mut session) = ended {
            if let Some(worker) = session.worker.take() {
                worker.cancel();
            }
            self.stop_engines(&session.reader).await;
            self.keep_mark(&session.chapter_id, session.mark());
            session.notice = Some(notice);
            self.emit(session.update(TtsState::Error));
            end_notice(&self.app).await;
        }
    }

    async fn handle(self: &Arc<Self>, report: Report) {
        match report {
            Report::Control(control) => self.control(control).await,
            Report::Speaking { run, index } => {
                let mark = {
                    let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
                    if state.run != run {
                        return;
                    }
                    let Some(session) = state.session.as_mut() else {
                        return;
                    };
                    let paragraph_changed = session
                        .items
                        .get(session.at)
                        .zip(session.items.get(index))
                        .is_some_and(|(was, now)| was.paragraph != now.paragraph);
                    session.at = index.min(session.items.len().saturating_sub(1));
                    session.word = if session.cut.0 == index {
                        session.cut.1
                    } else {
                        0
                    };
                    if let Some(worker) = &session.worker {
                        worker.played(index);
                    }
                    paragraph_changed
                        .then(|| {
                            session
                                .mark()
                                .map(|mark| (session.chapter_id.clone(), mark))
                        })
                        .flatten()
                };
                if let Some((chapter, mark)) = mark {
                    self.keep_mark(&chapter, Some(mark));
                }
                self.emit_now(TtsState::Speaking);
            }
            Report::Range { run, index, start } => {
                let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
                if state.run != run {
                    return;
                }
                if let Some(session) = state.session.as_mut()
                    && session.at == index
                {
                    let cut = if session.cut.0 == index {
                        session.cut.1
                    } else {
                        0
                    };
                    session.word = cut + start;
                }
            }
            Report::Ended { run } => {
                let ended = {
                    let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
                    if state.run != run {
                        return;
                    }
                    state.run += 1;
                    state.session.take()
                };
                if let Some(session) = ended {
                    self.keep_mark(&session.chapter_id, None);
                    // The player of downloaded voices lets go of its track and the sound
                    // (before the reader can start the next chapter).
                    if matches!(session.reader, Reader::Model(_)) {
                        clips(&self.app, ClipCommand::Stop).await;
                    }
                    self.emit(session.update(TtsState::Ended));
                    // The reader opens the next chapter and reads on; the notification stays
                    // meanwhile, and goes when nothing follows.
                    let service = self.clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(NEXT_CHAPTER_WAIT_MS))
                            .await;
                        let idle = service
                            .state
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .session
                            .is_none();
                        if idle {
                            end_notice(&service.app).await;
                            service.voices.release_later();
                        }
                    });
                }
            }
            Report::Paused { run } => {
                let current = self.state.lock().unwrap_or_else(|p| p.into_inner()).run == run;
                if current {
                    self.pause().await;
                }
            }
            Report::Error { run } => self.fail(run, TtsNotice::EngineFailed).await,
        }
    }

    async fn control(self: &Arc<Self>, control: Control) {
        let paused = {
            let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            match state.session.as_ref() {
                Some(session) => session.paused,
                None => return,
            }
        };
        match control {
            Control::Pause => self.pause().await,
            Control::Resume => self.resume().await,
            Control::Toggle if paused => self.resume().await,
            Control::Toggle => self.pause().await,
            Control::Next => self.step(1).await,
            Control::Previous => self.step(-1).await,
            Control::Stop => self.stop(true).await,
        }
    }

    /// Shows (or updates) the notification with its controls (Android).
    async fn notify(&self, title: &str, chapter: &str, playing: bool) {
        #[cfg(target_os = "android")]
        {
            let app = self.app.clone();
            let payload =
                serde_json::json!({ "title": title, "text": chapter, "playing": playing });
            let _ = tauri::async_runtime::spawn_blocking(move || {
                plugin(&app).map(|engine| engine.tts("ttsNotice", payload))
            })
            .await;
        }
        #[cfg(not(target_os = "android"))]
        let _ = (title, chapter, playing);
    }

    async fn preview(
        self: &Arc<Self>,
        key: Option<String>,
        language: Option<String>,
    ) -> Result<(), String> {
        self.stop(false).await;
        let settings = self.settings().await;
        let made = Made::of(&settings, None);
        let (rate, pitch, volume) = (made.rate(), made.pitch(), made.volume());
        let run = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.run += 1;
            state.run
        };
        let Some(key) = key else {
            let text = voices::sample_text(language.as_deref()).to_owned();
            let item = SpeechItem {
                id: session::item_id(run, 0, true),
                text,
                language: None,
            };
            return system_speak(&self.app, vec![item], rate, pitch, volume, language).await;
        };
        let voice =
            voices::installed_voice(&self.app, &key).ok_or("This voice isn't downloaded")?;
        let text = voices::sample_text(Some(&voice.voice.language)).to_owned();
        clips(&self.app, ClipCommand::Stop).await;
        clips(&self.app, ClipCommand::Volume(volume)).await;
        let clip = self.voices.speak(&voice, &text, rate, pitch).await;
        self.voices.release_later();
        send_clip(&self.app, &session::item_id(run, 0, true), clip?, true).await
    }
}

/// What a run plays.
struct Plan {
    reader: Reader,
    passages: BTreeMap<String, Box<ModelVoice>>,
    items: Vec<Utterance>,
    first: usize,
    last: usize,
    word: u32,
    rate: f64,
    pitch: f64,
    volume: f64,
    language: Option<String>,
    title: String,
    chapter: String,
}

// ---- Engines ----

#[cfg(target_os = "android")]
fn plugin(
    app: &AppHandle,
) -> Option<tauri::State<'_, tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>> {
    app.try_state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
}

/// Has the device's voices read `items`, replacing what they read.
async fn system_speak(
    app: &AppHandle,
    items: Vec<SpeechItem>,
    rate: f64,
    pitch: f64,
    volume: f64,
    language: Option<String>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            plugin(&app)
                .ok_or_else(|| "Text-to-speech isn't available".to_owned())?
                .tts_speak(&items, rate, pitch, volume, language.as_deref())
        })
        .await
        .map_err(|error| error.to_string())?
    }
    #[cfg(not(target_os = "android"))]
    {
        let speaks =
            service(app).is_some_and(|service| service.webview_speech.load(Ordering::Relaxed));
        if !speaks {
            return Err("This system has no voices to read aloud with".into());
        }
        crate::events::SpeechCommandEvent(SpeechCommand::Speak {
            items: items
                .into_iter()
                .map(|item| SpeechLine {
                    id: item.id,
                    text: item.text,
                    language: item.language,
                })
                .collect(),
            rate,
            pitch,
            volume,
            language,
        })
        .emit(app)
        .map_err(|error| error.to_string())
    }
}

async fn system_stop(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        let app = app.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            plugin(&app).map(|engine| engine.tts_stop())
        })
        .await;
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = crate::events::SpeechCommandEvent(SpeechCommand::Stop).emit(app);
    }
}

#[derive(Clone, Copy)]
enum ClipCommand {
    Pause,
    Resume,
    Stop,
    /// 0–1, at once.
    Volume(f64),
}

/// Tells the player of downloaded voices' speech to pause, go on, stop or change its volume.
/// Whether it could.
async fn clips(app: &AppHandle, command: ClipCommand) -> bool {
    #[cfg(target_os = "android")]
    {
        let (name, payload) = match command {
            ClipCommand::Pause => ("ttsClipPause", serde_json::json!({})),
            ClipCommand::Resume => ("ttsClipResume", serde_json::json!({})),
            ClipCommand::Stop => ("ttsClipStop", serde_json::json!({})),
            ClipCommand::Volume(volume) => {
                ("ttsClipVolume", serde_json::json!({ "volume": volume }))
            }
        };
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            plugin(&app).is_some_and(|engine| engine.tts(name, payload).is_ok())
        })
        .await
        .unwrap_or(false)
    }
    #[cfg(not(target_os = "android"))]
    {
        let command = match command {
            ClipCommand::Pause => SpeechCommand::Pause,
            ClipCommand::Resume => SpeechCommand::Resume,
            ClipCommand::Stop => SpeechCommand::Stop,
            ClipCommand::Volume(volume) => SpeechCommand::Volume { volume },
        };
        crate::events::SpeechCommandEvent(command).emit(app).is_ok()
    }
}

/// Hands a clip a downloaded voice made to the player, after the ones before it.
async fn send_clip(
    app: &AppHandle,
    id: &str,
    clip: voices::Clip,
    last: bool,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let app = app.clone();
        let payload = serde_json::json!({
            "id": id,
            "sampleRate": clip.sample_rate,
            "pcm": clip.pcm,
            "last": last,
        });
        tauri::async_runtime::spawn_blocking(move || {
            plugin(&app)
                .ok_or_else(|| "The player isn't ready".to_owned())?
                .tts("ttsClip", payload)
        })
        .await
        .map_err(|error| error.to_string())?
    }
    #[cfg(not(target_os = "android"))]
    {
        crate::events::SpeechCommandEvent(SpeechCommand::Clip {
            id: id.to_owned(),
            sample_rate: clip.sample_rate,
            pcm: clip.pcm,
            last,
        })
        .emit(app)
        .map_err(|error| error.to_string())
    }
}

async fn end_notice(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        let app = app.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            plugin(&app).map(|engine| engine.tts("ttsNoticeEnd", serde_json::json!({})))
        })
        .await;
    }
    #[cfg(not(target_os = "android"))]
    let _ = app;
}
