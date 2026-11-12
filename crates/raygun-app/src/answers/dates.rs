//! Dates and times: today's date, the time here and elsewhere, days until or since a date, dates
//! some days away, weekdays, week numbers, leap years, ages, and Unix time. Questions are read
//! in English.

use chrono::{
    DateTime, Datelike, Duration, FixedOffset, Months, NaiveDate, NaiveDateTime, NaiveTime,
    TimeZone, Utc, Weekday,
};
use raygun_model::{AnswerDate, AnswerDatePrecision, AnswerKind, AnswerRow, AnswerText};

use super::numbers::grouped;
use super::text::{count_of, split_last, strip_prefix_words, strip_suffix_words, without_article};
use super::zones::{self, Zone};
use super::{Found, blank};

/// When now is, and where.
#[derive(Debug, Clone, Copy)]
pub struct Clock {
    pub now: DateTime<Utc>,
    pub zone: Zone,
    /// Dates like `01/02/2027` read month first (US English).
    pub month_first: bool,
}

impl Clock {
    fn local(&self) -> DateTime<FixedOffset> {
        zones::local(self.zone, self.now)
    }

    fn today(&self) -> NaiveDate {
        self.local().date_naive()
    }
}

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn month_number(word: &str) -> Option<u32> {
    let word = word.trim_end_matches(['.', ',']);
    if word.len() < 3 {
        return None;
    }
    if word == "sept" {
        return Some(9);
    }
    MONTHS
        .iter()
        .position(|month| *month == word || (word.len() == 3 && month.starts_with(word)))
        .map(|index| index as u32 + 1)
}

fn weekday_of(word: &str) -> Option<Weekday> {
    Some(match word.trim_end_matches(['.', ',', 's']) {
        "monday" | "mon" => Weekday::Mon,
        "tuesday" | "tue" | "tues" => Weekday::Tue,
        "wednesday" | "wed" => Weekday::Wed,
        "thursday" | "thu" | "thur" | "thurs" => Weekday::Thu,
        "friday" | "fri" => Weekday::Fri,
        "saturday" | "sat" => Weekday::Sat,
        "sunday" | "sun" => Weekday::Sun,
        _ => return None,
    })
}

fn weekday_name(day: Weekday) -> &'static str {
    match day {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
}

/// `1`, `1st`, `22nd`, `3rd`, `4th`: a day of the month.
fn day_number(word: &str) -> Option<u32> {
    let word = word.trim_end_matches([',', '.']);
    let digits = word
        .strip_suffix("st")
        .or_else(|| word.strip_suffix("nd"))
        .or_else(|| word.strip_suffix("rd"))
        .or_else(|| word.strip_suffix("th"))
        .unwrap_or(word);
    let day: u32 = digits.parse().ok()?;
    (1..=31).contains(&day).then_some(day)
}

fn year_number(word: &str) -> Option<i32> {
    let word = word.trim_end_matches([',', '.']);
    if !(1..=4).contains(&word.len()) || !word.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let year: i32 = word.parse().ok()?;
    (1..=9999).contains(&year).then_some(year)
}

/// Which way a date without a year lies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Toward {
    Future,
    Past,
    Either,
}

/// How much of a date was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Given {
    Day,
    Month,
    Year,
}

/// A month and day placed in a year as `toward` wants it.
fn placed(
    month: u32,
    day: u32,
    year: Option<i32>,
    today: NaiveDate,
    toward: Toward,
) -> Option<NaiveDate> {
    if let Some(year) = year {
        return NaiveDate::from_ymd_opt(year, month, day);
    }
    let this_year = NaiveDate::from_ymd_opt(today.year(), month, day);
    match toward {
        Toward::Future => match this_year {
            Some(date) if date >= today => Some(date),
            _ => NaiveDate::from_ymd_opt(today.year() + 1, month, day),
        },
        Toward::Past => match this_year {
            Some(date) if date <= today => Some(date),
            _ => NaiveDate::from_ymd_opt(today.year() - 1, month, day),
        },
        Toward::Either => this_year,
    }
}

/// The `n`th `weekday` of a month (`n` from 1), or the last when `n` is 0.
fn nth_weekday(year: i32, month: u32, weekday: Weekday, n: u32) -> Option<NaiveDate> {
    if n == 0 {
        let next = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1)?
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1)?
        };
        let mut day = next.pred_opt()?;
        while day.weekday() != weekday {
            day = day.pred_opt()?;
        }
        return Some(day);
    }
    NaiveDate::from_weekday_of_month_opt(year, month, weekday, n as u8)
}

/// Easter Sunday (the Gregorian computus).
fn easter(year: i32) -> Option<NaiveDate> {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32)
}

/// Holidays by name, as the date they fall on in a year.
fn holiday(name: &str, year: i32) -> Option<NaiveDate> {
    let name = name.replace(['\'', '’'], "");
    let name = name.trim_end_matches(" day").trim();
    let fixed = |month, day| NaiveDate::from_ymd_opt(year, month, day);
    match name {
        "christmas" | "xmas" | "christmas day" => fixed(12, 25),
        "christmas eve" => fixed(12, 24),
        "boxing" => fixed(12, 26),
        "new year" | "new years" | "new years day" => fixed(1, 1),
        "new years eve" | "nye" => fixed(12, 31),
        "halloween" => fixed(10, 31),
        "valentines" | "valentine" | "st valentines" | "saint valentines" => fixed(2, 14),
        "independence" | "fourth of july" | "4th of july" | "july 4th" => fixed(7, 4),
        "st patricks" | "st. patricks" | "saint patricks" => fixed(3, 17),
        "april fools" | "april fool" => fixed(4, 1),
        "thanksgiving" => nth_weekday(year, 11, Weekday::Thu, 4),
        "mothers" => nth_weekday(year, 5, Weekday::Sun, 2),
        "fathers" => nth_weekday(year, 6, Weekday::Sun, 3),
        "labor" | "labour" => nth_weekday(year, 9, Weekday::Mon, 1),
        "memorial" => nth_weekday(year, 5, Weekday::Mon, 0),
        "easter" | "easter sunday" => easter(year),
        "good friday" => easter(year).and_then(|day| day.checked_sub_signed(Duration::days(2))),
        "easter monday" => easter(year).and_then(|day| day.checked_add_signed(Duration::days(1))),
        _ => None,
    }
}

