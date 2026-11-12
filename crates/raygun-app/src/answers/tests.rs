//! Search's answers end to end: what's answered on this device and how the words are read, so a
//! media search never turns into a calculation; live checks of Wikidata, Wikipedia and the ECB
//! are ignored (network).

use std::collections::HashMap;

use chrono::{TimeZone, Utc};

use super::*;

/// Wednesday 11 November 2026, 14:30 UTC, in Paris.
fn clock() -> dates::Clock {
    dates::Clock {
        now: Utc.with_ymd_and_hms(2026, 11, 11, 14, 30, 0).unwrap(),
        zone: zones::device(Some("Europe/Paris"), 60),
        month_first: false,
    }
}

fn rates() -> HashMap<String, f64> {
    HashMap::from([
        ("USD".to_owned(), 1.08),
        ("GBP".to_owned(), 0.85),
        ("JPY".to_owned(), 160.0),
    ])
}

fn local(query: &str) -> Option<Found> {
    let text = text::tidy(query);
    local_answer(&text, &text.to_lowercase(), &clock(), Some(&rates()), false)
}

/// Whether Search → All would show a card for `query`.
fn card(query: &str) -> bool {
    let text = text::tidy(query);
    let lower = text.to_lowercase();
    local(query).is_some_and(|found| found.strong)
        || questions::read(&lower, &text).is_some_and(|read| read.strong)
}

#[test]
fn media_searches_stay_media_searches() {
    for title in [
        "Blue",
        "1984",
        "Nineteen Eighty-Four",
        "The Matrix",
        "Spirited Away",
        "8 Mile",
        "60 Minutes",
        "50 Cent",
        "Maroon 5",
        "U2",
        "Sum 41",
        "Blink-182",
        "Catch-22",
        "24",
        "300",
        "2012",
        "1917",
        "21",
        "Seven",
        "Se7en",
        "Pi",
        "Tau",
        "Time",
        "Today",
        "Friday",
        "Halloween",
        "Christmas",
        "Up",
        "Her",
        "It",
        "Us",
        "Heat",
        "Speed",
        "Gravity",
        "Interstellar",
        "Moon",
        "Mercury",
        "Days of Thunder",
        "28 Days Later",
        "500 Days of Summer",
        "12 Angry Men",
        "Apollo 13",
        "Area 51",
        "Ocean's Eleven",
        "Plus One",
        "Zero Dark Thirty",
        "10 Things I Hate About You",
        "Two and a Half Men",
        "Fahrenheit 451",
        "The Day After Tomorrow",
        "Midnight in Paris",
        "Lost in Translation",
        "Children of Men",
        "Age of Empires",
        "Mean Girls",
        "Speed of Sound",
        "Back to the Future",
        "Paris",
        "Tokyo",
        "Tokyo Drift",
        "Mount Everest",
        "Albert Einstein",
        "What We Do in the Shadows",
        "Who Framed Roger Rabbit",
        "How to Train Your Dragon",
        "When Harry Met Sally",
        "Where the Wild Things Are",
        "How I Met Your Mother",
        "1-800-273-8255",
        "9/11",
        "7/11",
        "24/7",
        "3:10 to Yuma",
        "4:44",
        "7 Rings",
        "22",
        "Rolling in the Deep",
        "Born to Run",
        "Born in the USA",
        "Time in a Bottle",
        "Time After Time",
        "Hammer Time",
        "The Lord of the Rings",
        "Guardians of the Galaxy",
        "Mother",
        "The Weight of Water",
        "Cast Away",
        "Capital Cities",
        "Sleepless in Seattle",
        "The Age of Adaline",
        "Father of the Bride",
        "King of the Hill",
        "Gone with the Wind",
        "Shape of You",
        "1939-1945",
        "Next Friday",
        "Seven Days in Tibet",
        "Minority Report",
    ] {
        assert!(!card(title), "{title} would show an answer card");
    }
}

#[test]
fn calculations_and_conversions_are_answered_here() {
    for (query, expected) in [
        ("2+2", "4"),
        ("what is 15% of 80", "12"),
        ("sqrt(2)*3", "4.2426406871"),
        ("2^64", "18,446,744,073,709,551,616"),
        ("10!", "3,628,800"),
        ("1/3 + 1/6", "0.5"),
        ("5 km to miles", "3.1068559612 miles"),
        ("32 fahrenheit to celsius", "0°C"),
        ("1 TB in GB", "1,000 GB"),
        ("255 in binary", "11111111"),
        ("0xff to decimal", "255"),
        ("mean of 1, 2, 3", "2"),
        ("solve 2x + 3 = 7", "x = 2"),
        ("speed of light", "299,792,458 m/s"),
        ("what percent of 80 is 20", "25%"),
        ("100 usd to eur", "92.59 EUR"),
        ("days until 2027-01-01", "51 days"),
        ("what day is 2027-01-01", "Friday"),
        ("time in tokyo", "23:30"),
        ("is 2028 a leap year", "Yes, 2028 is a leap year"),
    ] {
        let found = local(query).unwrap_or_else(|| panic!("no answer for {query}"));
        assert!(found.strong, "{query} is a question");
        assert_eq!(found.answer.result.text, expected, "{query}");
        assert!(card(query), "{query} shows a card");
    }
}

