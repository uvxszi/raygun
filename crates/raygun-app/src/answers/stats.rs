//! Statistics of a list of numbers: `mean of 1, 2, 3`, `median 4 8 15 16 23 42`, `standard
//! deviation of …`.

use raygun_model::{AnswerKind, AnswerRow, AnswerText};

use super::numbers::show_f64;
use super::{Found, blank, math_text};

/// Numbers a list may hold.
const MAX_NUMBERS: usize = 1000;
/// Significant digits shown.
const DIGITS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Measure {
    Mean,
    Median,
    Mode,
    Sum,
    Product,
    Min,
    Max,
    Range,
    Variance,
    Deviation,
    Count,
    /// Everything at once (`statistics of …`).
    Summary,
}

const NAMES: &[(&str, Measure)] = &[
    ("arithmetic mean", Measure::Mean),
    ("mean", Measure::Mean),
    ("average", Measure::Mean),
    ("avg", Measure::Mean),
    ("median", Measure::Median),
    ("mode", Measure::Mode),
    ("sum", Measure::Sum),
    ("total", Measure::Sum),
    ("product", Measure::Product),
    ("minimum", Measure::Min),
    ("min", Measure::Min),
    ("smallest", Measure::Min),
    ("lowest", Measure::Min),
    ("maximum", Measure::Max),
    ("max", Measure::Max),
    ("largest", Measure::Max),
    ("biggest", Measure::Max),
    ("highest", Measure::Max),
    ("range", Measure::Range),
    ("variance", Measure::Variance),
    ("standard deviation", Measure::Deviation),
    ("std dev", Measure::Deviation),
    ("std deviation", Measure::Deviation),
    ("stdev", Measure::Deviation),
    ("stddev", Measure::Deviation),
    ("std", Measure::Deviation),
    ("sd", Measure::Deviation),
    ("count", Measure::Count),
    ("summary statistics", Measure::Summary),
    ("statistics", Measure::Summary),
    ("stats", Measure::Summary),
];

impl Measure {
    fn key(self) -> &'static str {
        match self {
            Self::Mean => "mean",
            Self::Median => "median",
            Self::Mode => "mode",
            Self::Sum => "sum",
            Self::Product => "product",
            Self::Min => "min",
            Self::Max => "max",
            Self::Range => "range",
            Self::Variance => "variance",
            Self::Deviation => "deviation",
            Self::Count => "count",
            Self::Summary => "summary",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Mean => "Mean",
            Self::Median => "Median",
            Self::Mode => "Mode",
            Self::Sum => "Sum",
            Self::Product => "Product",
            Self::Min => "Minimum",
            Self::Max => "Maximum",
            Self::Range => "Range",
            Self::Variance => "Sample variance",
            Self::Deviation => "Sample standard deviation",
            Self::Count => "Count",
            Self::Summary => "Statistics",
        }
    }
}

/// Numbers in a list: separated by commas, semicolons, spaces or `and`.
fn numbers(text: &str) -> Option<Vec<f64>> {
    let text = text.trim().trim_start_matches('(').trim_end_matches(')');
    let text = text.trim_start_matches('[').trim_end_matches(']');
    let mut found = Vec::new();
    for part in text
        .split([',', ';', ' '])
        .map(str::trim)
        .filter(|part| !part.is_empty() && *part != "and")
    {
        let number: f64 = part.parse().ok()?;
        if !number.is_finite() {
            return None;
        }
        found.push(number);
        if found.len() > MAX_NUMBERS {
            return None;
        }
    }
    (!found.is_empty()).then_some(found)
}

/// The measure asked for and the numbers, from `lower`.
pub fn read(lower: &str) -> Option<(Measure, Vec<f64>)> {
    let (rest, _) = super::text::strip_prefix_words(
        lower,
        &[
            "what is the",
            "what's the",
            "whats the",
            "find the",
            "calculate the",
            "calculate",
            "the",
        ],
    );
    for (name, measure) in NAMES {
        let Some(after) = rest.strip_prefix(name) else {
            continue;
        };
        if !(after.starts_with(' ') || after.starts_with('(')) {
            continue;
        }
        let after = super::text::strip_prefix_words(after.trim_start(), &["of", "for"]).0;
        let list = numbers(after)?;
        // One number has no spread; a sum of one is itself.
        if list.len() < 2 {
            return None;
        }
        return Some((*measure, list));
    }
    None
}

struct Summary {
    count: usize,
    sum: f64,
    product: f64,
    mean: f64,
    median: f64,
    modes: Vec<f64>,
    min: f64,
    max: f64,
    variance: Option<f64>,
}

fn summarize(list: &[f64]) -> Summary {
    let count = list.len();
    let sum: f64 = list.iter().sum();
    let product: f64 = list.iter().product();
    let mean = sum / count as f64;
    let mut sorted = list.to_vec();
    sorted.sort_by(f64::total_cmp);
    let median = if count % 2 == 1 {
        sorted[count / 2]
    } else {
        (sorted[count / 2 - 1] + sorted[count / 2]) / 2.0
    };
    // The values seen most often, when some value is seen more than once.
    let mut runs: Vec<(f64, usize)> = Vec::new();
    for value in &sorted {
        match runs.last_mut() {
            Some((last, seen)) if last == value => *seen += 1,
            _ => runs.push((*value, 1)),
        }
    }
    let most = runs.iter().map(|(_, seen)| *seen).max().unwrap_or(0);
    let modes = if most > 1 {
        runs.iter()
            .filter(|(_, seen)| *seen == most)
            .map(|(value, _)| *value)
            .collect()
    } else {
        Vec::new()
    };
    let variance = (count > 1)
        .then(|| list.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / (count - 1) as f64);
    Summary {
        count,
        sum,
        product,
        mean,
        median,
        modes,
        min: sorted[0],
        max: sorted[count - 1],
        variance,
    }
}