/// A holiday, in the year given or the next (or last) time it falls.
fn holiday_date(text: &str, today: NaiveDate, toward: Toward) -> Option<NaiveDate> {
    let (name, year) = match text.rsplit_once(' ') {
        Some((name, last)) if year_number(last).is_some_and(|year| year >= 100) => {
            (name, year_number(last))
        }
        _ => (text, None),
    };
    if let Some(year) = year {
        return holiday(name, year);
    }
    let this_year = holiday(name, today.year())?;
    Some(match toward {
        Toward::Future if this_year < today => holiday(name, today.year() + 1)?,
        Toward::Past if this_year > today => holiday(name, today.year() - 1)?,
        _ => this_year,
    })
}

/// A date written with digits: `2027-01-01`, `2027/1/1`, `1.2.2027`, `01/02/2027`.
fn numeric_date(text: &str, clock: &Clock) -> Option<NaiveDate> {
    let separator = ['-', '/', '.']
        .into_iter()
        .find(|separator| text.contains(*separator))?;
    let parts: Vec<&str> = text.split(separator).collect();
    let numbers: Vec<u32> = parts
        .iter()
        .map(|part| part.parse().ok())
        .collect::<Option<_>>()?;
    match (parts.as_slice(), numbers.as_slice()) {
        ([year, _, _], [y, m, d]) if year.len() == 4 => NaiveDate::from_ymd_opt(*y as i32, *m, *d),
        ([_, _, year], [a, b, y]) if year.len() == 4 => {
            let (month, day) = if separator == '.' || separator == '-' || *a > 12 {
                (*b, *a)
            } else if *b > 12 || clock.month_first {
                (*a, *b)
            } else {
                (*b, *a)
            };
            NaiveDate::from_ymd_opt(*y as i32, month, day)
        }
        _ => None,
    }
}

/// A date as people write it, with how much of it was given.
fn parse_date(text: &str, clock: &Clock, toward: Toward) -> Option<(NaiveDate, Given)> {
    let today = clock.today();
    let text = text.trim().trim_end_matches(['.', ',']);
    let text = strip_prefix_words(text, &["on", "the"]).0;
    let day_shift = |days: i64| today.checked_add_signed(Duration::days(days));
    let simple = match text {
        "today" | "now" => day_shift(0),
        "tomorrow" => day_shift(1),
        "yesterday" => day_shift(-1),
        "day after tomorrow" => day_shift(2),
        "day before yesterday" => day_shift(-2),
        "next week" => {
            let to_monday = 7 - i64::from(today.weekday().num_days_from_monday());
            day_shift(to_monday)
        }
        "next month" => today
            .with_day(1)
            .and_then(|first| first.checked_add_months(Months::new(1))),
        "next year" => NaiveDate::from_ymd_opt(today.year() + 1, 1, 1),
        "end of the month" | "end of month" | "end of this month" => today
            .with_day(1)
            .and_then(|first| first.checked_add_months(Months::new(1)))
            .and_then(|first| first.pred_opt()),
        "end of the year" | "end of year" | "end of this year" => {
            NaiveDate::from_ymd_opt(today.year(), 12, 31)
        }
        "end of the week" | "end of this week" | "the weekend" | "weekend" => {
            let to_saturday = (5 - i64::from(today.weekday().num_days_from_monday())).rem_euclid(7);
            day_shift(to_saturday)
        }
        _ => None,
    };
    if let Some(date) = simple {
        return Some((date, Given::Day));
    }
    // Weekdays: `next friday`, `last monday`, `this sunday`, `friday`.
    let words: Vec<&str> = text.split(' ').collect();
    if let [which, day] = words.as_slice()
        && let Some(weekday) = weekday_of(day)
    {
        let ahead = (i64::from(weekday.num_days_from_monday())
            - i64::from(today.weekday().num_days_from_monday()))
        .rem_euclid(7);
        let shift = match *which {
            "next" => {
                if ahead == 0 {
                    7
                } else {
                    ahead
                }
            }
            "last" | "previous" => {
                if ahead == 0 {
                    -7
                } else {
                    ahead - 7
                }
            }
            "this" => ahead,
            _ => return None,
        };
        return Some((day_shift(shift)?, Given::Day));
    }
    if words.len() == 1
        && let Some(weekday) = weekday_of(words[0])
    {
        let ahead = (i64::from(weekday.num_days_from_monday())
            - i64::from(today.weekday().num_days_from_monday()))
        .rem_euclid(7);
        let shift = if toward == Toward::Past {
            if ahead == 0 { 0 } else { ahead - 7 }
        } else {
            ahead
        };
        return Some((day_shift(shift)?, Given::Day));
    }
    if let Some(date) = holiday_date(text, today, toward) {
        return Some((date, Given::Day));
    }
    if !text.contains(' ')
        && let Some(date) = numeric_date(text, clock)
    {
        return Some((date, Given::Day));
    }
    // Words: `1 january 2027`, `january 1st, 2027`, `the 4th of july`, `march 2027`, `2027`.
    let words: Vec<&str> = text
        .split([' ', ','])
        .filter(|word| !word.is_empty() && *word != "of")
        .collect();
    match words.as_slice() {
        [day, month] | [day, month, ..]
            if day_number(day).is_some() && month_number(month).is_some() =>
        {
            let year = match words.get(2) {
                Some(year) => Some(year_number(year)?),
                None => None,
            };
            if words.len() > 3 {
                return None;
            }
            let date = placed(month_number(month)?, day_number(day)?, year, today, toward)?;
            Some((date, Given::Day))
        }
        [month, second, rest @ ..] if month_number(month).is_some() => {
            let month = month_number(month)?;
            if let (Some(day), Some(year)) = (day_number(second), rest.first()) {
                if rest.len() > 1 {
                    return None;
                }
                return Some((
                    placed(month, day, Some(year_number(year)?), today, toward)?,
                    Given::Day,
                ));
            }
            if rest.is_empty() {
                if second.len() == 4
                    && let Some(year) = year_number(second)
                {
                    return Some((NaiveDate::from_ymd_opt(year, month, 1)?, Given::Month));
                }
                let day = day_number(second)?;
                return Some((placed(month, day, None, today, toward)?, Given::Day));
            }
            None
        }
        [month] if month_number(month).is_some() => {
            let month = month_number(month)?;
            Some((placed(month, 1, None, today, toward)?, Given::Month))
        }
        [year] if year.len() == 4 => Some((
            NaiveDate::from_ymd_opt(year_number(year)?, 1, 1)?,
            Given::Year,
        )),
        _ => None,
    }
}

