//! The `answers` provider kind: answers to questions and calculations typed in Search. Raygun's
//! own engines (its calculator, unit and date engines, Wikidata, Wikipedia, the dictionaries and
//! translators) answer in the app; an extension adds another by registering a provider with an
//! `answer({ query, language, card })` call that answers `{ answers: [...] }` in the shape below.
//! Answers are read leniently: an answer without a result is dropped, links must be http(s),
//! formulas are TeX that Raygun turns into MathML itself, and text is kept to a sensible length.

use raygun_model::{
    Answer, AnswerCredit, AnswerDate, AnswerDatePrecision, AnswerKind, AnswerMath, AnswerRow,
    AnswerText,
};
use serde_json::Value;

use crate::math::Formula;

pub const ANSWERS: &str = "answers";

/// Answers one call may give, and what each may hold.
const MAX_ANSWERS: usize = 5;
const MAX_ROWS: usize = 30;
const MAX_CREDITS: usize = 5;

/// A trimmed text of at most `limit` characters, spaces collapsed; none when empty.
fn text(value: Option<&Value>, limit: usize) -> Option<String> {
    let text = value?.as_str()?;
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.is_empty() {
        return None;
    }
    Some(flat.chars().take(limit).collect())
}

/// A paragraph: line breaks kept, at most `limit` characters.
fn paragraph(value: Option<&Value>, limit: usize) -> Option<String> {
    let text = value?.as_str()?;
    let lines: Vec<String> = text
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }
    Some(lines.join("\n").chars().take(limit).collect())
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

/// A formula from TeX, as MathML the app draws.
pub fn math_of(tex: &str) -> Option<AnswerMath> {
    let formula = Formula::from_tex(tex, false)?;
    Some(AnswerMath {
        mathml: formula.mathml,
        text: formula.text,
        speech: formula.speech,
    })
}

/// Text in an answer: a string, or `{ text, tex, query }`.
fn answer_text(value: Option<&Value>, limit: usize) -> Option<AnswerText> {
    let value = value?;
    if value.is_string() {
        return text(Some(value), limit).map(AnswerText::plain);
    }
    let tex = text(value.get("tex"), 2000);
    let math = tex.as_deref().and_then(math_of);
    let shown = text(value.get("text"), limit).or(tex)?;
    Some(AnswerText {
        text: shown,
        math,
        date: None,
        query: text(value.get("query"), 300),
    })
}

/// `2026-11-11`, `2026-11`, `2026` or `2026-11-11T14:30`: a date the app shows its own way.
pub fn parse_date(iso: &str) -> Option<AnswerDate> {
    let iso = iso.trim();
    let (day, time) = match iso.split_once('T') {
        Some((day, time)) => (day, Some(time)),
        None => (iso, None),
    };
    let negative = day.starts_with('-');
    let parts: Vec<&str> = day.trim_start_matches('-').split('-').collect();
    let digits = |part: &str, length: std::ops::RangeInclusive<usize>| {
        length.contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_digit())
    };
    let precision = match parts.as_slice() {
        [year] if digits(year, 1..=6) => AnswerDatePrecision::Year,
        [year, month] if digits(year, 1..=6) && digits(month, 2..=2) => AnswerDatePrecision::Month,
        [year, month, date]
            if digits(year, 1..=6) && digits(month, 2..=2) && digits(date, 2..=2) =>
        {
            AnswerDatePrecision::Day
        }
        _ => return None,
    };
    if let Some(month) = parts.get(1).and_then(|month| month.parse::<u32>().ok())
        && !(1..=12).contains(&month)
    {
        return None;
    }
    if let Some(date) = parts.get(2).and_then(|date| date.parse::<u32>().ok())
        && !(1..=31).contains(&date)
    {
        return None;
    }
    let clock = time.and_then(|time| {
        let (hours, minutes) = time.get(..5)?.split_once(':')?;
        let ok = digits(hours, 2..=2)
            && digits(minutes, 2..=2)
            && hours.parse::<u32>().ok()? < 24
            && minutes.parse::<u32>().ok()? < 60;
        ok.then(|| format!("{hours}:{minutes}"))
    });
    let sign = if negative { "-" } else { "" };
    Some(match clock {
        Some(clock) if precision == AnswerDatePrecision::Day => AnswerDate {
            iso: format!("{sign}{day}T{clock}", day = day.trim_start_matches('-')),
            precision: AnswerDatePrecision::Minute,
        },
        _ => AnswerDate {
            iso: format!("{sign}{}", day.trim_start_matches('-')),
            precision,
        },
    })
}

fn kind_of(value: Option<&Value>) -> AnswerKind {
    match value.and_then(Value::as_str).unwrap_or_default() {
        "calculation" => AnswerKind::Calculation,
        "conversion" => AnswerKind::Conversion,
        "currency" => AnswerKind::Currency,
        "percentage" => AnswerKind::Percentage,
        "numberBase" => AnswerKind::NumberBase,
        "statistics" => AnswerKind::Statistics,
        "algebra" => AnswerKind::Algebra,
        "constant" => AnswerKind::Constant,
        "date" => AnswerKind::Date,
        "time" => AnswerKind::Time,
        "fact" => AnswerKind::Fact,
        "summary" => AnswerKind::Summary,
        "definition" => AnswerKind::Definition,
        "translation" => AnswerKind::Translation,
        _ => AnswerKind::Other,
    }
}

