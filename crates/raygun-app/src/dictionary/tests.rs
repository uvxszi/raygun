use std::collections::HashMap;
use std::sync::Mutex;

use raygun_model::{
    DictionaryDefinition, DictionaryEntry, DictionaryFormOf, DictionaryMeaning, TextBlock,
};
use serde_json::Value;

use super::merge::{self, SourceRef};
use super::{Reply, book_search, cache, find, forms, free_dictionary, wiktionary};

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/dictionary/fixtures")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

fn json(name: &str) -> Value {
    serde_json::from_str(&fixture(name)).unwrap()
}

fn meaning(part: &str, senses: &[&str]) -> DictionaryMeaning {
    DictionaryMeaning {
        part_of_speech: part.to_owned(),
        definitions: senses
            .iter()
            .map(|text| DictionaryDefinition {
                text: (*text).to_owned(),
                examples: Vec::new(),
            })
            .collect(),
        synonyms: Vec::new(),
        antonyms: Vec::new(),
        translations: Vec::new(),
    }
}

fn entry(word: &str, meanings: Vec<DictionaryMeaning>) -> DictionaryEntry {
    DictionaryEntry {
        word: word.to_owned(),
        language: "en".to_owned(),
        language_name: Some("English".to_owned()),
        pronunciations: Vec::new(),
        meanings,
        etymology: None,
        form_of: None,
        url: Some(format!("https://example.org/{word}")),
        license: None,
    }
}

fn source(id: &str) -> SourceRef {
    SourceRef {
        id: id.to_owned(),
        name: id.to_uppercase(),
    }
}

// ---- What to look up ----

#[test]
fn a_selection_is_tidied_and_kept_short() {
    assert_eq!(forms::tidy("  “Running,”\n"), "Running");
    assert_eq!(forms::tidy("'happy'."), "happy");
    assert_eq!(forms::tidy("(in  a   row)"), "in a row");
    assert_eq!(forms::tidy("l’homme…"), "l’homme");
    assert!(forms::is_lookup("in a row"));
    assert!(!forms::is_lookup(""));
    assert!(!forms::is_lookup("1, 2"), "digits alone are no word");
    assert!(!forms::is_lookup(
        "a sentence of more than six words is no lookup"
    ));
}

#[test]
fn forms_are_tried_from_the_word_as_written() {
    assert_eq!(
        forms::candidates("Running", "en"),
        ["Running", "running", "run", "runn", "runne"]
    );
    assert_eq!(
        forms::candidates("studies", "en"),
        ["studies", "study", "studie"]
    );
    assert_eq!(forms::candidates("wolves", "en")[1..3], ["wolf", "wolfe"]);
    assert!(forms::candidates("happier", "en").contains(&"happy".to_owned()));
    assert!(forms::candidates("children's", "en").contains(&"children".to_owned()));
    assert!(forms::candidates("loved", "en").contains(&"love".to_owned()));
    // Only English loses its endings; phrases never do.
    assert_eq!(forms::candidates("Häuser", "de"), ["Häuser", "häuser"]);
    assert_eq!(forms::candidates("in a row", "en"), ["in a row"]);
    // French and Italian articles join the next word.
    assert_eq!(
        forms::candidates("L’Homme", "fr"),
        ["L’Homme", "l’homme", "Homme", "homme"]
    );
    assert_eq!(forms::candidates("dell'acqua", "it")[1], "acqua");
}

#[test]
fn the_language_is_the_settings_then_the_books_then_the_apps() {
    assert_eq!(
        forms::choose_language("maison", "fr-CA", Some("en"), "en"),
        "fr"
    );
    assert_eq!(forms::choose_language("maison", "", Some("fr"), "en"), "fr");
    assert_eq!(forms::choose_language("house", "", None, "pt-BR"), "pt");
    // Words in another script than the book's are looked up in a language of theirs.
    assert_eq!(forms::choose_language("Tokyo", "", Some("ja"), "de"), "de");
    assert_eq!(forms::choose_language("Tokyo", "", Some("ja"), "ja"), "en");
    assert_eq!(forms::choose_language("дом", "", Some("en"), "en"), "ru");
    assert_eq!(forms::choose_language("дом", "", Some("uk"), "en"), "uk");
    assert_eq!(forms::choose_language("ひらがな漢字", "", None, "en"), "ja");
    assert_eq!(
        forms::choose_language("漢字", "", Some("zh-Hant"), "en"),
        "zh"
    );
    assert_eq!(forms::choose_language("42", "", Some("und"), ""), "en");
}

