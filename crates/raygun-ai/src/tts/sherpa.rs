//! sherpa-onnx's C API (version 1.13.8, the catalogue's runtime), loaded at run time from the
//! runtime's folder: ONNX Runtime first, then sherpa-onnx, which links against it. The structures
//! below mirror `sherpa-onnx/c-api/c-api.h` of that version field for field; another version of
//! the library needs them checked again. Loaded libraries stay loaded until the app ends.

use std::ffi::{CStr, CString, c_char, c_void};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::models::TtsSetup;

#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("the voice engine couldn't be loaded: {0}")]
    Load(String),
    #[error("the voice couldn't be opened")]
    Open,
    #[error("the voice made no sound for this text")]
    Silent,
}

/// What reads the words: Piper's VITS models, or Kokoro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Piper,
    Kokoro,
}

impl Engine {
    pub fn of(name: &str) -> Option<Self> {
        match name {
            "piper" => Some(Self::Piper),
            "kokoro" => Some(Self::Kokoro),
            _ => None,
        }
    }
}

// ---- c-api.h, v1.13.8 ----

#[repr(C)]
struct VitsConfig {
    model: *const c_char,
    lexicon: *const c_char,
    tokens: *const c_char,
    data_dir: *const c_char,
    noise_scale: f32,
    noise_scale_w: f32,
    length_scale: f32,
    dict_dir: *const c_char,
}

#[repr(C)]
struct MatchaConfig {
    acoustic_model: *const c_char,
    vocoder: *const c_char,
    lexicon: *const c_char,
    tokens: *const c_char,
    data_dir: *const c_char,
    noise_scale: f32,
    length_scale: f32,
    dict_dir: *const c_char,
}

#[repr(C)]
struct KokoroConfig {
    model: *const c_char,
    voices: *const c_char,
    tokens: *const c_char,
    data_dir: *const c_char,
    length_scale: f32,
    dict_dir: *const c_char,
    lexicon: *const c_char,
    lang: *const c_char,
}

#[repr(C)]
struct KittenConfig {
    model: *const c_char,
    voices: *const c_char,
    tokens: *const c_char,
    data_dir: *const c_char,
    length_scale: f32,
}

#[repr(C)]
struct ZipvoiceConfig {
    tokens: *const c_char,
    encoder: *const c_char,
    decoder: *const c_char,
    vocoder: *const c_char,
    data_dir: *const c_char,
    lexicon: *const c_char,
    feat_scale: f32,
    t_shift: f32,
    target_rms: f32,
    guidance_scale: f32,
}

#[repr(C)]
struct PocketConfig {
    lm_flow: *const c_char,
    lm_main: *const c_char,
    encoder: *const c_char,
    decoder: *const c_char,
    text_conditioner: *const c_char,
    vocab_json: *const c_char,
    token_scores_json: *const c_char,
    voice_embedding_cache_capacity: i32,
}

#[repr(C)]
struct SupertonicConfig {
    duration_predictor: *const c_char,
    text_encoder: *const c_char,
    vector_estimator: *const c_char,
    vocoder: *const c_char,
    tts_json: *const c_char,
    unicode_indexer: *const c_char,
    voice_style: *const c_char,
}

#[repr(C)]
struct ModelConfig {
    vits: VitsConfig,
    num_threads: i32,
    debug: i32,
    provider: *const c_char,
    matcha: MatchaConfig,
    kokoro: KokoroConfig,
    kitten: KittenConfig,
    zipvoice: ZipvoiceConfig,
    pocket: PocketConfig,
    supertonic: SupertonicConfig,
}

#[repr(C)]
struct TtsConfig {
    model: ModelConfig,
    rule_fsts: *const c_char,
    max_num_sentences: i32,
    rule_fars: *const c_char,
    silence_scale: f32,
}

#[repr(C)]
struct GeneratedAudio {
    samples: *const f32,
    n: i32,
    sample_rate: i32,
}

#[repr(C)]
struct GenerationConfig {
    silence_scale: f32,
    speed: f32,
    sid: i32,
    reference_audio: *const f32,
    reference_audio_len: i32,
    reference_sample_rate: i32,
    reference_text: *const c_char,
    num_steps: i32,
    extra: *const c_char,
}