/// A time of day: `3pm`, `3:30 pm`, `15:30`, `noon`, `midnight`.
fn parse_time(text: &str) -> Option<NaiveTime> {
    let text = text.trim().to_lowercase();
    match text.as_str() {
        "noon" | "midday" => return NaiveTime::from_hms_opt(12, 0, 0),
        "midnight" => return NaiveTime::from_hms_opt(0, 0, 0),
        _ => {}
    }
    let compact: String = text.replace(['.', ' '], "").replace("o'clock", "");
    let (clock, half) = if let Some(rest) = compact.strip_suffix("am") {
        (rest, Some(false))
    } else if let Some(rest) = compact.strip_suffix("pm") {
        (rest, Some(true))
    } else {
        (compact.as_str(), None)
    };
    let (hours, minutes) = match clock.split_once(':') {
        Some((hours, minutes)) => (hours.parse::<u32>().ok()?, minutes.parse::<u32>().ok()?),
        None if half.is_some() => (clock.parse::<u32>().ok()?, 0),
        // `15h30`, `1530` without a colon would be a number: only with a colon, or am/pm.
        None => return None,
    };
    let hours = match half {
        Some(pm) => {
            if !(1..=12).contains(&hours) {
                return None;
            }
            match (hours, pm) {
                (12, false) => 0,
                (12, true) => 12,
                (hours, true) => hours + 12,
                (hours, false) => hours,
            }
        }
        None => hours,
    };
    NaiveTime::from_hms_opt(hours, minutes, 0)
}

/// `Friday, 1 January 2027`.
pub fn long_date(date: NaiveDate) -> String {
    format!(
        "{}, {} {} {}",
        weekday_name(date.weekday()),
        date.day(),
        MONTH_NAMES[date.month0() as usize],
        date.year()
    )
}

fn day_text(date: NaiveDate) -> AnswerText {
    AnswerText {
        text: long_date(date),
        math: None,
        date: Some(AnswerDate {
            iso: date.format("%Y-%m-%d").to_string(),
            precision: AnswerDatePrecision::Day,
        }),
        query: None,
    }
}

fn clock_text(at: NaiveDateTime) -> AnswerText {
    AnswerText {
        text: at.format("%H:%M").to_string(),
        math: None,
        date: Some(AnswerDate {
            iso: at.format("%Y-%m-%dT%H:%M").to_string(),
            precision: AnswerDatePrecision::Minute,
        }),
        query: None,
    }
}

fn count(number: i64, one: &str, many: &str) -> String {
    let size = grouped(&number.unsigned_abs().to_string());
    let sign = if number < 0 { "-" } else { "" };
    format!(
        "{sign}{size} {}",
        if number.abs() == 1 { one } else { many }
    )
}

fn row(key: &str, label: &str, value: AnswerText) -> AnswerRow {
    AnswerRow {
        key: key.to_owned(),
        label: label.to_owned(),
        value,
    }
}

/// Whole months from `from` to `to` (forward), and the days left over.
fn months_and_days(from: NaiveDate, to: NaiveDate) -> (u32, i64) {
    let mut months = 0u32;
    while let Some(next) = from.checked_add_months(Months::new(months + 1)) {
        if next > to {
            break;
        }
        months += 1;
    }
    let base = from.checked_add_months(Months::new(months)).unwrap_or(from);
    (months, (to - base).num_days())
}

fn and(parts: Vec<String>) -> String {
    match parts.len() {
        0 => String::new(),
        1 => parts[0].clone(),
        _ => format!(
            "{} and {}",
            parts[..parts.len() - 1].join(", "),
            parts[parts.len() - 1]
        ),
    }
}

/// A span of days in other units, for the rows.
fn span_rows(from: NaiveDate, to: NaiveDate, days: i64) -> Vec<AnswerRow> {
    let mut rows = Vec::new();
    let (start, end) = if from <= to { (from, to) } else { (to, from) };
    let size = days.abs();
    if size >= 7 {
        let weeks = size / 7;
        let rest = size % 7;
        let mut parts = vec![count(weeks, "week", "weeks")];
        if rest > 0 {
            parts.push(count(rest, "day", "days"));
        }
        rows.push(row("weeks", "In weeks", AnswerText::plain(and(parts))));
    }
    let (months, rest) = months_and_days(start, end);
    if months >= 1 {
        let years = months / 12;
        let months = months % 12;
        let mut parts = Vec::new();
        if years > 0 {
            parts.push(count(i64::from(years), "year", "years"));
        }
        if months > 0 {
            parts.push(count(i64::from(months), "month", "months"));
        }
        if rest > 0 {
            parts.push(count(rest, "day", "days"));
        }
        rows.push(row("months", "In months", AnswerText::plain(and(parts))));
    }
    rows.push(row(
        "hours",
        "In hours",
        AnswerText::plain(count(size * 24, "hour", "hours")),
    ));
    // Weekdays only: Monday to Friday.
    let mut weekdays = 0;
    let mut day = start;
    while day < end && weekdays <= 100_000 {
        if day.weekday().num_days_from_monday() < 5 {
            weekdays += 1;
        }
        day = match day.succ_opt() {
            Some(next) => next,
            None => break,
        };
    }
    rows.push(row(
        "weekdays",
        "Weekdays (Monday to Friday)",
        AnswerText::plain(count(weekdays, "day", "days")),
    ));
    rows
}

fn date_answer(
    kind: AnswerKind,
    interpretation: String,
    result: AnswerText,
    rows: Vec<AnswerRow>,
) -> Found {
    let mut answer = blank(kind);
    answer.interpretation = Some(AnswerText::plain(interpretation));
    answer.copy = result.text.clone();
    answer.result = result;
    answer.rows = rows;
    Found {
        answer,
        strong: true,
    }
}

/// Units a span of time can be asked in.
fn span_unit(word: &str) -> Option<&'static str> {
    Some(match word {
        "day" | "days" => "days",
        "week" | "weeks" => "weeks",
        "month" | "months" => "months",
        "year" | "years" => "years",
        "hour" | "hours" => "hours",
        "minute" | "minutes" => "minutes",
        "second" | "seconds" => "seconds",
        "weekday" | "weekdays" | "business day" | "business days" | "working day"
        | "working days" => "weekdays",
        _ => return None,
    })
}

