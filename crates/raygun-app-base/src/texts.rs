//! Words Rust shows by itself (notifications), in the app's language. The page owns the
//! translations (`apps/raygun/src/i18n/locales`): whenever its language changes it sends the
//! `notices.*` strings, resolved for that language, with `app_texts_set`. They're kept in
//! `local_meta` so notifications from a start without the page (or before it loads) use them too.
//! Keys without a translation answer the English given at the call.

use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::AppDb;

/// Where the last texts the page sent are kept.
const STORED: &str = "app.texts";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Texts {
    /// The language they're in, a BCP 47 tag (`en` before the page said).
    language: String,
    strings: HashMap<String, String>,
}

static TEXTS: LazyLock<RwLock<Texts>> = LazyLock::new(Default::default);

/// The language Rust's texts are in: the app's, as the page resolved it ("System" follows the
/// device). `en` until the page has said.
pub fn language() -> String {
    let texts = TEXTS
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if texts.language.is_empty() {
        "en".to_owned()
    } else {
        texts.language.clone()
    }
}

fn replace(texts: Texts) {
    *TEXTS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = texts;
}

fn lookup(key: &str) -> Option<String> {
    let texts = TEXTS
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    texts
        .strings
        .get(key)
        .filter(|text| !text.trim().is_empty())
        .cloned()
}

/// Fills i18next-style `{{name}}` placeholders.
fn fill(text: &str, args: &[(&str, &str)]) -> String {
    let mut filled = text.to_owned();
    for (name, value) in args {
        filled = filled.replace(&format!("{{{{{name}}}}}"), value);
    }
    filled
}

/// `key`'s text in the app's language, else `english`.
pub fn text(key: &str, english: &str) -> String {
    lookup(key).unwrap_or_else(|| english.to_owned())
}

/// [`text`] with `{{name}}` placeholders filled from `args` (in the translation and in
/// `english` alike).
pub fn text_with(key: &str, english: &str, args: &[(&str, &str)]) -> String {
    fill(&text(key, english), args)
}

/// A counted text, i18next's way: `key_one` for one, `key_other` otherwise (the page sends both
/// forms, resolved for its language), `{{count}}` filled in.
pub fn count_text(key: &str, english_one: &str, english_other: &str, count: usize) -> String {
    let (form, english) = if count == 1 {
        ("one", english_one)
    } else {
        ("other", english_other)
    };
    let translated = lookup(&format!("{key}_{form}")).or_else(|| {
        (form == "one")
            .then(|| lookup(&format!("{key}_other")))
            .flatten()
    });
    fill(
        translated.as_deref().unwrap_or(english),
        &[("count", &count.to_string())],
    )
}

/// Puts back the texts kept from the last run (at start, before anything posts a notice).
pub async fn load(db: &raygun_db::Db) {
    let Ok(Some(stored)) = raygun_db::local_value(db, STORED).await else {
        return;
    };
    match serde_json::from_str::<Texts>(&stored) {
        Ok(texts) => replace(texts),
        Err(error) => tracing::warn!(%error, "kept notification texts didn't read"),
    }
}

/// The page's language and its `notices.*` strings (keys as in the locale files, without the
/// `notices.` prefix), used for notifications from now on and kept for the next start.
#[tauri::command]
#[specta::specta]
pub async fn app_texts_set(
    db: State<'_, AppDb>,
    language: String,
    texts: HashMap<String, String>,
) -> Result<(), String> {
    let texts = Texts {
        language,
        strings: texts,
    };
    let stored = serde_json::to_string(&texts).map_err(|error| error.to_string())?;
    replace(texts);
    raygun_db::set_local_value(&db.0, STORED, &stored)
        .await
        .map_err(|error| error.to_string())
}

/// Sets the texts in tests.
#[cfg(test)]
pub fn set_for_tests(language: &str, strings: &[(&str, &str)]) {
    replace(Texts {
        language: language.to_owned(),
        strings: strings
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test, with keys of its own: the texts are process-wide and other tests run beside it.
    #[test]
    fn translates_fills_and_falls_back_to_english() {
        set_for_tests("en", &[]);
        assert_eq!(text("test.downloads", "Downloads"), "Downloads");
        assert_eq!(
            count_text(
                "test.newChapters",
                "{{count}} new chapter",
                "{{count}} new chapters",
                3
            ),
            "3 new chapters"
        );
        assert_eq!(language(), "en");

        set_for_tests(
            "de",
            &[
                ("test.downloads", "Downloads (de)"),
                ("test.newChapters_one", "{{count}} neues Kapitel"),
                ("test.newChapters_other", "{{count}} neue Kapitel"),
                ("test.updateOut", "Raygun {{version}} ist da"),
                ("test.empty", " "),
            ],
        );
        assert_eq!(language(), "de");
        assert_eq!(text("test.downloads", "Downloads"), "Downloads (de)");
        assert_eq!(text("test.empty", "Fallback"), "Fallback");
        assert_eq!(text("test.missing", "Missing"), "Missing");
        assert_eq!(count_text("test.newChapters", "", "", 1), "1 neues Kapitel");
        assert_eq!(count_text("test.newChapters", "", "", 4), "4 neue Kapitel");
        assert_eq!(
            text_with(
                "test.updateOut",
                "Raygun {{version}} is out",
                &[("version", "2.0.1")]
            ),
            "Raygun 2.0.1 ist da"
        );
        set_for_tests("en", &[]);
    }
}