type ProgressCallback = unsafe extern "C" fn(*const f32, i32, f32, *mut c_void) -> i32;

/// The library's functions this module calls.
struct Api {
    create: unsafe extern "C" fn(*const TtsConfig) -> *const c_void,
    destroy: unsafe extern "C" fn(*const c_void),
    sample_rate: unsafe extern "C" fn(*const c_void) -> i32,
    speakers: unsafe extern "C" fn(*const c_void) -> i32,
    generate: unsafe extern "C" fn(
        *const c_void,
        *const c_char,
        *const GenerationConfig,
        Option<ProgressCallback>,
        *mut c_void,
    ) -> *const GeneratedAudio,
    free_audio: unsafe extern "C" fn(*const GeneratedAudio),
    version: unsafe extern "C" fn() -> *const c_char,
}

/// The loaded runtime: ONNX Runtime and sherpa-onnx, with espeak-ng's data beside them.
pub struct Runtime {
    api: Api,
    data_dir: PathBuf,
    /// Kept so the functions stay valid; never unloaded.
    _libraries: Vec<libloading::Library>,
}

/// Runtimes loaded so far, by folder.
static LOADED: Mutex<Vec<(PathBuf, Arc<Runtime>)>> = Mutex::new(Vec::new());

/// The library files of a runtime folder, in the order they load.
fn library_files() -> [&'static str; 2] {
    if cfg!(windows) {
        ["onnxruntime.dll", "sherpa-onnx-c-api.dll"]
    } else if cfg!(target_os = "macos") {
        ["libonnxruntime.dylib", "libsherpa-onnx-c-api.dylib"]
    } else {
        ["libonnxruntime.so", "libsherpa-onnx-c-api.so"]
    }
}

impl Runtime {
    /// Loads the runtime installed in `dir` (once; later calls get the same).
    pub fn load(dir: &Path) -> Result<Arc<Self>, TtsError> {
        let mut loaded = LOADED
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some((_, runtime)) = loaded.iter().find(|(path, _)| path == dir) {
            return Ok(runtime.clone());
        }
        let data_dir = dir.join("espeak-ng-data");
        if !data_dir.join("phontab").is_file() {
            return Err(TtsError::Load("espeak-ng-data is missing".into()));
        }
        let mut libraries = Vec::new();
        for name in library_files() {
            let path = dir.join(name);
            if !path.is_file() {
                return Err(TtsError::Load(format!("{name} is missing")));
            }
            read_only(&path);
            libraries.push(open_library(&path)?);
        }
        let api = {
            let library = &libraries[1];
            Api {
                create: symbol(library, b"SherpaOnnxCreateOfflineTts\0")?,
                destroy: symbol(library, b"SherpaOnnxDestroyOfflineTts\0")?,
                sample_rate: symbol(library, b"SherpaOnnxOfflineTtsSampleRate\0")?,
                speakers: symbol(library, b"SherpaOnnxOfflineTtsNumSpeakers\0")?,
                generate: symbol(library, b"SherpaOnnxOfflineTtsGenerateWithConfig\0")?,
                free_audio: symbol(library, b"SherpaOnnxDestroyOfflineTtsGeneratedAudio\0")?,
                version: symbol(library, b"SherpaOnnxGetVersionStr\0")?,
            }
        };
        let runtime = Arc::new(Self {
            api,
            data_dir,
            _libraries: libraries,
        });
        tracing::info!(version = %runtime.version(), "voice engine loaded");
        loaded.push((dir.to_owned(), runtime.clone()));
        Ok(runtime)
    }

    /// sherpa-onnx's version, as the library says.
    pub fn version(&self) -> String {
        // SAFETY: the library returns a static NUL-terminated string.
        unsafe { CStr::from_ptr((self.api.version)()) }
            .to_string_lossy()
            .into_owned()
    }
}

/// Files the runtime loads are made read-only first (Android expects code it loads not to be
/// writable).
fn read_only(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o444));
    }
    #[cfg(not(unix))]
    let _ = path;
}

