//! Calculations, conversions and number bases, computed by fend (fend-core: exact fractions,
//! arbitrary precision, units and currencies). Raygun reads the words first (`expr`, `units`),
//! decides whether they're a calculation at all, and writes the answer with its forms (fraction,
//! words, Roman numerals, bases, factors) and the units it can be toggled to.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use raygun_model::{AnswerKind, AnswerRow, AnswerText, AnswerUnit};

use super::expr::{self, Expr};
use super::numbers::{self, Shown};
use super::text::{split_last, strip_prefix_words};
use super::units::{self, CONVERTS, QUANTITIES, Quantity, System};
use super::{Found, blank, math_text};

/// How long one evaluation may take.
const EVALUATION: Duration = Duration::from_millis(150);
/// The longest input fend is given.
const MAX_INPUT: usize = 240;
/// Digits of a long decimal shown in the "more digits" row.
const MORE_DIGITS: usize = 30;

/// Words that come before a calculation.
const LEAD_INS: &[&str] = &[
    "what is the value of",
    "what is",
    "what's",
    "whats",
    "what are",
    "how much is",
    "how much are",
    "calculate",
    "compute",
    "evaluate",
    "convert",
    "please convert",
    "simplify",
];

struct Deadline(Instant);

impl fend_core::Interrupt for Deadline {
    fn should_interrupt(&self) -> bool {
        Instant::now() >= self.0
    }
}

/// Exchange rates against the euro (the ECB's: units of a currency per euro).
#[derive(Clone)]
struct Rates(HashMap<String, f64>);

impl fend_core::ExchangeRateFnV2 for Rates {
    fn relative_to_base_currency(
        &self,
        currency: &str,
        _options: &fend_core::ExchangeRateFnV2Options,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync + 'static>> {
        if currency == "EUR" {
            return Ok(1.0);
        }
        self.0
            .get(currency)
            .copied()
            .ok_or_else(|| format!("no exchange rate for {currency}").into())
    }
}

/// What fend made of an input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    /// The number as fend wrote it (`3.1068559612`, `1/3`, `ff`, `0 + i`), or the text of a
    /// date or a string (`MCMLXXXIV`).
    pub number: String,
    /// Its unit as fend wrote it (`miles`, `km / h`, `EUR`); empty for a plain number.
    pub unit: String,
    pub approximate: bool,
    /// Not a number: a date, words, a Roman numeral.
    pub text_only: bool,
}

/// How long all the evaluations for one query may take together.
const BUDGET: Duration = Duration::from_millis(900);

/// fend, with exchange rates when there are some.
pub struct Fend {
    context: fend_core::Context,
    /// No evaluation runs past this.
    until: Instant,
}

impl Fend {
    pub fn new(rates: Option<&HashMap<String, f64>>) -> Self {
        let mut context = fend_core::Context::new();
        if let Some(rates) = rates {
            context.set_exchange_rate_handler_v2(Rates(rates.clone()));
        }
        Self {
            context,
            until: Instant::now() + BUDGET,
        }
    }

    /// `input` computed, or why not.
    pub fn evaluate(&mut self, input: &str) -> Result<Value, String> {
        if input.len() > MAX_INPUT || input.contains([':', ';', '=', '@', '\\', '"']) {
            return Err("not a calculation".to_owned());
        }
        let lower = input.to_lowercase();
        if ["print", "roll", "sample", "version", "trans"]
            .iter()
            .any(|word| lower.contains(word))
        {
            return Err("not a calculation".to_owned());
        }
        let now = Instant::now();
        if now >= self.until {
            return Err("out of time".to_owned());
        }
        let deadline = Deadline((now + EVALUATION).min(self.until));
        let result = fend_core::evaluate_with_interrupt(input, &mut self.context, &deadline)?;
        if result.output_is_empty() {
            return Err("no answer".to_owned());
        }
        let mut number = String::new();
        let mut unit = String::new();
        let mut text_only = false;
        let mut approximate = false;
        for span in result.get_main_result_spans() {
            match span.kind() {
                fend_core::SpanKind::Number => number.push_str(span.string()),
                fend_core::SpanKind::Date | fend_core::SpanKind::String => {
                    text_only = true;
                    number.push_str(span.string());
                }
                _ => {
                    let text = span.string();
                    if let Some(rest) = text.strip_prefix("approx. ") {
                        approximate = true;
                        unit.push_str(rest);
                    } else {
                        unit.push_str(text);
                    }
                }
            }
        }
        let number = number.trim().to_owned();
        if number.is_empty() || number.len() > 20_000 {
            return Err("no number".to_owned());
        }
        Ok(Value {
            number,
            unit: unit.trim().to_owned(),
            approximate,
            text_only,
        })
    }
}

/// A value as answers show it: digits grouped, scientific notation, its unit.
pub fn shown(value: &Value) -> AnswerText {
    let unit = units::pretty(&value.unit);
    let number = if value.text_only {
        None
    } else {
        numbers::show(&value.number).or_else(|| fraction(&value.number))
    };
    let number = number.unwrap_or_else(|| Shown {
        text: value.number.clone(),
        tex: tex_escape(&value.number),
    });
    if unit.is_empty() {
        return math_text(number.text, &number.tex);
    }
    let joiner = if unit.starts_with('%') || unit.starts_with('°') && unit.len() <= 3 {
        ""
    } else {
        " "
    };
    math_text(
        format!("{}{joiner}{unit}", number.text),
        &format!("{}\\ {}", number.tex, units::tex(&value.unit)),
    )
}

