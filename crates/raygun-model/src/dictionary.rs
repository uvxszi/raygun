//! Word lookups in the book reader: what a dictionary says about a selected word or phrase, and
//! the reader's dictionary settings.

use serde::{Deserialize, Serialize};
use specta::Type;

/// The book reader's dictionary (Settings → Reader).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct DictionaryPrefs {
    /// Selecting words in the book reader looks them up; off, the text isn't selectable and a
    /// long press reads aloud from the paragraph, as before.
    pub enabled: bool,
    /// Sources in the order they're asked and shown (`wiktionary`, `freeDictionary`, an
    /// extension's `<extension>/<provider>`); sources not listed come after them.
    pub order: Vec<String>,
    /// Sources turned off.
    pub off: Vec<String>,
    /// The words' language as a BCP 47 tag; empty takes the book's language, then the app's.
    pub language: String,
}

impl Default for DictionaryPrefs {
    fn default() -> Self {
        Self {
            enabled: true,
            order: Vec::new(),
            off: Vec::new(),
            language: String::new(),
        }
    }
}

/// Sources a list may name, at most.
const MAX_SOURCES: usize = 64;

impl DictionaryPrefs {
    pub(crate) fn validate(&self) -> Result<(), String> {
        let language = self.language.trim();
        if !language.is_empty() {
            super::language_tag(language)?;
        }
        for ids in [&self.order, &self.off] {
            if ids.len() > MAX_SOURCES || ids.iter().any(|id| id.is_empty() || id.len() > 200) {
                return Err("a dictionary list names at most 64 sources by id".to_owned());
            }
        }
        Ok(())
    }
}

/// A word or phrase selected in the reader, to look up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryRequest {
    /// The selection as it was made.
    pub text: String,
    /// The book it's in: its language is the words' language.
    pub title_id: Option<String>,
    /// The app's language as the UI resolved it (a BCP 47 tag): the words' language when the
    /// book's isn't known, and the language translations are shown in.
    pub app_language: String,
    /// Asks the sources again, even when this device kept a recent answer.
    #[serde(default)]
    pub refresh: bool,
}

/// How a word sounds: a transcription, a recording, or both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryPronunciation {
    /// IPA, with its slashes or brackets (`/ˈhæpi/`).
    pub ipa: Option<String>,
    /// Whose pronunciation: an accent or a region (`UK`, `General American`).
    pub label: Option<String>,
    /// A recording. In a lookup's answer, a link through Raygun's own proxy.
    pub audio_url: Option<String>,
}

/// One sense of a word.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryDefinition {
    pub text: String,
    /// Sentences using the word in this sense.
    pub examples: Vec<String>,
}

/// The word in another language: the app's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryTranslation {
    /// The sense translated (`contented, joyous`), when the source tells senses apart.
    pub gloss: Option<String>,
    /// The language the terms are in, a BCP 47 tag.
    pub language: String,
    pub terms: Vec<String>,
}

/// The word as one part of speech.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryMeaning {
    /// As the source names it, in lower case (`noun`, `verb`, `proper noun`).
    pub part_of_speech: String,
    pub definitions: Vec<DictionaryDefinition>,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
    pub translations: Vec<DictionaryTranslation>,
}

/// The word is a form of another (`went`: simple past of `go`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryFormOf {
    /// The form looked up.
    pub form: String,
    /// The word it's a form of, whose entry is shown.
    pub lemma: String,
    /// What the source says the form is (`simple past of go`).
    pub note: String,
    /// Every sense the source gives is a form of the lemma (`ran`), so the lemma's entry is
    /// what's shown; otherwise (`building`) the word's own senses are, with the lemma offered.
    #[serde(default)]
    pub only: bool,
}

/// The licence a source's text comes under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryLicense {
    /// `CC BY-SA 4.0`.
    pub name: String,
    pub url: Option<String>,
}

/// What one source says about a word, in one language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryEntry {
    /// The headword as the source spells it.
    pub word: String,
    /// The language the word is in, a BCP 47 tag.
    pub language: String,
    /// The language's name as the source gives it (`French`).
    pub language_name: Option<String>,
    pub pronunciations: Vec<DictionaryPronunciation>,
    pub meanings: Vec<DictionaryMeaning>,
    /// Where the word comes from, as plain text.
    pub etymology: Option<String>,
    pub form_of: Option<DictionaryFormOf>,
    /// The source's full entry.
    pub url: Option<String>,
    pub license: Option<DictionaryLicense>,
}

/// A source that answered, credited under the answer with a link to its full entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryCredit {
    pub source: String,
    pub name: String,
    pub url: Option<String>,
    pub license: Option<DictionaryLicense>,
}

/// What the dictionaries say about a selection, merged in the order of the user's sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryLookup {
    /// The selection, tidied (surrounding punctuation and spaces left out).
    pub query: String,
    /// The headword shown; empty when no source knew the word.
    pub word: String,
    /// The language looked up in, a BCP 47 tag.
    pub language: String,
    pub language_name: Option<String>,
    pub form_of: Option<DictionaryFormOf>,
    pub pronunciations: Vec<DictionaryPronunciation>,
    pub meanings: Vec<DictionaryMeaning>,
    pub etymology: Option<String>,
    pub credits: Vec<DictionaryCredit>,
    /// Searches the web's dictionaries for the selection (Wiktionary's search).
    pub online_url: String,
    /// The answer is what this device kept from an earlier lookup.
    pub kept: bool,
    /// Offline Mode is on and nothing was kept for the word.
    pub offline: bool,
    /// Sources that had trouble, one sentence each.
    pub issues: Vec<String>,
}

/// A dictionary source, for Settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DictionarySourceInfo {
    /// `wiktionary`, `freeDictionary`, or an extension's `<extension>/<provider>`.
    pub id: String,
    pub name: String,
    /// The languages it knows words of, as BCP 47 tags; empty for any.
    pub languages: Vec<String>,
    /// Raygun's own, or an extension's.
    pub built_in: bool,
    pub enabled: bool,
}

/// Where a word appears in a book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BookSearchHit {
    pub chapter_id: String,
    pub chapter_name: String,
    /// The paragraph it's in, from 0, among the chapter's blocks.
    pub block: u32,
    pub before: String,
    pub matched: String,
    pub after: String,
}

/// A search through a book's text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BookSearch {
    pub hits: Vec<BookSearchHit>,
    /// Chapters whose text was searched, out of `chapters`: a source's chapters are on this
    /// device only once downloaded (or while open).
    pub searched: u32,
    pub chapters: u32,
    /// More places matched than are listed.
    pub more: bool,
}