// ---- Wiktionary ----

#[test]
fn wiktionary_definitions_are_read_for_the_words_language() {
    let maison =
        wiktionary::parse_definitions(&json("wiktionary_definition_maison.json"), "maison", "fr")
            .unwrap();
    assert_eq!(maison.code, "fr");
    assert_eq!(maison.language_name.as_deref(), Some("French"));
    let parts: Vec<&str> = maison
        .meanings
        .iter()
        .map(|meaning| meaning.part_of_speech.as_str())
        .collect();
    assert_eq!(parts, ["noun", "adjective"]);
    let home = &maison.meanings[0].definitions[1];
    assert_eq!(home.text, "home");
    assert_eq!(home.examples, ["Je rentre à la maison. — I'm going home."]);
    assert_eq!(maison.form_of, None);
    // No section for the language: nothing.
    assert!(
        wiktionary::parse_definitions(&json("wiktionary_definition_maison.json"), "maison", "de")
            .is_none()
    );
}

#[test]
fn wiktionary_senses_leave_out_their_sub_senses_and_empty_ones() {
    let building = wiktionary::parse_definitions(
        &json("wiktionary_definition_building.json"),
        "building",
        "en",
    )
    .unwrap();
    let noun = &building.meanings[1];
    let texts: Vec<&str> = noun
        .definitions
        .iter()
        .map(|definition| definition.text.as_str())
        .collect();
    assert_eq!(
        texts,
        [
            "A closed structure with walls and a roof.",
            "A sub-sense listed again on its own below.",
            "The act or process by which something is built."
        ]
    );
    assert_eq!(
        noun.definitions[0].examples,
        ["The building on the corner is a library."]
    );
    assert_eq!(
        noun.definitions[2].examples,
        ["The building of the bridge took two years."],
        "older answers' examples"
    );
    // A form with senses of its own: the lemma is offered, not shown instead.
    let form = building.form_of.unwrap();
    assert_eq!(form.lemma, "build");
    assert_eq!(form.note, "present participle and gerund of build");
    assert!(!form.only);
}

#[test]
fn a_word_that_is_only_a_form_names_its_lemma() {
    let ran = wiktionary::parse_definitions(&json("wiktionary_definition_ran.json"), "ran", "en")
        .unwrap();
    let form = ran.form_of.unwrap();
    assert_eq!(
        (form.form.as_str(), form.lemma.as_str(), form.only),
        ("ran", "run", true)
    );
    assert_eq!(form.note, "simple past of run");
    assert_eq!(
        ran.meanings[0].definitions[1].text,
        "(nonstandard) past participle of run"
    );
}

#[test]
fn wiktionary_codes_cover_languages_it_files_differently() {
    assert_eq!(wiktionary::section_codes("hr"), ["sh", "hr"]);
    assert_eq!(wiktionary::section_codes("nb"), ["nb", "no"]);
    assert_eq!(wiktionary::section_codes("fr"), ["fr"]);
}

