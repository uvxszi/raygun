//! Wiktionary (CC BY-SA), through the English Wiktionary: it describes words of every language
//! in English, each language a section of the word's page. Two requests, both answered from
//! Wikimedia's caches: the REST API's definitions (`/api/rest_v1/page/definition/<word>`: senses
//! by language and part of speech, with examples), then, once the word is known, the page itself
//! as Parsoid HTML (`/w/rest.php/v1/page/<word>/html`) for what the definitions leave out: the
//! pronunciation (IPA and recordings), the etymology, synonyms and antonyms, and translations
//! into the reader's language. Other Wiktionaries (fr.wiktionary.org…) have no definitions API
//! (they answer 501), and each lays its pages out its own way.

use raygun_model::{
    DictionaryDefinition, DictionaryEntry, DictionaryFormOf, DictionaryLicense, DictionaryMeaning,
    DictionaryPronunciation, DictionaryTranslation,
};
use scraper::{ElementRef, Html, Node};
use serde_json::Value;

use super::polite::{Fetched, Polite};

pub const NAME: &str = "Wiktionary";
const DEFINITIONS: &str = "https://en.wiktionary.org/api/rest_v1/page/definition/";
const PAGES: &str = "https://en.wiktionary.org/w/rest.php/v1/page/";
const WIKI: &str = "https://en.wiktionary.org/wiki/";
const LICENSE_URL: &str = "https://creativecommons.org/licenses/by-sa/4.0/";

/// Examples kept per sense, and what a page part may hold.
const EXAMPLES: usize = 2;
const PRONUNCIATIONS: usize = 6;
const NYMS: usize = 12;
const GLOSSES: usize = 6;
const TERMS: usize = 8;
const ETYMOLOGY_CHARS: usize = 1200;

pub fn license() -> DictionaryLicense {
    DictionaryLicense {
        name: "CC BY-SA 4.0".to_owned(),
        url: Some(LICENSE_URL.to_owned()),
    }
}

/// A page title in an address: spaces as underscores, the rest escaped.
fn title(word: &str) -> String {
    let underscored = word.trim().replace(' ', "_");
    url::form_urlencoded::byte_serialize(underscored.as_bytes())
        .collect::<String>()
        // Titles keep their own underscores; `+` would be read as a plus sign.
        .replace('+', "%20")
}

/// The word's page, at its language's section when that's known.
pub fn page_url(word: &str, language_name: Option<&str>) -> String {
    let anchor = language_name
        .map(|name| format!("#{}", name.replace(' ', "_")))
        .unwrap_or_default();
    format!("{WIKI}{}{anchor}", title(word))
}

/// Wiktionary's search for `query`: the page when there is one, else the pages that use it.
pub fn search_url(query: &str) -> String {
    let query: String = url::form_urlencoded::byte_serialize(query.trim().as_bytes()).collect();
    format!("https://en.wiktionary.org/w/index.php?search={query}")
}

/// The language codes Wiktionary may file a language's words under, the likeliest first.
pub fn section_codes(language: &str) -> Vec<String> {
    let codes: &[&str] = match language {
        "sr" | "hr" | "bs" | "sh" => &["sh"],
        "nb" | "no" => &["nb", "no"],
        "nn" => &["nn", "no"],
        "fil" | "tl" => &["tl"],
        "iw" | "he" => &["he"],
        "in" | "id" => &["id"],
        "ji" | "yi" => &["yi"],
        "cmn" | "yue" | "zh" => &["zh"],
        _ => &[],
    };
    let mut found: Vec<String> = codes.iter().map(|&code| code.to_owned()).collect();
    if !found.iter().any(|code| code == language) {
        found.push(language.to_owned());
    }
    found
}

/// The codes a translation table writes a language's terms under.
fn translation_codes(language: &str) -> Vec<String> {
    let codes: &[&str] = match language {
        "zh" => &["cmn", "zh"],
        "sr" | "hr" | "bs" => &["sh"],
        "nb" | "no" => &["nb", "no"],
        "fil" | "tl" => &["tl"],
        _ => &[],
    };
    let mut found: Vec<String> = codes.iter().map(|&code| code.to_owned()).collect();
    if !found.iter().any(|code| code == language) {
        found.push(language.to_owned());
    }
    found
}

