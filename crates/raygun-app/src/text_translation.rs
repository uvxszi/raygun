//! Words translated on their own: a selection in a book, a title to search for in another
//! language. From a language or detected, into one, by this device's models (through the readers'
//! translation, which keeps what it made a while) or a translator from Settings → AI. The answer
//! says who translated and what the words were read as: the translator's own detection (Google,
//! DeepL, LibreTranslate), else the hint the page gave (the dictionary's answer, the book's
//! language), else a guess from the words. The languages and translator picked for a book are
//! remembered for it, and as the last choice for books without their own.

use std::sync::{Arc, OnceLock};

use raygun_ai::marian;
use raygun_ai::translate::Content;
use raygun_model::{
    ReaderAiTranslateRequest, TextTranslateRequest, TextTranslation, TranslateChoice,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::ai::AiService;
use crate::reader_ai::ReaderAi;
use crate::startup::AppDb;

/// A translator from Settings → AI is named `ai:<provider id>`.
const AI_PREFIX: &str = "ai:";
/// `local_meta` key of the remembered choices (JSON).
const CHOICES: &str = "translate.choices";
/// Books whose choice is remembered; the ones picked longest ago go first.
const BOOKS_KEPT: usize = 300;

/// The choices remembered: the last one, and each book's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct Choices {
    last: Option<TranslateChoice>,
    /// By book id, the last picked first.
    books: Vec<(String, TranslateChoice)>,
}

impl Choices {
    fn remember(&mut self, book: &str, choice: TranslateChoice) {
        self.books.retain(|(id, _)| id != book);
        if !book.is_empty() {
            self.books.insert(0, (book.to_owned(), choice.clone()));
            self.books.truncate(BOOKS_KEPT);
        }
        self.last = Some(choice);
    }

    /// A book's own choice, else the last one anywhere, whose language to translate from (another
    /// book's) is detected instead.
    fn for_book(&self, book: Option<&str>) -> Option<TranslateChoice> {
        book.and_then(|book| {
            self.books
                .iter()
                .find(|(id, _)| id == book)
                .map(|(_, choice)| choice.clone())
        })
        .or_else(|| {
            self.last.clone().map(|last| TranslateChoice {
                from: String::new(),
                ..last
            })
        })
    }
}

/// One change to the remembered choices at a time.
fn saving() -> &'static tokio::sync::Mutex<()> {
    static SAVING: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    SAVING.get_or_init(|| tokio::sync::Mutex::new(()))
}

async fn load(app: &AppHandle) -> Result<Choices, String> {
    let db = app.state::<AppDb>().0.clone();
    let text = raygun_db::local_value(&db, CHOICES)
        .await
        .map_err(|error| error.to_string())?;
    Ok(text
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default())
}

/// Keeps `choice` for `book` (when given) and as the last one; nothing is kept while incognito
/// is on.
pub async fn remember(
    app: &AppHandle,
    book: Option<&str>,
    choice: TranslateChoice,
) -> Result<(), String> {
    if crate::privacy::incognito(app) {
        return Ok(());
    }
    let choice = TranslateChoice {
        from: choice.from.trim().to_owned(),
        to: choice.to.trim().to_owned(),
        translator: choice.translator.trim().to_owned(),
    };
    let _turn = saving().lock().await;
    let mut choices = load(app).await?;
    choices.remember(book.unwrap_or_default(), choice);
    let text = serde_json::to_string(&choices).map_err(|error| error.to_string())?;
    let db = app.state::<AppDb>().0.clone();
    raygun_db::set_local_value(&db, CHOICES, &text)
        .await
        .map_err(|error| error.to_string())
}

/// The languages and translator picked last for `book`'s selections, else for any book's.
pub async fn choice(
    app: &AppHandle,
    book: Option<&str>,
) -> Result<Option<TranslateChoice>, String> {
    Ok(load(app).await?.for_book(book))
}

/// What words are likely in when nobody said: the page's hint, else a guess from the words.
fn likely_language(hint: &str, text: &str) -> Option<String> {
    let hint = hint.trim();
    if !hint.is_empty() {
        return Some(hint.to_owned());
    }
    raygun_ai::tts::language::guess(text).map(str::to_owned)
}

/// The models this device's translation of `text` from `from` (empty: as it reads) into `to`
/// goes through, `preferred` first among those that write `to`.
fn device_models(to: &str, preferred: &str, from: &str, text: &str) -> Vec<String> {
    if !from.is_empty() && marian::same_language(from, to) {
        return Vec::new();
    }
    let catalog = raygun_ai::models::catalog();
    let preferred = Some(preferred).filter(|model| !model.is_empty());
    let Some(plan) = marian::plan(&catalog, to, preferred) else {
        return Vec::new();
    };
    let english = if from.is_empty() {
        marian::looks_english(&[text])
    } else {
        marian::same_language(from, "en")
    };
    if english && marian::same_language(to, "en") {
        return Vec::new();
    }
    let steps = if english && !plan.from_english.is_empty() {
        &plan.from_english
    } else {
        &plan.steps
    };
    steps.iter().map(|step| step.model.clone()).collect()
}