/// `1/3` as a fraction; `1 1/3` as a mixed one.
fn fraction(text: &str) -> Option<Shown> {
    let (whole, rest) = match text.split_once(' ') {
        Some((whole, rest)) if rest.contains('/') => (Some(whole), rest),
        _ => (None, text),
    };
    let (top, bottom) = rest.split_once('/')?;
    let digits = |part: &str| {
        !part.is_empty()
            && part
                .trim_start_matches('-')
                .bytes()
                .all(|byte| byte.is_ascii_digit())
    };
    if !digits(top) || !digits(bottom) || whole.is_some_and(|whole| !digits(whole)) {
        return None;
    }
    let (sign, top) = match top.strip_prefix('-') {
        Some(top) => ("-", top),
        None => ("", top),
    };
    Some(match whole {
        Some(whole) => Shown {
            text: format!("{whole} {top}/{bottom}"),
            tex: format!("{whole}\\frac{{{top}}}{{{bottom}}}"),
        },
        None => Shown {
            text: format!("{sign}{top}/{bottom}"),
            tex: format!("{sign}\\frac{{{top}}}{{{bottom}}}"),
        },
    })
}

fn tex_escape(text: &str) -> String {
    let escaped: String = text
        .chars()
        .filter(|char| !matches!(char, '\\' | '{' | '}' | '$' | '#' | '&' | '_' | '^' | '~'))
        .collect();
    format!("\\text{{{escaped}}}")
}

/// Word operators made symbols: `2 plus 2`, `3 times 4`, `5 squared`, `square root of 16`.
pub fn symbols(lower: &str) -> String {
    let words: Vec<&str> = lower.split(' ').collect();
    let numeric = |word: Option<&&str>| {
        word.is_some_and(|word| {
            word.chars()
                .next()
                .is_some_and(|first| first.is_ascii_digit() || first == '(' || first == '.')
                || word.ends_with(')')
                || word
                    .chars()
                    .last()
                    .is_some_and(|last| last.is_ascii_digit())
                || matches!(*word, "pi" | "e")
        })
    };
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut at = 0;
    while at < words.len() {
        let word = words[at];
        let before = out.last().map(String::as_str);
        let before_numeric = numeric(before.as_ref());
        let next = words.get(at + 1);
        let replaced = match word {
            "plus" if before_numeric && numeric(next) => Some("+"),
            "minus" if before_numeric && numeric(next) => Some("-"),
            "times" | "x" | "multiplied" if before_numeric => {
                if word == "multiplied" && next == Some(&"by") {
                    at += 1;
                }
                numeric(words.get(at + 1)).then_some("*")
            }
            "divided" if before_numeric && next == Some(&"by") => {
                at += 1;
                Some("/")
            }
            "over" if before_numeric && numeric(next) => Some("/"),
            "modulo" if before_numeric => Some("mod"),
            "squared" if before_numeric => Some("^2"),
            "cubed" if before_numeric => Some("^3"),
            _ => None,
        };
        if let Some(symbol) = replaced {
            if symbol.starts_with('^') {
                if let Some(last) = out.last_mut() {
                    last.push_str(symbol);
                }
            } else {
                out.push(symbol.to_owned());
            }
            at += 1;
            continue;
        }
        // `to the power of`, `raised to the power of`, `raised to`.
        if before_numeric {
            for phrase in [
                &["raised", "to", "the", "power", "of"][..],
                &["to", "the", "power", "of"],
                &["raised", "to"],
            ] {
                if words.get(at..at + phrase.len()) == Some(phrase) {
                    out.push("^".to_owned());
                    at += phrase.len();
                    break;
                }
            }
            if out.last().is_some_and(|last| last == "^") {
                continue;
            }
        }
        // `square root of 16`, `cube root of 27`, `factorial of 5`, `5 factorial`.
        for (phrase, function) in [
            (&["the", "square", "root", "of"][..], "sqrt"),
            (&["square", "root", "of"], "sqrt"),
            (&["square", "root"], "sqrt"),
            (&["sqrt", "of"], "sqrt"),
            (&["the", "cube", "root", "of"], "cbrt"),
            (&["cube", "root", "of"], "cbrt"),
            (&["factorial", "of"], "!"),
        ] {
            if words.get(at..at + phrase.len()) == Some(phrase)
                && let Some(argument) = words.get(at + phrase.len())
                && numeric(Some(argument))
            {
                out.push(if function == "!" {
                    format!("{argument}!")
                } else {
                    format!("{function}({argument})")
                });
                at += phrase.len() + 1;
                break;
            }
        }
        if at < words.len() && words[at] == word {
            if word == "factorial" && before_numeric {
                if let Some(last) = out.last_mut() {
                    last.push('!');
                }
            } else {
                out.push(word.to_owned());
            }
            at += 1;
        }
    }
    out.join(" ")
}

/// `3x4` and `3 x 4` as multiplications.
fn times_x(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (at, char) in chars.iter().enumerate() {
        if *char == 'x' {
            let before = chars[..at].iter().rev().find(|char| **char != ' ');
            let after = chars[at + 1..].iter().find(|char| **char != ' ');
            let lone = chars
                .get(at.wrapping_sub(1))
                .is_none_or(|char| !char.is_alphabetic())
                && chars.get(at + 1).is_none_or(|char| !char.is_alphabetic());
            // `0xff` is a number in hexadecimal.
            let hex = at >= 1
                && chars[at - 1] == '0'
                && (at < 2 || !chars[at - 2].is_ascii_alphanumeric());
            if lone
                && !hex
                && before.is_some_and(|char| char.is_ascii_digit() || *char == ')')
                && after.is_some_and(|char| char.is_ascii_digit() || *char == '(')
            {
                out.push('*');
                continue;
            }
        }
        out.push(*char);
    }
    out
}

