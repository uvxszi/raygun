//! Answers in Search: a question or a calculation typed in Search gets an answer, Wolfram|Alpha
//! style. Search → All shows one above the results when the words plainly ask something; the
//! Answers pill asks every engine and shows more about each answer.
//!
//! The engines, in the order they're tried:
//! - on this device, offline: dates and times (`dates`, `zones`), statistics (`stats`), algebra
//!   (`algebra`), named constants (`constants`), percentages (`percent`), and arithmetic, units
//!   and number bases computed by fend (`calc`, `expr`, `units`); currencies too, at the
//!   European Central Bank's rates (`currency`, kept for offline use);
//! - online, read from the words (`questions`): facts from Wikidata (`wikidata`), what something
//!   is from Wikipedia (`wikipedia`), what a word means from Raygun's dictionaries and
//!   translations by this device's models or Settings → AI's translators (`words`); kept on the
//!   device (`kept`), so asking again works offline;
//! - extensions' `answers` providers (`raygun_ext_host::answers`), asked on the Answers pill and,
//!   for a plain question nothing else answered, in All.
//!
//! Reading the words is cheap and local, so a normal search (`Blue`, `1984`) is told apart at
//! once and costs nothing; questions are read in English.

mod algebra;
mod calc;
mod constants;
mod currency;
mod dates;
mod expr;
mod kept;
mod languages;
mod numbers;
mod percent;
mod questions;
mod stats;
#[cfg(test)]
mod tests;
mod text;
mod units;
mod wikidata;
mod wikipedia;
mod words;
mod zones;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use raygun_model::{
    Answer, AnswerCredit, AnswerKind, AnswerRequest, AnswerSuggestion, AnswerText, Answers,
    AnswersPrefs,
};
use tauri::{AppHandle, Manager};

use crate::dictionary::cache::now_ms;
use crate::dictionary::polite::{Polite, user_agent_for};
use questions::{Question, Read};

/// The longest query answered.
const MAX_QUERY: usize = 300;
/// How long the engines on this device may take.
const LOCAL_LIMIT: Duration = Duration::from_secs(3);
/// Answers from extensions that one query shows at most.
const MAX_EXTENSION_ANSWERS: usize = 6;

/// An answer from this device's engines, and whether the words plainly asked for it.
#[derive(Debug, Clone)]
pub(crate) struct Found {
    pub answer: Answer,
    pub strong: bool,
}

/// An empty answer from Raygun's own engines.
pub(crate) fn blank(kind: AnswerKind) -> Answer {
    Answer {
        engine: "raygun".to_owned(),
        engine_name: String::new(),
        kind,
        interpretation: None,
        result: AnswerText::default(),
        approximate: false,
        rows: Vec::new(),
        units: Vec::new(),
        summary: None,
        image_url: None,
        url: None,
        credits: Vec::new(),
        as_of: None,
        copy: String::new(),
        kept: false,
    }
}

/// Text with the formula `tex` draws (as MathML), when it's TeX the converter reads.
pub(crate) fn math_text(text: String, tex: &str) -> AnswerText {
    AnswerText {
        text,
        math: raygun_ext_host::answers::math_of(tex),
        date: None,
        query: None,
    }
}

/// Dates like `01/02/2027` are read month first in US English.
fn month_first(language: &str) -> bool {
    let lower = language.to_lowercase();
    lower == "en" || lower.starts_with("en-us") || lower.starts_with("en-ph")
}

/// What this device's engines answer for `text` (`lower` in lower case).
fn local_answer(
    text: &str,
    lower: &str,
    clock: &dates::Clock,
    rates: Option<&HashMap<String, f64>>,
    card: bool,
) -> Option<Found> {
    dates::answer(lower, clock)
        .or_else(|| stats::answer(lower))
        .or_else(|| algebra::answer(lower))
        .or_else(|| constants::answer(lower))
        .or_else(|| percent::answer(lower))
        .or_else(|| calc::answer_for(text, rates, card))
}

/// What the online engines found for a question.
#[derive(Default)]
struct Online {
    answers: Vec<Answer>,
    related: Vec<AnswerSuggestion>,
    needs_translator: bool,
}