fn open_library(path: &Path) -> Result<libloading::Library, TtsError> {
    let failed = |error: libloading::Error| TtsError::Load(format!("{}: {error}", path.display()));
    // SAFETY: the runtime's libraries were checked against the catalogue's checksums when they
    // were downloaded; their initializers are ONNX Runtime's and sherpa-onnx's own.
    #[cfg(windows)]
    {
        use libloading::os::windows::{LOAD_WITH_ALTERED_SEARCH_PATH, Library};
        // The folder's own copy of a library it needs is found first (Windows has an
        // `onnxruntime.dll` of its own).
        unsafe { Library::load_with_flags(path, LOAD_WITH_ALTERED_SEARCH_PATH) }
            .map(Into::into)
            .map_err(failed)
    }
    #[cfg(not(windows))]
    unsafe { libloading::Library::new(path) }.map_err(failed)
}

fn symbol<T: Copy>(library: &libloading::Library, name: &[u8]) -> Result<T, TtsError> {
    // SAFETY: each name is a function of c-api.h whose signature `T` mirrors.
    unsafe { library.get::<T>(name) }
        .map(|symbol| *symbol)
        .map_err(|error| {
            TtsError::Load(format!(
                "{} is missing: {error}",
                String::from_utf8_lossy(&name[..name.len() - 1])
            ))
        })
}