fn row(value: &Value) -> Option<AnswerRow> {
    Some(AnswerRow {
        key: String::new(),
        label: text(value.get("label"), 120)?,
        value: answer_text(value.get("value"), 1000)?,
    })
}

fn credit(value: &Value) -> Option<AnswerCredit> {
    Some(AnswerCredit {
        name: text(value.get("name"), 120)?,
        url: link(value.get("url")),
        license: text(value.get("license"), 80),
        license_url: link(value.get("licenseUrl")),
    })
}

/// One answer of an `answer` call, from the provider `engine` (its `<extension>/<provider>` id)
/// named `name`.
pub fn parse_answer(value: &Value, engine: &str, name: &str) -> Option<Answer> {
    let result = answer_text(value.get("result"), 2000)?;
    let copy = text(value.get("copy"), 4000).unwrap_or_else(|| result.text.clone());
    Some(Answer {
        engine: engine.to_owned(),
        engine_name: name.to_owned(),
        kind: kind_of(value.get("kind")),
        interpretation: answer_text(value.get("interpretation"), 500),
        approximate: value
            .get("approximate")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        rows: list(value.get("rows"))
            .filter_map(row)
            .take(MAX_ROWS)
            .collect(),
        units: Vec::new(),
        summary: paragraph(value.get("summary"), 4000),
        image_url: link(value.get("imageUrl")),
        url: link(value.get("url")),
        credits: list(value.get("credits"))
            .filter_map(credit)
            .take(MAX_CREDITS)
            .collect(),
        as_of: value
            .get("asOf")
            .and_then(Value::as_str)
            .and_then(parse_date),
        copy,
        kept: false,
        result,
    })
}

/// A provider's `answer` answer, `{ answers: [...] }`; nothing for `{ answers: [] }` or `null`.
pub fn parse_answers(value: &Value, engine: &str, name: &str) -> Vec<Answer> {
    list(value.get("answers"))
        .filter_map(|answer| parse_answer(answer, engine, name))
        .take(MAX_ANSWERS)
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn an_answer_keeps_what_the_app_shows() {
        let answer = json!({ "answers": [
            {
                "kind": "fact",
                "interpretation": "weather in  Paris,\n France",
                "result": { "text": "12 °C", "query": "Paris" },
                "rows": [
                    { "label": "Wind", "value": "14 km/h" },
                    { "label": "", "value": "dropped" },
                    { "label": "Formula", "value": { "tex": "\\frac{1}{2}" } }
                ],
                "summary": "Line one.\n\n  Line   two. ",
                "imageUrl": "file:///etc/passwd",
                "url": "https://example.org/paris",
                "credits": [
                    { "name": "Open-Meteo", "url": "https://open-meteo.com", "license": "CC BY 4.0" },
                    { "url": "https://nameless.example" }
                ],
                "asOf": "2026-11-11T14:30"
            },
            { "result": "" },
            { "interpretation": "no result" }
        ]});
        let answers = parse_answers(&answer, "dev.example/weather", "Weather");
        assert_eq!(answers.len(), 1, "an answer needs a result");
        let answer = &answers[0];
        assert_eq!(answer.engine, "dev.example/weather");
        assert_eq!(answer.kind, AnswerKind::Fact);
        assert_eq!(
            answer.interpretation.as_ref().unwrap().text,
            "weather in Paris, France"
        );
        assert_eq!(answer.result.text, "12 °C");
        assert_eq!(answer.result.query.as_deref(), Some("Paris"));
        assert_eq!(answer.copy, "12 °C");
        assert_eq!(answer.rows.len(), 2, "a row needs a label");
        let formula = answer.rows[1]
            .value
            .math
            .as_ref()
            .expect("TeX becomes MathML");
        assert!(formula.mathml.starts_with("<math"));
        assert_eq!(answer.rows[1].value.text, "\\frac{1}{2}");
        assert_eq!(answer.summary.as_deref(), Some("Line one.\nLine two."));
        assert_eq!(answer.image_url, None, "only web pictures");
        assert_eq!(answer.credits.len(), 1, "a credit needs a name");
        let as_of = answer.as_of.as_ref().unwrap();
        assert_eq!(
            (as_of.iso.as_str(), as_of.precision),
            ("2026-11-11T14:30", AnswerDatePrecision::Minute)
        );
        assert!(parse_answers(&json!(null), "x/y", "X").is_empty());
        assert!(parse_answers(&json!({ "answers": [] }), "x/y", "X").is_empty());
    }

    #[test]
    fn dates_are_read_as_far_as_they_go() {
        let precision = |iso: &str| parse_date(iso).map(|date| date.precision);
        assert_eq!(precision("2026"), Some(AnswerDatePrecision::Year));
        assert_eq!(precision("2026-11"), Some(AnswerDatePrecision::Month));
        assert_eq!(precision("2026-11-11"), Some(AnswerDatePrecision::Day));
        assert_eq!(precision("-0500"), Some(AnswerDatePrecision::Year));
        assert_eq!(parse_date("-0500").unwrap().iso, "-0500");
        assert_eq!(precision("2026-13-01"), None);
        assert_eq!(precision("yesterday"), None);
        assert_eq!(
            parse_date("2026-11-11T25:00").unwrap().precision,
            AnswerDatePrecision::Day,
            "a time that isn't one is left out"
        );
    }
}