fn has_class(element: &scraper::node::Element, class: &str) -> bool {
    element
        .attr("class")
        .is_some_and(|classes| classes.split_whitespace().any(|name| name == class))
}

fn children(element: ElementRef<'_>) -> impl Iterator<Item = ElementRef<'_>> {
    element.children().filter_map(ElementRef::wrap)
}

/// An element's text with its spaces run together, leaving out the elements `skip` says.
fn text_without(element: ElementRef<'_>, skip: &dyn Fn(&scraper::node::Element) -> bool) -> String {
    fn collect(
        element: ElementRef<'_>,
        skip: &dyn Fn(&scraper::node::Element) -> bool,
        out: &mut String,
    ) {
        for child in element.children() {
            match child.value() {
                Node::Text(text) => out.push_str(text),
                Node::Element(inner) if !skip(inner) => {
                    if let Some(inner) = ElementRef::wrap(child) {
                        collect(inner, skip, out);
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = String::new();
    collect(element, skip, &mut out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// What never reads as text: styles, scripts, links to categories.
fn hidden(element: &scraper::node::Element) -> bool {
    matches!(element.name(), "style" | "script" | "link" | "meta")
}

/// A sense's own text: the senses under it (listed again on their own), quotations and
/// citations left out.
fn sense_text(element: ElementRef<'_>) -> String {
    let text = text_without(element, &|inner| {
        hidden(inner) || matches!(inner.name(), "ol" | "ul" | "dl")
    });
    text.trim_end_matches(':').trim().to_owned()
}

fn html_text(html: &str) -> String {
    let fragment = Html::parse_fragment(html);
    text_without(fragment.root_element(), &hidden)
}

/// A sense's text, and the word it's a form of when it says so (`simple past of go`).
fn sense(html: &str) -> (String, Option<String>) {
    let fragment = Html::parse_fragment(html);
    let root = fragment.root_element();
    let lemma = root
        .descendants()
        .filter_map(ElementRef::wrap)
        .find(|element| has_class(element.value(), "form-of-definition-link"))
        .map(|link| text_without(link, &hidden))
        .filter(|lemma| !lemma.is_empty());
    (sense_text(root), lemma)
}

/// One sense's examples: each with its translation after a dash when it's in another language.
fn examples(definition: &Value) -> Vec<String> {
    let parsed = definition
        .get("parsedExamples")
        .and_then(Value::as_array)
        .filter(|examples| !examples.is_empty());
    let found: Vec<String> = match parsed {
        Some(examples) => examples
            .iter()
            .filter_map(|example| {
                let text = html_text(example.get("example")?.as_str()?);
                let translation = example
                    .get("translation")
                    .and_then(Value::as_str)
                    .map(html_text)
                    .filter(|translation| !translation.is_empty());
                Some(match translation {
                    Some(translation) => format!("{text} — {translation}"),
                    None => text,
                })
            })
            .collect(),
        None => definition
            .get("examples")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|example| example.as_str().map(html_text))
            .collect(),
    };
    found
        .into_iter()
        .filter(|example| !example.is_empty())
        .take(EXAMPLES)
        .collect()
}

/// What the definitions API says about `word` in `language`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definitions {
    /// The section's code the senses were filed under (`sh` for Serbian).
    pub code: String,
    pub language_name: Option<String>,
    pub meanings: Vec<DictionaryMeaning>,
    pub form_of: Option<DictionaryFormOf>,
}

/// The senses the definitions API gives `word` in `language`; none when it has no section for
/// the language.
pub fn parse_definitions(answer: &Value, word: &str, language: &str) -> Option<Definitions> {
    let sections = answer.as_object()?;
    let (code, entries) = section_codes(language)
        .into_iter()
        .find_map(|code| Some((code.clone(), sections.get(&code)?.as_array()?)))?;
    let mut language_name = None;
    let mut meanings: Vec<DictionaryMeaning> = Vec::new();
    let (mut senses, mut forms) = (0usize, 0usize);
    let mut first_form: Option<(String, String)> = None;
    for entry in entries {
        if language_name.is_none() {
            language_name = entry
                .get("language")
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
        let part_of_speech = entry
            .get("partOfSpeech")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        let mut definitions = Vec::new();
        for definition in entry
            .get("definitions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(html) = definition.get("definition").and_then(Value::as_str) else {
                continue;
            };
            let (text, lemma) = sense(html);
            if text.is_empty() {
                continue;
            }
            senses += 1;
            if let Some(lemma) = lemma {
                forms += 1;
                first_form.get_or_insert((lemma, text.clone()));
            }
            definitions.push(DictionaryDefinition {
                text,
                examples: examples(definition),
            });
        }
        if definitions.is_empty() {
            continue;
        }
        match meanings
            .iter_mut()
            .find(|meaning| meaning.part_of_speech == part_of_speech)
        {
            Some(meaning) => meaning.definitions.extend(definitions),
            None => meanings.push(DictionaryMeaning {
                part_of_speech,
                definitions,
                synonyms: Vec::new(),
                antonyms: Vec::new(),
                translations: Vec::new(),
            }),
        }
    }
    if meanings.is_empty() {
        return None;
    }
    let form_of = first_form
        .filter(|(lemma, _)| lemma != word)
        .map(|(lemma, note)| DictionaryFormOf {
            form: word.to_owned(),
            lemma,
            note,
            only: forms == senses,
        });
    Some(Definitions {
        code,
        language_name,
        meanings,
        form_of,
    })
}

/// What a part of speech's section adds to its senses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartExtras {
    pub part_of_speech: String,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
    pub translations: Vec<DictionaryTranslation>,
}

/// What a word's page has beyond its senses, for one language.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageParts {
    pub pronunciations: Vec<DictionaryPronunciation>,
    pub etymology: Option<String>,
    pub parts: Vec<PartExtras>,
}

/// A section's heading: its level (2 for `h2`) and text. Newer HTML wraps headings in a
/// `<div class="mw-heading">`.
fn heading_of(section: ElementRef<'_>) -> Option<(u8, String)> {
    let first = children(section).next()?;
    let heading = if has_class(first.value(), "mw-heading") {
        children(first).next()?
    } else {
        first
    };
    let level = match heading.value().name() {
        "h2" => 2,
        "h3" => 3,
        "h4" => 4,
        "h5" => 5,
        "h6" => 6,
        _ => return None,
    };
    Some((level, text_without(heading, &hidden)))
}

fn heading(section: ElementRef<'_>) -> Option<String> {
    heading_of(section).map(|(_, text)| text)
}

/// Sections under a language that aren't a part of speech.
const NOT_PARTS: [&str; 30] = [
    "etymology",
    "pronunciation",
    "alternative forms",
    "usage notes",
    "synonyms",
    "antonyms",
    "hypernyms",
    "hyponyms",
    "meronyms",
    "holonyms",
    "coordinate terms",
    "derived terms",
    "related terms",
    "descendants",
    "translations",
    "references",
    "further reading",
    "anagrams",
    "see also",
    "collocations",
    "conjugation",
    "declension",
    "inflection",
    "mutation",
    "quotations",
    "statistics",
    "trivia",
    "gallery",
    "notes",
    "compounds",
];

fn is_part(heading: &str) -> bool {
    let lower = heading.to_lowercase();
    !NOT_PARTS.iter().any(|name| lower.starts_with(name))
}

/// Terms in `lang` under `element`: `<span lang>` with the word inside (not one inside a link,
/// such as "Thesaurus:happy").
fn terms_in(element: ElementRef<'_>, codes: &[String], out: &mut Vec<String>, limit: usize) {
    fn walk(element: ElementRef<'_>, codes: &[String], in_link: bool, out: &mut Vec<String>) {
        for child in children(element) {
            let value = child.value();
            if hidden(value) || matches!(value.name(), "section") {
                continue;
            }
            let in_link = in_link || value.name() == "a";
            let lang = value.attr("lang").unwrap_or_default();
            if !in_link && codes.iter().any(|code| code == lang) {
                let term = text_without(child, &hidden);
                if !term.is_empty() && !out.contains(&term) {
                    out.push(term);
                }
                continue;
            }
            walk(child, codes, in_link, out);
        }
    }
    walk(element, codes, false, out);
    out.truncate(limit);
}

/// Elements under `element` (not under a section within it) that `pick` takes.
fn find_all<'a>(
    element: ElementRef<'a>,
    pick: &dyn Fn(ElementRef<'a>) -> bool,
    out: &mut Vec<ElementRef<'a>>,
) {
    for child in children(element) {
        if child.value().name() == "section" {
            continue;
        }
        if pick(child) {
            out.push(child);
        } else {
            find_all(child, pick, out);
        }
    }
}

/// `(Received Pronunciation, General American)` → `Received Pronunciation, General American`.
fn label_text(text: &str) -> Option<String> {
    let text = text
        .trim()
        .trim_start_matches("Audio")
        .trim()
        .trim_end_matches(':')
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    (!text.is_empty()).then(|| text.chars().take(80).collect())
}

/// An item's own elements that `pick` takes, not those of the items listed under it.
fn own_parts<'a>(
    item: ElementRef<'a>,
    pick: &dyn Fn(ElementRef<'a>) -> bool,
) -> Vec<ElementRef<'a>> {
    fn walk<'a>(
        element: ElementRef<'a>,
        pick: &dyn Fn(ElementRef<'a>) -> bool,
        out: &mut Vec<ElementRef<'a>>,
    ) {
        for child in children(element) {
            if matches!(child.value().name(), "ul" | "ol" | "dl" | "section") {
                continue;
            }
            if pick(child) {
                out.push(child);
            } else {
                walk(child, pick, out);
            }
        }
    }
    let mut found = Vec::new();
    walk(item, pick, &mut found);
    found
}

/// Every list item under `element`, items inside items included (an audio row under its
/// transcription), not those of sections within it.
fn list_items<'a>(element: ElementRef<'a>, out: &mut Vec<ElementRef<'a>>) {
    for child in children(element) {
        match child.value().name() {
            "section" => {}
            "li" => {
                out.push(child);
                list_items(child, out);
            }
            _ => list_items(child, out),
        }
    }
}

fn pronunciations_in(section: ElementRef<'_>, out: &mut Vec<DictionaryPronunciation>) {
    let mut items = Vec::new();
    list_items(section, &mut items);
    for item in items {
        let label = own_parts(item, &|element| {
            has_class(element.value(), "usage-label-accent")
                || has_class(element.value(), "qualifier-content")
        })
        .first()
        .and_then(|label| label_text(&text_without(*label, &hidden)));
        for ipa in own_parts(item, &|element| has_class(element.value(), "IPA")) {
            let ipa = text_without(ipa, &hidden);
            if (ipa.starts_with('/') || ipa.starts_with('['))
                && !out
                    .iter()
                    .any(|known| known.ipa.as_deref() == Some(ipa.as_str()))
            {
                out.push(DictionaryPronunciation {
                    ipa: Some(ipa),
                    label: label.clone(),
                    audio_url: None,
                });
            }
        }
        for audio in own_parts(item, &|element| element.value().name() == "audio") {
            let sources: Vec<(&str, &str)> = children(audio)
                .filter(|child| child.value().name() == "source")
                .filter_map(|source| {
                    Some((
                        source.value().attr("src")?,
                        source.value().attr("type").unwrap_or_default(),
                    ))
                })
                .collect();
            let Some(src) = sources
                .iter()
                .find(|(_, kind)| kind.starts_with("audio/mpeg"))
                .or_else(|| sources.first())
                .map(|(src, _)| *src)
            else {
                continue;
            };
            let url = if src.starts_with("//") {
                format!("https:{src}")
            } else {
                src.to_owned()
            };
            if !url.starts_with("https://")
                || out
                    .iter()
                    .any(|known| known.audio_url.as_ref() == Some(&url))
            {
                continue;
            }
            // The audio table's first cell names the accent: "Audio (US):".
            let row_label = audio
                .ancestors()
                .filter_map(ElementRef::wrap)
                .find(|ancestor| ancestor.value().name() == "tr")
                .and_then(|row| children(row).next())
                .and_then(|cell| label_text(&text_without(cell, &hidden)));
            out.push(DictionaryPronunciation {
                ipa: None,
                label: row_label.or_else(|| label.clone()),
                audio_url: Some(url),
            });
        }
    }
    out.truncate(PRONUNCIATIONS);
}

fn etymology_in(section: ElementRef<'_>) -> Option<String> {
    let text = children(section)
        .filter(|child| child.value().name() == "p")
        .map(|paragraph| text_without(paragraph, &hidden))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        return None;
    }
    let mut kept: String = text.chars().take(ETYMOLOGY_CHARS).collect();
    if kept.len() < text.len() {
        kept.push('…');
    }
    Some(kept)
}

fn translations_in(
    section: ElementRef<'_>,
    target: &str,
    codes: &[String],
    out: &mut Vec<DictionaryTranslation>,
) {
    let mut tables = Vec::new();
    find_all(
        section,
        &|element| element.value().name() == "table" && has_class(element.value(), "translations"),
        &mut tables,
    );
    for table in tables {
        let gloss = table
            .value()
            .attr("data-gloss")
            .map(|gloss| gloss.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|gloss| !gloss.is_empty());
        if gloss.as_deref().is_some_and(|gloss| {
            gloss
                .to_lowercase()
                .starts_with("translations to be checked")
        }) {
            continue;
        }
        let mut terms = Vec::new();
        terms_in(table, codes, &mut terms, TERMS);
        if terms.is_empty() {
            continue;
        }
        out.push(DictionaryTranslation {
            gloss,
            language: target.to_owned(),
            terms,
        });
        if out.len() >= GLOSSES {
            break;
        }
    }
}

fn part_extras(
    section: ElementRef<'_>,
    part_of_speech: String,
    word_codes: &[String],
    target: Option<&str>,
) -> PartExtras {
    let mut extras = PartExtras {
        part_of_speech,
        ..PartExtras::default()
    };
    // Synonyms and antonyms under the senses: `<span class="nyms synonym">`.
    let mut nyms = Vec::new();
    find_all(
        section,
        &|element| has_class(element.value(), "nyms"),
        &mut nyms,
    );
    for nym in nyms {
        if has_class(nym.value(), "synonym") {
            terms_in(nym, word_codes, &mut extras.synonyms, NYMS);
        } else if has_class(nym.value(), "antonym") {
            terms_in(nym, word_codes, &mut extras.antonyms, NYMS);
        }
    }
    for inner in children(section).filter(|child| child.value().name() == "section") {
        let name = heading(inner).unwrap_or_default().to_lowercase();
        if name.starts_with("synonyms") {
            terms_in(inner, word_codes, &mut extras.synonyms, NYMS);
        } else if name.starts_with("antonyms") {
            terms_in(inner, word_codes, &mut extras.antonyms, NYMS);
        } else if name.starts_with("translations")
            && let Some(target) = target
        {
            translations_in(
                inner,
                target,
                &translation_codes(target),
                &mut extras.translations,
            );
        }
    }
    extras
}

fn read_sections(
    section: ElementRef<'_>,
    word_codes: &[String],
    target: Option<&str>,
    parts: &mut PageParts,
) {
    for inner in children(section).filter(|child| child.value().name() == "section") {
        let name = heading(inner).unwrap_or_default();
        let lower = name.to_lowercase();
        if lower.starts_with("etymology") {
            if parts.etymology.is_none() {
                parts.etymology = etymology_in(inner);
            }
            read_sections(inner, word_codes, target, parts);
        } else if lower.starts_with("pronunciation") {
            pronunciations_in(inner, &mut parts.pronunciations);
            read_sections(inner, word_codes, target, parts);
        } else if is_part(&name) {
            parts
                .parts
                .push(part_extras(inner, lower, word_codes, target));
        }
    }
}

/// The parts of a word's page (Parsoid HTML) under `language_name` (`English`): the words' own
/// language's terms are filed under `word_code`; translations are read into `target` when given.
pub fn parse_page(
    html: &str,
    language_name: &str,
    word_code: &str,
    target: Option<&str>,
) -> PageParts {
    let document = Html::parse_document(html);
    let wanted = language_name.trim().to_lowercase();
    let Some(section) = document
        .root_element()
        .descendants()
        .filter_map(ElementRef::wrap)
        .filter(|element| element.value().name() == "section")
        .find(|section| {
            heading_of(*section)
                .is_some_and(|(level, name)| level == 2 && name.trim().to_lowercase() == wanted)
        })
    else {
        return PageParts::default();
    };
    let mut parts = PageParts::default();
    read_sections(section, &[word_code.to_owned()], target, &mut parts);
    parts
}

/// Adds a page's parts to the senses: each part of speech's synonyms, antonyms and translations
/// go to the senses of the same name.
pub fn with_page(mut meanings: Vec<DictionaryMeaning>, page: &PageParts) -> Vec<DictionaryMeaning> {
    for extras in &page.parts {
        let Some(meaning) = meanings
            .iter_mut()
            .find(|meaning| meaning.part_of_speech == extras.part_of_speech)
        else {
            continue;
        };
        for synonym in &extras.synonyms {
            if !meaning.synonyms.contains(synonym) {
                meaning.synonyms.push(synonym.clone());
            }
        }
        for antonym in &extras.antonyms {
            if !meaning.antonyms.contains(antonym) {
                meaning.antonyms.push(antonym.clone());
            }
        }
        meaning
            .translations
            .extend(extras.translations.iter().cloned());
    }
    meanings
}

/// What Wiktionary says about `word` in `language`, with translations into `target` when it's
/// another language; `Ok(None)` when it doesn't know the word in that language.
pub async fn look_up(
    polite: &Polite,
    word: &str,
    language: &str,
    target: Option<&str>,
) -> Result<Option<DictionaryEntry>, String> {
    let url = format!("{DEFINITIONS}{}", title(word));
    let answer = match polite.get(&url, "application/json", NAME).await {
        Fetched::Body(body) => serde_json::from_str::<Value>(&body)
            .map_err(|_| format!("{NAME}'s answer couldn't be read"))?,
        Fetched::NotFound => return Ok(None),
        Fetched::Failed(issue) => return Err(issue),
    };
    let Some(definitions) = parse_definitions(&answer, word, language) else {
        return Ok(None);
    };
    // The page adds what the definitions leave out; without it, the senses still show.
    let page = match &definitions.language_name {
        Some(name) => {
            let url = format!("{PAGES}{}/html", title(word));
            match polite.get(&url, "text/html", NAME).await {
                Fetched::Body(html) => {
                    let name = name.clone();
                    let code = definitions.code.clone();
                    let target = target.map(str::to_owned);
                    tauri::async_runtime::spawn_blocking(move || {
                        parse_page(&html, &name, &code, target.as_deref())
                    })
                    .await
                    .unwrap_or_default()
                }
                Fetched::NotFound | Fetched::Failed(_) => PageParts::default(),
            }
        }
        None => PageParts::default(),
    };
    Ok(Some(DictionaryEntry {
        word: word.to_owned(),
        language: language.to_owned(),
        url: Some(page_url(word, definitions.language_name.as_deref())),
        language_name: definitions.language_name,
        pronunciations: page.pronunciations.clone(),
        meanings: with_page(definitions.meanings, &page),
        etymology: page.etymology.clone(),
        form_of: definitions.form_of,
        license: Some(license()),
    }))
}
