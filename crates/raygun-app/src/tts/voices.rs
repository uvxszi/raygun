//! Downloaded voices (Piper, Kokoro): which one reads a book (and a passage in another
//! language), the voices kept open between sentences (and let go when reading stops for a while),
//! and the synthesis that runs ahead of what plays: long sentences in pieces (a short one first
//! whenever little waits to be heard, so reading starts soon), the next sentences until enough
//! waits, and what was made kept a while to be played again.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use raygun_ai::models::{ModelInfo, VoiceInfo};
use raygun_ai::tts::{Engine, Runtime, Synthesizer, pcm, select, sentences};
use raygun_model::{ReadAloudEngine, ReadAloudPrefs, TtsNotice};
use tauri::{AppHandle, Manager};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::{Plan, ReadAloud, Reader, session};
use crate::ai::AiService;

/// Sentences are made ahead of the one playing until this much speech waits to be heard…
const LEAD: Duration = Duration::from_secs(20);
/// …the next one always, and never more than this many ahead.
const MOST_AHEAD: usize = 6;
/// With less than this waiting to be heard, the next piece is a short one.
const SHORT_LEAD: Duration = Duration::from_secs(3);
/// Longest piece of a sentence made at once (characters), and when little waits to be heard: a
/// voice takes memory and time by the length of what it reads before any of it plays.
const PIECE: usize = 180;
const SHORT_PIECE: usize = 60;
/// Speech kept to be played again (stepping back, a sentence read again), in bytes as sent.
const KEPT_BYTES: usize = 24 * 1024 * 1024;
/// How long an open voice stays open with nothing read, before its memory is given back.
const KEEP_OPEN: Duration = Duration::from_secs(180);
/// How many voice models stay open at once: the book's and a passage's.
const OPEN_AT_ONCE: usize = 2;

/// A downloaded voice that reads a book.
#[derive(Clone)]
pub struct ModelVoice {
    pub model: ModelInfo,
    pub dir: PathBuf,
    pub voice: VoiceInfo,
    pub engine: Engine,
    pub runtime_dir: PathBuf,
}

impl ModelVoice {
    /// Whether it speaks `language`.
    pub fn speaks(&self, language: Option<&str>) -> bool {
        language.is_some_and(|language| {
            select::primary(language) == select::primary(&self.voice.language)
        })
    }

    fn key(&self) -> String {
        select::voice_key(&self.model.id, &self.voice.id)
    }
}

/// Speech made for the player: 16-bit mono PCM as base64.
#[derive(Clone)]
pub struct Clip {
    pub sample_rate: u32,
    pub pcm: String,
    /// How long it plays.
    pub length: Duration,
}

impl Clip {
    fn of(samples: &[f32], sample_rate: u32) -> Self {
        Self {
            sample_rate,
            pcm: base64::engine::general_purpose::STANDARD.encode(pcm::to_pcm16(samples)),
            length: Duration::from_secs_f64(samples.len() as f64 / f64::from(sample_rate.max(1))),
        }
    }
}

/// The installed voice runtime's folder.
fn runtime_dir(ai: &AiService) -> Option<PathBuf> {
    let runtime = raygun_ai::models::runtime_for(raygun_ai::tts::RUNTIME_ENGINE)?;
    ai.installed_dir(&runtime.id)
}

/// Every downloaded voice, with what it needs to open.
fn installed(app: &AppHandle) -> Option<(PathBuf, Vec<(ModelInfo, PathBuf)>)> {
    let ai = app.try_state::<AiService>()?;
    Some((runtime_dir(&ai)?, ai.installed_of_kind("tts")))
}

fn model_voice(
    runtime_dir: &std::path::Path,
    models: &[(ModelInfo, PathBuf)],
    choice: select::Candidate<'_>,
) -> Option<ModelVoice> {
    let (model, dir) = models
        .iter()
        .find(|(model, _)| model.id == choice.model.id)?;
    Some(ModelVoice {
        model: model.clone(),
        dir: dir.clone(),
        voice: choice.voice.clone(),
        engine: Engine::of(choice.engine())?,
        runtime_dir: runtime_dir.to_owned(),
    })
}