/// What a query is, read before anything is computed.
#[derive(Debug, Clone, PartialEq)]
enum Shape {
    /// A calculation without units.
    Arithmetic(Expr),
    /// `<quantity> to <unit>`.
    Conversion { from: String, to: String },
    /// Something fend reads that isn't plainly either: a quantity alone, a sum of quantities.
    Other,
}

/// Targets a number can be written in.
const BASES: &[(&str, &str)] = &[
    ("hex", "hexadecimal"),
    ("hexadecimal", "hexadecimal"),
    ("binary", "binary"),
    ("bin", "binary"),
    ("octal", "octal"),
    ("oct", "octal"),
    ("decimal", "decimal"),
    ("dec", "decimal"),
    ("roman", "Roman numerals"),
    ("roman numerals", "Roman numerals"),
    ("roman numeral", "Roman numerals"),
    ("words", "words"),
    ("fraction", "fraction"),
    ("a fraction", "fraction"),
    ("scientific", "scientific notation"),
    ("scientific notation", "scientific notation"),
];

fn base_target(target: &str) -> Option<(&'static str, String)> {
    let target = target.trim();
    if let Some((_, name)) = BASES.iter().find(|(word, _)| *word == target) {
        let fend = match *name {
            "Roman numerals" => "roman",
            "hexadecimal" => "hex",
            other => other,
        };
        return Some((name, fend.to_owned()));
    }
    let base = target.strip_prefix("base ")?.trim();
    let number: u32 = base.parse().ok()?;
    (2..=36)
        .contains(&number)
        .then(|| ("base", format!("base {number}")))
}

/// A query that isn't anything else, computed: arithmetic, a conversion, a number base, a
/// currency. `rates` are the euro's exchange rates, when there are some.
#[cfg(test)]
pub fn answer(text: &str, rates: Option<&HashMap<String, f64>>) -> Option<Found> {
    answer_for(text, rates, false)
}

/// [`answer`] for Search → All's card (`card`) or the Answers pill: the card skips what would
/// only be shown on the pill (numbers and quantities alone), so a title costs no computing.
pub fn answer_for(text: &str, rates: Option<&HashMap<String, f64>>, card: bool) -> Option<Found> {
    let lower = text.to_lowercase();
    let (rest, asked) = strip_prefix_words(&lower, LEAD_INS);
    let rest = how_many(rest).unwrap_or_else(|| rest.to_owned());
    let rest = times_x(&symbols(&rest));
    // Keep the case of units fend tells apart (`MB`, `Mb`, `K`): words as typed where only
    // their case differs.
    let typed: Vec<&str> = text.split(' ').collect();
    let cased: Vec<String> = rest
        .split(' ')
        .map(|word| {
            typed
                .iter()
                .find(|typed| typed.to_lowercase() == word)
                .map_or_else(|| word.to_owned(), |typed| (*typed).to_owned())
        })
        .collect();
    let input = currency_codes(&units::for_fend(&cased.join(" ")), rates.is_some());
    let mut fend = Fend::new(rates);

    // A number written in another base.
    if let Some((from, _, to)) = split_last(&input, CONVERTS)
        && let Some((name, target)) = base_target(&to.to_lowercase())
    {
        return number_base(&mut fend, from, name, &target);
    }
    let lowered = input.to_lowercase();
    if (lowered.starts_with("0x") || lowered.starts_with("0b") || lowered.starts_with("0o"))
        && !lowered.contains(' ')
    {
        return number_base(&mut fend, &input, "decimal", "decimal");
    }

    let shape = match expr::parse(&input) {
        Some(parsed) => Shape::Arithmetic(parsed),
        None => match split_last(&input, CONVERTS) {
            Some((from, _, to)) => Shape::Conversion {
                from: from.to_owned(),
                to: to.to_owned(),
            },
            None => Shape::Other,
        },
    };
    match shape {
        Shape::Arithmetic(parsed) => {
            if !parsed.computes() {
                // A number alone: its forms, but no question.
                return if asked || card {
                    None
                } else {
                    number_facts(&mut fend, &input)
                };
            }
            let value = fend.evaluate(&input).ok()?;
            let strong = !parsed.looks_like_a_code(&input);
            let mut answer = blank(AnswerKind::Calculation);
            answer.interpretation = Some(math_text(input.clone(), &parsed.tex()));
            answer.result = shown(&value);
            answer.approximate = value.approximate;
            answer.copy = copy_text(&value);
            // The card shows the result alone; its other forms are for the pill.
            if !card {
                answer.rows = forms(&mut fend, &input, &value);
            }
            Some(Found { answer, strong })
        }
        Shape::Conversion { from, to } => {
            let value = fend.evaluate(&input).ok()?;
            if value.text_only || from.trim().is_empty() {
                return None;
            }
            let source = fend.evaluate(&from).ok()?;
            let currency = is_currency(&source.unit) || is_currency(&value.unit);
            let mut answer = blank(if currency {
                AnswerKind::Currency
            } else {
                AnswerKind::Conversion
            });
            answer.interpretation = Some(AnswerText::plain(format!(
                "{} → {}",
                shown(&source).text,
                units::pretty(&to)
            )));
            answer.result = shown(&value);
            answer.approximate = value.approximate;
            answer.copy = copy_text(&value);
            if !card {
                answer.units = toggles(&mut fend, &format!("({from})"), &value.unit);
            }
            if currency {
                answer.result = money(&value);
                answer.copy = answer.result.text.clone();
            }
            Some(Found {
                answer,
                strong: true,
            })
        }
        Shape::Other => {
            // Quantities alone (`8 miles`) or added up (`3 kg + 500 g`): only with a number.
            if !input.chars().any(|char| char.is_ascii_digit()) {
                return None;
            }
            // A quantity alone (`8 Mile`, `60 Minutes`) is only ever shown on the pill.
            let operation = ['+', '-', '*', '/', '^']
                .iter()
                .any(|op| input.contains(*op));
            if card && !operation {
                return None;
            }
            let value = fend.evaluate(&input).ok()?;
            if value.text_only || value.unit.is_empty() {
                return None;
            }
            let mut answer = blank(AnswerKind::Conversion);
            answer.interpretation = Some(AnswerText::plain(input.clone()));
            answer.units = toggles(&mut fend, &format!("({input})"), &value.unit);
            // A quantity alone is shown in the other system's closest unit.
            let other = counterpart(&mut fend, &input);
            let shown_value = other.as_ref().unwrap_or(&value);
            answer.result = shown(shown_value);
            answer.approximate = shown_value.approximate;
            answer.copy = copy_text(shown_value);
            Some(Found {
                answer,
                strong: operation,
            })
        }
    }
}

