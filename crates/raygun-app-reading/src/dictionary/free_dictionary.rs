//! The Free Dictionary API (dictionaryapi.dev): English words from Wiktionary's text (CC BY-SA),
//! with recordings from Wikimedia Commons, keyless. It answers other languages with 404, and now
//! and then with a 502 that a second ask a moment later doesn't get.

use std::time::Duration;

use raygun_model::{
    DictionaryDefinition, DictionaryEntry, DictionaryLicense, DictionaryMeaning,
    DictionaryPronunciation,
};
use serde_json::Value;

use super::polite::{Fetched, Polite};

pub const NAME: &str = "Free Dictionary API";
const ENTRIES: &str = "https://api.dictionaryapi.dev/api/v2/entries/en/";
/// The only language it knows.
pub const LANGUAGE: &str = "en";
/// How long a second ask waits after a gateway error.
const RETRY: Duration = Duration::from_millis(600);

fn text(value: Option<&Value>) -> Option<String> {
    let text = value?.as_str()?;
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!flat.is_empty()).then_some(flat)
}

fn strings(value: Option<&Value>) -> impl Iterator<Item = String> + '_ {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| text(Some(item)))
}

fn add_new(list: &mut Vec<String>, items: impl Iterator<Item = String>) {
    for item in items {
        if !list.contains(&item) {
            list.push(item);
        }
    }
}

/// The accent a recording's file name gives: `hello-uk.mp3` → `UK`.
fn accent_of(audio: &str) -> Option<String> {
    let name = audio.rsplit('/').next()?.rsplit_once('.')?.0;
    let accent = name.rsplit_once('-')?.1;
    Some(
        match accent {
            "uk" => "UK",
            "us" => "US",
            "au" => "Australia",
            "ca" => "Canada",
            "ie" => "Ireland",
            "nz" => "New Zealand",
            "in" => "India",
            _ => return None,
        }
        .to_owned(),
    )
}

/// The API's answer (a list of entries for one word, one per etymology) as one entry.
pub fn parse(answer: &Value, word: &str) -> Option<DictionaryEntry> {
    let entries = answer.as_array()?;
    let first = entries.first()?;
    let mut pronunciations: Vec<DictionaryPronunciation> = Vec::new();
    let mut meanings: Vec<DictionaryMeaning> = Vec::new();
    let mut etymology = None;
    for entry in entries {
        let phonetics = entry
            .get("phonetics")
            .and_then(Value::as_array)
            .into_iter()
            .flatten();
        for phonetic in phonetics {
            let ipa = text(phonetic.get("text"));
            let audio_url = text(phonetic.get("audio")).filter(|url| url.starts_with("https://"));
            if ipa.is_none() && audio_url.is_none() {
                continue;
            }
            let label = audio_url.as_deref().and_then(accent_of);
            let known = pronunciations.iter().any(|known| {
                (audio_url.is_some() && known.audio_url == audio_url)
                    || (audio_url.is_none() && known.ipa == ipa)
            });
            if !known {
                pronunciations.push(DictionaryPronunciation {
                    ipa,
                    label,
                    audio_url,
                });
            }
        }
        if let Some(ipa) = text(entry.get("phonetic"))
            && !pronunciations
                .iter()
                .any(|known| known.ipa.as_deref() == Some(ipa.as_str()))
        {
            pronunciations.push(DictionaryPronunciation {
                ipa: Some(ipa),
                label: None,
                audio_url: None,
            });
        }
        if etymology.is_none() {
            etymology = text(entry.get("origin"));
        }
        let senses = entry
            .get("meanings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten();
        for sense in senses {
            let part_of_speech = text(sense.get("partOfSpeech"))
                .unwrap_or_default()
                .to_lowercase();
            let mut definitions = Vec::new();
            let mut synonyms: Vec<String> = Vec::new();
            let mut antonyms: Vec<String> = Vec::new();
            add_new(&mut synonyms, strings(sense.get("synonyms")));
            add_new(&mut antonyms, strings(sense.get("antonyms")));
            for definition in sense
                .get("definitions")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(text_value) = text(definition.get("definition")) else {
                    continue;
                };
                add_new(&mut synonyms, strings(definition.get("synonyms")));
                add_new(&mut antonyms, strings(definition.get("antonyms")));
                definitions.push(DictionaryDefinition {
                    text: text_value,
                    examples: text(definition.get("example")).into_iter().collect(),
                });
            }
            if definitions.is_empty() {
                continue;
            }
            match meanings
                .iter_mut()
                .find(|meaning| meaning.part_of_speech == part_of_speech)
            {
                Some(meaning) => {
                    meaning.definitions.extend(definitions);
                    add_new(&mut meaning.synonyms, synonyms.into_iter());
                    add_new(&mut meaning.antonyms, antonyms.into_iter());
                }
                None => meanings.push(DictionaryMeaning {
                    part_of_speech,
                    definitions,
                    synonyms,
                    antonyms,
                    translations: Vec::new(),
                }),
            }
        }
    }
    if meanings.is_empty() {
        return None;
    }
    // Its words come from Wiktionary's pages, which are their full entries.
    let url = strings(first.get("sourceUrls")).find(|url| url.starts_with("https://"));
    let license = first.get("license").and_then(|license| {
        Some(DictionaryLicense {
            name: text(license.get("name"))?,
            url: text(license.get("url")).filter(|url| url.starts_with("https://")),
        })
    });
    Some(DictionaryEntry {
        word: text(first.get("word")).unwrap_or_else(|| word.to_owned()),
        language: LANGUAGE.to_owned(),
        language_name: Some("English".to_owned()),
        pronunciations,
        meanings,
        etymology,
        form_of: None,
        url,
        license,
    })
}

/// What the API says about an English `word`; `Ok(None)` when it doesn't know it.
pub async fn look_up(polite: &Polite, word: &str) -> Result<Option<DictionaryEntry>, String> {
    let path: String = url::form_urlencoded::byte_serialize(word.trim().as_bytes())
        .collect::<String>()
        .replace('+', "%20");
    let url = format!("{ENTRIES}{path}");
    let mut fetched = polite.get(&url, "application/json", NAME).await;
    if matches!(&fetched, Fetched::Failed(issue) if issue.contains("(502") || issue.contains("(504"))
    {
        tokio::time::sleep(RETRY).await;
        fetched = polite.get(&url, "application/json", NAME).await;
    }
    match fetched {
        Fetched::Body(body) => {
            let answer: Value = serde_json::from_str(&body)
                .map_err(|_| format!("{NAME}'s answer couldn't be read"))?;
            Ok(parse(&answer, word))
        }
        Fetched::NotFound => Ok(None),
        Fetched::Failed(issue) => Err(issue),
    }
}