/// The downloaded voices here, to pick from.
struct Installed {
    runtime_dir: PathBuf,
    models: Vec<(ModelInfo, PathBuf)>,
    infos: Vec<ModelInfo>,
}

impl Installed {
    fn of(app: &AppHandle) -> Option<Self> {
        let (runtime_dir, models) = installed(app)?;
        let infos = models.iter().map(|(model, _)| model.clone()).collect();
        Some(Self {
            runtime_dir,
            models,
            infos,
        })
    }

    /// `engine`'s voice for `language` (the one chosen for it, else one that speaks it); without
    /// a language, the one chosen for English or any language, else the engine's first.
    fn pick(
        &self,
        engine: &str,
        prefs: &ReadAloudPrefs,
        language: Option<&str>,
    ) -> Option<ModelVoice> {
        let candidates = select::candidates(&self.infos);
        let chosen = language.and_then(|language| prefs.voices.get(&select::primary(language)));
        let fallback = prefs
            .voices
            .get("en")
            .or_else(|| prefs.voices.values().next());
        let picked = select::pick(
            engine,
            &candidates,
            language,
            chosen.map(String::as_str),
            fallback.map(String::as_str),
        )?;
        model_voice(&self.runtime_dir, &self.models, picked)
    }
}

/// What reads a book in `language`: the downloaded voice settings choose, else the device's own
/// voices when `system` ones are here, else another downloaded voice; with why, when it's not the
/// one asked for. Reading never stops for want of a voice for the language.
pub(super) fn choose(
    app: &AppHandle,
    prefs: &ReadAloudPrefs,
    language: Option<&str>,
    system: bool,
) -> (Reader, Option<TtsNotice>) {
    let installed = Installed::of(app);
    let model = |voice: ModelVoice| Reader::Model(Box::new(voice));
    let Some(engine) = prefs.engine.model_engine() else {
        if system {
            return (Reader::System, None);
        }
        // No voices of the device's: a downloaded one reads.
        return match installed.and_then(|installed| any_of(&installed, prefs, language)) {
            Some(voice) => {
                let notice = (!voice.speaks(language)).then(|| TtsNotice::OtherVoice {
                    language: language.map(str::to_owned),
                });
                (model(voice), notice)
            }
            None => (Reader::System, None),
        };
    };
    if let Some(voice) = installed
        .as_ref()
        .and_then(|installed| installed.pick(engine, prefs, language))
    {
        return (model(voice), None);
    }
    let missing = language.map(str::to_owned);
    if system || language.is_none() {
        return (
            Reader::System,
            Some(TtsNotice::NoVoice { language: missing }),
        );
    }
    match installed.and_then(|installed| installed.pick(engine, prefs, None)) {
        Some(voice) => (
            model(voice),
            Some(TtsNotice::OtherVoice { language: missing }),
        ),
        None => (
            Reader::System,
            Some(TtsNotice::NoVoice { language: missing }),
        ),
    }
}

/// A downloaded voice for a passage in `language` within a book `book` reads: the same engine's,
/// when one speaks it and it isn't the book's.
pub(super) fn passage_voice(
    app: &AppHandle,
    prefs: &ReadAloudPrefs,
    book: &ModelVoice,
    language: &str,
) -> Option<ModelVoice> {
    let engine = book.model.engine.as_deref()?;
    let voice = Installed::of(app)?.pick(engine, prefs, Some(language))?;
    (voice.key() != book.key()).then_some(voice)
}

/// Any downloaded voice, for when the device's voices can't read: the chosen engine's first, one
/// that speaks `language` first.
pub(super) fn any_voice(
    app: &AppHandle,
    prefs: &ReadAloudPrefs,
    language: Option<&str>,
) -> Option<ModelVoice> {
    any_of(&Installed::of(app)?, prefs, language)
}

fn any_of(
    installed: &Installed,
    prefs: &ReadAloudPrefs,
    language: Option<&str>,
) -> Option<ModelVoice> {
    let mut engines: Vec<&str> = prefs.engine.model_engine().into_iter().collect();
    engines.extend(["piper", "kokoro"]);
    engines
        .iter()
        .find_map(|engine| installed.pick(engine, prefs, language))
        .or_else(|| {
            engines
                .iter()
                .find_map(|engine| installed.pick(engine, prefs, None))
        })
}

