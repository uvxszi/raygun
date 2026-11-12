//! Facts from Wikidata (CC0): the thing asked about found by name (`wbsearchentities`), the
//! property asked for read from its statements (`wbgetclaims`), and the names of the things
//! those statements point to (`wbgetentities`) in the app's language. Requests go through the
//! dictionaries' polite client (a User-Agent naming Raygun, paced, `Retry-After` kept).

use std::collections::{BTreeSet, HashMap};

use chrono::{Datelike, NaiveDate};
use raygun_model::{
    Answer, AnswerCredit, AnswerDate, AnswerDatePrecision, AnswerKind, AnswerRow, AnswerSuggestion,
    AnswerText,
};
use serde_json::Value;

use super::numbers;
use super::questions::{self, Property, Value as Shape};
use super::text::capitalized;
use crate::dictionary::polite::{Fetched, Polite};

const API: &str = "https://www.wikidata.org/w/api.php";
pub const NAME: &str = "Wikidata";
pub const LICENSE: &str = "CC0 1.0";
pub const LICENSE_URL: &str = "https://creativecommons.org/publicdomain/zero/1.0/";
/// Things found by name that are tried for the property, best match first.
const CANDIDATES: usize = 3;
/// Statements asked for at most for one question (things found × the property's ids).
const MAX_CLAIMS: usize = 6;

/// Something found by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub id: String,
    pub label: String,
    pub description: String,
}

/// A date as Wikidata keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Time {
    /// `+1879-03-14T00:00:00Z`.
    pub value: String,
    /// 11 a day, 10 a month, 9 a year, 8 a decade, 7 a century.
    pub precision: u8,
}

/// A statement's value.
#[derive(Debug, Clone, PartialEq)]
pub enum Datum {
    Item(String),
    Quantity {
        amount: String,
        unit: Option<String>,
    },
    Time(Time),
    Text(String),
    Coordinates {
        latitude: f64,
        longitude: f64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub preferred: bool,
    pub datum: Datum,
    /// When the value stood (`P585`, point in time).
    pub at: Option<Time>,
}

/// Names of a thing in the app's language (or English), and its Wikipedia articles.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entity {
    pub label: Option<String>,
    pub description: Option<String>,
    /// `enwiki` → `Japan`.
    pub sitelinks: HashMap<String, String>,
}

/// Units by their Wikidata item, as answers write them.
const UNITS: &[(&str, &str)] = &[
    ("Q11573", "m"),
    ("Q828224", "km"),
    ("Q174728", "cm"),
    ("Q174789", "mm"),
    ("Q3710", "ft"),
    ("Q218593", "in"),
    ("Q253276", "mi"),
    ("Q712226", "km²"),
    ("Q25343", "m²"),
    ("Q35852", "ha"),
    ("Q81292", "acres"),
    ("Q232291", "sq mi"),
    ("Q11570", "kg"),
    ("Q41803", "g"),
    ("Q191118", "t"),
    ("Q100995", "lb"),
    ("Q48013", "oz"),
    ("Q11574", "s"),
    ("Q7727", "min"),
    ("Q25235", "h"),
    ("Q573", "days"),
    ("Q577", "years"),
    ("Q25267", "°C"),
    ("Q42289", "°F"),
    ("Q11579", "K"),
    ("Q180154", "km/h"),
    ("Q182429", "m/s"),
    ("Q211256", "mph"),
    ("Q13147228", "g/cm³"),
    ("Q844211", "kg/m³"),
    ("Q483261", "Da"),
    ("Q1811", "au"),
    ("Q531", "ly"),
    ("Q12129", "pc"),
    ("Q794261", "m³/s"),
    ("Q11229", "%"),
    ("Q4917", "US$"),
    ("Q4916", "€"),
    ("Q25224", "£"),
    ("Q8146", "¥"),
    ("Q39099", "CN¥"),
    ("Q80524", "₹"),
];

fn url(params: &[(&str, &str)]) -> String {
    let mut all = vec![("format", "json"), ("formatversion", "2")];
    all.extend_from_slice(params);
    reqwest::Url::parse_with_params(API, &all)
        .map(String::from)
        .unwrap_or_else(|_| API.to_owned())
}

async fn json(polite: &Polite, address: &str) -> Result<Option<Value>, String> {
    match polite.get(address, "application/json", NAME).await {
        Fetched::Body(body) => serde_json::from_str(&body)
            .map(Some)
            .map_err(|_| format!("{NAME}'s answer couldn't be read")),
        Fetched::NotFound => Ok(None),
        Fetched::Failed(issue) => Err(issue),
    }
}

/// Wikipedia's language for the app's: `pt-BR` → `pt`, `nb` → `no`.
pub fn wiki_language(language: &str) -> String {
    let primary = language
        .split(['-', '_'])
        .next()
        .unwrap_or("en")
        .to_lowercase();
    match primary.as_str() {
        "nb" | "nn" => "no".to_owned(),
        "fil" => "tl".to_owned(),
        "yue" => "zh-yue".to_owned(),
        "" => "en".to_owned(),
        _ => primary,
    }
}

