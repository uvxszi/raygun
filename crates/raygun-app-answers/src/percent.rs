//! Percentages the way people ask them: `what percent of 80 is 20`, `20 is what percent of
//! 80`, `15% off 80`, `50 + 10%`, `percent change from 50 to 65`, `15% tip on 80`.

use raygun_model::{AnswerKind, AnswerRow};

use super::numbers::show_f64;
use super::text::{split_first, strip_prefix_words, strip_suffix_words};
use super::{Found, blank, math_text};

fn number(text: &str) -> Option<f64> {
    let text = text.trim().trim_start_matches(['$', '€', '£']);
    let value: f64 = text.replace(',', "").parse().ok()?;
    value.is_finite().then_some(value)
}

fn percent(text: &str) -> Option<f64> {
    let text = text.trim();
    let (rest, _) = strip_suffix_words(text, &["percent", "per cent", "pc"]);
    number(rest.trim_end_matches('%'))
}

/// `x` as written in an answer, 10 significant digits.
fn shown(value: f64) -> String {
    show_f64(value, 10).text
}

fn found(interpretation: (String, String), value: f64, unit: &str, rows: Vec<AnswerRow>) -> Found {
    let mut answer = blank(AnswerKind::Percentage);
    answer.interpretation = Some(math_text(interpretation.0, &interpretation.1));
    let shown_value = show_f64(value, 10);
    let text = format!("{}{unit}", shown_value.text);
    let tex = format!(
        "{}{}",
        shown_value.tex,
        if unit == "%" { "\\%" } else { "" }
    );
    answer.approximate = shown_value
        .text
        .replace(',', "")
        .parse::<f64>()
        .is_ok_and(|rounded| (rounded - value).abs() > f64::EPSILON * value.abs().max(1.0));
    answer.copy = text.clone();
    answer.result = math_text(text, &tex);
    answer.rows = rows;
    Found {
        answer,
        strong: true,
    }
}

fn row(key: &str, label: &str, value: f64) -> AnswerRow {
    let shown = show_f64(value, 10);
    AnswerRow {
        key: key.to_owned(),
        label: label.to_owned(),
        value: math_text(shown.text, &shown.tex),
    }
}

pub fn answer(lower: &str) -> Option<Found> {
    let (text, _) = strip_prefix_words(lower, &["what is", "what's", "whats", "calculate"]);
    // What percent of 80 is 20; what percentage is 20 of 80; 20 is what percent of 80.
    for lead in ["what percent of", "what percentage of"] {
        if let Some(rest) = text.strip_prefix(lead)
            && let Some((whole, _, part)) = split_first(rest.trim(), &["is", "are"])
        {
            return of(number(part)?, number(whole)?);
        }
    }
    for lead in ["what percent is", "what percentage is"] {
        if let Some(rest) = text.strip_prefix(lead)
            && let Some((part, _, whole)) = split_first(rest.trim(), &["of", "out of"])
        {
            return of(number(part)?, number(whole)?);
        }
    }
    for middle in [
        "is what percent of",
        "is what percentage of",
        "is what % of",
    ] {
        if let Some((part, whole)) = text.split_once(middle) {
            return of(number(part)?, number(whole)?);
        }
    }
    for end in [
        " as a percentage",
        " as percentage",
        " as a percent",
        " in percent",
    ] {
        if let Some(rest) = text.strip_suffix(end) {
            if let Some((part, _, whole)) = split_first(rest, &["out of", "of"]) {
                return of(number(part)?, number(whole)?);
            }
            if let Some((part, whole)) = rest.split_once('/') {
                return of(number(part)?, number(whole)?);
            }
        }
    }
    // Percent change from 50 to 65.
    for lead in [
        "percent change from",
        "percentage change from",
        "% change from",
        "percent increase from",
        "percentage increase from",
        "percent decrease from",
        "percentage decrease from",
    ] {
        if let Some(rest) = text.strip_prefix(lead)
            && let Some((from, _, to)) = split_first(rest.trim(), &["to"])
        {
            return change(number(from)?, number(to)?);
        }
    }
    // 15% off 80; 15% discount on 80.
    for middle in [" off ", " discount on ", " discount off "] {
        if let Some((rate, price)) = text.split_once(middle) {
            let rate = percent(rate)?;
            let price = number(price)?;
            let saved = price * rate / 100.0;
            return Some(found(
                (
                    format!("{} minus {}%", shown(price), shown(rate)),
                    format!("{} - {}\\%", shown(price), shown(rate)),
                ),
                price - saved,
                "",
                vec![row("saved", "You save", saved)],
            ));
        }
    }
    // 15% tip on 80; tip 15% of 80.
    let tip = text
        .strip_prefix("tip ")
        .and_then(|rest| split_first(rest, &["on", "of", "for"]))
        .map(|(rate, _, bill)| (rate, bill))
        .or_else(|| {
            split_first(text, &["tip on", "tip for", "tip of"]).map(|(rate, _, bill)| (rate, bill))
        });
    if let Some((rate, bill)) = tip {
        let rate = percent(rate)?;
        let bill = number(bill)?;
        let tip = bill * rate / 100.0;
        return Some(found(
            (
                format!("{}% tip on {}", shown(rate), shown(bill)),
                format!("{}\\% \\times {}", shown(rate), shown(bill)),
            ),
            tip,
            "",
            vec![row("total", "Total", bill + tip)],
        ));
    }
    // Increase 50 by 10%; decrease 50 by 10%.
    for (lead, sign) in [("increase", 1.0), ("decrease", -1.0), ("reduce", -1.0)] {
        if let Some(rest) = text.strip_prefix(lead)
            && let Some((base, _, rate)) = split_first(rest.trim(), &["by"])
        {
            return change_by(number(base)?, sign * percent(rate)?);
        }
    }
    // 50 + 10%, 50 - 10%, 50 plus 10%.
    for (middle, sign) in [
        (" + ", 1.0),
        (" - ", -1.0),
        (" plus ", 1.0),
        (" minus ", -1.0),
    ] {
        if let Some((base, rate)) = text.split_once(middle)
            && rate.trim_end().ends_with(['%'])
        {
            return change_by(number(base)?, sign * percent(rate)?);
        }
    }
    None
}