/// Translates `request.text`, saying who translated and from what.
pub async fn translate(
    app: &AppHandle,
    request: TextTranslateRequest,
) -> Result<TextTranslation, String> {
    let text = request.text.trim().to_owned();
    let to = request.to.trim().to_owned();
    if to.is_empty() {
        return Err("Pick a language to translate into".into());
    }
    if text.is_empty() {
        return Err("There's nothing to translate".into());
    }
    let from = request.from.trim().to_owned();
    let translator = request.translator.trim().to_owned();
    let detecting = from.is_empty();
    if let Some(id) = translator.strip_prefix(AI_PREFIX) {
        let asked = Some(from.as_str()).filter(|from| !from.is_empty());
        let done = app
            .state::<AiService>()
            .translate_detailed(
                id,
                std::slice::from_ref(&text),
                asked,
                &to,
                Content::Text,
                true,
            )
            .await?;
        let from = if detecting {
            done.detected
                .or_else(|| likely_language(&request.hint, &text))
                .unwrap_or_default()
        } else {
            from
        };
        return Ok(TextTranslation {
            text: done.texts.into_iter().next().unwrap_or_default(),
            from,
            detected: detecting,
            to,
            translator,
            translator_name: done.name,
            kind: Some(done.kind),
            models: Vec::new(),
        });
    }
    // This device reads the words as the page says they are (the dictionary's language, the
    // book's), else as they read.
    let source = if detecting {
        request.hint.trim().to_owned()
    } else {
        from.clone()
    };
    let translated = app
        .state::<Arc<ReaderAi>>()
        .translate(ReaderAiTranslateRequest {
            texts: vec![text.clone()],
            from: source.clone(),
            to: to.clone(),
            model: translator.clone(),
        })
        .await?;
    let from = if detecting {
        likely_language(&request.hint, &text).unwrap_or_default()
    } else {
        from
    };
    Ok(TextTranslation {
        text: translated.into_iter().next().unwrap_or_default(),
        models: device_models(&to, &translator, &source, &text),
        from,
        detected: detecting,
        to,
        translator,
        translator_name: String::new(),
        kind: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(from: &str, to: &str, translator: &str) -> TranslateChoice {
        TranslateChoice {
            from: from.into(),
            to: to.into(),
            translator: translator.into(),
        }
    }

    #[test]
    fn a_books_choice_beats_the_last_one_anywhere() {
        let mut choices = Choices::default();
        assert_eq!(choices.for_book(Some("b1")), None);
        choices.remember("b1", choice("", "de", "ai:g"));
        choices.remember("b2", choice("fr", "en", ""));
        assert_eq!(choices.for_book(Some("b1")), Some(choice("", "de", "ai:g")));
        assert_eq!(
            choices.for_book(Some("b2")),
            Some(choice("fr", "en", "")),
            "a book's own keeps its language"
        );
        assert_eq!(
            choices.for_book(Some("b3")),
            Some(choice("", "en", "")),
            "a book without its own takes the last, detecting what it's in"
        );
        assert_eq!(choices.for_book(None), Some(choice("", "en", "")));
        // Picking again moves the book to the front; the oldest books are let go.
        choices.remember("b1", choice("", "ja", ""));
        assert_eq!(choices.books[0].0, "b1");
        assert_eq!(choices.books.len(), 2);
        for book in 0..BOOKS_KEPT + 5 {
            choices.remember(&format!("x{book}"), choice("", "it", ""));
        }
        assert_eq!(choices.books.len(), BOOKS_KEPT);
        assert!(choices.books.iter().all(|(id, _)| id != "b1"));
        let text = serde_json::to_string(&choices).unwrap();
        assert_eq!(serde_json::from_str::<Choices>(&text).unwrap(), choices);
    }

    #[test]
    fn the_words_language_comes_from_the_hint_then_the_words() {
        assert_eq!(
            likely_language(" de ", "Hello there").as_deref(),
            Some("de")
        );
        assert_eq!(
            likely_language("", "これは日本語の文章です。とても長い文です。").as_deref(),
            Some("ja")
        );
        assert_eq!(likely_language("", "ok"), None);
    }

    #[test]
    fn device_models_follow_the_route_through_english() {
        assert_eq!(
            device_models("de", "", "", "The house was quiet and the night was long."),
            ["opus-mt-en-de"]
        );
        assert_eq!(
            device_models("de", "", "ja", "夜は長かった"),
            ["opus-mt-mul-en", "opus-mt-en-de"]
        );
        assert_eq!(device_models("en", "", "fr", "La nuit"), ["opus-mt-mul-en"]);
        assert!(device_models("de", "", "de-AT", "Guten Morgen").is_empty());
        assert!(
            device_models("en", "", "", "The night was long and the house was quiet.").is_empty()
        );
        assert!(device_models("tlh", "", "", "anything").is_empty());
    }
}