/// Whether a downloaded voice is set to read and is here.
pub(super) async fn usable(app: &AppHandle) -> bool {
    let db = app.state::<crate::startup::AppDb>().0.clone();
    let Ok(settings) = crate::settings::load(&db).await else {
        return false;
    };
    let Some(engine) = settings.read_aloud.engine.model_engine() else {
        return false;
    };
    installed(app).is_some_and(|(_, models)| {
        models
            .iter()
            .any(|(model, _)| model.engine.as_deref() == Some(engine))
    })
}

/// A downloaded voice by its key (`<model>/<voice>`).
pub(super) fn installed_voice(app: &AppHandle, key: &str) -> Option<ModelVoice> {
    let (runtime_dir, models) = installed(app)?;
    let infos: Vec<ModelInfo> = models.iter().map(|(model, _)| model.clone()).collect();
    let candidates = select::candidates(&infos);
    let choice = candidates
        .into_iter()
        .find(|candidate| candidate.key() == key)?;
    model_voice(&runtime_dir, &models, choice)
}

/// What a preview reads, in the voice's language when there's a sentence for it.
pub(super) fn sample_text(language: Option<&str>) -> &'static str {
    match language.map(select::primary).as_deref() {
        Some("de") => "Hallo! So klingen Bücher mit dieser Stimme.",
        Some("fr") => "Bonjour ! Voici comment les livres sonneront avec cette voix.",
        Some("es") => "¡Hola! Así sonarán los libros con esta voz.",
        Some("it") => "Ciao! Ecco come suoneranno i libri con questa voce.",
        Some("pt") => "Olá! É assim que os livros vão soar com esta voz.",
        Some("nl") => "Hallo! Zo klinken boeken met deze stem.",
        Some("pl") => "Cześć! Tak będą brzmieć książki tym głosem.",
        Some("ru") => "Привет! Так будут звучать книги этим голосом.",
        Some("uk") => "Привіт! Так звучатимуть книжки цим голосом.",
        Some("sv") => "Hej! Så här låter böcker med den här rösten.",
        Some("da") => "Hej! Sådan lyder bøger med denne stemme.",
        Some("nb") => "Hei! Slik høres bøker ut med denne stemmen.",
        Some("fi") => "Hei! Tältä kirjat kuulostavat tällä äänellä.",
        Some("cs") => "Ahoj! Takhle budou knihy znít tímto hlasem.",
        Some("sk") => "Ahoj! Takto budú knihy znieť týmto hlasom.",
        Some("sl") => "Živijo! Tako bodo knjige zvenele s tem glasom.",
        Some("hu") => "Szia! Így szólnak majd a könyvek ezzel a hanggal.",
        Some("ro") => "Bună! Așa vor suna cărțile cu această voce.",
        Some("tr") => "Merhaba! Kitaplar bu sesle böyle duyulacak.",
        Some("el") => "Γεια σας! Έτσι θα ακούγονται τα βιβλία με αυτή τη φωνή.",
        Some("ca") => "Hola! Així sonaran els llibres amb aquesta veu.",
        Some("vi") => "Xin chào! Sách sẽ được đọc bằng giọng này.",
        Some("lv") => "Sveiki! Tā grāmatas skanēs ar šo balsi.",
        Some("fa") => "سلام! کتاب‌ها با این صدا این‌گونه شنیده می‌شوند.",
        Some("hi") => "नमस्ते! इस आवाज़ में किताबें ऐसी सुनाई देंगी।",
        Some("zh") => "你好！用这个声音朗读书籍就是这样的。",
        _ => "Hello! This is how books will sound with this voice.",
    }
}

/// A voice kept open between sentences.
struct Opened {
    model_id: String,
    synthesizer: Arc<Mutex<Synthesizer>>,
    used: Instant,
}

#[derive(Default)]
pub(super) struct Voices {
    /// At most [`OPEN_AT_ONCE`].
    open: Arc<Mutex<Vec<Opened>>>,
    /// Speech made lately, to be played again without making it again.
    kept: Arc<Mutex<Kept>>,
    /// How long each voice model took to make a second of speech here, lately (by model).
    pace: Mutex<std::collections::HashMap<String, f64>>,
}