/// `how many feet in a mile` → `1 mile to feet`.
fn how_many(lower: &str) -> Option<String> {
    let rest = lower
        .strip_prefix("how many ")
        .or_else(|| lower.strip_prefix("how much "))?;
    for joiner in [
        " are in ",
        " are there in ",
        " in ",
        " is ",
        " make ",
        " in a ",
        " in an ",
        " per ",
    ] {
        if let Some((unit, quantity)) = rest.split_once(joiner) {
            let quantity = quantity.trim();
            let quantity = match quantity.split_once(' ') {
                Some((count, unit)) if super::text::count_of(count).is_some() => {
                    format!("{} {unit}", super::text::count_of(count)?)
                }
                _ => {
                    if quantity
                        .chars()
                        .next()
                        .is_some_and(|char| char.is_ascii_digit())
                    {
                        quantity.to_owned()
                    } else {
                        format!("1 {quantity}")
                    }
                }
            };
            let quantity = if joiner.ends_with(" a ") || joiner.ends_with(" an ") {
                format!("1 {}", quantity.trim_start_matches("1 "))
            } else {
                quantity
            };
            return Some(format!("{quantity} to {}", unit.trim()));
        }
    }
    None
}

/// The forms of a calculation's result: the exact fraction, more digits, words, Roman
/// numerals, other bases and prime factors.
fn forms(fend: &mut Fend, input: &str, value: &Value) -> Vec<AnswerRow> {
    let mut rows = Vec::new();
    if !value.unit.is_empty() && value.unit != "%" || value.text_only {
        return rows;
    }
    let plain = value.number.replace(',', "");
    let integer = plain
        .strip_prefix('-')
        .unwrap_or(&plain)
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        && !value.approximate;
    if value.approximate {
        if let Ok(exact) = fend.evaluate(&format!("({input}) to fraction"))
            && let Some(shown_fraction) = fraction(&exact.number)
            && !exact.approximate
            && exact.number.len() <= 24
        {
            rows.push(row(
                "fraction",
                "Exact form",
                math_text(shown_fraction.text, &shown_fraction.tex),
            ));
        }
        if let Ok(more) = fend.evaluate(&format!("({input}) to {MORE_DIGITS} sf")) {
            rows.push(row(
                "moreDigits",
                "More digits",
                AnswerText::plain(format!("{}…", more.number)),
            ));
        }
    }
    if let Some(shown_number) = numbers::show(&value.number)
        && shown_number.text.contains('×')
        && !value.approximate
        && plain.len() <= 400
    {
        // Every digit of a large whole number.
        rows.push(row(
            "digits",
            "Full number",
            AnswerText::plain(numbers::grouped(&plain)),
        ));
    }
    if integer && plain.len() <= 18 {
        let number: i128 = plain.parse().unwrap_or_default();
        if number.unsigned_abs() < 1_000_000_000_000_000
            && let Ok(words) = fend.evaluate(&format!("{plain} to words"))
        {
            rows.push(row("words", "In words", AnswerText::plain(words.number)));
        }
        if (1..4000).contains(&number)
            && let Ok(roman) = fend.evaluate(&format!("{plain} to roman"))
        {
            rows.push(row(
                "roman",
                "Roman numerals",
                AnswerText::plain(roman.number),
            ));
        }
        if number >= 0 {
            let number = number as u128;
            rows.push(row(
                "binary",
                "Binary",
                AnswerText::plain(format!("{number:b}")),
            ));
            rows.push(row(
                "hexadecimal",
                "Hexadecimal",
                AnswerText::plain(format!("{number:X}")),
            ));
            if let Some(factors) = u64::try_from(number).ok().and_then(numbers::prime_factors) {
                let prime = factors.len() == 1 && factors[0].1 == 1;
                let shown_factors = numbers::factors_shown(&factors);
                rows.push(if prime {
                    row(
                        "prime",
                        "Prime factorization",
                        AnswerText::plain(format!("{number} is a prime number")),
                    )
                } else {
                    row(
                        "factors",
                        "Prime factorization",
                        math_text(shown_factors.text, &shown_factors.tex),
                    )
                });
            }
        }
    }
    rows
}

fn row(key: &str, label: &str, value: AnswerText) -> AnswerRow {
    AnswerRow {
        key: key.to_owned(),
        label: label.to_owned(),
        value,
    }
}

/// A number alone: its forms (words, Roman numerals, bases, factors). Never a question.
fn number_facts(fend: &mut Fend, input: &str) -> Option<Found> {
    let value = fend.evaluate(input).ok()?;
    if !value.unit.is_empty() || value.text_only {
        return None;
    }
    let rows = forms(fend, input, &value);
    if rows.is_empty() {
        return None;
    }
    let mut answer = blank(AnswerKind::NumberBase);
    answer.result = shown(&value);
    answer.approximate = value.approximate;
    answer.copy = copy_text(&value);
    answer.rows = rows;
    Some(Found {
        answer,
        strong: false,
    })
}