/// A span between two dates in the unit asked for.
fn span(
    unit: &str,
    from: NaiveDate,
    to: NaiveDate,
    from_now: Option<DateTime<FixedOffset>>,
) -> AnswerText {
    let days = (to - from).num_days();
    let text = match unit {
        "weeks" => {
            let weeks = days / 7;
            let rest = days % 7;
            if rest == 0 {
                count(weeks, "week", "weeks")
            } else {
                and(vec![
                    count(weeks, "week", "weeks"),
                    count(rest.abs(), "day", "days"),
                ])
            }
        }
        "months" | "years" => {
            let (start, end) = if from <= to { (from, to) } else { (to, from) };
            let (months, rest) = months_and_days(start, end);
            let mut parts = Vec::new();
            if unit == "years" && months >= 12 {
                parts.push(count(i64::from(months / 12), "year", "years"));
                if !months.is_multiple_of(12) {
                    parts.push(count(i64::from(months % 12), "month", "months"));
                }
            } else if months > 0 {
                parts.push(count(i64::from(months), "month", "months"));
            }
            if rest > 0 || parts.is_empty() {
                parts.push(count(rest, "day", "days"));
            }
            and(parts)
        }
        "hours" | "minutes" | "seconds" => {
            // From this moment, when counting from now.
            let seconds = match from_now {
                Some(now) => {
                    let target = to
                        .and_hms_opt(0, 0, 0)
                        .map(|midnight| midnight - now.naive_local());
                    target.map_or(days * 86_400, |delta| delta.num_seconds())
                }
                None => days * 86_400,
            };
            match unit {
                "hours" => count(seconds / 3600, "hour", "hours"),
                "minutes" => count(seconds / 60, "minute", "minutes"),
                _ => count(seconds, "second", "seconds"),
            }
        }
        "weekdays" => {
            let (start, end) = if from <= to { (from, to) } else { (to, from) };
            let mut weekdays = 0;
            let mut day = start;
            while day < end && weekdays <= 100_000 {
                if day.weekday().num_days_from_monday() < 5 {
                    weekdays += 1;
                }
                day = match day.succ_opt() {
                    Some(next) => next,
                    None => break,
                };
            }
            count(weekdays, "weekday", "weekdays")
        }
        _ => count(days.abs(), "day", "days"),
    };
    AnswerText::plain(text)
}

/// Days until or since a date, between two dates.
fn until_since(lower: &str, clock: &Clock) -> Option<Found> {
    let today = clock.today();
    // `days until christmas`, `how many days until …`, `how long until …`, `weeks till …`.
    let (rest, _) = strip_prefix_words(lower, &["how many", "how long", "number of", "count"]);
    let (unit, rest) = match rest.split_once(' ') {
        Some((first, rest)) if span_unit(first).is_some() => (span_unit(first)?, rest),
        _ => {
            if lower.starts_with("how long") {
                ("days", rest)
            } else {
                return None;
            }
        }
    };
    let (rest, _) = strip_prefix_words(rest, &["are there", "is it", "are", "remain", "remaining"]);
    // Between two dates.
    for (lead, joiners) in [
        ("between", &["and", "to", "until", "till"][..]),
        ("from", &["to", "until", "till", "and"][..]),
    ] {
        if let Some(after) = rest
            .strip_prefix(lead)
            .filter(|after| after.starts_with(' '))
            && let Some((first, _, second)) = super::text::split_first(after.trim(), joiners)
        {
            let (from, _) = parse_date(first, clock, Toward::Either)?;
            let (to, _) = parse_date(second, clock, Toward::Either)?;
            let days = (to - from).num_days();
            return Some(date_answer(
                AnswerKind::Date,
                format!("{unit} from {} to {}", long_date(from), long_date(to)),
                span(unit, from, to, None),
                span_rows(from, to, days),
            ));
        }
    }
    for (lead, toward) in [
        ("until", Toward::Future),
        ("till", Toward::Future),
        ("til", Toward::Future),
        ("to", Toward::Future),
        ("before", Toward::Future),
        ("left until", Toward::Future),
        ("left till", Toward::Future),
        ("left in", Toward::Future),
        ("since", Toward::Past),
        ("from", Toward::Past),
        ("ago was", Toward::Past),
    ] {
        let Some(after) = rest
            .strip_prefix(lead)
            .filter(|after| after.starts_with(' '))
        else {
            continue;
        };
        let after = after.trim();
        // `days left in the year`, `days left in 2026`.
        let target = if lead == "left in" {
            match after {
                "the year" | "this year" | "year" => {
                    NaiveDate::from_ymd_opt(today.year() + 1, 1, 1)
                }
                "the month" | "this month" | "month" => today
                    .with_day(1)
                    .and_then(|first| first.checked_add_months(Months::new(1))),
                _ => None,
            }?
        } else {
            parse_date(after, clock, toward)?.0
        };
        let days = (target - today).num_days();
        let interpretation = if days >= 0 {
            format!(
                "{unit} from today ({}) until {}",
                long_date(today),
                long_date(target)
            )
        } else {
            format!(
                "{unit} since {} until today ({})",
                long_date(target),
                long_date(today)
            )
        };
        let (from, to) = if days >= 0 {
            (today, target)
        } else {
            (target, today)
        };
        let mut found = date_answer(
            AnswerKind::Date,
            interpretation,
            span(unit, from, to, (days >= 0).then(|| clock.local())),
            span_rows(today, target, days),
        );
        found
            .answer
            .rows
            .insert(0, row("date", "Date", day_text(target)));
        return Some(found);
    }
    None
}

