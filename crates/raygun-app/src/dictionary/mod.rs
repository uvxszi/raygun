//! Word lookups in the book reader: a word or a few words selected in a book's text are looked up
//! in the dictionaries, in the order Settings → Reader gives them, and the answers merged.
//!
//! Raygun's own dictionaries live here rather than in an extension: Raygun ships no sources, and a
//! reading tool has to work on a new install the way the translators do. They are Wiktionary
//! (every language, described in English; the definitions API, then the page for pronunciation,
//! etymology, synonyms and translations) and the Free Dictionary API (English, with recordings).
//! Extensions add more through the `dictionary` provider kind (`raygun_ext_host::dictionary`).
//!
//! The words' language is the one chosen in Settings, else the book's (an EPUB's `dc:language`,
//! a source's language), else the app's, as long as the words are written in its script. A word a
//! dictionary doesn't know as written is tried in lower case, without an elided article (`l'`),
//! and, in English, without its ending (`studies` → `study`); a word that's only a form of another
//! (`ran`) shows the other's entry. Answers are kept on the device (`cache.rs`), so a word looked
//! up before opens offline.

mod book_search;
mod cache;
mod forms;
mod free_dictionary;
mod merge;
mod polite;
#[cfg(test)]
mod tests;
mod wiktionary;

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use raygun_model::{
    BookSearch, DictionaryEntry, DictionaryFormOf, DictionaryLookup, DictionaryPrefs,
    DictionaryRequest, DictionarySourceInfo, ProviderInfo,
};
use tauri::{AppHandle, Manager};

use merge::SourceRef;

pub const WIKTIONARY: &str = "wiktionary";
pub const FREE_DICTIONARY: &str = "freeDictionary";

/// A dictionary that can be asked.
#[derive(Debug, Clone)]
enum Source {
    Wiktionary,
    FreeDictionary,
    Extension(ProviderInfo),
}

impl Source {
    fn id(&self) -> &str {
        match self {
            Self::Wiktionary => WIKTIONARY,
            Self::FreeDictionary => FREE_DICTIONARY,
            Self::Extension(provider) => &provider.id,
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Wiktionary => wiktionary::NAME,
            Self::FreeDictionary => free_dictionary::NAME,
            Self::Extension(provider) => &provider.name,
        }
    }

    /// The languages it knows words of; empty for any.
    fn languages(&self) -> Vec<String> {
        match self {
            Self::FreeDictionary => vec![free_dictionary::LANGUAGE.to_owned()],
            _ => Vec::new(),
        }
    }

    fn knows(&self, language: &str) -> bool {
        let languages = self.languages();
        languages.is_empty() || languages.iter().any(|known| known == language)
    }

    fn info(&self, enabled: bool) -> DictionarySourceInfo {
        DictionarySourceInfo {
            id: self.id().to_owned(),
            name: self.name().to_owned(),
            languages: self.languages(),
            built_in: !matches!(self, Self::Extension(_)),
            enabled,
        }
    }

    fn reference(&self) -> SourceRef {
        SourceRef {
            id: self.id().to_owned(),
            name: self.name().to_owned(),
        }
    }
}

/// `sources` in the user's order: the ones the order names first, the rest after them as they
/// came.
fn ordered<T>(mut sources: Vec<T>, id: impl Fn(&T) -> &str, order: &[String]) -> Vec<T> {
    sources.sort_by_key(|source| {
        order
            .iter()
            .position(|known| known == id(source))
            .unwrap_or(order.len())
    });
    sources
}

/// What a source answered for one word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reply {
    /// An entry; `kept` when it's this device's copy.
    Found {
        entry: Box<DictionaryEntry>,
        kept: bool,
    },
    /// It doesn't know the word.
    Missing,
    /// It couldn't be asked: one sentence naming it.
    Failed(String),
    /// Offline Mode is on and nothing is kept for the word.
    Offline,
}

/// The entries found for a selection, and how the search went.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Found {
    pub word: String,
    pub entries: Vec<(SourceRef, DictionaryEntry)>,
    /// Every entry is this device's copy.
    pub kept: bool,
    pub form_of: Option<DictionaryFormOf>,
    pub offline: bool,
    pub issues: Vec<String>,
}