fn number_base(fend: &mut Fend, from: &str, name: &str, target: &str) -> Option<Found> {
    let value = fend.evaluate(&format!("{from} to {target}")).ok()?;
    let source = fend.evaluate(from).ok()?;
    if !source.unit.is_empty() {
        return None;
    }
    let mut answer = blank(AnswerKind::NumberBase);
    answer.interpretation = Some(AnswerText::plain(format!("{} in {name}", from.trim())));
    answer.result = if value.text_only || name != "decimal" {
        AnswerText::plain(value.number.clone())
    } else {
        shown(&value)
    };
    answer.copy = value.number.clone();
    answer.approximate = value.approximate;
    let decimal = fend.evaluate(&format!("{from} to decimal")).ok();
    if let Some(decimal) = decimal.filter(|decimal| !decimal.approximate) {
        let mut rows = forms(fend, &decimal.number.replace(',', ""), &decimal);
        // The form asked for is the result: not a row as well.
        let asked = match name {
            "Roman numerals" => "roman",
            other => other,
        };
        rows.retain(|row| row.key != asked);
        if name != "decimal" {
            rows.insert(0, row("decimal", "Decimal", shown(&decimal)));
        }
        answer.rows = rows;
    }
    Some(Found {
        answer,
        strong: true,
    })
}

/// What Copy takes: the number and its unit, digits not grouped.
fn copy_text(value: &Value) -> String {
    let unit = units::pretty(&value.unit);
    if unit.is_empty() {
        value.number.clone()
    } else {
        format!("{} {unit}", value.number)
    }
}

/// The quantity fend's unit measures.
fn quantity_of(fend: &mut Fend, expression: &str) -> Option<&'static Quantity> {
    QUANTITIES.iter().find(|quantity| {
        let base = quantity.units[0].0;
        fend.evaluate(&format!("{expression} to {base}")).is_ok()
    })
}

/// The number fend wrote, when it's a plain decimal.
fn number_of(value: &Value) -> Option<f64> {
    value.number.replace(',', "").parse::<f64>().ok()
}

/// The unit of `quantity` an expression is written in: the one converting to which changes
/// nothing.
fn own_unit(
    fend: &mut Fend,
    expression: &str,
    quantity: &'static Quantity,
) -> Option<&'static units::Unit> {
    let value = number_of(&fend.evaluate(expression).ok()?)?;
    quantity.units.iter().find(|(fend_unit, _, _)| {
        fend.evaluate(&format!("{expression} to {fend_unit}"))
            .ok()
            .and_then(|converted| number_of(&converted))
            .is_some_and(|converted| (converted - value).abs() <= 1e-9 * value.abs().max(1e-300))
    })
}

/// The units of the result's quantity, each with the value in it, for the units toggle.
fn toggles(fend: &mut Fend, expression: &str, unit: &str) -> Vec<AnswerUnit> {
    if is_currency(unit) {
        return currency_toggles(fend, expression, unit);
    }
    let Some(quantity) = quantity_of(fend, expression) else {
        return Vec::new();
    };
    quantity
        .units
        .iter()
        .filter_map(|(fend_unit, shown_unit, system)| {
            let value = fend
                .evaluate(&format!("{expression} to {fend_unit}"))
                .ok()?;
            Some(AnswerUnit {
                symbol: (*shown_unit).to_owned(),
                system: system.name().to_owned(),
                value: rounded(&value, shown_unit),
            })
        })
        .collect()
}

/// A toggle's value: 6 significant digits and the unit as the toggle names it.
fn rounded(value: &Value, unit: &str) -> AnswerText {
    let number = value.number.replace(',', "").parse::<f64>().ok();
    let shown_number = match number {
        Some(number) => numbers::show_f64(number, 6),
        None => Shown {
            text: value.number.clone(),
            tex: tex_escape(&value.number),
        },
    };
    let joiner = if unit.starts_with('°') && unit.len() <= 3 {
        ""
    } else {
        " "
    };
    math_text(
        format!("{}{joiner}{unit}", shown_number.text),
        &format!("{}\\ {}", shown_number.tex, units::tex(unit)),
    )
}

/// The same quantity in the other system's unit that writes it best (between 1 and 1000 when
/// one does): 8 miles in kilometres, 100 °F in °C.
fn counterpart(fend: &mut Fend, input: &str) -> Option<Value> {
    let expression = format!("({input})");
    let quantity = quantity_of(fend, &expression)?;
    let own = own_unit(fend, &expression, quantity)?;
    let wanted = match own.2 {
        System::Metric => System::Imperial,
        System::Imperial => System::Metric,
        System::Neither => return None,
    };
    let mut best: Option<(f64, Value)> = None;
    for (fend_unit, _, system) in quantity.units {
        if *system != wanted {
            continue;
        }
        let Ok(value) = fend.evaluate(&format!("({input}) to {fend_unit}")) else {
            continue;
        };
        let Ok(number) = value.number.replace(',', "").parse::<f64>() else {
            continue;
        };
        let size = number.abs();
        // Distance from the range 1–1000, in powers of ten.
        let distance = if size == 0.0 {
            0.0
        } else if size < 1.0 {
            -size.log10()
        } else if size >= 1000.0 {
            size.log10() - 3.0
        } else {
            0.0
        };
        if best.as_ref().is_none_or(|(known, _)| distance < *known) {
            best = Some((distance, value));
        }
    }
    best.map(|(_, value)| value)
}

// ---- Currencies ----