/// Dates some days, weeks, months or years away: `30 days from now`, `in 2 weeks`, `100 days
/// ago`, `90 days after 2027-01-01`, `today + 45 days`.
fn date_shift(lower: &str, clock: &Clock) -> Option<Found> {
    let today = clock.today();
    let shift = |date: NaiveDate, amount: i64, unit: &str| -> Option<NaiveDate> {
        match unit {
            "days" => date.checked_add_signed(Duration::days(amount)),
            "weeks" => date.checked_add_signed(Duration::weeks(amount)),
            "months" => {
                if amount >= 0 {
                    date.checked_add_months(Months::new(amount as u32))
                } else {
                    date.checked_sub_months(Months::new(amount.unsigned_abs() as u32))
                }
            }
            "years" => {
                let months = (amount * 12).unsigned_abs() as u32;
                if amount >= 0 {
                    date.checked_add_months(Months::new(months))
                } else {
                    date.checked_sub_months(Months::new(months))
                }
            }
            _ => None,
        }
    };
    let amount_unit = |text: &str| -> Option<(i64, &'static str)> {
        let (amount, unit) = text.trim().split_once(' ')?;
        let amount = count_of(amount)?;
        let unit = span_unit(unit.trim())?;
        matches!(unit, "days" | "weeks" | "months" | "years").then_some((amount, unit))
    };
    let (rest, asked) = strip_prefix_words(
        lower,
        &[
            "what is the date",
            "what date is",
            "what's the date",
            "date",
            "what date will it be",
        ],
    );
    let rest = rest.trim();
    // `in 30 days`.
    if let Some(after) = rest.strip_prefix("in ")
        && let Some((amount, unit)) = amount_unit(after)
    {
        let date = shift(today, amount, unit)?;
        return Some(shifted(
            format!("{} plus {amount} {unit}", long_date(today)),
            date,
            today,
        ));
    }
    for (end, sign) in [
        (" from now", 1),
        (" from today", 1),
        (" after today", 1),
        (" hence", 1),
        (" ago", -1),
        (" before today", -1),
    ] {
        if let Some(before) = rest.strip_suffix(end)
            && let Some((amount, unit)) = amount_unit(before)
        {
            let date = shift(today, sign * amount, unit)?;
            let verb = if sign > 0 { "plus" } else { "minus" };
            return Some(shifted(
                format!("{} {verb} {amount} {unit}", long_date(today)),
                date,
                today,
            ));
        }
    }
    // `90 days after 2027-01-01`, `2 weeks before christmas`.
    for (middle, sign) in [(" after ", 1), (" from ", 1), (" before ", -1)] {
        if let Some((amount, base)) = rest.split_once(middle)
            && let Some((amount, unit)) = amount_unit(amount)
            && let Some((base, _)) = parse_date(base, clock, Toward::Either)
        {
            let date = shift(base, sign * amount, unit)?;
            let verb = if sign > 0 { "plus" } else { "minus" };
            return Some(shifted(
                format!("{} {verb} {amount} {unit}", long_date(base)),
                date,
                today,
            ));
        }
    }
    // `today + 45 days`, `2027-01-01 - 2 weeks`.
    for (middle, sign) in [(" + ", 1), (" plus ", 1), (" - ", -1), (" minus ", -1)] {
        if let Some((base, amount)) = rest.split_once(middle)
            && let Some((amount, unit)) = amount_unit(amount)
            && let Some((base, _)) = parse_date(base, clock, Toward::Either)
        {
            let date = shift(base, sign * amount, unit)?;
            let verb = if sign > 0 { "plus" } else { "minus" };
            return Some(shifted(
                format!("{} {verb} {amount} {unit}", long_date(base)),
                date,
                today,
            ));
        }
    }
    let _ = asked;
    None
}

fn shifted(interpretation: String, date: NaiveDate, today: NaiveDate) -> Found {
    let days = (date - today).num_days();
    let mut rows = vec![row(
        "weekday",
        "Day of the week",
        AnswerText::plain(weekday_name(date.weekday())),
    )];
    rows.push(row(
        if days >= 0 { "fromToday" } else { "ago" },
        if days >= 0 {
            "Days from today"
        } else {
            "Days ago"
        },
        AnswerText::plain(count(days.abs(), "day", "days")),
    ));
    date_answer(AnswerKind::Date, interpretation, day_text(date), rows)
}

/// The weekday of a date, and when a holiday falls.
fn weekday(lower: &str, clock: &Clock) -> Option<Found> {
    let today = clock.today();
    let (rest, asked) = strip_prefix_words(
        lower,
        &[
            "what day of the week is",
            "what day of the week was",
            "what day of the week will",
            "what day of week is",
            "what weekday is",
            "what weekday was",
            "which day is",
            "what day is",
            "what day was",
            "what day will",
            "what day does",
            "day of the week for",
            "day of the week of",
            "day of week for",
            "weekday of",
        ],
    );
    if asked {
        let rest = strip_suffix_words(rest, &["be on", "fall on", "be", "fall", "land on"]).0;
        let rest = strip_prefix_words(rest, &["it on", "it", "on"]).0;
        if rest.is_empty() || rest == "today" {
            return Some(today_answer(clock));
        }
        let (date, given) = parse_date(rest, clock, Toward::Future)?;
        if given != Given::Day {
            return None;
        }
        return Some(weekday_of_date(date, today));
    }
    // `when is easter`, `when is thanksgiving 2027`, `when is christmas`.
    let (rest, when) = strip_prefix_words(
        lower,
        &["when is", "when's", "whens", "when was", "date of"],
    );
    if when && let Some(date) = holiday_date(without_article(rest), today, Toward::Future) {
        let mut found = weekday_of_date(date, today);
        found.answer.result = day_text(date);
        found.answer.copy = found.answer.result.text.clone();
        return Some(found);
    }
    // A date alone, written with digits: `2027-01-01`.
    if !lower.contains(' ')
        && lower.chars().filter(char::is_ascii_digit).count() >= 6
        && let Some(date) = numeric_date(lower, clock)
    {
        return Some(weekday_of_date(date, today));
    }
    None
}

fn weekday_of_date(date: NaiveDate, today: NaiveDate) -> Found {
    let days = (date - today).num_days();
    let mut rows = vec![row("date", "Date", day_text(date))];
    rows.push(row(
        if days >= 0 { "fromToday" } else { "ago" },
        if days >= 0 {
            "Days from today"
        } else {
            "Days ago"
        },
        AnswerText::plain(count(days.abs(), "day", "days")),
    ));
    rows.push(row(
        "dayOfYear",
        "Day of the year",
        AnswerText::plain(date.ordinal().to_string()),
    ));
    date_answer(
        AnswerKind::Date,
        long_date(date),
        AnswerText::plain(weekday_name(date.weekday())),
        rows,
    )
}

fn today_answer(clock: &Clock) -> Found {
    let today = clock.today();
    let end = NaiveDate::from_ymd_opt(today.year(), 12, 31).unwrap_or(today);
    let week = today.iso_week();
    let rows = vec![
        row(
            "dayOfYear",
            "Day of the year",
            AnswerText::plain(today.ordinal().to_string()),
        ),
        row(
            "week",
            "Week (ISO)",
            AnswerText::plain(week.week().to_string()),
        ),
        row(
            "leftInYear",
            "Days left in the year",
            AnswerText::plain((end - today).num_days().to_string()),
        ),
    ];
    date_answer(
        AnswerKind::Date,
        "today's date".to_owned(),
        day_text(today),
        rows,
    )
}

