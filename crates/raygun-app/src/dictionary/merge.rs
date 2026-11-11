//! What several dictionaries said about one word, as one answer: in the order of the user's
//! sources, senses grouped by part of speech, a sense two sources share (the Free Dictionary's
//! text is Wiktionary's) shown once, and each source that added something credited.

use raygun_model::{
    DictionaryCredit, DictionaryDefinition, DictionaryEntry, DictionaryMeaning,
    DictionaryPronunciation, DictionaryTranslation,
};

/// A source's id and name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRef {
    pub id: String,
    pub name: String,
}

/// Senses kept per part of speech, and pronunciations, at most.
const DEFINITIONS: usize = 40;
const PRONUNCIATIONS: usize = 8;
const WORDS: usize = 16;

/// A sense's text as compared: letters and digits in lower case, single spaces.
fn folded(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_lowercase().next().unwrap_or(c)
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether two senses say the same: the same words, or one the start of the other (a source
/// that cut a sense short, or added a clause).
pub fn same_sense(a: &str, b: &str) -> bool {
    let (a, b) = (folded(a), folded(b));
    if a == b {
        return true;
    }
    let (short, long) = if a.len() <= b.len() {
        (&a, &b)
    } else {
        (&b, &a)
    };
    short.len() >= 30 && long.starts_with(short.as_str())
}

fn add_words(list: &mut Vec<String>, words: &[String]) {
    for word in words {
        if list.len() >= WORDS {
            break;
        }
        if !list.iter().any(|known| known.eq_ignore_ascii_case(word)) {
            list.push(word.clone());
        }
    }
}

fn add_translations(list: &mut Vec<DictionaryTranslation>, more: &[DictionaryTranslation]) {
    for translation in more {
        match list.iter_mut().find(|known| {
            known.gloss == translation.gloss && known.language == translation.language
        }) {
            Some(known) => add_words(&mut known.terms, &translation.terms),
            None => list.push(translation.clone()),
        }
    }
}

fn add_definitions(list: &mut Vec<DictionaryDefinition>, more: &[DictionaryDefinition]) {
    for definition in more {
        let same = list
            .iter()
            .position(|known| same_sense(&known.text, &definition.text));
        match same {
            // The same sense: examples it lacked come along.
            Some(place) => {
                if list[place].examples.is_empty() {
                    list[place].examples = definition.examples.clone();
                }
            }
            None if list.len() < DEFINITIONS => list.push(definition.clone()),
            None => {}
        }
    }
}

fn add_pronunciations(list: &mut Vec<DictionaryPronunciation>, more: &[DictionaryPronunciation]) {
    for pronunciation in more {
        if list.len() >= PRONUNCIATIONS {
            break;
        }
        let known = list.iter().any(|known| match &pronunciation.audio_url {
            Some(audio) => known.audio_url.as_ref() == Some(audio),
            None => known.audio_url.is_none() && known.ipa == pronunciation.ipa,
        });
        if !known {
            list.push(pronunciation.clone());
        }
    }
}

/// One answer's parts, merged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Merged {
    pub word: String,
    pub language_name: Option<String>,
    pub pronunciations: Vec<DictionaryPronunciation>,
    pub meanings: Vec<DictionaryMeaning>,
    pub etymology: Option<String>,
    pub credits: Vec<DictionaryCredit>,
}

/// Merges entries, first source first.
pub fn merge(entries: &[(SourceRef, DictionaryEntry)]) -> Merged {
    let mut merged = Merged::default();
    for (source, entry) in entries {
        if merged.word.is_empty() {
            merged.word = entry.word.clone();
        }
        if merged.language_name.is_none() {
            merged.language_name = entry.language_name.clone();
        }
        if merged.etymology.is_none() {
            merged.etymology = entry.etymology.clone().filter(|text| !text.is_empty());
        }
        add_pronunciations(&mut merged.pronunciations, &entry.pronunciations);
        for meaning in &entry.meanings {
            match merged
                .meanings
                .iter_mut()
                .find(|known| known.part_of_speech == meaning.part_of_speech)
            {
                Some(known) => {
                    add_definitions(&mut known.definitions, &meaning.definitions);
                    add_words(&mut known.synonyms, &meaning.synonyms);
                    add_words(&mut known.antonyms, &meaning.antonyms);
                    add_translations(&mut known.translations, &meaning.translations);
                }
                None => {
                    let mut fresh = meaning.clone();
                    fresh.definitions.truncate(DEFINITIONS);
                    fresh.synonyms.truncate(WORDS);
                    fresh.antonyms.truncate(WORDS);
                    merged.meanings.push(fresh);
                }
            }
        }
        if !merged
            .credits
            .iter()
            .any(|credit| credit.source == source.id)
        {
            merged.credits.push(DictionaryCredit {
                source: source.id.clone(),
                name: source.name.clone(),
                url: entry.url.clone(),
                license: entry.license.clone(),
            });
        }
    }
    merged
}
