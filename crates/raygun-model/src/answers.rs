//! Answers in Search: what Raygun computes or looks up for a question, a calculation or a
//! conversion typed in Search, and the settings for them.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Answers in Search (Settings → Browse).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct AnswersPrefs {
    /// Search → All shows an answer above the results when the words are a question, a
    /// calculation or a conversion.
    pub in_all: bool,
    /// Facts, summaries, definitions and exchange rates are looked up online (Wikidata,
    /// Wikipedia, Wiktionary, the European Central Bank); off, only what this device computes or
    /// kept from before is shown.
    pub online: bool,
}

impl Default for AnswersPrefs {
    fn default() -> Self {
        Self {
            in_all: true,
            online: true,
        }
    }
}

/// A question asked in Search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AnswerRequest {
    pub query: String,
    /// For Search → All's card: answered only when the words read as a question or a
    /// calculation, by the quick engines. Otherwise it's the Answers pill: every engine, and
    /// more about each answer.
    #[serde(default)]
    pub card: bool,
    /// The app's language (BCP 47): the language of facts' names and of summaries.
    pub language: String,
    /// This device's time zone (IANA, `Europe/Paris`), for dates and times.
    #[serde(default)]
    pub time_zone: Option<String>,
    /// Minutes east of UTC now, for when the zone isn't known.
    #[serde(default)]
    pub utc_offset_minutes: i32,
    /// Asks the online engines again, even when this device kept a recent answer.
    #[serde(default)]
    pub refresh: bool,
}

/// What an answer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AnswerKind {
    Calculation,
    Conversion,
    Currency,
    Percentage,
    NumberBase,
    Statistics,
    Algebra,
    Constant,
    Date,
    Time,
    Fact,
    Summary,
    Definition,
    Translation,
    Other,
}

/// A formula, drawn by the webview from MathML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AnswerMath {
    /// `<math>…</math>`: MathML elements and attributes only.
    pub mathml: String,
    /// The formula as text (its TeX, or a linear form).
    pub text: String,
    /// The formula in words, for screen readers.
    pub speech: String,
}

/// How much of a date is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AnswerDatePrecision {
    Century,
    Decade,
    Year,
    Month,
    Day,
    /// A day and a time of day.
    Minute,
}

/// A date shown in the app's language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AnswerDate {
    /// `2027-01-01`, `-0500-01-01` before the common era; with a time of day,
    /// `2027-01-01T15:30` (the clock time where it's said, no offset).
    pub iso: String,
    pub precision: AnswerDatePrecision,
}

/// Text in an answer: maybe a formula or a date the app shows its own way, maybe something to
/// ask about next.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AnswerText {
    /// Always set: what's copied, and shown when the formula or date can't be.
    pub text: String,
    pub math: Option<AnswerMath>,
    pub date: Option<AnswerDate>,
    /// Asking this tells more about it (a director's name: who they are).
    pub query: Option<String>,
}

impl AnswerText {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }
}

/// One line of an answer: another form of the result, another fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AnswerRow {
    /// What the app names in its language (`decimal`, `fraction`, `words`, `rate`…); empty when
    /// `label` is shown as it is.
    pub key: String,
    /// The name in English, or as an engine gave it.
    pub label: String,
    pub value: AnswerText,
}

/// The result in another unit, for the units toggle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AnswerUnit {
    /// As written after a number (`ft`, `°C`, `EUR`).
    pub symbol: String,
    /// `metric`, `imperial` (US customary and imperial units) or empty.
    pub system: String,
    pub value: AnswerText,
}

/// Who an answer comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AnswerCredit {
    pub name: String,
    pub url: Option<String>,
    /// `CC0`, `CC BY-SA 4.0`…
    pub license: Option<String>,
    pub license_url: Option<String>,
}

/// One answer to a question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    /// Who answered: `raygun` (computed on this device), `wikidata`, `wikipedia`, `dictionary`,
    /// `translation`, `ecb`, or an extension's `<extension>/<provider>`.
    pub engine: String,
    /// Empty for Raygun's own engines, which the app names itself.
    pub engine_name: String,
    pub kind: AnswerKind,
    /// How the words were read ("Input interpretation").
    pub interpretation: Option<AnswerText>,
    pub result: AnswerText,
    /// The result is rounded.
    pub approximate: bool,
    /// Other forms of the result, other facts.
    pub rows: Vec<AnswerRow>,
    /// The result in other units.
    pub units: Vec<AnswerUnit>,
    /// A paragraph about it (a Wikipedia article's start).
    pub summary: Option<String>,
    /// A picture, through Raygun's cover proxy.
    pub image_url: Option<String>,
    /// Where to read more.
    pub url: Option<String>,
    pub credits: Vec<AnswerCredit>,
    /// When the value stands (a population's count, the exchange rates' day).
    pub as_of: Option<AnswerDate>,
    /// Plain text Copy puts on the clipboard.
    pub copy: String,
    /// This device's copy of an earlier answer.
    pub kept: bool,
}

/// Something else to ask.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AnswerSuggestion {
    pub label: String,
    pub query: String,
}

/// What the answer engines said about a query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Answers {
    pub query: String,
    /// The words read as a question, a calculation or a conversion: Search → All shows its card
    /// only then.
    pub question: bool,
    /// Best first.
    pub answers: Vec<Answer>,
    pub related: Vec<AnswerSuggestion>,
    /// Online engines weren't asked: Offline Mode, or the setting is off.
    pub offline: bool,
    /// A translation was asked for, but no translator is set up (Settings → AI).
    pub needs_translator: bool,
    /// Engines that had trouble, one sentence each.
    pub issues: Vec<String>,
}

impl Answers {
    pub fn none(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            question: false,
            answers: Vec::new(),
            related: Vec::new(),
            offline: false,
            needs_translator: false,
            issues: Vec::new(),
        }
    }
}