/// The time here, or in a place.
fn time_in(lower: &str, clock: &Clock) -> Option<(Found, bool)> {
    let (rest, asked) = strip_prefix_words(
        lower,
        &[
            "what time is it in",
            "what's the time in",
            "whats the time in",
            "what is the time in",
            "current time in",
            "local time in",
            "the time in",
            "time in",
            "time now in",
            "what time is it",
            "what's the time",
            "whats the time",
            "current time",
            "local time",
            "time now",
            "the time",
        ],
    );
    let rest = strip_suffix_words(rest, &["now", "right now"]).0;
    let here = clock.local();
    if asked && rest.is_empty() {
        let rows = vec![
            row("date", "Date", day_text(here.date_naive())),
            row(
                "zone",
                "Time zone",
                AnswerText::plain(zone_line(clock.zone, clock.now)),
            ),
        ];
        return Some((
            date_answer(
                AnswerKind::Time,
                "the time here".to_owned(),
                clock_text(here.naive_local()),
                rows,
            ),
            true,
        ));
    }
    // `tokyo time`, `time in tokyo`.
    let place = if asked {
        rest
    } else {
        strip_suffix_words(lower, &["time now", "local time", "time"])
            .1
            .then(|| strip_suffix_words(lower, &["time now", "local time", "time"]).0)?
    };
    let (zone, shown) = zones::find(place)?;
    let there = zones::local(zone, clock.now);
    let difference = there.offset().local_minus_utc() - here.offset().local_minus_utc();
    let mut rows = vec![
        row("date", "Date", day_text(there.date_naive())),
        row(
            "zone",
            "Time zone",
            AnswerText::plain(zone_line(zone, clock.now)),
        ),
    ];
    rows.push(row(
        "difference",
        "Difference",
        AnswerText::plain(difference_text(difference)),
    ));
    Some((
        date_answer(
            AnswerKind::Time,
            format!("time in {shown}"),
            clock_text(there.naive_local()),
            rows,
        ),
        true,
    ))
}

/// `Asia/Tokyo · JST · UTC+09:00`.
fn zone_line(zone: Zone, at: DateTime<Utc>) -> String {
    let offset = zones::offset_text(zone.offset_at(at).local_minus_utc());
    let mut parts = vec![zone.name()];
    if let Some(abbreviation) = zone.abbreviation(at) {
        parts.push(abbreviation);
    }
    if !parts.contains(&offset) {
        parts.push(offset);
    }
    parts.join(" · ")
}

/// `9 hours ahead of you`, `5 hours 30 minutes behind you`, `the same time as you`.
fn difference_text(seconds: i32) -> String {
    if seconds == 0 {
        return "the same time as here".to_owned();
    }
    let minutes = seconds.unsigned_abs() / 60;
    let mut parts = Vec::new();
    if minutes / 60 > 0 {
        parts.push(count(i64::from(minutes / 60), "hour", "hours"));
    }
    if !minutes.is_multiple_of(60) {
        parts.push(count(i64::from(minutes % 60), "minute", "minutes"));
    }
    let side = if seconds > 0 { "ahead of" } else { "behind" };
    format!("{} {side} here", parts.join(" "))
}

/// A time somewhere, somewhere else: `3pm est to pst`, `15:00 utc in tokyo`, `9am in london`.
fn time_convert(lower: &str, clock: &Clock) -> Option<Found> {
    let (rest, _) = strip_prefix_words(lower, &["convert", "what is", "what's"]);
    let (left, right) = match split_last(rest, &["to", "in", "into"]) {
        Some((left, _, right)) => (left, Some(right)),
        None => (rest, None),
    };
    // The time is the first word or two: `3pm`, `3 pm`, `3:30 pm`.
    let words: Vec<&str> = left.split(' ').collect();
    let (time, after) = if words.len() >= 2
        && parse_time(&format!("{} {}", words[0], words[1])).is_some()
        && matches!(words[1], "am" | "pm" | "a.m" | "p.m")
    {
        (
            parse_time(&format!("{} {}", words[0], words[1]))?,
            words[2..].join(" "),
        )
    } else {
        (parse_time(words.first()?)?, words[1..].join(" "))
    };
    let after = strip_prefix_words(after.trim(), &["in", "at"]).0;
    let (from, from_name, to, to_name) = match right {
        Some(right) if !after.is_empty() => {
            let (from, from_name) = zones::find(after)?;
            let (to, to_name) = if matches!(right, "here" | "my time" | "local time" | "local") {
                (clock.zone, "here".to_owned())
            } else {
                zones::find(right)?
            };
            (from, from_name, to, to_name)
        }
        // `3pm in tokyo`: Tokyo's time, here. Only a time with digits (`Midnight in Paris` is a
        // film).
        Some(right) => {
            if !words[0].chars().any(|char| char.is_ascii_digit()) {
                return None;
            }
            let (from, from_name) = zones::find(right)?;
            (from, from_name, clock.zone, "here".to_owned())
        }
        None => return None,
    };
    let today_there = zones::local(from, clock.now).date_naive();
    let naive = today_there.and_time(time);
    let offset = from.offset_at(clock.now);
    let at = offset
        .from_local_datetime(&naive)
        .single()?
        .with_timezone(&Utc);
    let converted = zones::local(to, at);
    let mut rows = vec![row("date", "Date", day_text(converted.date_naive()))];
    rows.push(row(
        "zone",
        "Time zone",
        AnswerText::plain(zone_line(to, at)),
    ));
    Some(date_answer(
        AnswerKind::Time,
        format!("{} {from_name} in {to_name}", time.format("%H:%M")),
        clock_text(converted.naive_local()),
        rows,
    ))
}