/// Things named `name` (in English, as questions are read), best match first. Their
/// descriptions come in English too: they tell what kind of thing each is (`kind_of`).
pub async fn search(polite: &Polite, name: &str) -> Result<Vec<Hit>, String> {
    let address = url(&[
        ("action", "wbsearchentities"),
        ("type", "item"),
        ("limit", "10"),
        ("language", "en"),
        ("uselang", "en"),
        ("strictlanguage", "false"),
        ("search", name),
    ]);
    let Some(answer) = json(polite, &address).await? else {
        return Ok(Vec::new());
    };
    Ok(without_pages(parse_search(&answer)))
}

/// Things that are only names, numbers, years or Wikimedia's own pages go last: they have no
/// facts to ask about (`1984` the year before the novel).
pub fn without_pages(hits: Vec<Hit>) -> Vec<Hit> {
    let plain = |hit: &Hit| {
        let description = hit.description.to_lowercase();
        description == "year"
            || description.starts_with("wikimedia ")
            || [
                "disambiguation page",
                "list article",
                "family name",
                "given name",
                "natural number",
                "calendar year",
                "leap year starting",
                "common year starting",
                "decade",
            ]
            .iter()
            .any(|word| description.contains(word))
    };
    let (pages, mut things): (Vec<Hit>, Vec<Hit>) = hits.into_iter().partition(plain);
    things.extend(pages);
    things
}

pub fn parse_search(answer: &Value) -> Vec<Hit> {
    answer
        .get("search")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|hit| {
            let id = hit.get("id")?.as_str()?;
            if !id.starts_with('Q') {
                return None;
            }
            Some(Hit {
                id: id.to_owned(),
                label: hit
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .to_owned(),
                description: hit
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            })
        })
        .collect()
}

fn parse_time(value: &Value) -> Option<Time> {
    Some(Time {
        value: value.get("time")?.as_str()?.to_owned(),
        precision: value.get("precision")?.as_u64()? as u8,
    })
}

fn parse_datum(snak: &Value) -> Option<Datum> {
    if snak.get("snaktype").and_then(Value::as_str) != Some("value") {
        return None;
    }
    let datavalue = snak.get("datavalue")?;
    let value = datavalue.get("value")?;
    Some(match datavalue.get("type")?.as_str()? {
        "wikibase-entityid" => Datum::Item(value.get("id")?.as_str()?.to_owned()),
        "quantity" => {
            let unit = value.get("unit").and_then(Value::as_str).unwrap_or("1");
            Datum::Quantity {
                amount: value
                    .get("amount")?
                    .as_str()?
                    .trim_start_matches('+')
                    .to_owned(),
                unit: unit
                    .rsplit('/')
                    .next()
                    .filter(|id| id.starts_with('Q'))
                    .map(str::to_owned),
            }
        }
        "time" => Datum::Time(parse_time(value)?),
        "string" => Datum::Text(value.as_str()?.to_owned()),
        "monolingualtext" => Datum::Text(value.get("text")?.as_str()?.to_owned()),
        "globecoordinate" => Datum::Coordinates {
            latitude: value.get("latitude")?.as_f64()?,
            longitude: value.get("longitude")?.as_f64()?,
        },
        _ => return None,
    })
}

/// A `wbgetclaims` answer's statements of `property`, deprecated ones left out.
pub fn parse_claims(answer: &Value, property: &str) -> Vec<Statement> {
    answer
        .get("claims")
        .and_then(|claims| claims.get(property))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|claim| {
            let rank = claim
                .get("rank")
                .and_then(Value::as_str)
                .unwrap_or("normal");
            if rank == "deprecated" {
                return None;
            }
            let datum = parse_datum(claim.get("mainsnak")?)?;
            let at = claim
                .get("qualifiers")
                .and_then(|qualifiers| qualifiers.get("P585"))
                .and_then(Value::as_array)
                .and_then(|list| list.first())
                .and_then(|snak| match parse_datum(snak)? {
                    Datum::Time(time) => Some(time),
                    _ => None,
                });
            Some(Statement {
                preferred: rank == "preferred",
                datum,
                at,
            })
        })
        .collect()
}

pub async fn claims(polite: &Polite, id: &str, property: &str) -> Result<Vec<Statement>, String> {
    let address = url(&[
        ("action", "wbgetclaims"),
        ("entity", id),
        ("property", property),
    ]);
    let Some(answer) = json(polite, &address).await? else {
        return Ok(Vec::new());
    };
    Ok(parse_claims(&answer, property))
}

/// The statements that answer: preferred ones (or all), the latest first when they're
/// counts over time.
pub fn best(mut statements: Vec<Statement>) -> Vec<Statement> {
    if statements.iter().any(|statement| statement.preferred) {
        statements.retain(|statement| statement.preferred);
    }
    if statements.iter().any(|statement| statement.at.is_some()) {
        statements.sort_by(|a, b| {
            let when = |statement: &Statement| {
                statement
                    .at
                    .as_ref()
                    .map(|time| time.value.clone())
                    .unwrap_or_default()
            };
            when(b).cmp(&when(a))
        });
        statements.truncate(1);
    }
    statements
}