/// Currencies the ECB publishes rates for, with the euro.
pub const CURRENCIES: &[&str] = &[
    "EUR", "USD", "JPY", "BGN", "CZK", "DKK", "GBP", "HUF", "PLN", "RON", "SEK", "CHF", "ISK",
    "NOK", "TRY", "AUD", "BRL", "CAD", "CNY", "HKD", "IDR", "ILS", "INR", "KRW", "MXN", "MYR",
    "NZD", "PHP", "SGD", "THB", "ZAR",
];

/// Currencies by the names people type (and their symbols fend doesn't know).
const CURRENCY_NAMES: &[(&[&str], &str)] = &[
    (&["us", "dollars"], "USD"),
    (&["us", "dollar"], "USD"),
    (&["american", "dollars"], "USD"),
    (&["canadian", "dollars"], "CAD"),
    (&["canadian", "dollar"], "CAD"),
    (&["australian", "dollars"], "AUD"),
    (&["australian", "dollar"], "AUD"),
    (&["new", "zealand", "dollars"], "NZD"),
    (&["new", "zealand", "dollar"], "NZD"),
    (&["hong", "kong", "dollars"], "HKD"),
    (&["hong", "kong", "dollar"], "HKD"),
    (&["singapore", "dollars"], "SGD"),
    (&["singapore", "dollar"], "SGD"),
    (&["swiss", "francs"], "CHF"),
    (&["swiss", "franc"], "CHF"),
    (&["british", "pounds"], "GBP"),
    (&["pounds", "sterling"], "GBP"),
    (&["pound", "sterling"], "GBP"),
    (&["japanese", "yen"], "JPY"),
    (&["chinese", "yuan"], "CNY"),
    (&["indian", "rupees"], "INR"),
    (&["indian", "rupee"], "INR"),
    (&["south", "korean", "won"], "KRW"),
    (&["korean", "won"], "KRW"),
    (&["mexican", "pesos"], "MXN"),
    (&["mexican", "peso"], "MXN"),
    (&["brazilian", "reais"], "BRL"),
    (&["brazilian", "real"], "BRL"),
    (&["south", "african", "rand"], "ZAR"),
    (&["swedish", "kronor"], "SEK"),
    (&["swedish", "krona"], "SEK"),
    (&["norwegian", "kroner"], "NOK"),
    (&["norwegian", "krone"], "NOK"),
    (&["danish", "kroner"], "DKK"),
    (&["danish", "krone"], "DKK"),
    (&["icelandic", "kronur"], "ISK"),
    (&["icelandic", "krona"], "ISK"),
    (&["euros"], "EUR"),
    (&["euro"], "EUR"),
    (&["dollars"], "USD"),
    (&["dollar"], "USD"),
    (&["bucks"], "USD"),
    (&["pounds"], "GBP"),
    (&["pound"], "GBP"),
    (&["quid"], "GBP"),
    (&["sterling"], "GBP"),
    (&["yen"], "JPY"),
    (&["yuan"], "CNY"),
    (&["renminbi"], "CNY"),
    (&["rmb"], "CNY"),
    (&["rupees"], "INR"),
    (&["rupee"], "INR"),
    (&["won"], "KRW"),
    (&["francs"], "CHF"),
    (&["franc"], "CHF"),
    (&["reais"], "BRL"),
    (&["rand"], "ZAR"),
    (&["pesos"], "MXN"),
    (&["peso"], "MXN"),
    (&["zloty"], "PLN"),
    (&["złoty"], "PLN"),
    (&["zlotys"], "PLN"),
    (&["forint"], "HUF"),
    (&["forints"], "HUF"),
    (&["lira"], "TRY"),
    (&["ringgit"], "MYR"),
    (&["baht"], "THB"),
    (&["shekels"], "ILS"),
    (&["shekel"], "ILS"),
    (&["rupiah"], "IDR"),
    (&["koruna"], "CZK"),
    (&["korunas"], "CZK"),
    (&["kronor"], "SEK"),
    (&["krona"], "SEK"),
    (&["lev"], "BGN"),
    (&["leva"], "BGN"),
    (&["lei"], "RON"),
    (&["leu"], "RON"),
];

/// Symbols in front of an amount, with their currency.
const CURRENCY_SYMBOLS: &[(&str, &str)] = &[
    ("US$", "USD"),
    ("CA$", "CAD"),
    ("C$", "CAD"),
    ("AU$", "AUD"),
    ("A$", "AUD"),
    ("NZ$", "NZD"),
    ("HK$", "HKD"),
    ("S$", "SGD"),
    ("R$", "BRL"),
    ("$", "USD"),
    ("€", "EUR"),
    ("£", "GBP"),
    ("¥", "JPY"),
    ("₹", "INR"),
    ("₩", "KRW"),
    ("₺", "TRY"),
    ("₪", "ILS"),
    ("฿", "THB"),
    ("zł", "PLN"),
];

pub fn is_currency(unit: &str) -> bool {
    CURRENCIES.contains(&unit.trim())
}

/// The currency a word names (`dollars`, `usd`, `€`).
fn currency_of_word(word: &str) -> Option<&'static str> {
    let upper = word.to_uppercase();
    CURRENCIES
        .iter()
        .find(|code| **code == upper)
        .copied()
        .or_else(|| {
            CURRENCY_SYMBOLS
                .iter()
                .find(|(symbol, _)| *symbol == word)
                .map(|(_, code)| *code)
        })
}

/// Whether the words name currencies on both sides of a conversion, or add amounts of them:
/// then exchange rates are wanted.
pub fn mentions_currency(lower: &str) -> bool {
    let coded = currency_codes(&units::for_fend(lower), true);
    let words: Vec<&str> = coded.split(' ').collect();
    let currencies = words
        .iter()
        .filter(|word| CURRENCIES.contains(&word.trim_matches(['(', ')'])))
        .count();
    currencies >= 2 || (currencies == 1 && words.iter().any(|word| CONVERTS.contains(word)))
}