/// Week numbers.
fn week_number(lower: &str, clock: &Clock) -> Option<Found> {
    let (rest, asked) = strip_prefix_words(
        lower,
        &[
            "what week number is it",
            "what week is it",
            "current week number",
            "current week",
            "week number of",
            "week number for",
            "what week is",
            "week number",
            "week of the year",
        ],
    );
    if !asked {
        return None;
    }
    let date = if rest.is_empty() || rest == "today" {
        clock.today()
    } else {
        parse_date(rest, clock, Toward::Either)
            .filter(|(_, given)| *given == Given::Day)?
            .0
    };
    let week = date.iso_week();
    let monday = NaiveDate::from_isoywd_opt(week.year(), week.week(), Weekday::Mon)?;
    let sunday = NaiveDate::from_isoywd_opt(week.year(), week.week(), Weekday::Sun)?;
    let rows = vec![
        row("date", "Date", day_text(date)),
        row(
            "weekDays",
            "From Monday to Sunday",
            AnswerText::plain(format!("{} – {}", long_date(monday), long_date(sunday))),
        ),
    ];
    Some(date_answer(
        AnswerKind::Date,
        format!("ISO week of {}", long_date(date)),
        AnswerText::plain(format!("Week {} of {}", week.week(), week.year())),
        rows,
    ))
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn leap_year(lower: &str, clock: &Clock) -> Option<Found> {
    let year = if let Some(rest) = lower.strip_prefix("is ") {
        let (year, rest) = rest.split_once(' ')?;
        if !matches!(rest, "a leap year" | "leap year") {
            if year == "it" && rest == "a leap year" {
                clock.today().year()
            } else {
                return None;
            }
        } else if year == "it" || year == "this" {
            clock.today().year()
        } else {
            year_number(year)?
        }
    } else if let Some(year) = lower.strip_prefix("leap year ") {
        year_number(year)?
    } else if let Some(year) = lower.strip_suffix(" leap year") {
        year_number(year)?
    } else if matches!(
        lower,
        "is this a leap year" | "is it a leap year" | "leap year"
    ) {
        clock.today().year()
    } else {
        return None;
    };
    let leap = is_leap(year);
    let mut next = year + 1;
    while !is_leap(next) {
        next += 1;
    }
    let rows = vec![
        row(
            "days",
            "Days in the year",
            AnswerText::plain(if leap { "366" } else { "365" }),
        ),
        row(
            "nextLeapYear",
            "Next leap year",
            AnswerText::plain(next.to_string()),
        ),
    ];
    Some(date_answer(
        AnswerKind::Date,
        format!("is {year} a leap year"),
        AnswerText::plain(if leap {
            format!("Yes, {year} is a leap year")
        } else {
            format!("No, {year} isn't a leap year")
        }),
        rows,
    ))
}

/// How old someone born on a date is.
fn age(lower: &str, clock: &Clock) -> Option<Found> {
    let (rest, asked) = strip_prefix_words(
        lower,
        &[
            "how old is someone born",
            "how old is a person born",
            "how old is somebody born",
            "how old am i if i was born",
            "how old am i if born",
            "how old would i be if i was born",
            "age of someone born",
            "age of a person born",
            "age if born",
            "age born",
            "age calculator",
            "born",
        ],
    );
    if !asked {
        return None;
    }
    let rest = strip_suffix_words(rest, &["how old", "how old am i", "age"]).0;
    let rest = strip_prefix_words(rest, &["on", "in"]).0;
    let today = clock.today();
    let (born, given) = parse_date(rest, clock, Toward::Past)?;
    if born > today {
        return None;
    }
    let (months, days) = months_and_days(born, today);
    let years = months / 12;
    let mut parts = vec![count(i64::from(years), "year", "years")];
    if given == Given::Day {
        if !months.is_multiple_of(12) {
            parts.push(count(i64::from(months % 12), "month", "months"));
        }
        if days > 0 {
            parts.push(count(days, "day", "days"));
        }
    }
    let mut rows = vec![row("exact", "Exactly", AnswerText::plain(and(parts)))];
    if given == Given::Day {
        rows.push(row(
            "daysAlive",
            "In days",
            AnswerText::plain(count((today - born).num_days(), "day", "days")),
        ));
        let next = placed(born.month(), born.day(), None, today, Toward::Future)
            .or_else(|| placed(3, 1, None, today, Toward::Future));
        if let Some(next) = next {
            rows.push(row(
                "nextBirthday",
                "Next birthday",
                AnswerText::plain(format!(
                    "{} (in {})",
                    long_date(next),
                    count((next - today).num_days(), "day", "days")
                )),
            ));
        }
    }
    Some(date_answer(
        AnswerKind::Date,
        format!("age of someone born {}", long_date(born)),
        AnswerText::plain(count(i64::from(years), "year", "years")),
        rows,
    ))
}

fn days_in(lower: &str, clock: &Clock) -> Option<Found> {
    let (rest, asked) = strip_prefix_words(
        lower,
        &[
            "how many days are there in",
            "how many days are in",
            "how many days in",
            "how many days does",
            "number of days in",
            "days in",
        ],
    );
    if !asked {
        return None;
    }
    let rest = strip_suffix_words(rest, &["have"]).0;
    let today = clock.today();
    let words: Vec<&str> = rest.split(' ').filter(|word| !word.is_empty()).collect();
    let (month, year) = match words.as_slice() {
        [month] if month_number(month).is_some() => (month_number(month)?, today.year()),
        [month, year] if month_number(month).is_some() => {
            (month_number(month)?, year_number(year)?)
        }
        [year] if year.len() == 4 => {
            let year = year_number(year)?;
            let days = if is_leap(year) { 366 } else { 365 };
            return Some(date_answer(
                AnswerKind::Date,
                format!("days in {year}"),
                AnswerText::plain(format!("{days} days")),
                Vec::new(),
            ));
        }
        _ => return None,
    };
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let next = first.checked_add_months(Months::new(1))?;
    let days = (next - first).num_days();
    Some(date_answer(
        AnswerKind::Date,
        format!("days in {} {year}", MONTH_NAMES[month as usize - 1]),
        AnswerText::plain(format!("{days} days")),
        Vec::new(),
    ))
}

/// Unix time: now, or a timestamp as a date.
fn unix(lower: &str, clock: &Clock) -> Option<Found> {
    if matches!(
        lower,
        "unix time"
            | "unix timestamp"
            | "epoch time"
            | "current unix time"
            | "current timestamp"
            | "current unix timestamp"
            | "timestamp now"
            | "unix time now"
            | "current epoch time"
    ) {
        let seconds = clock.now.timestamp();
        return Some(date_answer(
            AnswerKind::Time,
            "Unix time now".to_owned(),
            AnswerText::plain(seconds.to_string()),
            vec![row(
                "milliseconds",
                "In milliseconds",
                AnswerText::plain(clock.now.timestamp_millis().to_string()),
            )],
        ));
    }
    let (rest, unix_first) = strip_prefix_words(
        lower,
        &["unix time", "unix timestamp", "epoch", "timestamp"],
    );
    let rest = if unix_first {
        rest
    } else {
        let (rest, unix_last) = strip_suffix_words(
            lower,
            &[
                "unix time to date",
                "unix timestamp to date",
                "unix time",
                "unix timestamp",
                "unix",
                "epoch",
                "timestamp to date",
                "to date",
            ],
        );
        if !unix_last {
            return None;
        }
        rest
    };
    let rest = strip_suffix_words(rest, &["to date", "in date"]).0;
    if !(9..=13).contains(&rest.len()) || !rest.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let number: i64 = rest.parse().ok()?;
    let at = if rest.len() == 13 {
        DateTime::from_timestamp_millis(number)?
    } else {
        DateTime::from_timestamp(number, 0)?
    };
    let local = zones::local(clock.zone, at);
    let rows = vec![
        row("utc", "UTC", clock_text(at.naive_utc())),
        row("date", "Date", day_text(local.date_naive())),
    ];
    Some(date_answer(
        AnswerKind::Time,
        format!("Unix time {number}"),
        clock_text(local.naive_local()),
        rows,
    ))
}

/// Today's date when asked for.
fn today(lower: &str, clock: &Clock) -> Option<(Found, bool)> {
    let strong = matches!(
        lower,
        "what is today's date"
            | "what's today's date"
            | "whats todays date"
            | "what is todays date"
            | "what is the date"
            | "what's the date"
            | "whats the date"
            | "what is the date today"
            | "what's the date today"
            | "todays date"
            | "today's date"
            | "date today"
            | "current date"
            | "the date today"
            | "what day is it"
            | "what day is today"
            | "what day is it today"
            | "what is today"
            | "date of today"
    );
    let weak = matches!(lower, "today" | "date");
    (strong || weak).then(|| (today_answer(clock), strong))
}

/// A question about dates or times, answered.
pub fn answer(lower: &str, clock: &Clock) -> Option<Found> {
    if let Some((found, strong)) = today(lower, clock) {
        return Some(Found { strong, ..found });
    }
    if let Some(found) = leap_year(lower, clock)
        .or_else(|| unix(lower, clock))
        .or_else(|| week_number(lower, clock))
        .or_else(|| age(lower, clock))
        .or_else(|| days_in(lower, clock))
        .or_else(|| until_since(lower, clock))
        .or_else(|| date_shift(lower, clock))
        .or_else(|| weekday(lower, clock))
        .or_else(|| time_convert(lower, clock))
    {
        return Some(found);
    }
    time_in(lower, clock).map(|(found, strong)| Found { strong, ..found })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wednesday 11 November 2026, 14:30 UTC, in Paris.
    fn clock() -> Clock {
        Clock {
            now: Utc.with_ymd_and_hms(2026, 11, 11, 14, 30, 0).unwrap(),
            zone: zones::device(Some("Europe/Paris"), 60),
            month_first: false,
        }
    }

    fn result(query: &str) -> String {
        answer(query, &clock())
            .unwrap_or_else(|| panic!("no answer for {query}"))
            .answer
            .result
            .text
    }

    #[test]
    fn days_until_and_since() {
        assert_eq!(result("days until 2027-01-01"), "51 days");
        assert_eq!(result("how many days until christmas"), "44 days");
        assert_eq!(result("days till new year"), "51 days");
        assert_eq!(result("weeks until christmas"), "6 weeks and 2 days");
        assert_eq!(result("days since 1 january 2026"), "314 days");
        assert_eq!(
            result("how many days between 2026-01-01 and 2026-12-31"),
            "364 days"
        );
        assert_eq!(result("days from 1 march 2026 to 1 april 2026"), "31 days");
        assert_eq!(result("days left in the year"), "51 days");
        assert_eq!(result("how long until halloween"), "354 days");
        let christmas = answer("days until christmas", &clock()).unwrap().answer;
        assert_eq!(
            christmas.rows[0].value.date.as_ref().unwrap().iso,
            "2026-12-25"
        );
    }

    #[test]
    fn dates_some_days_away() {
        assert_eq!(result("30 days from now"), "Friday, 11 December 2026");
        assert_eq!(result("in 2 weeks"), "Wednesday, 25 November 2026");
        assert_eq!(result("100 days ago"), "Monday, 3 August 2026");
        assert_eq!(result("90 days after 2027-01-01"), "Thursday, 1 April 2027");
        assert_eq!(result("today + 45 days"), "Saturday, 26 December 2026");
        assert_eq!(result("3 months from today"), "Thursday, 11 February 2027");
    }

    #[test]
    fn weekdays_and_holidays() {
        assert_eq!(result("what day is 2027-01-01"), "Friday");
        assert_eq!(result("what day of the week was 4 july 1776"), "Thursday");
        assert_eq!(result("2027-01-01"), "Friday");
        assert_eq!(result("when is easter 2027"), "Sunday, 28 March 2027");
        assert_eq!(result("when is thanksgiving"), "Thursday, 26 November 2026");
    }

    #[test]
    fn today_and_week_numbers() {
        assert_eq!(
            result("what is today's date"),
            "Wednesday, 11 November 2026"
        );
        assert!(
            answer("today", &clock()).is_some_and(|found| !found.strong),
            "a title too"
        );
        assert_eq!(result("what week is it"), "Week 46 of 2026");
        assert_eq!(result("is 2028 a leap year"), "Yes, 2028 is a leap year");
        assert_eq!(result("is 2100 a leap year"), "No, 2100 isn't a leap year");
        assert_eq!(result("days in february 2028"), "29 days");
    }

    #[test]
    fn ages() {
        assert_eq!(result("how old is someone born on 1990-05-04"), "36 years");
        assert_eq!(result("age of someone born 11 november 2000"), "26 years");
    }

    #[test]
    fn times_here_and_elsewhere() {
        let tokyo = answer("time in tokyo", &clock()).unwrap();
        assert!(tokyo.strong);
        assert_eq!(tokyo.answer.result.text, "23:30");
        assert_eq!(
            tokyo.answer.result.date.as_ref().unwrap().iso,
            "2026-11-11T23:30"
        );
        let difference = tokyo
            .answer
            .rows
            .iter()
            .find(|row| row.key == "difference")
            .unwrap();
        assert_eq!(difference.value.text, "8 hours ahead of here");
        assert_eq!(result("what time is it in new york"), "09:30");
        assert_eq!(result("London time"), "14:30");
        assert_eq!(result("what time is it"), "15:30");
        assert_eq!(result("3pm est to pst"), "12:00");
        assert_eq!(result("15:00 utc in tokyo"), "00:00");
        assert_eq!(result("9am in tokyo"), "01:00");
    }

    #[test]
    fn unix_time() {
        assert_eq!(result("unix time"), "1794407400");
        assert_eq!(result("1700000000 unix"), "23:13");
    }

    #[test]
    fn titles_are_not_dates() {
        for title in [
            "Blue",
            "28 Days Later",
            "500 Days of Summer",
            "Seven Days in Tibet",
            "The Day After Tomorrow",
            "Time",
            "Hammer Time",
            "Adventure Time",
            "1984",
            "Before Sunrise",
            "Christmas Vacation",
            "Days of Thunder",
            "Friday",
            "Next Friday",
            "Halloween",
            "Midnight in Paris",
            "Sleepless in Seattle",
            "Born in the USA",
        ] {
            let lower = title.to_lowercase();
            assert!(
                answer(&lower, &clock()).is_none_or(|found| !found.strong),
                "{title}"
            );
        }
    }
}