pub struct AnswersService {
    app: AppHandle,
    polite: Polite,
    kept: kept::Kept,
    rates: currency::ExchangeRates,
}

impl AnswersService {
    pub fn new(app: AppHandle, dir: PathBuf) -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self {
            app,
            polite: Polite::with_agent(user_agent_for("answers in Search"))?,
            rates: currency::ExchangeRates::new(dir.clone()),
            kept: kept::Kept::new(dir),
        }))
    }

    async fn prefs(&self) -> AnswersPrefs {
        let db = self.app.state::<crate::startup::AppDb>().inner().0.clone();
        crate::settings::load(&db)
            .await
            .map(|settings| settings.answers)
            .unwrap_or_default()
    }

    /// Whether online engines may be asked now.
    fn online(&self, prefs: &AnswersPrefs) -> bool {
        prefs.online && !raygun_net::network_mode::offline()
    }

    /// Answers to `request`'s words.
    pub async fn ask(&self, request: AnswerRequest) -> Answers {
        let mut out = Answers::none(request.query.clone());
        let text = text::tidy(&request.query);
        if text.is_empty() || text.chars().count() > MAX_QUERY {
            return out;
        }
        let prefs = self.prefs().await;
        if request.card && !prefs.in_all {
            return out;
        }
        let lower = text.to_lowercase();
        let online = self.online(&prefs);

        // Exchange rates first, when the words name currencies.
        let rates = if calc::mentions_currency(&lower) {
            match self.rates.get(&self.polite, online).await {
                Ok(rates) => Some(rates),
                Err(issue) => {
                    out.issues.push(issue);
                    None
                }
            }
        } else {
            None
        };
        let now = DateTime::<Utc>::from_timestamp_millis(now_ms() as i64).unwrap_or_default();
        let clock = dates::Clock {
            now,
            zone: zones::device(request.time_zone.as_deref(), request.utc_offset_minutes),
            month_first: month_first(&request.language),
        };
        let local = {
            let text = text.clone();
            let lower = lower.clone();
            let table = rates.as_ref().map(|rates| rates.rates.clone());
            let card = request.card;
            let computed = tauri::async_runtime::spawn_blocking(move || {
                local_answer(&text, &lower, &clock, table.as_ref(), card)
            });
            match tokio::time::timeout(LOCAL_LIMIT, computed).await {
                Ok(Ok(found)) => found,
                _ => None,
            }
        };
        let read = questions::read(&lower, &text);
        let strong_local = local.as_ref().is_some_and(|found| found.strong);
        let strong_online = read.as_ref().is_some_and(|read| read.strong);
        out.question = strong_local || strong_online;
        if request.card && !out.question {
            return out;
        }

        let mut weak_local = None;
        if let Some(mut found) = local {
            if found.answer.kind == AnswerKind::Currency
                && let Some(rates) = &rates
            {
                found.answer.as_of = raygun_ext_host::answers::parse_date(&rates.date);
                found.answer.credits.push(AnswerCredit {
                    name: currency::ECB_NAME.to_owned(),
                    url: Some(currency::ECB_PAGE.to_owned()),
                    license: None,
                    license_url: None,
                });
                found.answer.url = Some(currency::ECB_PAGE.to_owned());
            }
            if found.strong {
                out.answers.push(found.answer);
            } else if !request.card {
                weak_local = Some(found.answer);
            }
        }

        // Online engines, for a question nothing here answered plainly.
        if !strong_local {
            let question = match read {
                Some(read) if !request.card || read.strong => Some(read),
                // The Answers pill looks plain words up too: what they name (`Blue`, `1984`).
                None if !request.card => Some(Read {
                    question: Question::About(text.clone()),
                    strong: false,
                }),
                _ => None,
            };
            if let Some(read) = question {
                let found = self
                    .look_up(&read, &request, online, &clock, &mut out)
                    .await;
                out.needs_translator |= found.needs_translator;
                out.answers.extend(found.answers);
                out.related.extend(found.related);
            }
        }
        if let Some(answer) = weak_local {
            out.answers.push(answer);
        }

        // Extensions: always on the pill; in All only for a plain question nothing answered.
        if !request.card || (out.answers.is_empty() && out.question) {
            let extra = self.extensions(&request, &text, &mut out).await;
            out.answers.extend(extra);
        }
        out.related
            .dedup_by(|a, b| a.query.eq_ignore_ascii_case(&b.query));
        out.related.truncate(8);
        self.link_pictures(&mut out.answers);
        out
    }

    /// An online question answered: kept answers while fresh (any kept answer offline or when
    /// the engines fail), else the engines asked and their answers kept.
    async fn look_up(
        &self,
        read: &Read,
        request: &AnswerRequest,
        online: bool,
        clock: &dates::Clock,
        out: &mut Answers,
    ) -> Online {
        // Dictionaries keep their own answers; translations may run on this device.
        match &read.question {
            Question::Define(word) => {
                return match words::define(&self.app, word, &request.language, request.refresh)
                    .await
                {
                    Ok(Some(answer)) => Online {
                        answers: vec![answer],
                        ..Online::default()
                    },
                    Ok(None) => Online::default(),
                    Err(issue) => {
                        out.issues.push(issue);
                        Online::default()
                    }
                };
            }
            Question::Translate { text, to } => {
                return match words::translate(&self.app, text, to).await {
                    Ok(words::Translated::Done(answer)) => Online {
                        answers: vec![*answer],
                        ..Online::default()
                    },
                    Ok(words::Translated::NeedsTranslator) => Online {
                        needs_translator: true,
                        ..Online::default()
                    },
                    Err(issue) => {
                        out.issues.push(issue);
                        Online::default()
                    }
                };
            }
            Question::Fact { .. } | Question::About(_) => {}
        }
        let depth = if request.card { "card" } else { "full" };
        let key = kept::key(
            &format!("{depth}:{}", question_key(&read.question)),
            &request.language,
            "",
        );
        let kept = self.kept.get(&key).await;
        let from_kept = |entry: kept::Entry, stale: bool| Online {
            answers: entry
                .answers
                .into_iter()
                .map(|mut answer| {
                    answer.kept = stale;
                    answer
                })
                .collect(),
            related: entry.related,
            needs_translator: false,
        };
        if let Some(entry) = &kept
            && (!online || (!request.refresh && entry.fresh(now_ms())))
        {
            return from_kept(entry.clone(), !online);
        }
        if !online {
            out.offline = true;
            return Online::default();
        }
        match self.fetch(read, request, clock).await {
            Ok(found) => {
                if !crate::privacy::incognito(&self.app) {
                    self.kept.put(&key, &found.answers, &found.related).await;
                }
                found
            }
            Err(issue) => match kept {
                Some(entry) => from_kept(entry, true),
                None => {
                    if raygun_net::network_mode::mentions_offline(&issue) {
                        out.offline = true;
                    } else {
                        out.issues.push(issue);
                    }
                    Online::default()
                }
            },
        }
    }

    /// Facts and summaries, asked of Wikidata and Wikipedia.
    async fn fetch(
        &self,
        read: &Read,
        request: &AnswerRequest,
        clock: &dates::Clock,
    ) -> Result<Online, String> {
        let language = request.language.as_str();
        let today = zones::local(clock.zone, clock.now).date_naive();
        match &read.question {
            Question::Fact { property, subject } => {
                let Some(found) =
                    wikidata::fact(&self.polite, property, subject, language, today).await?
                else {
                    return Ok(Online::default());
                };
                let mut answers = vec![found.answer];
                // The pill adds what the thing is.
                if !request.card
                    && let Some((wiki, title)) = wikipedia::article(&found.entity, language)
                    && let Ok(Some(summary)) = wikipedia::summary(&self.polite, &wiki, &title).await
                {
                    answers.push(wikipedia::answer_of(summary, &found.hit, &found.entity));
                }
                Ok(Online {
                    answers,
                    related: found.related,
                    needs_translator: false,
                })
            }
            Question::About(subject) => {
                let Some((mut answer, hit, entity)) =
                    wikipedia::about(&self.polite, subject, language).await?
                else {
                    return Ok(Online::default());
                };
                if !request.card {
                    match wikidata::key_facts(&self.polite, &hit, language).await {
                        Ok(rows) => answer.rows.extend(rows),
                        Err(issue) => tracing::debug!(%issue, "no key facts"),
                    }
                }
                let name = entity.label.clone().unwrap_or_else(|| hit.label.clone());
                let related = wikidata::related(wikidata::kind_of(&hit.description), &name, None);
                Ok(Online {
                    answers: vec![answer],
                    related,
                    needs_translator: false,
                })
            }
            Question::Define(_) | Question::Translate { .. } => Ok(Online::default()),
        }
    }

    /// What extensions' `answers` providers say.
    async fn extensions(
        &self,
        request: &AnswerRequest,
        text: &str,
        out: &mut Answers,
    ) -> Vec<Answer> {
        let Some(extensions) = self
            .app
            .try_state::<Arc<crate::extensions::ExtensionService>>()
        else {
            return Vec::new();
        };
        let (_ready, host) = extensions.ready().await;
        let providers = host.providers(raygun_ext_host::answers::ANSWERS).await;
        if providers.is_empty() {
            return Vec::new();
        }
        let args = serde_json::json!({
            "query": text,
            "language": request.language,
            "card": request.card,
        });
        let replies = futures_util::future::join_all(providers.iter().map(|provider| {
            let args = args.clone();
            async move { host.call_interactive(&provider.id, "answer", args).await }
        }))
        .await;
        let mut found = Vec::new();
        for (provider, reply) in providers.iter().zip(replies) {
            match reply {
                Ok(answer) => found.extend(raygun_ext_host::answers::parse_answers(
                    &answer,
                    &provider.id,
                    &provider.name,
                )),
                Err(error) if error.is_missing() || error.is_needs_setup() => {}
                Err(error) => {
                    tracing::info!(provider = %provider.id, %error, "an answers source had trouble");
                    out.issues
                        .push(raygun_net::issue::named(&provider.name, &error.to_string()));
                }
            }
        }
        found.truncate(MAX_EXTENSION_ANSWERS);
        found
    }

    /// Pictures are kept as their sources' addresses and shown through the cover proxy.
    fn link_pictures(&self, answers: &mut [Answer]) {
        let Some(images) = self.app.try_state::<raygun_net::ImageLinks>() else {
            return;
        };
        let headers =
            BTreeMap::from([("User-Agent".to_owned(), user_agent_for("answers in Search"))]);
        for answer in answers {
            if let Some(url) = answer.image_url.take() {
                answer.image_url = Some(images.cover(&url, &headers));
            }
        }
    }

    /// Forgets the kept answers and exchange rates (Settings → Data → Caches).
    pub async fn clear(&self) -> Result<(), String> {
        self.kept.clear().await
    }

    /// Kept answers and the bytes they take.
    pub fn usage(&self) -> (u32, u64) {
        self.kept.usage()
    }
}

/// The words a question's kept answer is found by.
fn question_key(question: &Question) -> String {
    match question {
        Question::Fact { property, subject } => {
            format!("fact:{}:{subject}", property.ids.join(","))
        }
        Question::About(subject) => format!("about:{}", subject.to_lowercase()),
        Question::Define(word) => format!("define:{word}"),
        Question::Translate { text, to } => format!("translate:{to}:{text}"),
    }
}

/// Answers to a question or calculation typed in Search.
#[tauri::command]
#[specta::specta]
pub async fn answers_ask(
    answers: tauri::State<'_, Arc<AnswersService>>,
    request: AnswerRequest,
) -> Result<Answers, String> {
    Ok(answers.ask(request).await)
}

/// Opens an answer's page (an article, a source) in Raygun's browser.
#[tauri::command]
#[specta::specta]
pub async fn answers_open_page(
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
        .open_page(&format!("{}answers", crate::websites::PAGE_PREFIX), url)
        .await
}