fn of(part: f64, whole: f64) -> Option<Found> {
    if whole == 0.0 {
        return None;
    }
    Some(found(
        (
            format!("{} / {} as a percentage", shown(part), shown(whole)),
            format!(
                "\\frac{{{}}}{{{}}} \\times 100\\%",
                shown(part),
                shown(whole)
            ),
        ),
        part / whole * 100.0,
        "%",
        Vec::new(),
    ))
}

fn change(from: f64, to: f64) -> Option<Found> {
    if from == 0.0 {
        return None;
    }
    Some(found(
        (
            format!("change from {} to {}", shown(from), shown(to)),
            format!(
                "\\frac{{{} - {}}}{{{}}} \\times 100\\%",
                shown(to),
                shown(from),
                shown(from)
            ),
        ),
        (to - from) / from.abs() * 100.0,
        "%",
        vec![row("difference", "Difference", to - from)],
    ))
}

fn change_by(base: f64, rate: f64) -> Option<Found> {
    let sign = if rate < 0.0 { "-" } else { "+" };
    Some(found(
        (
            format!("{} {sign} {}%", shown(base), shown(rate.abs())),
            format!("{} {sign} {}\\%", shown(base), shown(rate.abs())),
        ),
        base * (1.0 + rate / 100.0),
        "",
        vec![row("change", "Change", base * rate / 100.0)],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(query: &str) -> String {
        answer(query).unwrap().answer.result.text
    }

    #[test]
    fn percentages_are_worked_out() {
        assert_eq!(result("what percent of 80 is 20"), "25%");
        assert_eq!(result("20 is what percent of 80"), "25%");
        assert_eq!(result("what percentage is 20 of 80"), "25%");
        assert_eq!(result("3 out of 4 as a percentage"), "75%");
        assert_eq!(result("percent change from 50 to 65"), "30%");
        assert_eq!(result("15% off 80"), "68");
        assert_eq!(result("15% tip on 80"), "12");
        assert_eq!(result("increase 50 by 10%"), "55");
        assert_eq!(result("50 + 10%"), "55");
        assert_eq!(result("200 - 25%"), "150");
        assert!(answer("10% of 50").is_none(), "fend computes it");
        assert!(answer("off the wall").is_none());
        assert!(answer("50 + 10").is_none());
    }
}
