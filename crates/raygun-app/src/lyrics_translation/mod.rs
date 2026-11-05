//! Translations under lyrics (M46.48): what lyrics sources give with their lyrics (KuGou's Chinese
//! translation and romanization), and translations made on request by a model on this device
//! (OPUS-MT through `raygun_ai::marian`) or a translator from Settings → AI. Each is kept with
//! the words of the lines it translates (`raygun_db::lyrics_cache`), so it lines up again with
//! whichever lyrics show: the source's own, another source's or the song's own (`align`).

mod align;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use raygun_ai::marian::{self, Marian, Step};
use raygun_db::lyrics_cache::{self as store, SOURCE_PREFIX, TranslationRow};
use raygun_model::{
    AiProviderKind, Lyrics, LyricsTranslation, LyricsTranslationKind, LyricsTranslationOrigin,
    LyricsTranslationProgress, LyricsTranslatorOption, TrackQuery,
};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

pub use align::{KeptLine, align_onto, kept_lines};

use crate::ai::AiService;
use crate::events::LyricsTranslationEvent;
use crate::startup::AppDb;

/// The translator id of this device's models.
pub const DEVICE: &str = "device";
/// AI translators' ids: this, then the provider's id.
pub const AI_PREFIX: &str = "ai:";
/// Different lines one translation takes at most.
const MAX_LINES: usize = 400;
/// A longer line is cut before it's translated.
const MAX_LINE_CHARS: usize = 500;
/// On-device models stay loaded this long after a translation, for the next song.
const KEEP_LOADED: Duration = Duration::from_secs(120);
/// Progress is reported at most this often.
const PROGRESS_EVERY: Duration = Duration::from_millis(400);

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn kind_name(kind: LyricsTranslationKind) -> &'static str {
    match kind {
        LyricsTranslationKind::Translation => "translation",
        LyricsTranslationKind::Romanization => "romanization",
    }
}

fn kind_of(name: &str) -> Option<LyricsTranslationKind> {
    match name {
        "translation" => Some(LyricsTranslationKind::Translation),
        "romanization" => Some(LyricsTranslationKind::Romanization),
        _ => None,
    }
}

/// Whether a line has words to translate (not "♪" or blank).
pub(crate) fn has_words(line: &str) -> bool {
    line.chars().any(char::is_alphanumeric)
}

/// What the lyrics sources gave with their lyrics, as rows to keep: each source's translations
/// with the words of its own lines.
pub fn source_rows(answers: &[Lyrics], now_ms: i64) -> Vec<TranslationRow> {
    let mut rows: Vec<TranslationRow> = Vec::new();
    for answer in answers {
        for translation in &answer.translations {
            let translator = format!("{SOURCE_PREFIX}{}", answer.provider_name);
            let kind = kind_name(translation.kind);
            let taken = rows.iter().any(|row| {
                row.translator == translator
                    && row.language == translation.language
                    && row.kind == kind
            });
            let lines = kept_lines(answer, &translation.lines);
            if taken || lines.iter().all(|line| line.translation.is_empty()) {
                continue;
            }
            let Ok(lines) = serde_json::to_string(&lines) else {
                continue;
            };
            rows.push(TranslationRow {
                translator,
                language: translation.language.clone(),
                kind: kind.to_owned(),
                by_name: answer.provider_name.clone(),
                lines,
                made_at: now_ms,
            });
        }
    }
    rows
}

/// Every kept translation of a song laid onto `lyrics`: the newest picked or made first, a
/// source's own before another's made at the same time. Translations matching no line are left out.
pub fn lay_onto(lyrics: &Lyrics, rows: &[TranslationRow]) -> Vec<LyricsTranslation> {
    let rank = |origin: LyricsTranslationOrigin| match origin {
        LyricsTranslationOrigin::Source => 0,
        LyricsTranslationOrigin::Ai | LyricsTranslationOrigin::Device => 1,
        LyricsTranslationOrigin::OtherSource => 2,
    };
    let mut laid: Vec<(i64, LyricsTranslation)> = rows
        .iter()
        .filter_map(|row| {
            let kind = kind_of(&row.kind)?;
            let kept: Vec<KeptLine> = serde_json::from_str(&row.lines).ok()?;
            let lines = align_onto(lyrics, &kept);
            if lines.iter().all(String::is_empty) {
                return None;
            }
            let origin = match row.translator.strip_prefix(SOURCE_PREFIX) {
                Some(source) if source == lyrics.provider_name && !lyrics.edited => {
                    LyricsTranslationOrigin::Source
                }
                Some(_) => LyricsTranslationOrigin::OtherSource,
                None if row.translator == DEVICE => LyricsTranslationOrigin::Device,
                None => LyricsTranslationOrigin::Ai,
            };
            Some((
                row.made_at,
                LyricsTranslation {
                    language: row.language.clone(),
                    kind,
                    origin,
                    by: row.by_name.clone(),
                    translator: row.translator.clone(),
                    lines,
                },
            ))
        })
        .collect();
    laid.sort_by_key(|(made_at, translation)| (-made_at, rank(translation.origin)));
    laid.into_iter()
        .map(|(_, translation)| translation)
        .collect()
}