/// Seconds of speech a character reads as, about, at speed 1 (English: about 14 a second).
const SPEECH_PER_CHARACTER: f64 = 0.07;
/// A voice's pace before it's known: half a second for a second of speech.
const FIRST_PACE: f64 = 0.5;

/// How long the next piece may be (characters), with `lead` of speech waiting to be heard and the
/// voice taking `pace` seconds to make one at speed `rate`: what it can make before the speech
/// waiting runs out, within [`SHORT_PIECE`] and [`PIECE`]; short with little waiting.
fn piece_limit(lead: Duration, pace: f64, rate: f64) -> usize {
    if lead < SHORT_LEAD {
        return SHORT_PIECE;
    }
    let seconds_per_character = SPEECH_PER_CHARACTER / rate.max(0.25);
    let affordable = lead.as_secs_f64() * 0.8 / (pace.max(0.01) * seconds_per_character);
    (affordable as usize).clamp(SHORT_PIECE, PIECE)
}

/// Speech made lately, by what was read and how ([`speech_key`]), the oldest used going first
/// past [`KEPT_BYTES`].
#[derive(Default)]
struct Kept {
    clips: VecDeque<(String, Clip)>,
    bytes: usize,
}

impl Kept {
    fn get(&mut self, key: &str) -> Option<Clip> {
        let at = self.clips.iter().position(|(kept, _)| kept == key)?;
        let entry = self.clips.remove(at)?;
        let clip = entry.1.clone();
        self.clips.push_back(entry);
        Some(clip)
    }

    fn put(&mut self, key: String, clip: Clip) {
        if clip.pcm.len() > KEPT_BYTES / 4 {
            return;
        }
        if let Some(at) = self.clips.iter().position(|(kept, _)| *kept == key)
            && let Some((_, old)) = self.clips.remove(at)
        {
            self.bytes -= old.pcm.len();
        }
        self.bytes += clip.pcm.len();
        self.clips.push_back((key, clip));
        while self.bytes > KEPT_BYTES {
            let Some((_, old)) = self.clips.pop_front() else {
                break;
            };
            self.bytes -= old.pcm.len();
        }
    }

    fn clear(&mut self) {
        self.clips.clear();
        self.bytes = 0;
    }
}

/// What tells a piece of speech from another: the voice, how fast and high it reads, the words.
fn speech_key(voice: &ModelVoice, rate: f64, pitch: f64, text: &str) -> String {
    format!("{}\u{1f}{rate:.2}\u{1f}{pitch:.2}\u{1f}{text}", voice.key())
}

/// Whether sentence `index` waits before it's made, with sentence `playing` being heard and
/// `lead` of speech waiting after what plays: the next one is always made, others while less
/// than [`LEAD`] waits, up to [`MOST_AHEAD`].
fn waits(index: usize, playing: usize, lead: Duration) -> bool {
    index > playing + 1 && (lead >= LEAD || index > playing + MOST_AHEAD)
}

/// Lets the voice used longest ago go until fewer than `room` are open.
fn make_room(open: &mut Vec<Opened>, room: usize) {
    while open.len() >= room {
        let Some(oldest) = open
            .iter()
            .enumerate()
            .min_by_key(|(_, opened)| opened.used)
            .map(|(index, _)| index)
        else {
            return;
        };
        open.remove(oldest);
    }
}

