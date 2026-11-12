//! Numbers as answers show them: digits grouped by thousands, very large and very small numbers
//! in scientific notation, each with the TeX that draws it.

/// Digits after the point shown at most when a decimal is written out.
const MAX_FRACTION_DIGITS: usize = 10;
/// Significant digits shown in scientific notation.
const MAX_SIGNIFICANT: usize = 10;

/// A number as text and as TeX.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub text: String,
    pub tex: String,
}

/// A decimal number's parts: sign, digits before the point, digits after it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Decimal {
    negative: bool,
    whole: String,
    fraction: String,
}

fn parse_decimal(text: &str) -> Option<Decimal> {
    let text = text.trim();
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let rest: String = rest.chars().filter(|char| *char != ',').collect();
    let (whole, fraction) = rest.split_once('.').unwrap_or((&rest, ""));
    let digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    if whole.is_empty() && fraction.is_empty() || !digits(whole) || !digits(fraction) {
        return None;
    }
    let whole = whole.trim_start_matches('0');
    Some(Decimal {
        negative,
        whole: if whole.is_empty() {
            "0".to_owned()
        } else {
            whole.to_owned()
        },
        fraction: fraction.trim_end_matches('0').to_owned(),
    })
}

/// `1234567` as `1,234,567`.
pub fn grouped(whole: &str) -> String {
    let mut out = String::with_capacity(whole.len() + whole.len() / 3);
    for (place, digit) in whole.chars().enumerate() {
        if place > 0 && (whole.len() - place).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// Superscript digits for a power of ten written as text (`10⁻³⁴`).
pub fn superscript(exponent: i64) -> String {
    exponent
        .to_string()
        .chars()
        .map(|char| match char {
            '-' => '⁻',
            '0' => '⁰',
            '1' => '¹',
            '2' => '²',
            '3' => '³',
            '4' => '⁴',
            '5' => '⁵',
            '6' => '⁶',
            '7' => '⁷',
            '8' => '⁸',
            _ => '⁹',
        })
        .collect()
}

/// Scientific notation from a decimal's significant digits and the power of their first.
fn scientific(negative: bool, significant: &str, exponent: i64) -> Shown {
    let digits: String = significant.chars().take(MAX_SIGNIFICANT).collect();
    let (first, rest) = digits.split_at(1);
    let rest = rest.trim_end_matches('0');
    let mantissa = if rest.is_empty() {
        first.to_owned()
    } else {
        format!("{first}.{rest}")
    };
    let sign = if negative { "-" } else { "" };
    Shown {
        text: format!("{sign}{mantissa} × 10{}", superscript(exponent)),
        tex: format!("{sign}{mantissa} \\times 10^{{{exponent}}}"),
    }
}

/// A decimal as answers show it, or `None` when `text` isn't one (`1/3`, `0 + i`, `ff`).
/// Numbers of up to 21 whole digits and decimals from a millionth on are written out, grouped;
/// the rest in scientific notation.
pub fn show(text: &str) -> Option<Shown> {
    let decimal = parse_decimal(text)?;
    let sign = if decimal.negative && (decimal.whole != "0" || !decimal.fraction.is_empty()) {
        "-"
    } else {
        ""
    };
    if decimal.whole.len() > 21 {
        let all = format!("{}{}", decimal.whole, decimal.fraction);
        return Some(scientific(
            decimal.negative,
            &all,
            decimal.whole.len() as i64 - 1,
        ));
    }
    if decimal.whole == "0" && !decimal.fraction.is_empty() {
        let zeros = decimal.fraction.len() - decimal.fraction.trim_start_matches('0').len();
        if zeros >= 6 {
            let significant = decimal.fraction.trim_start_matches('0');
            return Some(scientific(
                decimal.negative,
                significant,
                -(zeros as i64) - 1,
            ));
        }
    }
    let whole = grouped(&decimal.whole);
    let fraction: String = decimal.fraction.chars().take(MAX_FRACTION_DIGITS).collect();
    Some(if fraction.is_empty() {
        Shown {
            text: format!("{sign}{whole}"),
            tex: format!("{sign}{}", whole.replace(',', "{,}")),
        }
    } else {
        Shown {
            text: format!("{sign}{whole}.{fraction}"),
            tex: format!("{sign}{}.{fraction}", whole.replace(',', "{,}")),
        }
    })
}

/// An `f64` written with at most `digits` significant digits, without trailing zeros, as
/// [`show`] writes it.
pub fn show_f64(value: f64, digits: usize) -> Shown {
    if !value.is_finite() {
        return Shown {
            text: value.to_string(),
            tex: value.to_string(),
        };
    }
    if value == 0.0 {
        return Shown {
            text: "0".to_owned(),
            tex: "0".to_owned(),
        };
    }
    let rounded = format!("{:.*e}", digits.saturating_sub(1), value);
    let (mantissa, exponent) = rounded.split_once('e').unwrap_or((&rounded, "0"));
    let exponent: i64 = exponent.parse().unwrap_or(0);
    let negative = mantissa.starts_with('-');
    let significant: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    if !(-6..=14).contains(&exponent) {
        return scientific(negative, &significant, exponent);
    }
    // Written out: the significant digits around the point.
    let point = exponent + 1;
    let (whole, fraction) = if point <= 0 {
        (
            "0".to_owned(),
            format!("{}{significant}", "0".repeat((-point) as usize)),
        )
    } else if point as usize >= significant.len() {
        (
            format!(
                "{significant}{}",
                "0".repeat(point as usize - significant.len())
            ),
            String::new(),
        )
    } else {
        let (whole, fraction) = significant.split_at(point as usize);
        (whole.to_owned(), fraction.to_owned())
    };
    let sign = if negative { "-" } else { "" };
    show(&format!("{sign}{whole}.{fraction}")).unwrap_or_else(|| Shown {
        text: value.to_string(),
        tex: value.to_string(),
    })
}

/// A whole number's prime factors, smallest first, with their powers; none for numbers below 2
/// or too large to factor at once.
pub fn prime_factors(mut number: u64) -> Option<Vec<(u64, u32)>> {
    if !(2..=1_000_000_000_000).contains(&number) {
        return None;
    }
    let mut factors = Vec::new();
    let mut divisor = 2u64;
    while divisor * divisor <= number {
        let mut power = 0;
        while number.is_multiple_of(divisor) {
            number /= divisor;
            power += 1;
        }
        if power > 0 {
            factors.push((divisor, power));
        }
        divisor += if divisor == 2 { 1 } else { 2 };
    }
    if number > 1 {
        factors.push((number, 1));
    }
    Some(factors)
}

/// Prime factors as text (`2⁶ × 31`) and TeX.
pub fn factors_shown(factors: &[(u64, u32)]) -> Shown {
    let text = factors
        .iter()
        .map(|(prime, power)| {
            if *power == 1 {
                prime.to_string()
            } else {
                format!("{prime}{}", superscript(i64::from(*power)))
            }
        })
        .collect::<Vec<_>>()
        .join(" × ");
    let tex = factors
        .iter()
        .map(|(prime, power)| {
            if *power == 1 {
                prime.to_string()
            } else {
                format!("{prime}^{{{power}}}")
            }
        })
        .collect::<Vec<_>>()
        .join(" \\times ");
    Shown { text, tex }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_are_grouped_and_trimmed() {
        assert_eq!(show("1234567").unwrap().text, "1,234,567");
        assert_eq!(show("1234567").unwrap().tex, "1{,}234{,}567");
        assert_eq!(show("-1234.5000").unwrap().text, "-1,234.5");
        assert_eq!(show("0.0174532925").unwrap().text, "0.0174532925");
        assert_eq!(show("3.14159265358979").unwrap().text, "3.1415926535");
        assert_eq!(show("-0").unwrap().text, "0");
        assert_eq!(show("1/3"), None);
        assert_eq!(show("ff"), None);
        assert_eq!(show("0 + i"), None);
    }

    #[test]
    fn very_large_and_small_numbers_go_scientific() {
        let avogadro = show("602214076000000000000000").unwrap();
        assert_eq!(avogadro.text, "6.02214076 × 10²³");
        assert_eq!(avogadro.tex, "6.02214076 \\times 10^{23}");
        let planck = show("0.000000000000000000000000000000000662607015").unwrap();
        assert_eq!(planck.text, "6.62607015 × 10⁻³⁴");
        assert_eq!(show("0.000001").unwrap().text, "0.000001");
        assert_eq!(show("0.0000001").unwrap().text, "1 × 10⁻⁷");
    }

    #[test]
    fn floats_keep_their_significant_digits() {
        assert_eq!(show_f64(1.0 / 3.0, 6).text, "0.333333");
        assert_eq!(show_f64(2.5, 10).text, "2.5");
        assert_eq!(show_f64(1234567.0, 10).text, "1,234,567");
        assert_eq!(show_f64(-0.000_000_12, 4).text, "-1.2 × 10⁻⁷");
        assert_eq!(show_f64(6.02e23, 4).text, "6.02 × 10²³");
        assert_eq!(show_f64(0.0, 4).text, "0");
    }

    #[test]
    fn whole_numbers_factor() {
        assert_eq!(prime_factors(1984), Some(vec![(2, 6), (31, 1)]));
        assert_eq!(factors_shown(&prime_factors(1984).unwrap()).text, "2⁶ × 31");
        assert_eq!(prime_factors(97), Some(vec![(97, 1)]));
        assert_eq!(prime_factors(1), None);
    }
}