/// What each source says about `word`, and how many of them looked and didn't know it.
async fn ask_all<A, F>(
    sources: &[SourceRef],
    word: &str,
    ask: &A,
    found: &mut Found,
) -> (Vec<(SourceRef, DictionaryEntry, bool)>, usize)
where
    A: Fn(SourceRef, String) -> F,
    F: Future<Output = Reply>,
{
    let replies = futures_util::future::join_all(
        sources
            .iter()
            .map(|source| ask(source.clone(), word.to_owned())),
    )
    .await;
    let mut entries = Vec::new();
    let mut missing = 0;
    for (source, reply) in sources.iter().zip(replies) {
        match reply {
            Reply::Found { entry, kept } => entries.push((source.clone(), *entry, kept)),
            Reply::Missing => missing += 1,
            Reply::Failed(issue) => {
                if !found.issues.contains(&issue) {
                    found.issues.push(issue);
                }
            }
            Reply::Offline => {
                found.offline = true;
                // Offline, another form may still be kept on the device.
                missing += 1;
            }
        }
    }
    (entries, missing)
}

fn settle(found: &mut Found, word: String, entries: Vec<(SourceRef, DictionaryEntry, bool)>) {
    found.word = word;
    found.kept = entries.iter().all(|(_, _, kept)| *kept);
    found.entries = entries
        .into_iter()
        .map(|(source, entry, _)| (source, entry))
        .collect();
}

/// Looks `query` up in `sources` (in order) through `ask`: each form of it in turn until a source
/// knows one, then, for a word that's only a form of another, the other.
pub(crate) async fn find<A, F>(sources: &[SourceRef], query: &str, language: &str, ask: A) -> Found
where
    A: Fn(SourceRef, String) -> F,
    F: Future<Output = Reply>,
{
    let mut found = Found::default();
    for candidate in forms::candidates(query, language) {
        let (entries, missing) = ask_all(sources, &candidate, &ask, &mut found).await;
        if entries.is_empty() {
            // Only a source that looked and didn't know the word makes another form worth asking.
            if missing == 0 {
                break;
            }
            continue;
        }
        let lemma = entries.iter().find_map(|(_, entry, _)| {
            entry.form_of.clone().filter(|form| {
                form.only && !form.lemma.trim().is_empty() && form.lemma != candidate
            })
        });
        if let Some(form) = lemma {
            let (lemma_entries, _) = ask_all(sources, &form.lemma, &ask, &mut found).await;
            if !lemma_entries.is_empty() {
                settle(&mut found, form.lemma.clone(), lemma_entries);
                found.form_of = Some(form);
                return found;
            }
        }
        found.form_of = entries
            .iter()
            .find_map(|(_, entry, _)| entry.form_of.clone());
        settle(&mut found, candidate, entries);
        return found;
    }
    found
}

/// One provider's entries as one: the first, with the others' senses after its own.
fn combined(mut entries: Vec<DictionaryEntry>) -> Option<DictionaryEntry> {
    if entries.is_empty() {
        return None;
    }
    let mut first = entries.remove(0);
    for entry in entries {
        first.meanings.extend(entry.meanings);
        first.pronunciations.extend(entry.pronunciations);
    }
    Some(first)
}

pub struct DictionaryService {
    app: AppHandle,
    polite: polite::Polite,
    answers: cache::Answers,
    /// Books' languages, by title id, once worked out.
    book_languages: Mutex<HashMap<String, Option<String>>>,
}