#[test]
fn a_wiktionary_page_adds_sound_history_and_related_words() {
    let page = wiktionary::parse_page(
        &fixture("wiktionary_page_happy.html"),
        "English",
        "en",
        Some("fr"),
    );
    assert_eq!(page.pronunciations.len(), 2, "{:?}", page.pronunciations);
    let ipa = &page.pronunciations[0];
    assert_eq!(ipa.ipa.as_deref(), Some("/ˈhæpi/"));
    assert_eq!(
        ipa.label.as_deref(),
        Some("Received Pronunciation, General American")
    );
    let audio = &page.pronunciations[1];
    assert_eq!(
        audio.audio_url.as_deref(),
        Some(
            "https://upload.wikimedia.org/wikipedia/commons/transcoded/a/ab/En-us-happy.ogg/En-us-happy.ogg.mp3"
        ),
        "the MP3 copy"
    );
    assert_eq!(audio.label.as_deref(), Some("US"));
    assert_eq!(
        page.etymology.as_deref(),
        Some("From Middle English happy (“lucky”), from hap (“chance”) + -y."),
        "the text, not the tree"
    );
    let adjective = page
        .parts
        .iter()
        .find(|part| part.part_of_speech == "adjective")
        .unwrap();
    assert_eq!(adjective.synonyms, ["cheerful", "glad", "joyful"]);
    assert_eq!(adjective.antonyms, ["sad", "unhappy"]);
    assert_eq!(adjective.translations.len(), 2, "unchecked ones left out");
    assert_eq!(
        adjective.translations[0].gloss.as_deref(),
        Some("contented, joyous")
    );
    assert_eq!(adjective.translations[0].terms, ["heureux", "content"]);
    assert_eq!(adjective.translations[1].terms, ["chanceux"]);
    // Chinese translations are filed under Mandarin.
    let chinese = wiktionary::parse_page(
        &fixture("wiktionary_page_happy.html"),
        "English",
        "en",
        Some("zh"),
    );
    let adjective = &chinese.parts[0];
    assert_eq!(adjective.translations[0].terms, ["快樂"]);
    // Another language's section is its own.
    let german =
        wiktionary::parse_page(&fixture("wiktionary_page_happy.html"), "German", "de", None);
    assert_eq!(german.pronunciations[0].ipa.as_deref(), Some("/ˈhɛpi/"));
    assert_eq!(german.etymology, None);
    assert!(
        wiktionary::parse_page(&fixture("wiktionary_page_happy.html"), "Dutch", "nl", None)
            .pronunciations
            .is_empty()
    );
}

#[test]
fn page_parts_join_their_part_of_speech() {
    let page = wiktionary::parse_page(
        &fixture("wiktionary_page_happy.html"),
        "English",
        "en",
        Some("de"),
    );
    let meanings = wiktionary::with_page(
        vec![
            meaning("adjective", &["Feeling pleasure."]),
            meaning("noun", &["A happy event."]),
        ],
        &page,
    );
    assert_eq!(meanings[0].synonyms, ["cheerful", "glad", "joyful"]);
    assert_eq!(meanings[0].translations[0].terms, ["froh", "glücklich"]);
    assert_eq!(meanings[0].translations[0].language, "de");
    assert!(meanings[1].synonyms.is_empty());
}

#[test]
fn wiktionary_addresses() {
    assert_eq!(
        wiktionary::page_url("in a row", Some("English")),
        "https://en.wiktionary.org/wiki/in_a_row#English"
    );
    assert_eq!(
        wiktionary::page_url("Haus", Some("Norwegian Bokmål")),
        "https://en.wiktionary.org/wiki/Haus#Norwegian_Bokmål"
    );
    assert_eq!(
        wiktionary::page_url("fête", None),
        "https://en.wiktionary.org/wiki/f%C3%AAte"
    );
    assert_eq!(
        wiktionary::search_url("in a row"),
        "https://en.wiktionary.org/w/index.php?search=in+a+row"
    );
}

// ---- The Free Dictionary API ----

