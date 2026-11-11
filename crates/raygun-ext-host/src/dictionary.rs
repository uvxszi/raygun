//! The `dictionary` provider kind: what a word means, for the book reader's lookups. Raygun's
//! own dictionaries (Wiktionary, the Free Dictionary API) answer in the app; an extension adds
//! another by registering a provider with a `define({ word, language })` call that answers
//! `{ entries: [...] }` in the shape below. Answers are read leniently: an entry without a word
//! or a definition is dropped, links must be http(s), and text is kept to a sensible length.

use raygun_model::{
    DictionaryDefinition, DictionaryEntry, DictionaryFormOf, DictionaryLicense, DictionaryMeaning,
    DictionaryPronunciation, DictionaryTranslation,
};
use serde_json::Value;

pub const DICTIONARY: &str = "dictionary";

/// Entries one answer may give, and what each may hold.
const MAX_ENTRIES: usize = 10;
const MAX_MEANINGS: usize = 12;
const MAX_DEFINITIONS: usize = 30;
const MAX_EXAMPLES: usize = 4;
const MAX_WORDS: usize = 30;
const MAX_PRONUNCIATIONS: usize = 8;

/// A trimmed text of at most `limit` characters, without line breaks; none when empty.
fn text(value: Option<&Value>, limit: usize) -> Option<String> {
    let text = value?.as_str()?;
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.is_empty() {
        return None;
    }
    Some(flat.chars().take(limit).collect())
}

fn link(value: Option<&Value>) -> Option<String> {
    let url = text(value, 2048)?;
    reqwest::Url::parse(&url)
        .is_ok_and(|parsed| matches!(parsed.scheme(), "http" | "https"))
        .then_some(url)
}

fn list(value: Option<&Value>) -> impl Iterator<Item = &Value> {
    value.and_then(Value::as_array).into_iter().flatten()
}

/// Words in a list (synonyms, terms), each once.
fn words(value: Option<&Value>) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for word in list(value).filter_map(|word| text(Some(word), 120)) {
        if !found.contains(&word) {
            found.push(word);
        }
        if found.len() == MAX_WORDS {
            break;
        }
    }
    found
}

fn definition(value: &Value) -> Option<DictionaryDefinition> {
    // A plain string is a definition without examples.
    let text_value = if value.is_string() {
        Some(value)
    } else {
        value.get("text").or_else(|| value.get("definition"))
    };
    Some(DictionaryDefinition {
        text: text(text_value, 2000)?,
        examples: list(value.get("examples"))
            .filter_map(|example| text(Some(example), 600))
            .take(MAX_EXAMPLES)
            .collect(),
    })
}

fn meaning(value: &Value) -> Option<DictionaryMeaning> {
    let definitions: Vec<DictionaryDefinition> = list(value.get("definitions"))
        .filter_map(definition)
        .take(MAX_DEFINITIONS)
        .collect();
    if definitions.is_empty() {
        return None;
    }
    Some(DictionaryMeaning {
        part_of_speech: text(value.get("partOfSpeech"), 60)
            .map(|part| part.to_lowercase())
            .unwrap_or_default(),
        definitions,
        synonyms: words(value.get("synonyms")),
        antonyms: words(value.get("antonyms")),
        translations: list(value.get("translations"))
            .filter_map(|translation| {
                let terms = words(translation.get("terms"));
                (!terms.is_empty()).then(|| DictionaryTranslation {
                    gloss: text(translation.get("gloss"), 200),
                    language: text(translation.get("language"), 35).unwrap_or_default(),
                    terms,
                })
            })
            .take(MAX_MEANINGS)
            .collect(),
    })
}

fn pronunciation(value: &Value) -> Option<DictionaryPronunciation> {
    let found = DictionaryPronunciation {
        ipa: text(value.get("ipa"), 120),
        label: text(value.get("label"), 80),
        audio_url: link(value.get("audioUrl")),
    };
    (found.ipa.is_some() || found.audio_url.is_some()).then_some(found)
}

