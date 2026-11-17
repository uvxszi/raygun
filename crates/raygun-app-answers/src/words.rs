//! Words: what one means (Raygun's dictionaries: Wiktionary, the Free Dictionary API and
//! extensions' `dictionary` providers) and translations (this device's models, or the
//! translators set up in Settings → AI). No model is downloaded from here.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use raygun_model::{
    AnswerCredit, AnswerKind, AnswerRow, AnswerText, DictionaryLookup, DictionaryRequest,
};
use tauri::{AppHandle, Manager};

use super::languages;

/// Senses shown at most.
const MAX_SENSES: usize = 6;

/// A dictionary's answer as an answer: the first sense, the others as rows.
pub fn definition_of(lookup: &DictionaryLookup) -> Option<raygun_model::Answer> {
    let first = lookup.meanings.first()?;
    let sense = first.definitions.first()?;
    let mut answer = super::blank(AnswerKind::Definition);
    answer.engine = "dictionary".to_owned();
    let ipa = lookup
        .pronunciations
        .iter()
        .find_map(|pronunciation| pronunciation.ipa.clone());
    let headword = match &ipa {
        Some(ipa) => format!("{} {ipa}", lookup.word),
        None => lookup.word.clone(),
    };
    answer.interpretation = Some(AnswerText::plain(if first.part_of_speech.is_empty() {
        headword
    } else {
        format!("{headword} · {}", first.part_of_speech)
    }));
    answer.result = AnswerText::plain(sense.text.clone());
    answer.copy = sense.text.clone();
    let mut shown = 1;
    for meaning in &lookup.meanings {
        for definition in &meaning.definitions {
            if std::ptr::eq(definition, sense) || shown >= MAX_SENSES {
                continue;
            }
            shown += 1;
            answer.rows.push(AnswerRow {
                key: String::new(),
                label: meaning.part_of_speech.clone(),
                value: AnswerText::plain(definition.text.clone()),
            });
        }
    }
    let synonyms: Vec<String> = lookup
        .meanings
        .iter()
        .flat_map(|meaning| meaning.synonyms.iter().cloned())
        .take(8)
        .collect();
    if !synonyms.is_empty() {
        answer.rows.push(AnswerRow {
            key: "synonyms".to_owned(),
            label: "Synonyms".to_owned(),
            value: AnswerText::plain(synonyms.join(", ")),
        });
    }
    if let Some(etymology) = &lookup.etymology {
        answer.rows.push(AnswerRow {
            key: "etymology".to_owned(),
            label: "Etymology".to_owned(),
            value: AnswerText::plain(etymology.clone()),
        });
    }
    answer.url = lookup
        .credits
        .iter()
        .find_map(|credit| credit.url.clone())
        .or_else(|| Some(lookup.online_url.clone()));
    answer.credits = lookup
        .credits
        .iter()
        .map(|credit| AnswerCredit {
            name: credit.name.clone(),
            url: credit.url.clone(),
            license: credit.license.as_ref().map(|license| license.name.clone()),
            license_url: credit
                .license
                .as_ref()
                .and_then(|license| license.url.clone()),
        })
        .collect();
    answer.kept = lookup.kept;
    Some(answer)
}

/// Where words are looked up and translated: the app's dictionaries and translators, set at start
/// as a [`Words`] state.
pub trait WordSources: Send + Sync {
    /// What the dictionaries answer; `None` without dictionaries.
    fn lookup(&self, request: DictionaryRequest) -> Looking<'_>;

    /// `text` translated into `to` by this device's models when they're there, else by the first
    /// translator from Settings → AI; `None` when no translator writes that language yet.
    fn translate<'a>(&'a self, text: &'a str, to: &'a str) -> Translating<'a>;
}

/// A dictionary lookup on its way.
pub type Looking<'a> =
    Pin<Box<dyn Future<Output = Option<Result<DictionaryLookup, String>>> + Send + 'a>>;

/// A translation on its way.
pub type Translating<'a> =
    Pin<Box<dyn Future<Output = Result<Option<Translation>, String>> + Send + 'a>>;

/// The word sources, as Tauri state.
pub type Words = Arc<dyn WordSources>;

/// What a translator made of a text.
pub struct Translation {
    /// The chosen translator's name.
    pub chosen_name: String,
    /// Whether it was this device's models.
    pub on_device: bool,
    /// The name the translation gave itself, if any.
    pub name: String,
    /// The text's lines translated.
    pub lines: Vec<String>,
}

/// What the dictionaries say about `word`.
pub async fn define(
    app: &AppHandle,
    word: &str,
    language: &str,
    refresh: bool,
) -> Result<Option<raygun_model::Answer>, String> {
    let Some(words) = app.try_state::<Words>() else {
        return Ok(None);
    };
    let Some(lookup) = words
        .lookup(DictionaryRequest {
            text: word.to_owned(),
            title_id: None,
            app_language: language.to_owned(),
            refresh,
        })
        .await
    else {
        return Ok(None);
    };
    Ok(definition_of(&lookup?))
}

/// What a translation found.
pub enum Translated {
    Done(Box<raygun_model::Answer>),
    /// No translator writes that language yet: one has to be set up or downloaded.
    NeedsTranslator,
}

/// `text` translated into `to` by this device's models when they're there, else by the first
/// translator from Settings → AI.
pub async fn translate(app: &AppHandle, text: &str, to: &str) -> Result<Translated, String> {
    let Some(words) = app.try_state::<Words>() else {
        return Ok(Translated::NeedsTranslator);
    };
    let Some(Translation {
        chosen_name,
        on_device,
        name,
        lines: translated,
    }) = words.translate(text, to).await?
    else {
        return Ok(Translated::NeedsTranslator);
    };
    let Some(result) = translated
        .into_iter()
        .next()
        .filter(|line| !line.trim().is_empty())
    else {
        return Ok(Translated::NeedsTranslator);
    };
    let mut answer = super::blank(AnswerKind::Translation);
    answer.engine = "translation".to_owned();
    let language = languages::name_of(to).unwrap_or_else(|| to.to_owned());
    answer.interpretation = Some(AnswerText::plain(format!("“{text}” in {language}")));
    answer.copy = result.trim().to_owned();
    answer.result = AnswerText::plain(result.trim().to_owned());
    answer.credits.push(AnswerCredit {
        // This device's models are named by the app.
        name: if name.is_empty() {
            chosen_name.clone()
        } else {
            name
        },
        url: None,
        license: None,
        license_url: None,
    });
    answer.engine_name = if on_device {
        String::new()
    } else {
        chosen_name.clone()
    };
    Ok(Translated::Done(Box::new(answer)))
}