impl DictionaryService {
    pub fn new(app: AppHandle, dir: PathBuf) -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self {
            app,
            polite: polite::Polite::new()?,
            answers: cache::Answers::new(dir),
            book_languages: Mutex::new(HashMap::new()),
        }))
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<crate::startup::AppDb>().inner().0.clone()
    }

    async fn prefs(&self) -> DictionaryPrefs {
        crate::settings::load(&self.db())
            .await
            .map(|settings| settings.dictionary)
            .unwrap_or_default()
    }

    /// Every source: Raygun's own, then the extensions' `dictionary` providers.
    async fn all_sources(&self) -> Vec<Source> {
        let mut sources = vec![Source::Wiktionary, Source::FreeDictionary];
        if let Some(extensions) = self
            .app
            .try_state::<Arc<crate::extensions::ExtensionService>>()
        {
            let (_ready, host) = extensions.ready().await;
            sources.extend(
                host.providers(raygun_ext_host::dictionary::DICTIONARY)
                    .await
                    .into_iter()
                    .map(Source::Extension),
            );
        }
        sources
    }

    /// The sources in the user's order, with whether each is on (Settings).
    pub async fn sources(&self) -> Vec<DictionarySourceInfo> {
        let prefs = self.prefs().await;
        ordered(self.all_sources().await, Source::id, &prefs.order)
            .into_iter()
            .map(|source| {
                let enabled = !prefs.off.iter().any(|off| off == source.id());
                source.info(enabled)
            })
            .collect()
    }

    /// The language a book says it's written in: an imported EPUB's own, or its source's.
    async fn book_language(&self, title_id: &str) -> Option<String> {
        if let Some(known) = self
            .book_languages
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(title_id)
        {
            return known.clone();
        }
        let title = raygun_db::titles::title(&self.db(), title_id)
            .await
            .ok()
            .flatten()?;
        let source = title.fields.source_id.clone();
        let language = if source == crate::novel::LOCAL_EPUB {
            let dir = self
                .app
                .state::<crate::startup::AppPaths>()
                .0
                .downloads()
                .join(crate::novel::BOOKS)
                .join(&title.fields.remote_id);
            tauri::async_runtime::spawn_blocking(move || crate::epub::unpacked_language(&dir))
                .await
                .ok()
                .flatten()
        } else if source == crate::novel::LOCAL_PDF {
            None
        } else {
            self.source_language(&source).await
        };
        let language = language.filter(|tag| {
            let tag = forms::primary(tag);
            !tag.is_empty() && !matches!(tag.as_str(), "multi" | "all" | "und" | "mul")
        });
        self.book_languages
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(title_id.to_owned(), language.clone());
        language
    }

    /// A catalogue source's language: its repository's word, else the one in its id
    /// (`mihon.fr.name`).
    async fn source_language(&self, source_id: &str) -> Option<String> {
        let extension_id = source_id.split('/').next().unwrap_or(source_id);
        let extensions = self
            .app
            .try_state::<Arc<crate::extensions::ExtensionService>>()?;
        let listed = extensions
            .catalog(false)
            .await
            .ok()
            .into_iter()
            .flatten()
            .flat_map(|repo| repo.entries)
            .find(|entry| entry.id == extension_id)
            .and_then(|entry| entry.lang);
        listed.or_else(|| {
            let mut parts = extension_id.split('.');
            match (parts.next(), parts.next()) {
                (Some("mihon" | "shosetsu" | "lnreader" | "miru"), Some(lang))
                    if (2..=3).contains(&lang.len()) =>
                {
                    Some(lang.to_owned())
                }
                _ => None,
            }
        })
    }

    async fn fetch(
        &self,
        source: &Source,
        word: &str,
        language: &str,
        target: Option<&str>,
    ) -> Result<Option<DictionaryEntry>, String> {
        match source {
            Source::Wiktionary => wiktionary::look_up(&self.polite, word, language, target).await,
            Source::FreeDictionary => free_dictionary::look_up(&self.polite, word).await,
            Source::Extension(provider) => {
                let extensions = self.app.state::<Arc<crate::extensions::ExtensionService>>();
                let (_ready, host) = extensions.ready().await;
                let args = serde_json::json!({ "word": word, "language": language });
                match host.call_interactive(&provider.id, "define", args).await {
                    Ok(answer) => Ok(combined(raygun_ext_host::dictionary::parse_entries(
                        &answer, language,
                    ))),
                    Err(error) if error.is_missing() => Ok(None),
                    Err(error) => Err(error.to_string()),
                }
            }
        }
    }

    /// One source's answer for `word`: the kept one while it's fresh (any kept one offline, or
    /// when the source fails), else the source's, kept for next time.
    async fn ask(
        &self,
        source: &Source,
        word: &str,
        language: &str,
        target: Option<&str>,
        refresh: bool,
    ) -> Reply {
        let key = cache::key(source.id(), language, target.unwrap_or_default(), word);
        let kept = self.answers.get(&key).await;
        let offline = raygun_net::network_mode::offline();
        let from_kept = |kept: cache::Kept| match kept.entry {
            Some(entry) => Reply::Found {
                entry: Box::new(entry),
                kept: true,
            },
            None => Reply::Missing,
        };
        if let Some(known) = &kept
            && (offline || (!refresh && known.fresh(cache::now_ms())))
        {
            return from_kept(known.clone());
        }
        if offline {
            return Reply::Offline;
        }
        match self.fetch(source, word, language, target).await {
            Ok(Some(entry)) => {
                self.answers.put(&key, Some(&entry)).await;
                Reply::Found {
                    entry: Box::new(entry),
                    kept: false,
                }
            }
            Ok(None) => {
                self.answers.put(&key, None).await;
                Reply::Missing
            }
            Err(issue) => match kept {
                Some(known) => from_kept(known),
                None if raygun_net::network_mode::mentions_offline(&issue) => Reply::Offline,
                None => Reply::Failed(issue),
            },
        }
    }

    /// Looks a selection up in the sources that are on, in the user's order.
    pub async fn lookup(&self, request: DictionaryRequest) -> Result<DictionaryLookup, String> {
        let query = forms::tidy(&request.text);
        if !forms::is_lookup(&query) {
            return Err("Select a word, or a few words, to look them up".to_owned());
        }
        let prefs = self.prefs().await;
        let book = match request.title_id.as_deref() {
            Some(id) => self.book_language(id).await,
            None => None,
        };
        let language = forms::choose_language(
            &query,
            &prefs.language,
            book.as_deref(),
            &request.app_language,
        );
        let target = Some(forms::primary(&request.app_language))
            .filter(|target| !target.is_empty() && *target != language);
        let sources: Vec<Source> = ordered(self.all_sources().await, Source::id, &prefs.order)
            .into_iter()
            .filter(|source| !prefs.off.iter().any(|off| off == source.id()))
            .filter(|source| source.knows(&language))
            .collect();
        let by_id: HashMap<String, Source> = sources
            .iter()
            .map(|source| (source.id().to_owned(), source.clone()))
            .collect();
        let references: Vec<SourceRef> = sources.iter().map(Source::reference).collect();
        let refresh = request.refresh;
        let found = find(&references, &query, &language, |reference, word| {
            let source = by_id.get(&reference.id).cloned();
            let language = language.clone();
            let target = target.clone();
            async move {
                match source {
                    Some(source) => {
                        self.ask(&source, &word, &language, target.as_deref(), refresh)
                            .await
                    }
                    None => Reply::Missing,
                }
            }
        })
        .await;
        if !found.issues.is_empty() {
            tracing::info!(issues = ?found.issues, "dictionaries had trouble");
        }
        Ok(self.answer(query, language, found))
    }

    /// The answer the reader shows: entries merged, recordings linked through the proxy.
    fn answer(&self, query: String, language: String, found: Found) -> DictionaryLookup {
        let merged = merge::merge(&found.entries);
        let media = self.app.try_state::<raygun_net::MediaLinks>();
        let headers = BTreeMap::from([("User-Agent".to_owned(), polite::user_agent())]);
        let pronunciations = merged
            .pronunciations
            .into_iter()
            .filter_map(|mut pronunciation| {
                if let Some(url) = pronunciation.audio_url.take() {
                    pronunciation.audio_url =
                        media.as_ref().and_then(|media| media.link(&url, &headers));
                }
                (pronunciation.ipa.is_some() || pronunciation.audio_url.is_some())
                    .then_some(pronunciation)
            })
            .collect();
        let empty = found.entries.is_empty();
        DictionaryLookup {
            online_url: wiktionary::search_url(&query),
            word: if merged.word.is_empty() {
                found.word
            } else {
                merged.word
            },
            query,
            language,
            language_name: merged.language_name,
            form_of: found.form_of,
            pronunciations,
            meanings: merged.meanings,
            etymology: merged.etymology,
            credits: merged.credits,
            kept: found.kept && !empty,
            offline: found.offline && empty,
            issues: found.issues,
        }
    }

    /// Forgets the kept answers (Settings → Data → Caches).
    pub async fn clear(&self) -> Result<(), String> {
        self.answers.clear().await
    }

    /// Kept answers and the bytes they take.
    pub fn usage(&self) -> (u32, u64) {
        self.answers.usage()
    }
}