/// Currency names and symbols as ISO codes (`50 euros in dollars` → `50 EUR in USD`), only
/// where rates are known and a currency is plainly meant: `pounds` is a weight unless the other
/// side of the conversion is money.
pub fn currency_codes(text: &str, rates: bool) -> String {
    if !rates {
        return text.to_owned();
    }
    let words: Vec<&str> = text.split(' ').filter(|word| !word.is_empty()).collect();
    let lower: Vec<String> = words.iter().map(|word| word.to_lowercase()).collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut at = 0;
    'words: while at < words.len() {
        for (phrase, code) in CURRENCY_NAMES {
            let end = at + phrase.len();
            if end <= lower.len()
                && lower[at..end]
                    .iter()
                    .zip(phrase.iter())
                    .all(|(a, b)| a == b)
            {
                out.push((*code).to_owned());
                at = end;
                continue 'words;
            }
        }
        // `$50`, `50€`, `€50`.
        let word = words[at];
        let mut done = false;
        for (symbol, code) in CURRENCY_SYMBOLS {
            if let Some(amount) = word.strip_prefix(symbol)
                && amount.parse::<f64>().is_ok()
            {
                out.push(format!("{amount} {code}"));
                done = true;
                break;
            }
            if let Some(amount) = word.strip_suffix(symbol)
                && amount.parse::<f64>().is_ok()
            {
                out.push(format!("{amount} {code}"));
                done = true;
                break;
            }
        }
        if !done {
            out.push(match currency_of_word(word) {
                Some(code) if word.len() == 3 || !word.chars().all(char::is_alphabetic) => {
                    code.to_owned()
                }
                _ => word.to_owned(),
            });
        }
        at += 1;
    }
    let coded = out.join(" ");
    // `pounds`, `rand`, `won`… only count as money beside money.
    let codes = coded
        .split(' ')
        .filter(|word| CURRENCIES.contains(word))
        .count();
    if codes >= 2 || (codes == 1 && !text.split(' ').any(units_word)) {
        coded
    } else {
        text.to_owned()
    }
}

/// A word that's a unit of something other than money (`kg`, `miles`).
fn units_word(word: &str) -> bool {
    let word = word.to_lowercase();
    matches!(
        word.as_str(),
        "kg" | "g"
            | "lb"
            | "lbs"
            | "oz"
            | "km"
            | "m"
            | "mi"
            | "miles"
            | "ft"
            | "feet"
            | "l"
            | "ml"
            | "stone"
            | "grams"
            | "kilograms"
            | "ounces"
    )
}

/// An amount of money: two decimals (none for yen and won), digits grouped, then the code.
fn money(value: &Value) -> AnswerText {
    let number = value
        .number
        .replace(',', "")
        .parse::<f64>()
        .unwrap_or(f64::NAN);
    let decimals = if matches!(value.unit.as_str(), "JPY" | "KRW" | "ISK" | "IDR") {
        0
    } else {
        2
    };
    let fixed = format!("{number:.decimals$}");
    let (whole, fraction) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let (sign, whole) = match whole.strip_prefix('-') {
        Some(whole) => ("-", whole),
        None => ("", whole),
    };
    let grouped = numbers::grouped(whole);
    let point = if fraction.is_empty() {
        String::new()
    } else {
        format!(".{fraction}")
    };
    let text = format!("{sign}{grouped}{point} {}", value.unit);
    math_text(
        text,
        &format!(
            "{sign}{}{point}\\ \\mathrm{{{}}}",
            grouped.replace(',', "{,}"),
            value.unit
        ),
    )
}

/// Other currencies the toggle offers.
const TOGGLED_CURRENCIES: &[&str] = &[
    "EUR", "USD", "GBP", "JPY", "CHF", "CNY", "INR", "CAD", "AUD",
];