impl Voices {
    /// The voice's synthesizer, opened now unless it's kept open.
    async fn synthesizer(&self, voice: &ModelVoice) -> Result<Arc<Mutex<Synthesizer>>, String> {
        {
            let mut open = self.open.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(opened) = open
                .iter_mut()
                .find(|opened| opened.model_id == voice.model.id)
            {
                opened.used = Instant::now();
                return Ok(opened.synthesizer.clone());
            }
            // Another voice's memory goes before this one's is taken.
            make_room(&mut open, OPEN_AT_ONCE);
        }
        let voice = voice.clone();
        let synthesizer = tauri::async_runtime::spawn_blocking(move || {
            let runtime = Runtime::load(&voice.runtime_dir).map_err(|error| error.to_string())?;
            let setup = voice.model.tts.clone().unwrap_or_default();
            let started = Instant::now();
            let threads = raygun_ai::tts::threads();
            let opened = Synthesizer::open(runtime, voice.engine, &voice.dir, &setup, threads);
            tracing::info!(model = %voice.model.id, threads, took = ?started.elapsed(), "voice opened");
            opened
                .map(|synthesizer| (voice.model.id, synthesizer))
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| error.to_string())??;
        let (model_id, synthesizer) = synthesizer;
        let synthesizer = Arc::new(Mutex::new(synthesizer));
        let mut open = self.open.lock().unwrap_or_else(|p| p.into_inner());
        open.retain(|opened| opened.model_id != model_id);
        make_room(&mut open, OPEN_AT_ONCE);
        open.push(Opened {
            model_id,
            synthesizer: synthesizer.clone(),
            used: Instant::now(),
        });
        Ok(synthesizer)
    }

    /// `text` read by `voice` at `rate` and `pitch` (1 = as made), as a clip: made now, unless
    /// it was lately.
    pub(super) async fn speak(
        &self,
        voice: &ModelVoice,
        text: &str,
        rate: f64,
        pitch: f64,
    ) -> Result<Clip, String> {
        let key = speech_key(voice, rate, pitch, text);
        if let Some(clip) = self
            .kept
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&key)
        {
            return Ok(clip);
        }
        let synthesizer = self.synthesizer(voice).await?;
        let (words, speaker, lang) = (
            text.to_owned(),
            voice.voice.speaker,
            voice.voice.lang.clone(),
        );
        let (speed, high) = (rate as f32, pitch as f32);
        let (clip, took) = tauri::async_runtime::spawn_blocking(move || {
            let synthesizer = synthesizer.lock().unwrap_or_else(|p| p.into_inner());
            let started = Instant::now();
            let speech = synthesizer
                .generate(
                    &words,
                    speaker,
                    pcm::speed_for(speed, high),
                    lang.as_deref(),
                )
                .map_err(|error| error.to_string())?;
            let took = started.elapsed();
            let clip = Clip::of(&pcm::resample(&speech.samples, high), speech.sample_rate);
            tracing::debug!(
                characters = words.chars().count(),
                ?took,
                length = ?clip.length,
                "speech made"
            );
            Ok::<_, String>((clip, took))
        })
        .await
        .map_err(|error| error.to_string())??;
        if !clip.length.is_zero() {
            let made = took.as_secs_f64() / clip.length.as_secs_f64();
            let mut pace = self.pace.lock().unwrap_or_else(|p| p.into_inner());
            let lately = pace.entry(voice.model.id.clone()).or_insert(made);
            *lately = *lately * 0.7 + made * 0.3;
        }
        self.kept
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .put(key, clip.clone());
        Ok(clip)
    }

    /// How long `voice` lately took to make a second of speech here.
    fn pace(&self, voice: &ModelVoice) -> f64 {
        self.pace
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&voice.model.id)
            .copied()
            .unwrap_or(FIRST_PACE)
    }

    /// The next piece of `rest` (what's left of a sentence) read by `voice`, and how much of
    /// `rest` it read (bytes): all of it when it was made lately or fits, else up to a pause
    /// within what `voice` can make before the `lead` waiting to be heard runs out
    /// ([`piece_limit`]).
    async fn piece(
        &self,
        voice: &ModelVoice,
        rest: &str,
        lead: Duration,
        rate: f64,
        pitch: f64,
    ) -> Result<(Clip, usize), String> {
        let whole = speech_key(voice, rate, pitch, rest);
        if let Some(clip) = self
            .kept
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&whole)
        {
            return Ok((clip, rest.len()));
        }
        let limit = piece_limit(lead, self.pace(voice), rate);
        let end = sentences::piece_end(rest, limit);
        let clip = self.speak(voice, &rest[..end], rate, pitch).await?;
        Ok((clip, end))
    }

    /// Lets the open voices (and the speech kept) go once nothing has been read for a while.
    pub(super) fn release_later(&self) {
        let (open, kept) = (self.open.clone(), self.kept.clone());
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(KEEP_OPEN).await;
            let mut open = open.lock().unwrap_or_else(|p| p.into_inner());
            open.retain(|opened| opened.used.elapsed() < KEEP_OPEN);
            if open.is_empty() {
                kept.lock().unwrap_or_else(|p| p.into_inner()).clear();
            }
        });
    }
}