/// What the dictionaries say about a word or a few words selected in the book reader.
#[tauri::command]
#[specta::specta]
pub async fn dictionary_lookup(
    dictionary: tauri::State<'_, Arc<DictionaryService>>,
    request: DictionaryRequest,
) -> Result<DictionaryLookup, String> {
    dictionary.lookup(request).await
}

/// The dictionaries in the order they're asked, with whether each is on.
#[tauri::command]
#[specta::specta]
pub async fn dictionary_sources(
    dictionary: tauri::State<'_, Arc<DictionaryService>>,
) -> Result<Vec<DictionarySourceInfo>, String> {
    Ok(dictionary.sources().await)
}

/// Opens a dictionary's page (an entry, a search) in Raygun's browser.
#[tauri::command]
#[specta::specta]
pub async fn dictionary_open_page(
    websites: tauri::State<'_, Arc<crate::websites::WebsiteService>>,
    url: String,
) -> Result<(), String> {
    let url = url.trim();
    if !url.starts_with("https://") {
        return Err("That isn't a web address".into());
    }
    websites
        .inner()
        .clone()
        .open_page(&format!("{}dictionary", crate::websites::PAGE_PREFIX), url)
        .await
}

/// Where words appear in a book: every chapter of an imported book, a source's downloaded
/// chapters and the one open (`chapter_id`).
#[tauri::command]
#[specta::specta]
pub async fn novel_search_book(
    app: AppHandle,
    title_id: String,
    chapter_id: Option<String>,
    query: String,
) -> Result<BookSearch, String> {
    book_search::search(&app, &title_id, chapter_id.as_deref(), &query).await
}
