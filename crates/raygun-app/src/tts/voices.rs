//! Downloaded voices (Piper, Kokoro): which one reads a book, the voice kept open between
//! sentences (and let go when reading stops for a while), and the synthesis that runs a few
//! sentences ahead of what plays.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use raygun_ai::models::{ModelInfo, VoiceInfo};
use raygun_ai::tts::{Engine, Runtime, Synthesizer, pcm, select};
use raygun_model::{ReadAloudEngine, ReadAloudPrefs, TtsNotice};
use tauri::{AppHandle, Manager};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::{Plan, ReadAloud, Reader, session};
use crate::ai::AiService;

/// How many sentences are made ahead of the one playing.
const AHEAD: usize = 2;
/// How long an open voice stays open with nothing read, before its memory is given back.
const KEEP_OPEN: Duration = Duration::from_secs(180);

/// A downloaded voice that reads a book.
#[derive(Clone)]
pub struct ModelVoice {
    pub model: ModelInfo,
    pub dir: PathBuf,
    pub voice: VoiceInfo,
    pub engine: Engine,
    pub runtime_dir: PathBuf,
}

/// Speech made for the player: 16-bit mono PCM as base64.
pub struct Clip {
    pub sample_rate: u32,
    pub pcm: String,
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

/// What reads a book in `language`: the downloaded voice settings choose, else the device's own
/// voices (with why, when downloaded ones were chosen).
pub(super) fn choose(
    app: &AppHandle,
    prefs: &ReadAloudPrefs,
    language: Option<&str>,
) -> (Reader, Option<TtsNotice>) {
    let Some(engine) = prefs.engine.model_engine() else {
        return (Reader::System, None);
    };
    let no_voice = || {
        (
            Reader::System,
            Some(TtsNotice::NoVoice {
                language: language.map(str::to_owned),
            }),
        )
    };
    let Some((runtime_dir, models)) = installed(app) else {
        return no_voice();
    };
    let infos: Vec<ModelInfo> = models.iter().map(|(model, _)| model.clone()).collect();
    let candidates = select::candidates(&infos);
    let chosen = language.and_then(|language| prefs.voices.get(&select::primary(language)));
    let picked = select::pick(
        engine,
        &candidates,
        language,
        chosen.map(String::as_str),
        None,
    );
    match picked.and_then(|choice| model_voice(&runtime_dir, &models, choice)) {
        Some(voice) => (Reader::Model(Box::new(voice)), None),
        None => no_voice(),
    }
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

/// The voice kept open between sentences.
struct Opened {
    model_id: String,
    synthesizer: Arc<Mutex<Synthesizer>>,
    used: Instant,
}

#[derive(Default)]
pub(super) struct Voices {
    open: Arc<Mutex<Option<Opened>>>,
}

impl Voices {
    /// The voice's synthesizer, opened now unless it's the one kept open.
    async fn synthesizer(&self, voice: &ModelVoice) -> Result<Arc<Mutex<Synthesizer>>, String> {
        {
            let mut open = self.open.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(opened) = open.as_mut()
                && opened.model_id == voice.model.id
            {
                opened.used = Instant::now();
                return Ok(opened.synthesizer.clone());
            }
            // Another voice's memory goes before this one's is taken.
            *open = None;
        }
        let voice = voice.clone();
        let synthesizer = tauri::async_runtime::spawn_blocking(move || {
            let runtime = Runtime::load(&voice.runtime_dir).map_err(|error| error.to_string())?;
            let setup = voice.model.tts.clone().unwrap_or_default();
            let threads =
                std::thread::available_parallelism().map_or(2, |count| count.get().min(4));
            Synthesizer::open(runtime, voice.engine, &voice.dir, &setup, threads as u32)
                .map(|synthesizer| (voice.model.id, synthesizer))
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| error.to_string())??;
        let (model_id, synthesizer) = synthesizer;
        let synthesizer = Arc::new(Mutex::new(synthesizer));
        *self.open.lock().unwrap_or_else(|p| p.into_inner()) = Some(Opened {
            model_id,
            synthesizer: synthesizer.clone(),
            used: Instant::now(),
        });
        Ok(synthesizer)
    }

    /// `text` read by `voice` at `rate` and `pitch` (1 = as made), as a clip.
    pub(super) async fn speak(
        &self,
        voice: &ModelVoice,
        text: &str,
        rate: f64,
        pitch: f64,
    ) -> Result<Clip, String> {
        let synthesizer = self.synthesizer(voice).await?;
        let (text, speaker, lang) = (
            text.to_owned(),
            voice.voice.speaker,
            voice.voice.lang.clone(),
        );
        let (rate, pitch) = (rate as f32, pitch as f32);
        tauri::async_runtime::spawn_blocking(move || {
            let synthesizer = synthesizer.lock().unwrap_or_else(|p| p.into_inner());
            let speech = synthesizer
                .generate(&text, speaker, pcm::speed_for(rate, pitch), lang.as_deref())
                .map_err(|error| error.to_string())?;
            let samples = pcm::resample(&speech.samples, pitch);
            Ok(Clip {
                sample_rate: speech.sample_rate,
                pcm: base64::engine::general_purpose::STANDARD.encode(pcm::to_pcm16(&samples)),
            })
        })
        .await
        .map_err(|error| error.to_string())?
    }

    /// Lets the open voice go once nothing has been read with it for a while.
    pub(super) fn release_later(&self) {
        let open = self.open.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(KEEP_OPEN).await;
            let mut open = open.lock().unwrap_or_else(|p| p.into_inner());
            if open
                .as_ref()
                .is_some_and(|opened| opened.used.elapsed() >= KEEP_OPEN)
            {
                *open = None;
            }
        });
    }
}

/// A short silence, for a sentence a voice couldn't read (playing goes on past it).
fn silence(sample_rate: u32) -> Clip {
    let samples = vec![0.0f32; (sample_rate / 10) as usize];
    Clip {
        sample_rate,
        pcm: base64::engine::general_purpose::STANDARD.encode(pcm::to_pcm16(&samples)),
    }
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
    /// Makes run `run`'s sentences with `voice`, at most [`AHEAD`] ahead of the one playing,
    /// and hands each to the player. A voice that can't read the first one gives way to the
    /// device's own voices.
    pub(super) async fn synthesize(
        self: &Arc<Self>,
        run: u64,
        voice: Box<ModelVoice>,
        plan: Plan,
        mut worker: WorkerHandle,
    ) {
        let mut sample_rate = 0;
        for (offset, item) in plan.items.iter().enumerate() {
            let index = plan.first + offset;
            while index > *worker.playing.borrow() + AHEAD {
                tokio::select! {
                    () = worker.cancel.cancelled() => return,
                    changed = worker.playing.changed() => if changed.is_err() { return },
                }
            }
            if worker.cancel.is_cancelled() {
                return;
            }
            let made = self
                .voices
                .speak(&voice, &item.text, plan.rate, plan.pitch)
                .await;
            let clip = match made {
                Ok(clip) => clip,
                Err(error) if sample_rate == 0 => {
                    tracing::warn!(%error, model = %voice.model.id, "the downloaded voice couldn't read");
                    self.fall_back(run).await;
                    return;
                }
                // A sentence it can't read (symbols only, say) is passed over.
                Err(_) => silence(sample_rate),
            };
            sample_rate = clip.sample_rate;
            if worker.cancel.is_cancelled() {
                return;
            }
            let last = index == plan.last;
            let id = session::item_id(run, index, last);
            if let Err(error) = super::send_clip(&self.app, &id, clip, last).await {
                tracing::warn!(%error, "the player didn't take the speech");
                self.fail(run, TtsNotice::EngineFailed).await;
                return;
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