/// Names and Wikipedia articles of things, in the app's language (English where it has none).
pub async fn entities(
    polite: &Polite,
    ids: &[String],
    language: &str,
) -> Result<HashMap<String, Entity>, String> {
    let mut found = HashMap::new();
    let wiki = wiki_language(language);
    let languages = if wiki == "en" {
        "en".to_owned()
    } else {
        format!("{language}|{wiki}|en")
    };
    let sites = if wiki == "en" {
        "enwiki".to_owned()
    } else {
        format!("{}wiki|enwiki", wiki.replace('-', "_"))
    };
    for chunk in ids.chunks(50) {
        let joined = chunk.join("|");
        let address = url(&[
            ("action", "wbgetentities"),
            ("ids", &joined),
            ("props", "labels|descriptions|sitelinks"),
            ("languages", &languages),
            ("languagefallback", "1"),
            ("sitefilter", &sites),
        ]);
        if let Some(answer) = json(polite, &address).await? {
            found.extend(parse_entities(&answer, language));
        }
    }
    Ok(found)
}

pub fn parse_entities(answer: &Value, language: &str) -> HashMap<String, Entity> {
    let wiki = wiki_language(language);
    let pick = |names: Option<&Value>| -> Option<String> {
        let names = names?;
        [language, wiki.as_str(), "en"]
            .iter()
            .find_map(|code| names.get(*code))
            .or_else(|| names.as_object().and_then(|all| all.values().next()))
            .and_then(|name| name.get("value"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    answer
        .get("entities")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(id, entity)| {
            let sitelinks = entity
                .get("sitelinks")
                .and_then(Value::as_object)
                .map(|links| {
                    links
                        .iter()
                        .filter_map(|(site, link)| {
                            Some((site.clone(), link.get("title")?.as_str()?.to_owned()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            (
                id.clone(),
                Entity {
                    label: pick(entity.get("labels")),
                    description: pick(entity.get("descriptions")),
                    sitelinks,
                },
            )
        })
        .collect()
}

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

/// A Wikidata date as text (`14 March 1879`, `March 1879`, `1870s`, `19th century`, `500 BC`)
/// and as a date the app shows its own way.
pub fn time_text(time: &Time) -> AnswerText {
    let value = time.value.trim_start_matches('+');
    let negative = value.starts_with('-');
    let value = value.trim_start_matches('-');
    let mut parts = value.split(['-', 'T']);
    let year: i64 = parts.next().and_then(|year| year.parse().ok()).unwrap_or(0);
    let month: u32 = parts
        .next()
        .and_then(|month| month.parse().ok())
        .unwrap_or(0);
    let day: u32 = parts.next().and_then(|day| day.parse().ok()).unwrap_or(0);
    let era = if negative { " BC" } else { "" };
    let (text, precision) = match time.precision {
        11.. if month > 0 && day > 0 => (
            format!("{day} {} {year}{era}", MONTH_NAMES[month as usize - 1]),
            AnswerDatePrecision::Day,
        ),
        10.. if month > 0 => (
            format!("{} {year}{era}", MONTH_NAMES[month as usize - 1]),
            AnswerDatePrecision::Month,
        ),
        9.. => (format!("{year}{era}"), AnswerDatePrecision::Year),
        8 => (format!("{}0s{era}", year / 10), AnswerDatePrecision::Decade),
        _ => {
            let century = (year + 99) / 100;
            let suffix = match century % 100 {
                11..=13 => "th",
                _ => match century % 10 {
                    1 => "st",
                    2 => "nd",
                    3 => "rd",
                    _ => "th",
                },
            };
            (
                format!("{century}{suffix} century{era}"),
                AnswerDatePrecision::Century,
            )
        }
    };
    let sign = if negative { "-" } else { "" };
    let iso = match precision {
        AnswerDatePrecision::Day => format!("{sign}{year:04}-{month:02}-{day:02}"),
        AnswerDatePrecision::Month => format!("{sign}{year:04}-{month:02}"),
        _ => format!("{sign}{year:04}"),
    };
    AnswerText {
        text,
        math: None,
        // Before the common era and before year 1000 the app's own formatting goes astray.
        date: (!negative && year >= 1000).then_some(AnswerDate { iso, precision }),
        query: None,
    }
}

fn date_of(time: &Time) -> Option<NaiveDate> {
    let value = time.value.trim_start_matches('+');
    if value.starts_with('-') || time.precision < 9 {
        return None;
    }
    let mut parts = value.split(['-', 'T']);
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse::<u32>().ok()?.max(1);
    let day: u32 = parts.next()?.parse::<u32>().ok()?.max(1);
    NaiveDate::from_ymd_opt(year, month, day)
}

/// Whole years from `from` to `to`.
fn years_between(from: NaiveDate, to: NaiveDate) -> i32 {
    let mut years = to.year() - from.year();
    if (to.month(), to.day()) < (from.month(), from.day()) {
        years -= 1;
    }
    years
}

/// A quantity as text: `8,848.86 m`, `125,440,000`.
fn quantity_text(amount: &str, unit: Option<&str>, names: &HashMap<String, Entity>) -> String {
    let number = numbers::show(amount).map_or_else(|| amount.to_owned(), |shown| shown.text);
    let unit = unit.and_then(|id| {
        UNITS
            .iter()
            .find(|(known, _)| *known == id)
            .map(|(_, symbol)| (*symbol).to_owned())
            .or_else(|| names.get(id).and_then(|entity| entity.label.clone()))
    });
    match unit {
        Some(unit) if unit == "%" || unit.starts_with('°') => format!("{number}{unit}"),
        Some(unit) if ["US$", "€", "£", "¥", "CN¥", "₹"].contains(&unit.as_str()) => {
            format!("{unit}{number}")
        }
        Some(unit) => format!("{number} {unit}"),
        None => number,
    }
}

/// A statement's value as text.
pub fn datum_text(datum: &Datum, names: &HashMap<String, Entity>) -> Option<AnswerText> {
    Some(match datum {
        Datum::Item(id) => {
            let label = names.get(id).and_then(|entity| entity.label.clone())?;
            AnswerText {
                query: Some(label.clone()),
                ..AnswerText::plain(label)
            }
        }
        Datum::Quantity { amount, unit } => {
            AnswerText::plain(quantity_text(amount, unit.as_deref(), names))
        }
        Datum::Time(time) => time_text(time),
        Datum::Text(text) => AnswerText::plain(text.clone()),
        Datum::Coordinates {
            latitude,
            longitude,
        } => AnswerText::plain(format!(
            "{:.4}° {}, {:.4}° {}",
            latitude.abs(),
            if *latitude >= 0.0 { "N" } else { "S" },
            longitude.abs(),
            if *longitude >= 0.0 { "E" } else { "W" }
        )),
    })
}

/// Item and unit ids the statements name.
fn named_ids(statements: &[Statement]) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for statement in statements {
        match &statement.datum {
            Datum::Item(id) => {
                ids.insert(id.clone());
            }
            Datum::Quantity {
                unit: Some(unit), ..
            } if !UNITS.iter().any(|(known, _)| known == unit) => {
                ids.insert(unit.clone());
            }
            _ => {}
        }
    }
    ids.into_iter().collect()
}

/// What a thing is, from its description: which facts to show about it and what to ask next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Person,
    Country,
    City,
    Mountain,
    River,
    Film,
    Series,
    Book,
    Music,
    Band,
    Company,
    Element,
    Compound,
    Building,
    Planet,
    Other,
}

/// Whether `phrase` is in `text` as whole words (`band` isn't in `husband`).
fn has_words(text: &str, phrase: &str) -> bool {
    let edge = |char: Option<char>| char.is_none_or(|char| !char.is_alphanumeric());
    text.match_indices(phrase).any(|(start, _)| {
        let end = start + phrase.len();
        edge(text[..start].chars().next_back()) && edge(text[end..].chars().next())
    })
}

pub fn kind_of(description: &str) -> Kind {
    let text = description.to_lowercase();
    let has = |words: &[&str]| {
        words.iter().any(|word| {
            if word.starts_with('(') {
                // `(born 1941)`, `(1879–1955)`: the start of a person's years.
                text.contains(word)
            } else {
                has_words(&text, word)
            }
        })
    };
    if has(&["chemical element"]) {
        Kind::Element
    } else if has(&["chemical compound", "molecule", "acid"]) {
        Kind::Compound
    } else if has(&[
        "television series",
        "tv series",
        "anime series",
        "web series",
        "sitcom",
        "miniseries",
    ]) {
        Kind::Series
    } else if has(&["film"])
        && !has(&[
            "film director",
            "film producer",
            "filmmaker",
            "film actor",
            "film actress",
            "film critic",
            "film composer",
        ])
    {
        Kind::Film
    } else if has(&[
        "novel",
        "book by",
        "poem",
        "literary work",
        "short story",
        "play by",
        "manga series",
        "comic book",
        "memoir",
    ]) {
        Kind::Book
    } else if has(&[
        "studio album",
        "album by",
        "live album",
        "single by",
        "song by",
        "song",
        "single",
    ]) {
        Kind::Music
    } else if has(&[
        "band",
        "musical group",
        "musical duo",
        "music group",
        "boy group",
        "girl group",
    ]) {
        Kind::Band
    } else if has(&[
        "company",
        "corporation",
        "conglomerate",
        "manufacturer",
        "business",
        "brand",
        "enterprise",
        "organization",
        "organisation",
        "university",
    ]) {
        Kind::Company
    } else if has(&[
        "country",
        "sovereign state",
        "island nation",
        "nation in",
        "kingdom in",
        "republic in",
    ]) {
        Kind::Country
    } else if has(&[
        "city",
        "town",
        "capital of",
        "village",
        "municipality",
        "commune",
        "metropolis",
        "prefecture",
        "province",
        "state of the united states",
        "region of",
    ]) {
        Kind::City
    } else if has(&["mountain", "volcano", "massif", "peak"]) {
        Kind::Mountain
    } else if has(&["river"]) {
        Kind::River
    } else if has(&[
        "planet",
        "dwarf planet",
        "moon of",
        "natural satellite",
        "star in",
    ]) {
        Kind::Planet
    } else if has(&[
        "tower",
        "building",
        "skyscraper",
        "bridge",
        "stadium",
        "cathedral",
        "castle",
        "monument",
        "palace",
        "statue",
        "museum",
    ]) {
        Kind::Building
    } else if has(&[
        "(born",
        "(1",
        "(2",
        "(c.",
        "actor",
        "actress",
        "singer",
        "musician",
        "writer",
        "author",
        "politician",
        "physicist",
        "scientist",
        "painter",
        "composer",
        "footballer",
        "player",
        "rapper",
        "director",
        "philosopher",
        "mathematician",
        "poet",
        "businessman",
        "businesswoman",
        "entrepreneur",
        "engineer",
        "artist",
        "emperor",
        "chemist",
        "biologist",
        "inventor",
        "astronaut",
        "athlete",
        "comedian",
        "journalist",
        "songwriter",
        "dancer",
        "lawyer",
        "economist",
        "historian",
        "pope",
        "monarch",
        "president of",
        "king of",
        "queen of",
        "prime minister",
    ]) {
        Kind::Person
    } else {
        Kind::Other
    }
}

/// The kinds of things a property is usually asked of, most likely first: `who wrote 1984`
/// means the novel, not the year or the album.
fn prefers(property: &Property) -> &'static [Kind] {
    match property.ids[0] {
        "P50" | "P676" | "P58" => &[Kind::Book, Kind::Music, Kind::Film, Kind::Series],
        "P57" | "P161" | "P2047" | "P2130" | "P2142" => &[Kind::Film, Kind::Series],
        "P86" | "P175" | "P264" => &[Kind::Music, Kind::Band, Kind::Film],
        "P1082" | "P36" | "P38" | "P37" | "P35" | "P6" | "P2046" | "P421" | "P2131" | "P2250"
        | "P85" | "P1451" | "P30" => &[Kind::Country, Kind::City],
        "P2044" | "P4552" => &[Kind::Mountain, Kind::City],
        "P2048" | "P1101" | "P84" | "P287" | "P1619" => {
            &[Kind::Building, Kind::Mountain, Kind::Person]
        }
        "P2043" | "P403" | "P885" | "P2225" => &[Kind::River],
        "P569" | "P570" | "P19" | "P20" | "P26" | "P40" | "P22" | "P25" | "P27" | "P106" => {
            &[Kind::Person]
        }
        "P571" | "P112" | "P169" | "P159" | "P1128" | "P127" => &[Kind::Company, Kind::Band],
        "P577" => &[Kind::Film, Kind::Music, Kind::Book, Kind::Series],
        "P1086" | "P246" => &[Kind::Element],
        "P274" | "P2101" | "P2102" | "P2054" => &[Kind::Compound, Kind::Element],
        "P2067" | "P2583" | "P2146" | "P2120" | "P2386" => {
            &[Kind::Planet, Kind::Element, Kind::Compound]
        }
        "P1113" | "P2437" | "P449" => &[Kind::Series],
        "P527" => &[Kind::Band],
        _ => &[],
    }
}

/// Things found by name, the kinds `property` is usually asked of first (in the order found
/// otherwise).
fn ordered_for(property: &Property, mut hits: Vec<Hit>) -> Vec<Hit> {
    let kinds = prefers(property);
    hits.sort_by_key(|hit| {
        let kind = kind_of(&hit.description);
        kinds
            .iter()
            .position(|preferred| *preferred == kind)
            .unwrap_or(kinds.len())
    });
    hits
}

/// Facts worth showing about something of `kind`, in order.
pub fn key_properties(kind: Kind) -> &'static [Property] {
    use questions as q;
    match kind {
        Kind::Person => &[
            q::BORN,
            q::BIRTHPLACE,
            q::DIED,
            q::OCCUPATION,
            q::CITIZENSHIP,
        ],
        Kind::Country => &[
            q::CAPITAL,
            q::POPULATION,
            q::AREA,
            q::LANGUAGES,
            q::CURRENCY,
        ],
        Kind::City => &[
            q::COUNTRY,
            q::POPULATION,
            q::AREA,
            q::ELEVATION,
            q::TIME_ZONE,
        ],
        Kind::Mountain => &[q::ELEVATION, q::COUNTRY, q::MOUNTAIN_RANGE],
        Kind::River => &[q::LENGTH, q::MOUTH, q::SOURCE, q::COUNTRY],
        Kind::Film => &[q::DIRECTOR, q::RELEASED, q::DURATION, q::CAST, q::GENRE],
        Kind::Series => &[q::CREATOR, q::SEASONS, q::EPISODES, q::NETWORK, q::GENRE],
        Kind::Book => &[q::AUTHOR, q::RELEASED, q::ORIGINAL_LANGUAGE, q::GENRE],
        Kind::Music => &[q::PERFORMER, q::RELEASED, q::GENRE, q::LABEL],
        Kind::Band => &[q::FOUNDED, q::MEMBERS, q::GENRE, q::COUNTRY_OF_ORIGIN],
        Kind::Company => &[
            q::FOUNDED,
            q::FOUNDER,
            q::HEADQUARTERS,
            q::CEO,
            q::EMPLOYEES,
        ],
        Kind::Element => &[q::SYMBOL, q::ATOMIC_NUMBER, q::MASS, q::MELTING, q::BOILING],
        Kind::Compound => &[q::FORMULA, q::MASS, q::MELTING, q::BOILING, q::DENSITY],
        Kind::Building => &[q::HEIGHT, q::OPENED, q::ARCHITECT, q::LOCATION, q::FLOORS],
        Kind::Planet => &[q::MASS, q::RADIUS, q::ORBITAL_PERIOD, q::DISTANCE],
        Kind::Other => &[],
    }
}

/// Questions to ask next about something of `kind` named `name`, the one asked left out.
pub fn related(kind: Kind, name: &str, asked: Option<&str>) -> Vec<AnswerSuggestion> {
    let questions: &[(&str, &str)] = match kind {
        Kind::Person => &[
            ("Date of birth", "when was {} born"),
            ("Place of birth", "where was {} born"),
            ("Occupation", "occupation of {}"),
            ("Spouse", "spouse of {}"),
        ],
        Kind::Country => &[
            ("Population", "population of {}"),
            ("Capital", "capital of {}"),
            ("Area", "area of {}"),
            ("Currency", "currency of {}"),
            ("Official languages", "official language of {}"),
            ("Head of state", "head of state of {}"),
        ],
        Kind::City => &[
            ("Population", "population of {}"),
            ("Country", "country of {}"),
            ("Elevation", "elevation of {}"),
            ("Time zone", "time in {}"),
        ],
        Kind::Mountain => &[
            ("Elevation", "elevation of {}"),
            ("Country", "country of {}"),
        ],
        Kind::River => &[("Length", "length of {}"), ("Flows into", "mouth of {}")],
        Kind::Film => &[
            ("Director", "who directed {}"),
            ("Cast", "cast of {}"),
            ("Published", "when was {} released"),
            ("Duration", "running time of {}"),
        ],
        Kind::Series => &[
            ("Creator", "who created {}"),
            ("Seasons", "number of seasons of {}"),
            ("Episodes", "number of episodes of {}"),
        ],
        Kind::Book => &[
            ("Author", "who wrote {}"),
            ("Published", "when was {} published"),
        ],
        Kind::Music => &[
            ("Performer", "who sang {}"),
            ("Published", "when was {} released"),
        ],
        Kind::Band => &[
            ("Members", "members of {}"),
            ("Founded", "when was {} formed"),
        ],
        Kind::Company => &[
            ("Founded by", "who founded {}"),
            ("Chief executive officer", "ceo of {}"),
            ("Headquarters", "headquarters of {}"),
            ("Employees", "number of employees of {}"),
        ],
        Kind::Element => &[
            ("Atomic number", "atomic number of {}"),
            ("Melting point", "melting point of {}"),
            ("Boiling point", "boiling point of {}"),
        ],
        Kind::Compound => &[
            ("Chemical formula", "chemical formula of {}"),
            ("Boiling point", "boiling point of {}"),
        ],
        Kind::Building => &[
            ("Height", "height of {}"),
            ("Architect", "architect of {}"),
            ("Opened", "when was {} built"),
        ],
        Kind::Planet => &[
            ("Mass", "mass of {}"),
            ("Radius", "radius of {}"),
            ("Distance from Earth", "distance of {} from earth"),
        ],
        Kind::Other => &[],
    };
    questions
        .iter()
        .filter(|(label, _)| Some(*label) != asked)
        .map(|(label, template)| AnswerSuggestion {
            label: format!("{label} of {name}").replace("Date of birth of", "Birth date of"),
            query: template.replace("{}", name),
        })
        .collect()
}

/// A fact's answer: the values of `property` that `hit`'s statements give.
fn fact_answer(
    property: &Property,
    hit: &Hit,
    statements: &[Statement],
    names: &HashMap<String, Entity>,
    today: NaiveDate,
) -> Option<Answer> {
    let subject = names
        .get(&hit.id)
        .and_then(|entity| entity.label.clone())
        .unwrap_or_else(|| hit.label.clone());
    let description = names
        .get(&hit.id)
        .and_then(|entity| entity.description.clone())
        .unwrap_or_else(|| hit.description.clone());
    let mut answer = super::blank(AnswerKind::Fact);
    answer.engine = "wikidata".to_owned();
    answer.interpretation = Some(AnswerText::plain(if description.is_empty() {
        format!("{} of {subject}", property.label)
    } else {
        format!("{} of {subject} ({description})", property.label)
    }));
    if property.value == Shape::Age {
        let born = statements
            .iter()
            .find_map(|statement| match &statement.datum {
                Datum::Time(time) => date_of(time),
                _ => None,
            })?;
        let years = years_between(born, today);
        answer.result = AnswerText::plain(format!("{years} years"));
        answer.rows.push(AnswerRow {
            key: String::new(),
            label: if property.ids[0] == "P569" {
                "Born"
            } else {
                "Since"
            }
            .to_owned(),
            value: time_text(&Time {
                value: format!("+{}T00:00:00Z", born.format("%Y-%m-%d")),
                precision: 11,
            }),
        });
    } else {
        let values: Vec<AnswerText> = statements
            .iter()
            .filter_map(|statement| datum_text(&statement.datum, names))
            .take(property.many)
            .collect();
        if values.is_empty() {
            return None;
        }
        if values.len() == 1 {
            answer.result = values[0].clone();
        } else {
            answer.result = AnswerText::plain(
                values
                    .iter()
                    .map(|value| value.text.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            // Each of them can be asked about.
            for value in &values {
                answer.rows.push(AnswerRow {
                    key: String::new(),
                    label: property.label.to_owned(),
                    value: value.clone(),
                });
            }
        }
        if property.ids.contains(&"P856")
            && let Some(Datum::Text(address)) = statements.first().map(|statement| &statement.datum)
            && address.starts_with("https://")
        {
            answer.url = Some(address.clone());
        }
        answer.as_of = statements
            .first()
            .and_then(|statement| statement.at.as_ref())
            .and_then(|time| time_text(time).date);
    }
    answer.copy = answer.result.text.clone();
    if answer.url.is_none() {
        answer.url = Some(format!("https://www.wikidata.org/wiki/{}", hit.id));
    }
    answer.credits.push(AnswerCredit {
        name: NAME.to_owned(),
        url: Some(format!("https://www.wikidata.org/wiki/{}", hit.id)),
        license: Some(LICENSE.to_owned()),
        license_url: Some(LICENSE_URL.to_owned()),
    });
    Some(answer)
}

/// What a fact question found: the answer, the thing it's about, and what to ask next.
pub struct FactFound {
    pub answer: Answer,
    pub hit: Hit,
    pub entity: Entity,
    pub related: Vec<AnswerSuggestion>,
}

/// The fact `property` of the thing named `subject`: the best of the first few things found by
/// that name that has the property.
pub async fn fact(
    polite: &Polite,
    property: &Property,
    subject: &str,
    language: &str,
    today: NaiveDate,
) -> Result<Option<FactFound>, String> {
    let hits = ordered_for(property, search(polite, subject).await?);
    let mut asked = 0;
    for hit in hits.into_iter().take(CANDIDATES) {
        for id in property.ids {
            // Wikidata is asked a few times at most for one question.
            if asked == MAX_CLAIMS {
                return Ok(None);
            }
            asked += 1;
            let statements = best(claims(polite, &hit.id, id).await?);
            if statements.is_empty() {
                continue;
            }
            let mut ids = named_ids(&statements);
            ids.insert(0, hit.id.clone());
            let names = entities(polite, &ids, language).await?;
            let Some(mut answer) = fact_answer(property, &hit, &statements, &names, today) else {
                continue;
            };
            let entity = names.get(&hit.id).cloned().unwrap_or_default();
            let shown_name = entity.label.clone().unwrap_or_else(|| hit.label.clone());
            let mut related = related(kind_of(&hit.description), &shown_name, Some(property.label));
            // Values that are things can be asked about.
            for row in answer
                .rows
                .iter()
                .map(|row| &row.value)
                .chain([&answer.result])
            {
                if let Some(query) = &row.query
                    && !related.iter().any(|known| &known.query == query)
                    && related.len() < 8
                {
                    related.insert(
                        0,
                        AnswerSuggestion {
                            label: capitalized(query),
                            query: query.clone(),
                        },
                    );
                }
            }
            answer.engine_name = String::new();
            return Ok(Some(FactFound {
                answer,
                hit,
                entity,
                related,
            }));
        }
    }
    Ok(None)
}

/// A few facts about a thing, for the Answers pill: what its kind usually has.
pub async fn key_facts(
    polite: &Polite,
    hit: &Hit,
    language: &str,
) -> Result<Vec<AnswerRow>, String> {
    let properties = key_properties(kind_of(&hit.description));
    if properties.is_empty() {
        return Ok(Vec::new());
    }
    let asked = futures_util::future::join_all(
        properties
            .iter()
            .map(|property| claims(polite, &hit.id, property.ids[0])),
    )
    .await;
    let mut found: Vec<(&Property, Vec<Statement>)> = Vec::new();
    for (property, statements) in properties.iter().zip(asked) {
        let statements = best(statements?);
        if !statements.is_empty() {
            found.push((property, statements));
        }
    }
    let ids: Vec<String> = {
        let all: Vec<Statement> = found
            .iter()
            .flat_map(|(_, statements)| statements.clone())
            .collect();
        named_ids(&all)
    };
    let names = if ids.is_empty() {
        HashMap::new()
    } else {
        entities(polite, &ids, language).await?
    };
    let mut rows = Vec::new();
    for (property, statements) in found {
        let value = if property.value == Shape::Age {
            None
        } else if property.ids[0] == "P570" || property.ids[0] == "P569" {
            statements
                .first()
                .and_then(|statement| datum_text(&statement.datum, &names))
        } else {
            let values: Vec<AnswerText> = statements
                .iter()
                .filter_map(|statement| datum_text(&statement.datum, &names))
                .take(property.many)
                .collect();
            match values.len() {
                0 => None,
                1 => values.into_iter().next(),
                _ => Some(AnswerText::plain(
                    values
                        .iter()
                        .map(|value| value.text.clone())
                        .collect::<Vec<_>>()
                        .join(", "),
                )),
            }
        };
        if let Some(value) = value {
            rows.push(AnswerRow {
                key: String::new(),
                label: property.label.to_owned(),
                value,
            });
        }
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn statements_are_read() {
        let answer = json!({ "claims": { "P1082": [
            {
                "mainsnak": { "snaktype": "value", "datavalue": { "type": "quantity", "value": { "amount": "+125440000", "unit": "1" } } },
                "rank": "normal",
                "qualifiers": { "P585": [ { "snaktype": "value", "datavalue": { "type": "time", "value": { "time": "+2022-10-01T00:00:00Z", "precision": 11 } } } ] }
            },
            {
                "mainsnak": { "snaktype": "value", "datavalue": { "type": "quantity", "value": { "amount": "+127094745", "unit": "1" } } },
                "rank": "normal",
                "qualifiers": { "P585": [ { "snaktype": "value", "datavalue": { "type": "time", "value": { "time": "+2015-10-01T00:00:00Z", "precision": 11 } } } ] }
            },
            {
                "mainsnak": { "snaktype": "value", "datavalue": { "type": "quantity", "value": { "amount": "+1", "unit": "1" } } },
                "rank": "deprecated"
            }
        ]}});
        let statements = best(parse_claims(&answer, "P1082"));
        assert_eq!(statements.len(), 1, "the latest count");
        let names = HashMap::new();
        assert_eq!(
            datum_text(&statements[0].datum, &names).unwrap().text,
            "125,440,000"
        );
        assert_eq!(
            time_text(statements[0].at.as_ref().unwrap())
                .date
                .unwrap()
                .iso,
            "2022-10-01"
        );
    }

    #[test]
    fn values_are_written() {
        let names = HashMap::from([(
            "Q55400".to_owned(),
            Entity {
                label: Some("Hayao Miyazaki".to_owned()),
                ..Entity::default()
            },
        )]);
        let director = datum_text(&Datum::Item("Q55400".to_owned()), &names).unwrap();
        assert_eq!(director.text, "Hayao Miyazaki");
        assert_eq!(director.query.as_deref(), Some("Hayao Miyazaki"));
        let height = Datum::Quantity {
            amount: "8848.86".to_owned(),
            unit: Some("Q11573".to_owned()),
        };
        assert_eq!(datum_text(&height, &names).unwrap().text, "8,848.86 m");
        let born = Time {
            value: "+1879-03-14T00:00:00Z".to_owned(),
            precision: 11,
        };
        assert_eq!(time_text(&born).text, "14 March 1879");
        assert_eq!(time_text(&born).date.unwrap().iso, "1879-03-14");
        let old = Time {
            value: "-0500-00-00T00:00:00Z".to_owned(),
            precision: 9,
        };
        assert_eq!(time_text(&old).text, "500 BC");
        let century = Time {
            value: "+1850-00-00T00:00:00Z".to_owned(),
            precision: 7,
        };
        assert_eq!(time_text(&century).text, "19th century");
    }

    #[test]
    fn things_are_told_apart_by_their_description() {
        assert_eq!(kind_of("island country in East Asia"), Kind::Country);
        assert_eq!(
            kind_of("2001 Japanese animated film by Hayao Miyazaki"),
            Kind::Film
        );
        assert_eq!(kind_of("Japanese film director (born 1941)"), Kind::Person);
        assert_eq!(kind_of("1949 dystopian novel by George Orwell"), Kind::Book);
        assert_eq!(
            kind_of("Earth's highest mountain above sea level"),
            Kind::Mountain
        );
        assert_eq!(
            kind_of("chemical element with symbol Fe and atomic number 26"),
            Kind::Element
        );
        assert_eq!(
            kind_of("German-born theoretical physicist (1879–1955)"),
            Kind::Person
        );
        assert_eq!(kind_of("capital and largest city of Japan"), Kind::City);
    }

    #[test]
    fn names_and_years_go_last() {
        let hit = |id: &str, description: &str| Hit {
            id: id.to_owned(),
            label: "1984".to_owned(),
            description: description.to_owned(),
        };
        let hits = without_pages(vec![
            hit(
                "Q2432",
                "leap year starting on Sunday of the Gregorian calendar",
            ),
            hit("Q1", "Wikimedia disambiguation page"),
            hit("Q208460", "1949 dystopian novel by George Orwell"),
        ]);
        assert_eq!(hits[0].id, "Q208460");
        assert_eq!(hits.len(), 3);
        // Asked who wrote it, the novel goes before the album and the film of the same name.
        let ordered = ordered_for(
            &questions::AUTHOR,
            vec![
                hit("Q1", "1984 studio album by Van Halen"),
                hit("Q2", "1956 film by Michael Anderson"),
                hit("Q3", "1949 dystopian novel by George Orwell"),
            ],
        );
        assert_eq!(ordered[0].id, "Q3");
        assert_eq!(ordered[1].id, "Q1");
    }

    #[test]
    fn languages_map_to_wikipedias() {
        assert_eq!(wiki_language("pt-BR"), "pt");
        assert_eq!(wiki_language("nb"), "no");
        assert_eq!(wiki_language("en"), "en");
    }
}