/// Whether `language` looks like a BCP 47 tag.
pub(crate) fn language_tag(language: &str) -> Result<(), String> {
    let well_formed = (2..=35).contains(&language.len())
        && language
            .split('-')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric()));
    if well_formed {
        Ok(())
    } else {
        Err(format!("\"{language}\" isn't a language"))
    }
}

/// Each different line with words once, in order, cut to a translatable length; and where every
/// line of the lyrics finds its translation in that list.
fn distinct_lines(lines: &[&str]) -> (Vec<String>, Vec<Option<usize>>) {
    let mut distinct: Vec<String> = Vec::new();
    let mut index_of: HashMap<&str, usize> = HashMap::new();
    let places = lines
        .iter()
        .map(|line| {
            let line = line.trim();
            if !has_words(line) {
                return None;
            }
            if let Some(index) = index_of.get(line) {
                return Some(*index);
            }
            if distinct.len() >= MAX_LINES {
                return None;
            }
            index_of.insert(line, distinct.len());
            distinct.push(line.chars().take(MAX_LINE_CHARS).collect());
            Some(distinct.len() - 1)
        })
        .collect();
    (distinct, places)
}

/// The models a line goes through: straight from English when the song is English and the line
/// has no other letters, else through English.
fn route<'a>(plan: &'a marian::Plan, english: bool, line: &str) -> &'a [Step] {
    if english && !plan.from_english.is_empty() && !marian::has_other_letters(line) {
        &plan.from_english
    } else {
        &plan.steps
    }
}

pub struct LyricsTranslations {
    app: AppHandle,
    /// The loaded on-device models, by catalogue id.
    loaded: Arc<Mutex<HashMap<String, Marian>>>,
    /// Counts on-device translations; models are let go only if no newer one started.
    uses: Arc<AtomicU64>,
    /// One on-device translation at a time: the models use every core.
    device: tokio::sync::Mutex<()>,
    /// Translations of this session while incognito is on (M47): kept in memory only.
    held: Mutex<HashMap<String, Vec<TranslationRow>>>,
}