#[test]
fn free_dictionary_entries_become_one() {
    let hello = free_dictionary::parse(&json("free_dictionary_hello.json"), "hello").unwrap();
    assert_eq!(hello.word, "hello");
    assert_eq!(hello.language, "en");
    let sounds: Vec<(Option<&str>, Option<&str>, bool)> = hello
        .pronunciations
        .iter()
        .map(|sound| {
            (
                sound.ipa.as_deref(),
                sound.label.as_deref(),
                sound.audio_url.is_some(),
            )
        })
        .collect();
    assert_eq!(
        sounds,
        [
            (None, Some("Australia"), true),
            (Some("/həˈləʊ/"), Some("UK"), true),
            (Some("/həˈloʊ/"), None, false),
            (Some("/hɛˈləʊ/"), None, false),
        ]
    );
    let parts: Vec<&str> = hello
        .meanings
        .iter()
        .map(|meaning| meaning.part_of_speech.as_str())
        .collect();
    assert_eq!(parts, ["noun", "interjection"], "a part of speech once");
    assert_eq!(hello.meanings[0].definitions.len(), 2);
    let interjection = &hello.meanings[1];
    assert_eq!(interjection.definitions[0].examples, ["Hello, everyone."]);
    assert_eq!(interjection.synonyms, ["hi"]);
    assert_eq!(interjection.antonyms, ["bye", "goodbye"]);
    assert_eq!(
        hello.etymology.as_deref(),
        Some("A variant of an older greeting.")
    );
    assert_eq!(
        hello.url.as_deref(),
        Some("https://en.wiktionary.org/wiki/hello")
    );
    assert_eq!(hello.license.unwrap().name, "CC BY-SA 3.0");
    assert!(
        free_dictionary::parse(&serde_json::json!({ "title": "No Definitions Found" }), "x")
            .is_none()
    );
}

// ---- Merging ----

#[test]
fn sources_merge_by_part_of_speech_in_their_order() {
    let mut first = entry(
        "happy",
        vec![meaning("adjective", &["Feeling pleasure and contentment."])],
    );
    first.meanings[0].synonyms = vec!["glad".to_owned()];
    first.etymology = Some("From Middle English.".to_owned());
    let mut second = entry(
        "happy",
        vec![
            meaning(
                "adjective",
                &[
                    "feeling pleasure and contentment",
                    "Favoured by luck; fortunate in what happens to one.",
                ],
            ),
            meaning("verb", &["To become happy."]),
        ],
    );
    second.meanings[0].synonyms = vec!["Glad".to_owned(), "cheerful".to_owned()];
    second.meanings[0].definitions[0].examples = vec!["Music makes me happy.".to_owned()];
    second.etymology = Some("Another story.".to_owned());
    let merged = merge::merge(&[(source("wiktionary"), first), (source("free"), second)]);
    assert_eq!(merged.word, "happy");
    let adjective = &merged.meanings[0];
    assert_eq!(adjective.definitions.len(), 2, "a shared sense once");
    assert_eq!(
        adjective.definitions[0].examples,
        ["Music makes me happy."],
        "examples it lacked come along"
    );
    assert_eq!(adjective.synonyms, ["glad", "cheerful"]);
    assert_eq!(merged.meanings[1].part_of_speech, "verb");
    assert_eq!(merged.etymology.as_deref(), Some("From Middle English."));
    let credits: Vec<&str> = merged
        .credits
        .iter()
        .map(|credit| credit.source.as_str())
        .collect();
    assert_eq!(credits, ["wiktionary", "free"]);
    assert_eq!(
        merged.credits[1].url.as_deref(),
        Some("https://example.org/happy")
    );
}

#[test]
fn senses_count_as_the_same_when_one_starts_the_other() {
    assert!(merge::same_sense(
        "A closed structure with walls and a roof.",
        "a closed structure with walls and a roof"
    ));
    assert!(merge::same_sense(
        "To move swiftly upon two feet, faster than walking",
        "To move swiftly upon two feet, faster than walking; to run along."
    ));
    assert!(!merge::same_sense("home", "homemade"));
}

// ---- Finding a word ----

/// Answers from a table of words each source knows; records what was asked.
struct Fake {
    known: HashMap<(String, String), DictionaryEntry>,
    asked: Mutex<Vec<String>>,
    failing: Vec<String>,
}

impl Fake {
    fn new(known: Vec<(&str, DictionaryEntry)>) -> Self {
        Self {
            known: known
                .into_iter()
                .map(|(source, entry)| ((source.to_owned(), entry.word.clone()), entry))
                .collect(),
            asked: Mutex::new(Vec::new()),
            failing: Vec::new(),
        }
    }