/// Speech made by a voice: samples (−1…1) at `sample_rate`.
#[derive(Debug, Clone)]
pub struct Speech {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// One opened voice model. Calls are made one at a time (the app keeps it behind a lock).
pub struct Synthesizer {
    runtime: Arc<Runtime>,
    tts: *const c_void,
    pub engine: Engine,
    pub sample_rate: u32,
    pub speakers: u32,
}

// SAFETY: sherpa-onnx's TTS object isn't tied to the thread that made it; the app uses it from
// one thread at a time.
unsafe impl Send for Synthesizer {}

/// A path as the C API takes it.
fn c_path(path: &Path) -> CString {
    c_text(&path.to_string_lossy())
}

/// Text without the NULs a C string can't hold.
fn c_text(text: &str) -> CString {
    CString::new(text.replace('\0', " ")).unwrap_or_default()
}

impl Synthesizer {
    /// Opens the voice model in `dir`, set up as `setup` says, on `threads` threads.
    pub fn open(
        runtime: Arc<Runtime>,
        engine: Engine,
        dir: &Path,
        setup: &TtsSetup,
        threads: u32,
    ) -> Result<Self, TtsError> {
        let model = c_path(&dir.join(&setup.model));
        let tokens = c_path(&dir.join(&setup.tokens));
        let data_dir = c_path(&runtime.data_dir);
        let voices = setup
            .voices_file
            .as_ref()
            .map(|name| c_path(&dir.join(name)));
        let lexicon = c_text(
            &setup
                .lexicon
                .iter()
                .map(|name| dir.join(name).to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(","),
        );
        // SAFETY: an all-zero config is the C API's "everything default" (NULL strings read as
        // empty, zero numbers as their defaults).
        let mut config: TtsConfig = unsafe { std::mem::zeroed() };
        config.model.num_threads = threads.clamp(1, 8) as i32;
        match engine {
            Engine::Piper => {
                config.model.vits.model = model.as_ptr();
                config.model.vits.tokens = tokens.as_ptr();
                config.model.vits.data_dir = data_dir.as_ptr();
            }
            Engine::Kokoro => {
                let Some(voices) = &voices else {
                    return Err(TtsError::Open);
                };
                config.model.kokoro.model = model.as_ptr();
                config.model.kokoro.voices = voices.as_ptr();
                config.model.kokoro.tokens = tokens.as_ptr();
                config.model.kokoro.data_dir = data_dir.as_ptr();
                config.model.kokoro.lexicon = lexicon.as_ptr();
                config.model.kokoro.length_scale = 1.0;
            }
        }
        // SAFETY: `config` and every string it points to live until the call returns; the
        // library copies what it keeps.
        let tts = unsafe { (runtime.api.create)(&config) };
        if tts.is_null() {
            return Err(TtsError::Open);
        }
        // SAFETY: `tts` is the live object just made.
        let (sample_rate, speakers) =
            unsafe { ((runtime.api.sample_rate)(tts), (runtime.api.speakers)(tts)) };
        Ok(Self {
            runtime,
            tts,
            engine,
            sample_rate: sample_rate.max(1) as u32,
            speakers: speakers.max(1) as u32,
        })
    }

    /// Reads `text` aloud with speaker `speaker` at `speed` (1 = as made). `lang` is Kokoro's
    /// espeak-ng language for voices its pronunciation lists don't cover.
    pub fn generate(
        &self,
        text: &str,
        speaker: u32,
        speed: f32,
        lang: Option<&str>,
    ) -> Result<Speech, TtsError> {
        let text = c_text(text);
        let extra = lang
            .filter(|_| self.engine == Engine::Kokoro)
            .map(|lang| c_text(&serde_json::json!({ "lang": lang }).to_string()));
        // SAFETY: as in `open`, zero is the default for every field set to nothing here.
        let mut config: GenerationConfig = unsafe { std::mem::zeroed() };
        config.silence_scale = 0.2;
        config.speed = speed.clamp(0.25, 4.0);
        config.sid = speaker.min(self.speakers.saturating_sub(1)) as i32;
        config.extra = extra
            .as_ref()
            .map_or(std::ptr::null(), |extra| extra.as_ptr());
        // SAFETY: `self.tts` is live, the strings outlive the call, and no callback is passed.
        let audio = unsafe {
            (self.runtime.api.generate)(
                self.tts,
                text.as_ptr(),
                &config,
                None,
                std::ptr::null_mut(),
            )
        };
        if audio.is_null() {
            return Err(TtsError::Silent);
        }
        // SAFETY: the library's audio holds `n` samples until it's freed just below.
        let speech = unsafe {
            let audio = &*audio;
            let samples = if audio.samples.is_null() || audio.n <= 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(audio.samples, audio.n as usize).to_vec()
            };
            Speech {
                samples,
                sample_rate: audio.sample_rate.max(1) as u32,
            }
        };
        // SAFETY: freed once, with the library's own function.
        unsafe { (self.runtime.api.free_audio)(audio) };
        if speech.samples.is_empty() {
            return Err(TtsError::Silent);
        }
        Ok(speech)
    }
}

impl Drop for Synthesizer {
    fn drop(&mut self) {
        // SAFETY: made by `create`, destroyed once.
        unsafe { (self.runtime.api.destroy)(self.tts) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirrors_the_c_structures() {
        // Sizes on 64-bit targets, as c-api.h v1.13.8 lays them out (pointers 8 bytes, floats
        // and ints 4, each pointer aligned to 8).
        if std::mem::size_of::<usize>() == 8 {
            assert_eq!(std::mem::size_of::<VitsConfig>(), 56);
            assert_eq!(std::mem::size_of::<KokoroConfig>(), 64);
            assert_eq!(std::mem::size_of::<GenerationConfig>(), 56);
            assert_eq!(std::mem::size_of::<GeneratedAudio>(), 16);
        }
    }

    #[test]
    fn a_folder_without_the_runtime_is_refused() {
        let dir = std::env::temp_dir().join("raygun-no-voice-engine");
        let error = Runtime::load(&dir).err().expect("nothing to load");
        assert!(matches!(error, TtsError::Load(_)), "{error}");
    }

    /// Reads a sentence with a real runtime and voices: `SHERPA_RUNTIME` (a runtime folder:
    /// the libraries and `espeak-ng-data`), `PIPER_MODEL` (a folder with `model.onnx` and
    /// `tokens.txt`) and/or `KOKORO_MODEL` (Kokoro's files; `KOKORO_FILE` names its model,
    /// `model.onnx` unless set); `TTS_OUT` keeps WAV files to hear.
    /// `cargo test -p raygun-ai --release -- --ignored speaks_live --nocapture`
    #[test]
    #[ignore = "needs a downloaded voice engine and voices"]
    fn speaks_live() {
        let runtime_dir = std::env::var("SHERPA_RUNTIME").expect("SHERPA_RUNTIME");
        let runtime = Runtime::load(Path::new(&runtime_dir)).expect("runtime");
        println!("sherpa-onnx {}", runtime.version());
        let threads = std::thread::available_parallelism().map_or(2, |count| count.get() as u32);
        let mut cases = Vec::new();
        if let Ok(dir) = std::env::var("PIPER_MODEL") {
            let setup = TtsSetup {
                model: "model.onnx".into(),
                tokens: "tokens.txt".into(),
                ..TtsSetup::default()
            };
            cases.push((
                Engine::Piper,
                dir,
                setup,
                vec![(
                    0,
                    None,
                    "It was a dark and stormy night; the rain fell in torrents.",
                )],
            ));
        }
        if let Ok(dir) = std::env::var("KOKORO_MODEL") {
            let setup = TtsSetup {
                model: std::env::var("KOKORO_FILE").unwrap_or_else(|_| "model.onnx".into()),
                tokens: "tokens.txt".into(),
                voices_file: Some("voices.bin".into()),
                lexicon: vec!["lexicon-us-en.txt".into(), "lexicon-zh.txt".into()],
                ..TtsSetup::default()
            };
            cases.push((
                Engine::Kokoro,
                dir,
                setup,
                vec![
                    (
                        3,
                        None,
                        "It was a dark and stormy night; the rain fell in torrents.",
                    ),
                    (28, Some("es"), "En un lugar de la Mancha vivía un hidalgo."),
                    (45, None, "今天天气很好，我们去公园散步吧。"),
                ],
            ));
        }
        assert!(!cases.is_empty(), "set PIPER_MODEL and/or KOKORO_MODEL");
        for (engine, dir, setup, lines) in cases {
            let started = std::time::Instant::now();
            let voice =
                Synthesizer::open(runtime.clone(), engine, Path::new(&dir), &setup, threads)
                    .expect("voice");
            println!(
                "{engine:?}: opened in {:?}, {} Hz, {} speakers",
                started.elapsed(),
                voice.sample_rate,
                voice.speakers
            );
            for (speaker, lang, text) in lines {
                let started = std::time::Instant::now();
                let speech = voice.generate(text, speaker, 1.0, lang).expect("speech");
                let seconds = speech.samples.len() as f64 / f64::from(speech.sample_rate);
                let made = started.elapsed().as_secs_f64();
                let loudness = (speech.samples.iter().map(|s| s * s).sum::<f32>()
                    / speech.samples.len() as f32)
                    .sqrt();
                println!(
                    "  {speaker} {lang:?}: {seconds:.2} s of speech in {made:.2} s (RTF {:.2}), RMS {loudness:.3}",
                    made / seconds
                );
                assert!(seconds > 1.0, "{text}");
                assert!(loudness > 0.01, "{text} is silent");
                if let Ok(out) = std::env::var("TTS_OUT") {
                    let path = Path::new(&out).join(format!("{engine:?}-{speaker}.wav"));
                    let _ = std::fs::write(path, wav(&speech));
                }
            }
        }
    }

    /// How fast voices read on this machine, by thread count: each sentence's real-time factor
    /// (time to make it ÷ its length), a long sentence whole and its first piece alone as the app
    /// cuts it when nothing waits to be heard (the wait before reading starts), and how long
    /// opening takes. `SHERPA_RUNTIME` and `PIPER_MODEL` / `KOKORO_MODEL` as for [`speaks_live`];
    /// `TTS_THREADS` lists the counts (the app's choice for this device unless set).
    /// `cargo test -p raygun-ai --release -- --ignored speed_live --nocapture`
    #[test]
    #[ignore = "needs a downloaded voice engine and voices"]
    fn speed_live() {
        const SENTENCES: [&str; 4] = [
            "Letter one.",
            "I am already far north of London, and as I walk in the streets of Petersburgh, I \
             feel a cold northern breeze play upon my cheeks.",
            "Do you understand this feeling?",
            "There, Margaret, the sun is for ever visible, its broad disk just skirting the \
             horizon and diffusing a perpetual splendour; there snow and frost are banished, \
             and, sailing over a calm sea, we may be wafted to a land surpassing in wonders and \
             in beauty every region hitherto discovered on the habitable globe.",
        ];
        let runtime_dir = std::env::var("SHERPA_RUNTIME").expect("SHERPA_RUNTIME");
        let runtime = Runtime::load(Path::new(&runtime_dir)).expect("runtime");
        let threads: Vec<u32> = std::env::var("TTS_THREADS")
            .unwrap_or_else(|_| super::super::threads().to_string())
            .split(',')
            .filter_map(|count| count.trim().parse().ok())
            .collect();
        let mut voices = Vec::new();
        if let Ok(dir) = std::env::var("PIPER_MODEL") {
            let setup = TtsSetup {
                model: "model.onnx".into(),
                tokens: "tokens.txt".into(),
                ..TtsSetup::default()
            };
            voices.push((Engine::Piper, dir, setup, 0));
        }
        if let Ok(dir) = std::env::var("KOKORO_MODEL") {
            let model = std::env::var("KOKORO_FILE").unwrap_or_else(|_| "model.onnx".into());
            let lexicon = if Path::new(&dir).join("lexicon-us-en.txt").is_file() {
                vec!["lexicon-us-en.txt".into(), "lexicon-zh.txt".into()]
            } else {
                Vec::new()
            };
            let setup = TtsSetup {
                model,
                tokens: "tokens.txt".into(),
                voices_file: Some("voices.bin".into()),
                lexicon,
                ..TtsSetup::default()
            };
            voices.push((Engine::Kokoro, dir, setup, 3));
        }
        assert!(!voices.is_empty(), "set PIPER_MODEL and/or KOKORO_MODEL");
        let long = SENTENCES[3];
        // As `raygun-app`'s `tts/voices.rs` cuts it with nothing waiting (its `SHORT_PIECE`, at
        // a comma too).
        let first_clause = &long[..super::super::sentences::piece_end(long, 60, true)];
        for (engine, dir, setup, speaker) in voices {
            for &count in &threads {
                let started = std::time::Instant::now();
                let voice =
                    Synthesizer::open(runtime.clone(), engine, Path::new(&dir), &setup, count)
                        .expect("voice");
                let opened = started.elapsed().as_secs_f64();
                let first_call = std::time::Instant::now();
                let _ = voice.generate("Hello.", speaker, 1.0, None);
                let warm_up = first_call.elapsed().as_secs_f64();
                let timed = |text: &str| {
                    let started = std::time::Instant::now();
                    let speech = voice.generate(text, speaker, 1.0, None).expect("speech");
                    let seconds = speech.samples.len() as f64 / f64::from(speech.sample_rate);
                    (started.elapsed().as_secs_f64(), seconds)
                };
                let (mut made, mut heard) = (0.0, 0.0);
                let mut each = Vec::new();
                for text in SENTENCES {
                    let (took, seconds) = timed(text);
                    made += took;
                    heard += seconds;
                    each.push(format!("{:.2}", took / seconds));
                }
                let (long_took, long_seconds) = timed(long);
                let (clause_took, clause_seconds) = timed(first_clause);
                println!(
                    "{engine:?} {count} threads: opened {opened:.2} s, first call {warm_up:.2} s, \
                     RTF {:.3} (each {}); the long sentence ({long_seconds:.1} s) takes \
                     {long_took:.2} s, its first piece ({clause_seconds:.1} s) {clause_took:.2} s",
                    made / heard,
                    each.join(" "),
                );
            }
        }
    }

    /// A mono 16-bit WAV of `speech`.
    fn wav(speech: &Speech) -> Vec<u8> {
        let pcm = super::super::pcm::to_pcm16(&speech.samples);
        let mut out = Vec::with_capacity(44 + pcm.len());
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&speech.sample_rate.to_le_bytes());
        out.extend_from_slice(&(speech.sample_rate * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
        out.extend_from_slice(&pcm);
        out
    }
}