impl LyricsTranslations {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app,
            loaded: Arc::default(),
            uses: Arc::default(),
            device: tokio::sync::Mutex::new(()),
            held: Mutex::default(),
        })
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().0.clone()
    }

    /// `lyrics` with every kept translation of the song lined up under its lines.
    pub async fn attach(&self, query: &TrackQuery, mut lyrics: Lyrics) -> Lyrics {
        let key = raygun_resolver::track_key(query);
        let mut rows = match store::translations(&self.db(), &key).await {
            Ok(rows) => rows,
            Err(error) => {
                tracing::warn!(%error, "couldn't read a song's lyrics translations");
                Vec::new()
            }
        };
        rows.extend(self.held_rows(&key));
        lyrics.translations = lay_onto(&lyrics, &rows);
        lyrics
    }

    fn held_rows(&self, key: &str) -> Vec<TranslationRow> {
        self.held
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    /// Holds translations in memory while incognito is on (M47), replacing what the translators
    /// in `replaced` gave before.
    pub fn hold(&self, key: &str, replaced: &[String], rows: Vec<TranslationRow>) {
        let mut held = self
            .held
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let song = held.entry(key.to_owned()).or_default();
        song.retain(|row| {
            !replaced.contains(&row.translator)
                && !rows.iter().any(|new| {
                    new.translator == row.translator
                        && new.language == row.language
                        && new.kind == row.kind
                })
        });
        song.extend(rows);
        song.sort_by_key(|row| -row.made_at);
    }

    /// The lyrics that show for a song, every line kept: its own, else a source's.
    async fn shown_lyrics(&self, query: &TrackQuery) -> Result<Option<Lyrics>, String> {
        let sound = self.app.state::<Arc<crate::sound::SoundService>>();
        let fetched = if sound.has_own_lyrics(query) {
            None
        } else {
            self.app
                .state::<Arc<crate::lyrics_store::LyricsStore>>()
                .lyrics(query)
                .await?
        };
        Ok(sound.lyrics_untrimmed(query, fetched))
    }

    async fn preferred_model(&self) -> Option<String> {
        crate::settings::load(&self.db())
            .await
            .ok()
            .map(|settings| settings.lyrics_translation.local_model)
            .filter(|model| !model.is_empty())
    }

    /// Who can translate the song's lyrics into `language`: this device (with what it would
    /// download first), then the translators from Settings → AI.
    pub async fn options(
        &self,
        query: &TrackQuery,
        language: &str,
    ) -> Result<Vec<LyricsTranslatorOption>, String> {
        language_tag(language)?;
        let lyrics = self.shown_lyrics(query).await?;
        let lines = lyrics.as_ref().map(Lyrics::line_texts).unwrap_or_default();
        self.translators(&lines, language).await
    }

    /// Who can translate `lines` into `language`: this device, with the models those lines go
    /// through and what of them it would download first, then the translators from Settings → AI.
    /// Lyrics and a video's subtitles alike.
    pub async fn translators(
        &self,
        lines: &[&str],
        language: &str,
    ) -> Result<Vec<LyricsTranslatorOption>, String> {
        language_tag(language)?;
        let english = marian::looks_english(lines);
        let catalog = raygun_ai::models::catalog();
        let preferred = self.preferred_model().await;
        let ai = self.app.state::<AiService>();
        let device = match marian::plan(&catalog, language, preferred.as_deref()) {
            Some(plan) => {
                let mut models: Vec<String> = Vec::new();
                for line in lines.iter().filter(|line| has_words(line)) {
                    for step in route(&plan, english, line) {
                        if !models.contains(&step.model) {
                            models.push(step.model.clone());
                        }
                    }
                }
                if models.is_empty() {
                    models = plan.models();
                }
                let download_bytes = models
                    .iter()
                    .filter(|id| ai.installed_dir(id).is_none())
                    .filter_map(|id| catalog.iter().find(|model| model.id == **id))
                    .map(|model| model.size() as f64)
                    .sum();
                LyricsTranslatorOption {
                    id: DEVICE.to_owned(),
                    kind: None,
                    name: String::new(),
                    models,
                    download_bytes,
                    supported: true,
                }
            }
            None => LyricsTranslatorOption {
                id: DEVICE.to_owned(),
                kind: None,
                name: String::new(),
                models: Vec::new(),
                download_bytes: 0.0,
                supported: false,
            },
        };
        let mut options = vec![device];
        for provider in ai.providers().await? {
            options.push(LyricsTranslatorOption {
                id: format!("{AI_PREFIX}{}", provider.id),
                supported: provider.has_key || provider.kind == AiProviderKind::OpenAiCompatible,
                kind: Some(provider.kind),
                name: provider.name,
                models: Vec::new(),
                download_bytes: 0.0,
            });
        }
        Ok(options)
    }

    /// Translates the song's lyrics into `language` with `translator` (`device` or
    /// `ai:<provider id>`), each different line once and all in one go, and keeps the result.
    pub async fn translate(
        &self,
        query: &TrackQuery,
        language: &str,
        translator: &str,
    ) -> Result<LyricsTranslation, String> {
        language_tag(language)?;
        let lyrics = self
            .shown_lyrics(query)
            .await?
            .ok_or_else(|| "This song has no lyrics to translate".to_owned())?;
        let lines = lyrics.line_texts();
        let (distinct, places) = distinct_lines(&lines);
        if distinct.is_empty() {
            return Err("These lyrics have no words to translate".into());
        }
        let key = raygun_resolver::track_key(query);
        let (translated, by, origin) = if translator == DEVICE {
            let translated = self.on_device(&key, language, &distinct).await?;
            (translated, String::new(), LyricsTranslationOrigin::Device)
        } else if let Some(id) = translator.strip_prefix(AI_PREFIX) {
            let ai = self.app.state::<AiService>();
            let (name, translated) = ai.translate_lyrics(id, &distinct, language).await?;
            (translated, name, LyricsTranslationOrigin::Ai)
        } else {
            return Err("Pick a translator".into());
        };
        let per_line: Vec<String> = places
            .iter()
            .map(|place| {
                place
                    .and_then(|index| translated.get(index))
                    .map(|line| line.trim().to_owned())
                    .unwrap_or_default()
            })
            .collect();
        let row = TranslationRow {
            translator: translator.to_owned(),
            language: language.to_owned(),
            kind: kind_name(LyricsTranslationKind::Translation).to_owned(),
            by_name: by.clone(),
            lines: serde_json::to_string(&kept_lines(&lyrics, &per_line)).map_err(to_message)?,
            made_at: now_ms(),
        };
        if crate::privacy::incognito(&self.app) {
            self.hold(&key, &[], vec![row]);
        } else {
            store::put_translation(&self.db(), &key, &row)
                .await
                .map_err(to_message)?;
        }
        tracing::info!(song = %key, %translator, %language, lines = distinct.len(), "lyrics translated");
        Ok(LyricsTranslation {
            language: language.to_owned(),
            kind: LyricsTranslationKind::Translation,
            origin,
            by,
            translator: translator.to_owned(),
            lines: per_line,
        })
    }

    /// Puts a kept translation first for the song, as the one picked.
    pub async fn pick(
        &self,
        query: &TrackQuery,
        language: &str,
        translator: &str,
    ) -> Result<(), String> {
        // Nothing is remembered while incognito is on (M47); the menu's pick still shows.
        if crate::privacy::incognito(&self.app) {
            return Ok(());
        }
        let key = raygun_resolver::track_key(query);
        store::pick_translation(&self.db(), &key, translator, language, now_ms())
            .await
            .map_err(to_message)
    }

    /// Translates `lines` with the models that write `language`, line by line on a blocking
    /// thread, reporting progress.
    async fn on_device(
        &self,
        key: &str,
        language: &str,
        lines: &[String],
    ) -> Result<Vec<String>, String> {
        let app = self.app.clone();
        let progress = LyricsTranslationProgress {
            key: key.to_owned(),
            language: language.to_owned(),
            translator: DEVICE.to_owned(),
            done: 0,
            total: lines.len() as u32,
        };
        self.translate_on_device(language, lines, move |done, _| {
            let _ = LyricsTranslationEvent(LyricsTranslationProgress {
                done,
                ..progress.clone()
            })
            .emit(&app);
        })
        .await
    }

    /// Translates `lines` with this device's models that write `language` (each line through
    /// English unless it's English already), one translation at a time on a blocking thread;
    /// `progress` hears lines done out of all, a few times a second. The models stay loaded a
    /// while for the next lines (a song, or a video's next subtitles).
    pub async fn translate_on_device(
        &self,
        language: &str,
        lines: &[String],
        progress: impl Fn(u32, u32) + Send + 'static,
    ) -> Result<Vec<String>, String> {
        let catalog = raygun_ai::models::catalog();
        let preferred = self.preferred_model().await;
        let plan = marian::plan(&catalog, language, preferred.as_deref())
            .ok_or_else(|| "No model on this device translates into this language".to_owned())?;
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let english = marian::looks_english(&refs);
        let routes: Vec<Vec<Step>> = lines
            .iter()
            .map(|line| route(&plan, english, line).to_vec())
            .collect();
        let ai = self.app.state::<AiService>();
        let mut dirs: HashMap<String, PathBuf> = HashMap::new();
        for step in routes.iter().flatten() {
            if dirs.contains_key(&step.model) {
                continue;
            }
            let dir = ai
                .installed_dir(&step.model)
                .ok_or_else(|| "Download the translation model first".to_owned())?;
            dirs.insert(step.model.clone(), dir);
        }
        let _turn = self.device.lock().await;
        self.uses.fetch_add(1, Ordering::SeqCst);
        let loaded = self.loaded.clone();
        let total = lines.len() as u32;
        let lines = lines.to_vec();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let mut models = loaded.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            // Only the models this song needs stay in memory.
            models.retain(|id, _| dirs.contains_key(id));
            for (id, dir) in &dirs {
                if !models.contains_key(id) {
                    let started = Instant::now();
                    let model = Marian::load(dir).map_err(to_message)?;
                    tracing::info!(model = %id, elapsed = ?started.elapsed(), "translation model loaded");
                    models.insert(id.clone(), model);
                }
            }
            progress(0, total);
            let mut out = Vec::with_capacity(lines.len());
            let mut reported = Instant::now();
            for (index, (line, steps)) in lines.iter().zip(&routes).enumerate() {
                let mut text = line.clone();
                for step in steps {
                    let model = models
                        .get_mut(&step.model)
                        .ok_or_else(|| "The translation model isn't loaded".to_owned())?;
                    text = model
                        .translate(&text, step.target_token.as_deref())
                        .map_err(to_message)?;
                }
                out.push(text);
                if reported.elapsed() >= PROGRESS_EVERY || index + 1 == lines.len() {
                    reported = Instant::now();
                    progress(index as u32 + 1, total);
                }
            }
            Ok::<_, String>(out)
        })
        .await
        .map_err(to_message)?;
        self.release_later();
        result
    }

    /// Lets the models go a while after the last translation, unless another one started since.
    fn release_later(&self) {
        let generation = self.uses.load(Ordering::SeqCst);
        let uses = self.uses.clone();
        let loaded = self.loaded.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(KEEP_LOADED).await;
            if uses.load(Ordering::SeqCst) == generation {
                loaded
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clear();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use raygun_model::LyricLine;

    use super::*;

    fn lyrics(provider: &str, lines: &[(u32, &str)]) -> Lyrics {
        Lyrics {
            provider_name: provider.into(),
            synced: lines
                .iter()
                .map(|(time_ms, text)| LyricLine {
                    time_ms: *time_ms,
                    text: (*text).into(),
                })
                .collect(),
            plain: None,
            offset_ms: 0,
            edited: false,
            translations: Vec::new(),
        }
    }

    fn translation(
        language: &str,
        kind: LyricsTranslationKind,
        lines: &[&str],
    ) -> LyricsTranslation {
        LyricsTranslation {
            language: language.into(),
            kind,
            origin: LyricsTranslationOrigin::Source,
            by: String::new(),
            translator: String::new(),
            lines: lines.iter().map(|line| (*line).to_owned()).collect(),
        }
    }

    #[test]
    fn sources_translations_follow_whichever_lyrics_show() {
        let mut lines = lyrics(
            "Lyric Lines",
            &[(1_000, "夜に駆ける"), (4_000, "沈むように")],
        );
        lines.translations = vec![
            translation(
                "zh-Hans",
                LyricsTranslationKind::Translation,
                &["奔向夜晚", "宛如沉溺般"],
            ),
            translation(
                "ja-Latn",
                LyricsTranslationKind::Romanization,
                &["yo ru ni ka ke ru", "shi zu mu yo u ni"],
            ),
            translation("en", LyricsTranslationKind::Translation, &["", ""]),
        ];
        let rows = source_rows(&[lyrics("Lyrics Base", &[(0, "x")]), lines.clone()], 10);
        // Nothing from a source without translations, nor an empty one.
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter()
                .all(|row| row.translator == "source:Lyric Lines")
        );

        // On Lyric Lines' own lyrics they're the source's; on Lyrics Base's another source's.
        let own = lay_onto(&lines, &rows);
        assert_eq!(own.len(), 2);
        assert!(
            own.iter()
                .all(|t| t.origin == LyricsTranslationOrigin::Source)
        );
        let base = lyrics(
            "Lyrics Base",
            &[(1_100, "夜に駆ける"), (2_000, "♪"), (4_100, "沈むように")],
        );
        let other = lay_onto(&base, &rows);
        assert_eq!(other[0].origin, LyricsTranslationOrigin::OtherSource);
        assert_eq!(other[0].lines, ["奔向夜晚", "", "宛如沉溺般"]);
        // The song's own lyrics (picked from Lyric Lines) take them too, as another source's.
        let picked = Lyrics {
            edited: true,
            ..lyrics(
                "Your lyrics",
                &[(1_000, "夜に駆ける"), (4_000, "沈むように")],
            )
        };
        assert_eq!(
            lay_onto(&picked, &rows)[0].origin,
            LyricsTranslationOrigin::OtherSource
        );
    }

    #[test]
    fn newer_translations_come_first() {
        let song = lyrics("Lyric Lines", &[(0, "Hello")]);
        let row = |translator: &str, made_at: i64| TranslationRow {
            translator: translator.into(),
            language: "de".into(),
            kind: "translation".into(),
            by_name: translator.into(),
            lines: serde_json::to_string(&[KeptLine {
                time_ms: Some(0),
                text: "Hello".into(),
                translation: format!("Hallo ({translator})"),
            }])
            .unwrap(),
            made_at,
        };
        let laid = lay_onto(
            &song,
            &[
                row("source:Other", 5),
                row("ai:x", 9),
                row("source:Lyric Lines", 5),
                row("device", 7),
            ],
        );
        let order: Vec<&str> = laid.iter().map(|t| t.translator.as_str()).collect();
        assert_eq!(
            order,
            ["ai:x", "device", "source:Lyric Lines", "source:Other"]
        );
        assert_eq!(laid[0].origin, LyricsTranslationOrigin::Ai);
        assert_eq!(laid[1].origin, LyricsTranslationOrigin::Device);
    }

    #[test]
    fn each_line_is_translated_once() {
        let (distinct, places) =
            distinct_lines(&["Oh baby", " ♪ ", "I love you", "Oh baby ", "", "I love you"]);
        assert_eq!(distinct, ["Oh baby", "I love you"]);
        assert_eq!(places, [Some(0), None, Some(1), Some(0), None, Some(1)]);
        assert!(language_tag("pt-BR").is_ok() && language_tag("no way").is_err());
    }
}