    async fn ask(&self, source: SourceRef, word: String) -> Reply {
        self.asked
            .lock()
            .unwrap()
            .push(format!("{}:{word}", source.id));
        if self.failing.contains(&source.id) {
            return Reply::Failed(format!("{} didn't answer in time", source.name));
        }
        match self.known.get(&(source.id.clone(), word)) {
            Some(entry) => Reply::Found {
                entry: Box::new(entry.clone()),
                kept: false,
            },
            None => Reply::Missing,
        }
    }
}

#[tokio::test]
async fn a_word_not_known_as_written_is_found_in_another_form() {
    let fake = Fake::new(vec![(
        "w",
        entry("run", vec![meaning("verb", &["To move fast."])]),
    )]);
    let found = find(&[source("w")], "Running", "en", |source, word| {
        fake.ask(source, word)
    })
    .await;
    assert_eq!(found.word, "run");
    assert_eq!(found.entries.len(), 1);
    assert_eq!(
        *fake.asked.lock().unwrap(),
        ["w:Running", "w:running", "w:run"],
        "forms stop at the first one known"
    );
}

#[tokio::test]
async fn a_word_that_is_only_a_form_shows_its_lemma() {
    let mut ran = entry("ran", vec![meaning("verb", &["simple past of run"])]);
    ran.form_of = Some(DictionaryFormOf {
        form: "ran".to_owned(),
        lemma: "run".to_owned(),
        note: "simple past of run".to_owned(),
        only: true,
    });
    let run = entry("run", vec![meaning("verb", &["To move fast."])]);
    let fake = Fake::new(vec![
        ("w", ran.clone()),
        ("w", run.clone()),
        ("f", run.clone()),
    ]);
    let found = find(&[source("w"), source("f")], "ran", "en", |source, word| {
        fake.ask(source, word)
    })
    .await;
    assert_eq!(found.word, "run");
    assert_eq!(found.entries.len(), 2, "every source asked for the lemma");
    assert_eq!(found.form_of.unwrap().form, "ran");
    // A form with senses of its own stays itself, the lemma offered.
    let mut building = entry("building", vec![meaning("noun", &["A structure."])]);
    building.form_of = Some(DictionaryFormOf {
        form: "building".to_owned(),
        lemma: "build".to_owned(),
        note: "present participle of build".to_owned(),
        only: false,
    });
    let fake = Fake::new(vec![("w", building)]);
    let found = find(&[source("w")], "building", "en", |source, word| {
        fake.ask(source, word)
    })
    .await;
    assert_eq!(found.word, "building");
    assert_eq!(found.form_of.unwrap().lemma, "build");
}

#[tokio::test]
async fn sources_that_fail_are_named_and_no_other_form_is_tried() {
    let mut fake = Fake::new(Vec::new());
    fake.failing = vec!["w".to_owned()];
    let found = find(&[source("w")], "Running", "en", |source, word| {
        fake.ask(source, word)
    })
    .await;
    assert!(found.entries.is_empty());
    assert_eq!(found.issues, ["W didn't answer in time"]);
    assert_eq!(*fake.asked.lock().unwrap(), ["w:Running"]);
    // One source failing doesn't hide another's answer.
    let mut fake = Fake::new(vec![(
        "f",
        entry("happy", vec![meaning("adjective", &["Glad."])]),
    )]);
    fake.failing = vec!["w".to_owned()];
    let found = find(
        &[source("w"), source("f")],
        "happy",
        "en",
        |source, word| fake.ask(source, word),
    )
    .await;
    assert_eq!(found.entries.len(), 1);
    assert_eq!(found.issues.len(), 1);
}

// ---- Kept answers ----