fn shown(value: f64) -> AnswerText {
    let shown = show_f64(value, DIGITS);
    math_text(shown.text, &shown.tex)
}

fn value_of(summary: &Summary, measure: Measure) -> Option<AnswerText> {
    Some(match measure {
        Measure::Mean | Measure::Summary => shown(summary.mean),
        Measure::Median => shown(summary.median),
        Measure::Mode => {
            if summary.modes.is_empty() {
                return None;
            }
            let texts: Vec<String> = summary
                .modes
                .iter()
                .map(|value| show_f64(*value, DIGITS).text)
                .collect();
            AnswerText::plain(texts.join(", "))
        }
        Measure::Sum => shown(summary.sum),
        Measure::Product => shown(summary.product),
        Measure::Min => shown(summary.min),
        Measure::Max => shown(summary.max),
        Measure::Range => shown(summary.max - summary.min),
        Measure::Variance => shown(summary.variance?),
        Measure::Deviation => shown(summary.variance?.sqrt()),
        Measure::Count => shown(summary.count as f64),
    })
}

/// A statistic asked for, with the others as rows.
pub fn answer(lower: &str) -> Option<Found> {
    let (measure, list) = read(lower)?;
    let summary = summarize(&list);
    let result = value_of(&summary, measure)?;
    let mut found = blank(AnswerKind::Statistics);
    let listed: Vec<String> = list
        .iter()
        .take(12)
        .map(|value| show_f64(*value, DIGITS).text)
        .collect();
    let more = if list.len() > 12 { ", …" } else { "" };
    found.interpretation = Some(AnswerText::plain(format!(
        "{} of {{{}{more}}}",
        measure.label(),
        listed.join(", ")
    )));
    found.copy = result.text.clone();
    found.approximate = !matches!(measure, Measure::Count | Measure::Min | Measure::Max)
        && result
            .text
            .replace(',', "")
            .parse::<f64>()
            .is_ok_and(|shown| {
                let exact = match measure {
                    Measure::Deviation => summary.variance.map(f64::sqrt),
                    Measure::Variance => summary.variance,
                    Measure::Mean | Measure::Summary => Some(summary.mean),
                    _ => None,
                };
                exact.is_some_and(|exact| {
                    (exact - shown).abs() > f64::EPSILON * exact.abs().max(1.0)
                })
            });
    found.result = result;
    for other in [
        Measure::Count,
        Measure::Sum,
        Measure::Mean,
        Measure::Median,
        Measure::Mode,
        Measure::Min,
        Measure::Max,
        Measure::Range,
        Measure::Deviation,
        Measure::Variance,
    ] {
        if other == measure || (measure == Measure::Summary && other == Measure::Mean) {
            continue;
        }
        if let Some(value) = value_of(&summary, other) {
            found.rows.push(AnswerRow {
                key: other.key().to_owned(),
                label: other.label().to_owned(),
                value,
            });
        }
    }
    if summary.count > 1
        && let Some(variance) = summary.variance
    {
        let population = variance * (summary.count - 1) as f64 / summary.count as f64;
        found.rows.push(AnswerRow {
            key: "populationDeviation".to_owned(),
            label: "Population standard deviation".to_owned(),
            value: shown(population.sqrt()),
        });
    }
    Some(Found {
        answer: found,
        strong: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_are_read() {
        assert_eq!(
            read("mean of 1, 2, 3"),
            Some((Measure::Mean, vec![1.0, 2.0, 3.0]))
        );
        assert_eq!(
            read("average 4 5 6"),
            Some((Measure::Mean, vec![4.0, 5.0, 6.0]))
        );
        assert_eq!(
            read("standard deviation of 2; 4; 4; 4; 5; 5; 7; 9")
                .map(|(measure, list)| (measure, list.len())),
            Some((Measure::Deviation, 8))
        );
        assert_eq!(
            read("median(1,2,3,4)").map(|(measure, _)| measure),
            Some(Measure::Median)
        );
        assert_eq!(
            read("sum of 1 and 2").map(|(_, list)| list),
            Some(vec![1.0, 2.0])
        );
        assert_eq!(read("mean of 5"), None, "one number");
        assert_eq!(read("mean girls"), None);
        assert_eq!(read("sum 41"), None, "a band");
        assert_eq!(read("total recall"), None);
        assert_eq!(read("mode of transport"), None);
    }

    #[test]
    fn statistics_are_computed() {
        let mean = answer("mean of 1, 2, 3, 4").unwrap().answer;
        assert_eq!(mean.result.text, "2.5");
        assert!(!mean.approximate);
        let median = answer("median 3 1 2").unwrap().answer;
        assert_eq!(median.result.text, "2");
        let deviation = answer("sd of 2 4 4 4 5 5 7 9").unwrap().answer;
        assert_eq!(deviation.result.text, "2.138089935");
        assert!(deviation.approximate);
        let population = deviation
            .rows
            .iter()
            .find(|row| row.key == "populationDeviation")
            .unwrap();
        assert_eq!(population.value.text, "2");
        let mode = answer("mode of 1, 2, 2, 3").unwrap().answer;
        assert_eq!(mode.result.text, "2");
        assert!(
            answer("mode of 1, 2, 3").is_none(),
            "no value is seen twice"
        );
    }
}