/// One entry of a `define` answer, for words of `language` (the language asked for, unless the
/// entry names its own).
pub fn parse_entry(value: &Value, language: &str) -> Option<DictionaryEntry> {
    let word = text(value.get("word"), 200)?;
    let meanings: Vec<DictionaryMeaning> = list(value.get("meanings"))
        .filter_map(meaning)
        .take(MAX_MEANINGS)
        .collect();
    if meanings.is_empty() {
        return None;
    }
    let license = value.get("license").and_then(|license| {
        Some(DictionaryLicense {
            name: text(license.get("name"), 80)?,
            url: link(license.get("url")),
        })
    });
    // A form whose other senses aren't given is only that form.
    let form_of = value.get("formOf").and_then(|form| {
        Some(DictionaryFormOf {
            form: word.clone(),
            lemma: text(form.get("lemma"), 200)?,
            note: text(form.get("note"), 300).unwrap_or_default(),
            only: form.get("only").and_then(Value::as_bool).unwrap_or(true),
        })
    });
    Some(DictionaryEntry {
        language: text(value.get("language"), 35).unwrap_or_else(|| language.to_owned()),
        language_name: text(value.get("languageName"), 80),
        pronunciations: list(value.get("pronunciations"))
            .filter_map(pronunciation)
            .take(MAX_PRONUNCIATIONS)
            .collect(),
        meanings,
        etymology: text(value.get("etymology"), 2000),
        form_of,
        url: link(value.get("url")),
        license,
        word,
    })
}

/// A provider's `define` answer, `{ entries: [...] }`; nothing for `{ entries: [] }` or `null`.
pub fn parse_entries(value: &Value, language: &str) -> Vec<DictionaryEntry> {
    list(value.get("entries"))
        .filter_map(|entry| parse_entry(entry, language))
        .take(MAX_ENTRIES)
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn an_answer_keeps_what_the_app_shows() {
        let answer = json!({ "entries": [
            {
                "word": " haus ",
                "language": "de",
                "pronunciations": [
                    { "ipa": "/haʊ̯s/", "audioUrl": "https://example.org/haus.ogg" },
                    { "label": "only a label" },
                    { "ipa": "[haʊ̯s]", "audioUrl": "file:///etc/passwd" }
                ],
                "meanings": [
                    { "partOfSpeech": "Noun", "definitions": [
                        { "text": "house\n building", "examples": ["Das Haus ist alt.", ""] },
                        "home",
                        { "text": "  " }
                    ], "synonyms": ["Gebäude", "Gebäude", ""], "translations": [
                        { "language": "en", "terms": ["house"], "gloss": "building" },
                        { "language": "fr", "terms": [] }
                    ] },
                    { "partOfSpeech": "Verb", "definitions": [] }
                ],
                "etymology": "From Old High German hūs.",
                "url": "https://example.org/Haus",
                "license": { "name": "CC BY-SA 4.0", "url": "https://creativecommons.org/licenses/by-sa/4.0/" }
            },
            { "word": "", "meanings": [{ "definitions": ["x"] }] },
            { "word": "leer", "meanings": [] }
        ]});
        let entries = parse_entries(&answer, "en");
        assert_eq!(entries.len(), 1, "entries need a word and a definition");
        let entry = &entries[0];
        assert_eq!(entry.word, "haus");
        assert_eq!(entry.language, "de");
        assert_eq!(entry.pronunciations.len(), 2, "a label alone says nothing");
        assert_eq!(
            entry.pronunciations[1].audio_url, None,
            "only web recordings"
        );
        let noun = &entry.meanings[0];
        assert_eq!(noun.part_of_speech, "noun");
        assert_eq!(noun.definitions.len(), 2);
        assert_eq!(noun.definitions[0].text, "house building");
        assert_eq!(noun.definitions[0].examples, ["Das Haus ist alt."]);
        assert_eq!(noun.definitions[1].text, "home");
        assert_eq!(noun.synonyms, ["Gebäude"]);
        assert_eq!(noun.translations.len(), 1);
        assert_eq!(
            entry.meanings.len(),
            1,
            "a part of speech without definitions goes"
        );
        assert_eq!(entry.license.as_ref().unwrap().name, "CC BY-SA 4.0");
        assert!(parse_entries(&json!(null), "en").is_empty());
        assert!(parse_entries(&json!({ "entries": [] }), "en").is_empty());
    }

    #[test]
    fn a_form_names_its_lemma() {
        let answer = json!({ "entries": [{
            "word": "went",
            "formOf": { "lemma": "go", "note": "simple past of go" },
            "meanings": [{ "partOfSpeech": "verb", "definitions": ["simple past of go"] }]
        }]});
        let entry = &parse_entries(&answer, "en")[0];
        assert_eq!(entry.language, "en", "the language asked for");
        let form = entry.form_of.as_ref().unwrap();
        assert_eq!((form.form.as_str(), form.lemma.as_str()), ("went", "go"));
    }
}