#[tokio::test]
async fn answers_are_kept_and_age() {
    let dir = tempfile::tempdir().unwrap();
    let answers = cache::Answers::new(dir.path().join("dictionary"));
    let key = cache::key("wiktionary", "en", "fr", "happy");
    assert!(answers.get(&key).await.is_none());
    let happy = entry("happy", vec![meaning("adjective", &["Glad."])]);
    answers.put(&key, Some(&happy)).await;
    let kept = answers.get(&key).await.unwrap();
    assert_eq!(kept.entry.as_ref(), Some(&happy));
    let now = cache::now_ms();
    assert!(kept.fresh(now));
    assert!(
        !kept.fresh(now + 31 * 24 * 60 * 60 * 1000),
        "a month on, asked again"
    );
    // A word nobody knew is asked again sooner.
    let missing = cache::key("wiktionary", "en", "fr", "qwzxv");
    answers
        .put_at(&missing, None, now - 4 * 24 * 60 * 60 * 1000)
        .await;
    let kept = answers.get(&missing).await.unwrap();
    assert_eq!(kept.entry, None);
    assert!(!kept.fresh(now));
    // Keys never share a file's answer.
    assert!(
        answers
            .get(&cache::key("wiktionary", "en", "de", "happy"))
            .await
            .is_none()
    );
    assert_eq!(answers.usage().0, 2);
    answers.clear().await.unwrap();
    assert!(answers.get(&key).await.is_none());
    assert_eq!(answers.usage(), (0, 0));
}

#[test]
fn the_folder_keeps_the_newest_answers() {
    let dir = tempfile::tempdir().unwrap();
    for index in 0..5 {
        std::fs::write(dir.path().join(format!("{index}.json")), "{}").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    cache::trim(dir.path(), 3);
    let mut left: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["2.json", "3.json", "4.json"]);
}

// ---- Searching a book ----

#[test]
fn a_book_search_finds_whole_words_case_and_accents_aside() {
    assert_eq!(
        book_search::matches_in("Start the art; ART and Árt.", "art"),
        [(10, 13), (15, 18), (23, 26)]
    );
    assert_eq!(book_search::matches_in("l’homme", "l'homme"), [(0, 7)]);
    let blocks = vec![
        TextBlock::Heading {
            text: "Chapter One".to_owned(),
        },
        TextBlock::Separator,
        TextBlock::Paragraph {
            text: "It was a happy day, and everyone in the village knew it would be a happy one."
                .to_owned(),
            parts: None,
        },
    ];
    let mut hits = Vec::new();
    let more = book_search::search_blocks("c1", "One", &blocks, "happy", &mut hits);
    assert!(!more);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].block, 2);
    assert_eq!(hits[0].matched, "happy");
    assert_eq!(hits[0].before, "It was a ");
    assert_eq!(hits[1].after, " one.");
    assert!(hits[1].before.starts_with('…'));
}

// ---- Live: the real services (network) ----

#[tokio::test]
#[ignore = "network: asks Wiktionary"]
async fn wiktionary_live() {
    let polite = super::polite::Polite::new().unwrap();
    let happy = wiktionary::look_up(&polite, "happy", "en", Some("fr"))
        .await
        .unwrap()
        .expect("Wiktionary knows happy");
    assert!(
        happy
            .meanings
            .iter()
            .any(|meaning| meaning.part_of_speech == "adjective")
    );
    assert!(happy.pronunciations.iter().any(|sound| sound.ipa.is_some()));
    assert!(happy.etymology.is_some());
    assert!(
        happy
            .meanings
            .iter()
            .any(|meaning| !meaning.translations.is_empty()),
        "French translations"
    );
    let maison = wiktionary::look_up(&polite, "maison", "fr", Some("en"))
        .await
        .unwrap()
        .expect("Wiktionary knows maison in French");
    assert_eq!(maison.language_name.as_deref(), Some("French"));
    let went = wiktionary::look_up(&polite, "went", "en", None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(went.form_of.unwrap().lemma, "go");
    assert!(
        wiktionary::look_up(&polite, "qwzxvblorp", "en", None)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
#[ignore = "network: asks the Free Dictionary API"]
async fn free_dictionary_live() {
    let polite = super::polite::Polite::new().unwrap();
    let hello = free_dictionary::look_up(&polite, "hello")
        .await
        .unwrap()
        .expect("the Free Dictionary knows hello");
    assert!(!hello.meanings.is_empty());
    assert!(
        hello
            .pronunciations
            .iter()
            .any(|sound| sound.audio_url.is_some())
    );
    assert!(
        free_dictionary::look_up(&polite, "qwzxvblorp")
            .await
            .unwrap()
            .is_none()
    );
}