#[test]
fn online_questions_are_read_as_questions() {
    for query in [
        "population of Japan",
        "What is the height of Mount Everest?",
        "who directed Spirited Away",
        "who wrote 1984",
        "how tall is the Eiffel Tower",
        "define serendipity",
        "translate good morning to French",
        "what is a black hole",
        "who is Ada Lovelace",
    ] {
        assert!(card(query), "{query} shows a card");
        assert!(
            local(query).is_none_or(|found| !found.strong),
            "{query} is asked online"
        );
    }
}

#[test]
fn calculations_are_written_as_formulas() {
    let root = local("sqrt(2)*3").unwrap().answer;
    let interpretation = root.interpretation.unwrap();
    let math = interpretation.math.expect("a formula");
    assert!(math.mathml.contains("<msqrt>"), "{}", math.mathml);
    let fraction = local("1/3 + 1/4").unwrap().answer;
    assert!(
        fraction
            .interpretation
            .unwrap()
            .math
            .unwrap()
            .mathml
            .contains("<mfrac>")
    );
    let quadratic = local("x^2 - 2 = 0").unwrap().answer;
    assert!(quadratic.result.math.unwrap().mathml.contains("<msqrt>"));
    let big = local("2^100").unwrap().answer;
    assert_eq!(big.result.text, "1.2676506 × 10³⁰");
    assert!(big.result.math.unwrap().mathml.contains("<msup>"));
    assert!(big.rows.iter().any(|row| row.key == "digits"));
}

#[test]
fn pathological_inputs_stop_in_time() {
    let started = std::time::Instant::now();
    for query in ["10^10^10", "1000000!", "2^1000000", "fib(100000)"] {
        let _ = local(query);
    }
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn month_first_follows_the_language() {
    assert!(month_first("en"));
    assert!(month_first("en-US"));
    assert!(!month_first("en-GB"));
    assert!(!month_first("de"));
}

fn polite() -> Polite {
    Polite::with_agent(user_agent_for("answers in Search, tests")).unwrap()
}

/// `cargo test -p raygun-app -- --ignored answers_wikidata_live --nocapture`
#[tokio::test]
#[ignore = "network"]
async fn answers_wikidata_live() {
    let polite = polite();
    let today = clock().now.date_naive();
    let ask = |property: questions::Property, subject: &'static str| {
        let polite = &polite;
        async move {
            wikidata::fact(polite, &property, subject, "en", today)
                .await
                .unwrap()
                .unwrap_or_else(|| panic!("nothing for {subject}"))
        }
    };
    let japan = ask(questions::POPULATION, "japan").await;
    println!(
        "population of Japan: {} as of {:?}",
        japan.answer.result.text, japan.answer.as_of
    );
    assert!(
        japan
            .answer
            .result
            .text
            .chars()
            .filter(char::is_ascii_digit)
            .count()
            >= 8
    );
    assert!(japan.answer.as_of.is_some());
    let everest = ask(questions::HEIGHT, "mount everest").await;
    println!("height of Mount Everest: {}", everest.answer.result.text);
    assert!(everest.answer.result.text.ends_with(" m"));
    let film = ask(questions::DIRECTOR, "spirited away").await;
    assert_eq!(film.answer.result.text, "Hayao Miyazaki");
    // The year 1984 has no author: the novel does.
    let novel = ask(questions::AUTHOR, "1984").await;
    assert_eq!(novel.answer.result.text, "George Orwell");
    let born = ask(questions::BORN, "albert einstein").await;
    assert_eq!(born.answer.result.date.unwrap().iso, "1879-03-14");
    let facts = wikidata::key_facts(&polite, &japan.hit, "en")
        .await
        .unwrap();
    println!("key facts of Japan: {facts:#?}");
    assert!(
        facts
            .iter()
            .any(|row| row.label == "Capital" && row.value.text == "Tokyo")
    );
}

/// `cargo test -p raygun-app -- --ignored answers_wikipedia_live --nocapture`
#[tokio::test]
#[ignore = "network"]
async fn answers_wikipedia_live() {
    let polite = polite();
    let (answer, hit, _) = wikipedia::about(&polite, "black hole", "en")
        .await
        .unwrap()
        .expect("an article");
    println!("{hit:?}\n{}", answer.summary.as_deref().unwrap_or_default());
    assert!(answer.summary.unwrap().to_lowercase().contains("gravity"));
    assert!(answer.url.unwrap().starts_with("https://en.wikipedia.org/"));
    let (german, _, _) = wikipedia::about(&polite, "mount everest", "de")
        .await
        .unwrap()
        .expect("an article");
    assert!(german.url.unwrap().starts_with("https://de.wikipedia.org/"));
}

/// `cargo test -p raygun-app -- --ignored answers_ecb_live --nocapture`
#[tokio::test]
#[ignore = "network"]
async fn answers_ecb_live() {
    let dir = tempfile::tempdir().unwrap();
    let rates = currency::ExchangeRates::new(dir.path().to_owned());
    let found = rates.get(&polite(), true).await.unwrap();
    println!("ECB rates of {}: USD {}", found.date, found.rates["USD"]);
    assert!(found.rates.len() >= 25);
    assert!(
        dir.path().join("ecb-rates.json").exists(),
        "kept for offline use"
    );
    // Offline, the kept rates answer.
    let kept = currency::ExchangeRates::new(dir.path().to_owned());
    assert_eq!(kept.get(&polite(), false).await.unwrap().date, found.date);
    let euros = calc::answer("100 usd to eur", Some(&found.rates)).unwrap();
    println!("100 USD = {}", euros.answer.result.text);
}