/// A short silence, for a sentence a voice couldn't read (playing goes on past it).
fn silence(sample_rate: u32) -> Clip {
    Clip::of(&vec![0.0f32; (sample_rate / 10) as usize], sample_rate)
}

/// A run's synthesis: stopped with the run, told which sentence plays.
pub(super) struct VoiceWorker {
    cancel: CancellationToken,
    playing: watch::Sender<usize>,
}

/// What the synthesis task holds of its worker.
pub(super) struct WorkerHandle {
    cancel: CancellationToken,
    playing: watch::Receiver<usize>,
}

impl VoiceWorker {
    pub(super) fn new(at: usize) -> Self {
        Self {
            cancel: CancellationToken::new(),
            playing: watch::channel(at).0,
        }
    }

    pub(super) fn handle(&self) -> WorkerHandle {
        WorkerHandle {
            cancel: self.cancel.clone(),
            playing: self.playing.subscribe(),
        }
    }

    pub(super) fn played(&self, index: usize) {
        self.playing.send_replace(index);
    }

    pub(super) fn cancel(&self) {
        self.cancel.cancel();
    }
}

impl ReadAloud {
    /// Makes run `run`'s sentences with `voice` (passages in other languages with theirs) ahead
    /// of the one playing (until [`LEAD`] of speech waits, [`waits`]), long ones in pieces, and
    /// hands each piece to the player as it's made. A voice that can't read the first one gives
    /// way to the device's own voices.
    pub(super) async fn synthesize(
        self: &Arc<Self>,
        run: u64,
        voice: Box<ModelVoice>,
        plan: Plan,
        mut worker: WorkerHandle,
    ) {
        let mut sample_rate = 0;
        // When what the player was given will have been heard (it plays clips as they come).
        let mut heard_by = Instant::now();
        let lead = |heard_by: Instant| heard_by.saturating_duration_since(Instant::now());
        for (offset, item) in plan.items.iter().enumerate() {
            let index = plan.first + offset;
            loop {
                let playing = *worker.playing.borrow();
                if !waits(index, playing, lead(heard_by)) {
                    break;
                }
                // Waiting ends when another sentence starts being heard, or (when it's only
                // that enough waits) once less does.
                let until = (index <= playing + MOST_AHEAD)
                    .then(|| lead(heard_by).saturating_sub(LEAD) + Duration::from_millis(250));
                let lead_runs_down = async {
                    match until {
                        Some(until) => tokio::time::sleep(until).await,
                        None => std::future::pending().await,
                    }
                };
                tokio::select! {
                    () = worker.cancel.cancelled() => return,
                    changed = worker.playing.changed() => if changed.is_err() { return },
                    () = lead_runs_down => {}
                }
            }
            if worker.cancel.is_cancelled() {
                return;
            }
            let speaker = item
                .language
                .as_ref()
                .and_then(|language| plan.passages.get(language))
                .unwrap_or(&voice);
            let mut rest = item.spoken.trim();
            // Every sentence hands the player something (the chapter's last ends the reading).
            let mut handed = false;
            while !rest.is_empty() || !handed {
                handed = true;
                let made = self
                    .voices
                    .piece(speaker, rest, lead(heard_by), plan.rate, plan.pitch)
                    .await;
                let clip = match made {
                    Ok((clip, read)) => {
                        rest = rest[read..].trim_start();
                        clip
                    }
                    Err(error) if sample_rate == 0 => {
                        tracing::warn!(%error, model = %speaker.model.id, "the downloaded voice couldn't read");
                        self.fall_back(run).await;
                        return;
                    }
                    // A sentence it can't read (symbols only, say) is passed over.
                    Err(_) => {
                        rest = "";
                        silence(sample_rate)
                    }
                };
                sample_rate = clip.sample_rate;
                if worker.cancel.is_cancelled() {
                    return;
                }
                heard_by = heard_by.max(Instant::now()) + clip.length;
                let last = index == plan.last && rest.is_empty();
                let id = session::item_id(run, index, last);
                if let Err(error) = super::send_clip(&self.app, &id, clip, last).await {
                    tracing::warn!(%error, "the player didn't take the speech");
                    self.fail(run, TtsNotice::EngineFailed).await;
                    return;
                }
            }
        }
    }