fn currency_toggles(fend: &mut Fend, expression: &str, unit: &str) -> Vec<AnswerUnit> {
    let mut codes: Vec<&str> = TOGGLED_CURRENCIES.to_vec();
    if !codes.contains(&unit) {
        codes.insert(0, unit);
    }
    codes
        .into_iter()
        .filter_map(|code| {
            let value = fend.evaluate(&format!("{expression} to {code}")).ok()?;
            Some(AnswerUnit {
                symbol: code.to_owned(),
                system: String::new(),
                value: money(&value),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rates() -> HashMap<String, f64> {
        HashMap::from([
            ("USD".to_owned(), 1.08),
            ("GBP".to_owned(), 0.85),
            ("JPY".to_owned(), 160.0),
            ("CHF".to_owned(), 0.95),
        ])
    }

    fn found(query: &str) -> Found {
        answer(query, Some(&rates())).unwrap_or_else(|| panic!("no answer for {query}"))
    }

    fn result(query: &str) -> String {
        found(query).answer.result.text
    }

    #[test]
    fn arithmetic_is_computed() {
        assert_eq!(result("1+1"), "2");
        assert_eq!(result("2+2*3"), "8");
        assert_eq!(result("what is 2 + 2"), "4");
        assert_eq!(result("2^10"), "1,024");
        assert_eq!(result("5!"), "120");
        assert_eq!(result("10% of 50"), "5");
        assert_eq!(result("1/3 + 1/6"), "0.5");
        assert_eq!(result("3 x 4"), "12");
        assert_eq!(result("3x4"), "12");
        assert_eq!(result("12 divided by 4"), "3");
        assert_eq!(result("2 plus 2"), "4");
        assert_eq!(result("9 squared"), "81");
        assert_eq!(result("square root of 16"), "4");
        assert_eq!(result("2 to the power of 8"), "256");
        let root = found("sqrt(2)");
        assert!(root.strong);
        assert!(root.answer.approximate);
        assert_eq!(root.answer.result.text, "1.4142135624");
        assert!(root.answer.interpretation.as_ref().unwrap().math.is_some());
        assert!(root.answer.rows.iter().any(|row| row.key == "moreDigits"));
    }

    #[test]
    fn whole_results_have_their_forms() {
        let rows = found("1000 + 984").answer.rows;
        let value = |key: &str| {
            rows.iter()
                .find(|row| row.key == key)
                .map(|row| row.value.text.clone())
        };
        assert_eq!(value("roman").as_deref(), Some("MCMLXXXIV"));
        assert_eq!(value("binary").as_deref(), Some("11111000000"));
        assert_eq!(value("hexadecimal").as_deref(), Some("7C0"));
        assert_eq!(value("factors").as_deref(), Some("2⁶ × 31"));
        assert!(
            value("words")
                .unwrap()
                .starts_with("one thousand nine hundred")
        );
        let third = found("1/3 + 1/4").answer;
        assert_eq!(
            third
                .rows
                .iter()
                .find(|row| row.key == "fraction")
                .unwrap()
                .value
                .text,
            "7/12"
        );
    }

    #[test]
    fn units_convert() {
        assert_eq!(result("5 km to miles"), "3.1068559612 miles");
        assert_eq!(result("100 F to C"), "37.7777777778°C");
        assert_eq!(
            result("100 degrees fahrenheit in celsius"),
            "37.7777777778°C"
        );
        assert_eq!(result("1 GiB to MB"), "1,073.741824 MB");
        assert_eq!(result("3 hours in minutes"), "180 minutes");
        assert_eq!(result("how many feet in a mile"), "5,280 feet");
        assert_eq!(result("convert 6 ft to m"), "1.8288 m");
        assert_eq!(result("5 fl oz to ml"), "147.8676478125 ml");
        assert_eq!(result("1 nautical mile to km"), "1.852 km");
        let miles = found("5 km to mi").answer;
        assert_eq!(miles.kind, AnswerKind::Conversion);
        assert!(
            miles
                .units
                .iter()
                .any(|unit| unit.symbol == "ft" && unit.system == "imperial")
        );
        assert!(
            miles
                .units
                .iter()
                .any(|unit| unit.symbol == "m" && unit.value.text == "5,000 m")
        );
    }

    #[test]
    fn bases_convert() {
        assert_eq!(result("255 to hex"), "ff");
        assert_eq!(result("255 in binary"), "11111111");
        assert_eq!(result("0xff to decimal"), "255");
        assert_eq!(result("0xff"), "255");
        assert_eq!(result("1984 in roman numerals"), "MCMLXXXIV");
        assert_eq!(result("10 in base 3"), "101");
    }

    #[test]
    fn currencies_convert_at_the_rates_given() {
        let euros = found("100 usd to eur").answer;
        assert_eq!(euros.kind, AnswerKind::Currency);
        assert_eq!(euros.result.text, "92.59 EUR");
        assert_eq!(result("50 euros in dollars"), "54.00 USD");
        assert_eq!(result("$50 to gbp"), "39.35 GBP");
        assert_eq!(result("1000 yen to usd"), "6.75 USD");
        assert!(
            euros
                .units
                .iter()
                .any(|unit| unit.symbol == "JPY" && unit.value.text == "14,815 JPY")
        );
        // Pounds are a weight beside kilograms.
        assert_eq!(result("10 pounds to kg"), "4.5359237 kg");
        assert!(
            answer("100 usd to eur", None).is_none(),
            "no rates, no answer"
        );
    }

    #[test]
    fn titles_and_numbers_alone_are_not_questions() {
        for title in [
            "Blue",
            "The Matrix",
            "Hot Fuzz",
            "Maroon 5",
            "U2",
            "Catch-22",
            "12 Angry Men",
        ] {
            assert!(answer(title, None).is_none(), "{title}");
        }
        for weak in [
            "1984",
            "8 Mile",
            "60 Minutes",
            "7 seconds",
            "50 Cent",
            "9/11",
            "1939-1945",
            "1-800-273-8255",
        ] {
            if let Some(found) = answer(weak, None) {
                assert!(!found.strong, "{weak} isn't a question");
            }
        }
        let number = answer("1984", None).unwrap().answer;
        assert!(number.rows.iter().any(|row| row.key == "roman"));
        let mile = answer("8 mile", None).unwrap().answer;
        assert_eq!(mile.result.text, "12.874752 km");
        assert!(answer("seven", None).is_none_or(|found| !found.strong));
        assert!(answer("x = 5", None).is_none());
        assert!(answer("roll d6", None).is_none());
    }

    #[test]
    fn words_become_symbols_only_between_numbers() {
        assert_eq!(symbols("2 plus 2"), "2 + 2");
        assert_eq!(symbols("plus one"), "plus one");
        assert_eq!(symbols("3 times 4"), "3 * 4");
        assert_eq!(symbols("5 squared"), "5^2");
        assert_eq!(symbols("factorial of 5"), "5!");
        assert_eq!(symbols("5 factorial"), "5!");
        assert_eq!(symbols("square root of 16"), "sqrt(16)");
        assert_eq!(symbols("20 divided by 4"), "20 / 4");
        assert_eq!(times_x("3x4"), "3*4");
        assert_eq!(times_x("max 3x"), "max 3x");
        assert_eq!(
            how_many("how many feet in a mile").as_deref(),
            Some("1 mile to feet")
        );
        assert_eq!(
            how_many("how many cups in 2 liters").as_deref(),
            Some("2 liters to cups")
        );
    }
}