    /// Run `run`'s downloaded voice couldn't read: the device's voices read on from the same
    /// sentence, and the reader says why.
    async fn fall_back(self: &Arc<Self>, run: u64) {
        let at = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.run != run {
                return;
            }
            let Some(session) = state.session.as_mut() else {
                return;
            };
            session.reader = Reader::System;
            session.notice = Some(TtsNotice::VoiceFailed);
            session.at
        };
        super::clips(&self.app, super::ClipCommand::Stop).await;
        // The device's voices read on (directly: a new run would start this synthesis again).
        if let Some((run, plan, _)) = self.begin_run(at, 0) {
            self.emit_now(raygun_model::TtsState::Speaking);
            self.speak_system(run, plan).await;
        }
    }
}

/// Which engine a downloaded model belongs to, as settings name it.
pub(super) fn engine_of(model: &ModelInfo) -> ReadAloudEngine {
    match model.engine.as_deref() {
        Some("kokoro") => ReadAloudEngine::Kokoro,
        _ => ReadAloudEngine::Piper,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(bytes: usize) -> Clip {
        Clip {
            sample_rate: 24_000,
            pcm: "a".repeat(bytes),
            length: Duration::from_secs(1),
        }
    }

    #[test]
    fn makes_the_next_sentence_then_more_while_little_waits() {
        let little = Duration::from_secs(5);
        assert!(!waits(4, 4, LEAD * 2), "the one playing");
        assert!(!waits(5, 4, LEAD * 2), "the next is always made");
        assert!(!waits(6, 4, little));
        assert!(waits(6, 4, LEAD), "enough waits to be heard");
        assert!(!waits(4 + MOST_AHEAD, 4, little));
        assert!(
            waits(5 + MOST_AHEAD, 4, Duration::ZERO),
            "never too far ahead"
        );
    }

    #[test]
    fn pieces_are_what_the_voice_makes_before_the_waiting_speech_runs_out() {
        // Nothing (or little) waits: a short piece, so reading starts soon.
        assert_eq!(piece_limit(Duration::ZERO, 0.1, 1.0), SHORT_PIECE);
        assert_eq!(piece_limit(Duration::from_secs(2), 0.1, 1.0), SHORT_PIECE);
        // A fast voice with time to spare: the longest piece.
        assert_eq!(piece_limit(Duration::from_secs(10), 0.2, 1.0), PIECE);
        // A phone making speech at about the pace it's heard: what fits in the time left.
        assert_eq!(piece_limit(Duration::from_secs(8), 1.0, 1.0), 91);
        assert_eq!(piece_limit(Duration::from_secs(4), 1.0, 1.0), SHORT_PIECE);
        // Faster reading: less speech to make for each character, so more fit.
        assert!(piece_limit(Duration::from_secs(8), 1.0, 2.0) > 91);
    }

    #[test]
    fn keeps_speech_lately_made_within_its_room() {
        let mut kept = Kept::default();
        kept.put("a".into(), clip(KEPT_BYTES / 4));
        kept.put("b".into(), clip(KEPT_BYTES / 4));
        assert!(kept.get("a").is_some());
        for key in ["c", "d", "e"] {
            kept.put(key.into(), clip(KEPT_BYTES / 4));
        }
        // "b" was used longest ago: it went to make room.
        assert!(kept.get("b").is_none());
        assert!(kept.get("a").is_some());
        assert!(kept.bytes <= KEPT_BYTES);
        // The same words again replace what was kept of them.
        kept.put("a".into(), clip(10));
        assert_eq!(kept.clips.iter().filter(|(key, _)| key == "a").count(), 1);
        // Too large a clip to be worth keeping.
        kept.put("huge".into(), clip(KEPT_BYTES / 2));
        assert!(kept.get("huge").is_none());
        kept.clear();
        assert_eq!((kept.clips.len(), kept.bytes), (0, 0));
    }

    #[test]
    fn clips_know_how_long_they_play() {
        let clip = Clip::of(&vec![0.0; 36_000], 24_000);
        assert_eq!(clip.length, Duration::from_millis(1500));
        assert_eq!(clip.pcm.len(), 96_000, "72 000 bytes as base64");
    }
}
