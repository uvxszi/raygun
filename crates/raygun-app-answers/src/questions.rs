//! Questions answered online, read from the words alone: facts about something (`population of
//! Japan`, `who directed Spirited Away`, `how tall is the Eiffel Tower`), what something is
//! (`what is a black hole`), what a word means (`define serendipity`) and translations
//! (`translate good morning to French`). Questions are read in English.

use super::languages;
use super::text::{
    split_first, split_last, strip_prefix_words, strip_suffix_words, unquoted, without_article,
};

/// What a fact is about, as Wikidata keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    /// A number with its unit.
    Quantity,
    /// Something else in Wikidata: a person, a place, a work.
    Item,
    /// A date.
    Time,
    /// An age, worked out from a date of birth (and of death).
    Age,
    /// Text: a formula, a symbol, an address.
    Text,
}

/// A property asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Property {
    /// Wikidata's properties, tried in turn: the first the subject has is answered.
    pub ids: &'static [&'static str],
    /// The fact's name: `Population`.
    pub label: &'static str,
    pub value: Value,
    /// Values shown at most (cast members, official languages).
    pub many: usize,
}

const fn property(
    ids: &'static [&'static str],
    label: &'static str,
    value: Value,
    many: usize,
) -> Property {
    Property {
        ids,
        label,
        value,
        many,
    }
}

pub const POPULATION: Property = property(&["P1082"], "Population", Value::Quantity, 1);
pub const AREA: Property = property(&["P2046"], "Area", Value::Quantity, 1);
pub const CAPITAL: Property = property(&["P36"], "Capital", Value::Item, 2);
pub const CURRENCY: Property = property(&["P38"], "Currency", Value::Item, 3);
pub const LANGUAGES: Property = property(&["P37", "P2936"], "Official languages", Value::Item, 6);
pub const HEAD_OF_STATE: Property = property(&["P35"], "Head of state", Value::Item, 1);
pub const HEAD_OF_GOVERNMENT: Property = property(&["P6"], "Head of government", Value::Item, 1);
pub const HEIGHT: Property = property(&["P2048", "P2044"], "Height", Value::Quantity, 1);
pub const ELEVATION: Property = property(&["P2044", "P2048"], "Elevation", Value::Quantity, 1);
pub const LENGTH: Property = property(&["P2043", "P2047"], "Length", Value::Quantity, 1);
pub const WIDTH: Property = property(&["P2049"], "Width", Value::Quantity, 1);
pub const DEPTH: Property = property(&["P4511", "P4044"], "Depth", Value::Quantity, 1);
pub const MASS: Property = property(&["P2067"], "Mass", Value::Quantity, 1);
pub const DIAMETER: Property = property(&["P2386", "P2120"], "Diameter", Value::Quantity, 1);
pub const RADIUS: Property = property(&["P2120"], "Radius", Value::Quantity, 1);
pub const SPEED: Property = property(&["P2052"], "Speed", Value::Quantity, 1);
pub const DIRECTOR: Property = property(&["P57"], "Director", Value::Item, 3);
pub const AUTHOR: Property = property(
    &["P50", "P86", "P676", "P58", "P170"],
    "Author",
    Value::Item,
    3,
);
pub const COMPOSER: Property = property(&["P86"], "Composer", Value::Item, 3);
pub const CREATOR: Property = property(&["P170", "P50", "P86", "P178"], "Creator", Value::Item, 3);
pub const FOUNDER: Property = property(&["P112"], "Founded by", Value::Item, 4);
pub const INVENTOR: Property = property(&["P61"], "Discoverer or inventor", Value::Item, 3);
pub const ARCHITECT: Property = property(&["P84", "P287"], "Architect", Value::Item, 3);
pub const DESIGNER: Property = property(&["P287", "P84"], "Designed by", Value::Item, 3);
pub const PERFORMER: Property = property(&["P175"], "Performer", Value::Item, 3);
pub const PRODUCER: Property = property(&["P162"], "Producer", Value::Item, 4);
pub const DEVELOPER: Property = property(&["P178"], "Developer", Value::Item, 3);
pub const MANUFACTURER: Property = property(&["P176"], "Manufacturer", Value::Item, 2);
pub const PUBLISHER: Property = property(&["P123"], "Publisher", Value::Item, 2);
pub const CAST: Property = property(&["P161"], "Cast", Value::Item, 8);
pub const CEO: Property = property(&["P169"], "Chief executive officer", Value::Item, 1);
pub const OWNER: Property = property(&["P127"], "Owned by", Value::Item, 3);
pub const HEADQUARTERS: Property = property(&["P159"], "Headquarters", Value::Item, 1);
pub const EMPLOYEES: Property = property(&["P1128"], "Employees", Value::Quantity, 1);
pub const BORN: Property = property(&["P569"], "Date of birth", Value::Time, 1);
pub const DIED: Property = property(&["P570"], "Date of death", Value::Time, 1);
pub const BIRTHPLACE: Property = property(&["P19"], "Place of birth", Value::Item, 1);
pub const DEATHPLACE: Property = property(&["P20"], "Place of death", Value::Item, 1);
pub const AGE: Property = property(&["P569", "P571"], "Age", Value::Age, 1);
pub const SPOUSE: Property = property(&["P26"], "Spouse", Value::Item, 3);
pub const CHILDREN: Property = property(&["P40"], "Children", Value::Item, 8);
pub const FATHER: Property = property(&["P22"], "Father", Value::Item, 1);
pub const MOTHER: Property = property(&["P25"], "Mother", Value::Item, 1);
pub const CITIZENSHIP: Property = property(&["P27"], "Citizenship", Value::Item, 3);
pub const OCCUPATION: Property = property(&["P106"], "Occupation", Value::Item, 4);
pub const FOUNDED: Property = property(&["P571", "P580"], "Founded", Value::Time, 1);
pub const RELEASED: Property = property(&["P577", "P571"], "Published", Value::Time, 1);
pub const OPENED: Property = property(&["P1619", "P571"], "Opened", Value::Time, 1);
pub const DISCOVERED: Property = property(&["P575"], "Discovered", Value::Time, 1);
pub const GENRE: Property = property(&["P136"], "Genre", Value::Item, 4);
pub const COUNTRY: Property = property(&["P17", "P495"], "Country", Value::Item, 2);
pub const LOCATION: Property = property(&["P131", "P276", "P17"], "Location", Value::Item, 2);
pub const CONTINENT: Property = property(&["P30"], "Continent", Value::Item, 2);
pub const TIME_ZONE: Property = property(&["P421"], "Time zone", Value::Item, 3);
pub const GDP: Property = property(&["P2131"], "GDP (nominal)", Value::Quantity, 1);
pub const LIFE_EXPECTANCY: Property = property(&["P2250"], "Life expectancy", Value::Quantity, 1);
pub const BOILING: Property = property(&["P2102"], "Boiling point", Value::Quantity, 1);
pub const MELTING: Property = property(&["P2101"], "Melting point", Value::Quantity, 1);
pub const DENSITY: Property = property(&["P2054"], "Density", Value::Quantity, 1);
pub const ATOMIC_NUMBER: Property = property(&["P1086"], "Atomic number", Value::Quantity, 1);
pub const FORMULA: Property = property(&["P274"], "Chemical formula", Value::Text, 1);
pub const SYMBOL: Property = property(&["P246"], "Element symbol", Value::Text, 1);
pub const DISTANCE: Property = property(&["P2583"], "Distance from Earth", Value::Quantity, 1);
pub const ORBITAL_PERIOD: Property = property(&["P2146"], "Orbital period", Value::Quantity, 1);
pub const TEMPERATURE: Property = property(&["P2076"], "Temperature", Value::Quantity, 1);
pub const WEBSITE: Property = property(&["P856"], "Official website", Value::Text, 1);
pub const MEMBERS: Property = property(&["P527"], "Members", Value::Item, 8);
pub const LABEL: Property = property(&["P264"], "Record label", Value::Item, 3);
pub const EPISODES: Property = property(&["P1113"], "Episodes", Value::Quantity, 1);
pub const SEASONS: Property = property(&["P2437"], "Seasons", Value::Quantity, 1);
pub const DURATION: Property = property(&["P2047"], "Duration", Value::Quantity, 1);
pub const BUDGET: Property = property(&["P2130"], "Budget", Value::Quantity, 1);
pub const BOX_OFFICE: Property = property(&["P2142"], "Box office", Value::Quantity, 1);
pub const MOTTO: Property = property(&["P1451"], "Motto", Value::Text, 1);
pub const ANTHEM: Property = property(&["P85"], "Anthem", Value::Item, 1);
pub const HIGHEST_POINT: Property = property(&["P610"], "Highest point", Value::Item, 1);
pub const FLOORS: Property = property(&["P1101"], "Floors", Value::Quantity, 1);
pub const ORIGINAL_LANGUAGE: Property =
    property(&["P364", "P407"], "Original language", Value::Item, 2);
pub const BASED_ON: Property = property(&["P144"], "Based on", Value::Item, 2);
pub const SEQUEL: Property = property(&["P156"], "Followed by", Value::Item, 1);
pub const PREQUEL: Property = property(&["P155"], "Follows", Value::Item, 1);
pub const MOUNTAIN_RANGE: Property = property(&["P4552"], "Mountain range", Value::Item, 1);
pub const MOUTH: Property = property(&["P403"], "Flows into", Value::Item, 1);
pub const SOURCE: Property = property(&["P885"], "Source", Value::Item, 1);
pub const DISCHARGE: Property = property(&["P2225"], "Discharge", Value::Quantity, 1);
pub const NETWORK: Property = property(&["P449"], "Original network", Value::Item, 2);
pub const COUNTRY_OF_ORIGIN: Property =
    property(&["P495", "P17"], "Country of origin", Value::Item, 2);

/// `<name> of <subject>`.
const OF: &[(&str, Property)] = &[
    ("population", POPULATION),
    ("number of inhabitants", POPULATION),
    ("area", AREA),
    ("surface area", AREA),
    ("size", AREA),
    ("capital city", CAPITAL),
    ("capital", CAPITAL),
    ("currency", CURRENCY),
    ("official languages", LANGUAGES),
    ("official language", LANGUAGES),
    ("languages", LANGUAGES),
    ("language", LANGUAGES),
    ("president", HEAD_OF_STATE),
    ("head of state", HEAD_OF_STATE),
    ("king", HEAD_OF_STATE),
    ("queen", HEAD_OF_STATE),
    ("monarch", HEAD_OF_STATE),
    ("prime minister", HEAD_OF_GOVERNMENT),
    ("head of government", HEAD_OF_GOVERNMENT),
    ("chancellor", HEAD_OF_GOVERNMENT),
    ("mayor", HEAD_OF_GOVERNMENT),
    ("leader", HEAD_OF_GOVERNMENT),
    ("height", HEIGHT),
    ("elevation", ELEVATION),
    ("altitude", ELEVATION),
    ("length", LENGTH),
    ("width", WIDTH),
    ("depth", DEPTH),
    ("mass", MASS),
    ("weight", MASS),
    ("diameter", DIAMETER),
    ("radius", RADIUS),
    ("top speed", SPEED),
    ("speed", SPEED),
    ("director", DIRECTOR),
    ("directors", DIRECTOR),
    ("author", AUTHOR),
    ("authors", AUTHOR),
    ("writer", AUTHOR),
    ("composer", COMPOSER),
    ("creator", CREATOR),
    ("creators", CREATOR),
    ("founder", FOUNDER),
    ("founders", FOUNDER),
    ("inventor", INVENTOR),
    ("discoverer", INVENTOR),
    ("architect", ARCHITECT),
    ("designer", DESIGNER),
    ("performer", PERFORMER),
    ("singer", PERFORMER),
    ("artist", PERFORMER),
    ("producer", PRODUCER),
    ("producers", PRODUCER),
    ("developer", DEVELOPER),
    ("manufacturer", MANUFACTURER),
    ("maker", MANUFACTURER),
    ("publisher", PUBLISHER),
    ("cast", CAST),
    ("actors", CAST),
    ("ceo", CEO),
    ("chief executive", CEO),
    ("owner", OWNER),
    ("headquarters", HEADQUARTERS),
    ("number of employees", EMPLOYEES),
    ("employees", EMPLOYEES),
    ("birthday", BORN),
    ("birth date", BORN),
    ("date of birth", BORN),
    ("birthdate", BORN),
    ("death date", DIED),
    ("date of death", DIED),
    ("birthplace", BIRTHPLACE),
    ("place of birth", BIRTHPLACE),
    ("place of death", DEATHPLACE),
    ("age", AGE),
    ("spouse", SPOUSE),
    ("wife", SPOUSE),
    ("husband", SPOUSE),
    ("children", CHILDREN),
    ("kids", CHILDREN),
    ("father", FATHER),
    ("mother", MOTHER),
    ("nationality", CITIZENSHIP),
    ("citizenship", CITIZENSHIP),
    ("occupation", OCCUPATION),
    ("profession", OCCUPATION),
    ("job", OCCUPATION),
    ("founding date", FOUNDED),
    ("date of founding", FOUNDED),
    ("inception", FOUNDED),
    ("release date", RELEASED),
    ("publication date", RELEASED),
    ("date of publication", RELEASED),
    ("genre", GENRE),
    ("genres", GENRE),
    ("country", COUNTRY),
    ("location", LOCATION),
    ("continent", CONTINENT),
    ("time zone", TIME_ZONE),
    ("timezone", TIME_ZONE),
    ("gdp", GDP),
    ("life expectancy", LIFE_EXPECTANCY),
    ("boiling point", BOILING),
    ("melting point", MELTING),
    ("density", DENSITY),
    ("atomic number", ATOMIC_NUMBER),
    ("chemical formula", FORMULA),
    ("formula", FORMULA),
    ("symbol", SYMBOL),
    ("distance", DISTANCE),
    ("distance from earth", DISTANCE),
    ("orbital period", ORBITAL_PERIOD),
    ("temperature", TEMPERATURE),
    ("official website", WEBSITE),
    ("website", WEBSITE),
    ("members", MEMBERS),
    ("band members", MEMBERS),
    ("record label", LABEL),
    ("label", LABEL),
    ("number of episodes", EPISODES),
    ("episodes", EPISODES),
    ("number of seasons", SEASONS),
    ("seasons", SEASONS),
    ("running time", DURATION),
    ("runtime", DURATION),
    ("duration", DURATION),
    ("budget", BUDGET),
    ("box office", BOX_OFFICE),
    ("motto", MOTTO),
    ("national anthem", ANTHEM),
    ("anthem", ANTHEM),
    ("highest point", HIGHEST_POINT),
    ("number of floors", FLOORS),
    ("floors", FLOORS),
    ("original language", ORIGINAL_LANGUAGE),
    ("sequel", SEQUEL),
    ("prequel", PREQUEL),
    ("network", NETWORK),
    ("country of origin", COUNTRY_OF_ORIGIN),
    ("source", SOURCE),
    ("mouth", MOUTH),
    ("based on", BASED_ON),
    ("discharge", DISCHARGE),
];

/// Names of facts that ask a question even without `what is the`: titles aren't named after
/// them.
const PLAINLY_ASKED: &[&str] = &[
    "population",
    "number of inhabitants",
    "area",
    "surface area",
    "capital",
    "capital city",
    "currency",
    "official language",
    "official languages",
    "gdp",
    "life expectancy",
    "boiling point",
    "melting point",
    "density",
    "atomic number",
    "chemical formula",
    "date of birth",
    "birth date",
    "birthdate",
    "date of death",
    "death date",
    "place of birth",
    "place of death",
    "birthplace",
    "elevation",
    "altitude",
    "height",
    "diameter",
    "headquarters",
    "ceo",
    "number of employees",
    "time zone",
    "timezone",
    "release date",
    "publication date",
    "founding date",
    "date of founding",
    "distance from earth",
    "orbital period",
    "prime minister",
    "president",
    "head of state",
    "head of government",
    "running time",
    "runtime",
    "box office",
    "number of episodes",
    "number of seasons",
    "number of floors",
];

/// `who <verb> <subject>`.
const WHO: &[(&str, Property)] = &[
    ("directed", DIRECTOR),
    ("wrote", AUTHOR),
    ("writes", AUTHOR),
    ("authored", AUTHOR),
    ("composed", COMPOSER),
    ("painted", CREATOR),
    ("drew", CREATOR),
    ("sculpted", CREATOR),
    ("created", CREATOR),
    ("made", CREATOR),
    ("founded", FOUNDER),
    ("started", FOUNDER),
    ("invented", INVENTOR),
    ("discovered", INVENTOR),
    ("designed", DESIGNER),
    ("built", ARCHITECT),
    ("sang", PERFORMER),
    ("sings", PERFORMER),
    ("performed", PERFORMER),
    ("produced", PRODUCER),
    ("developed", DEVELOPER),
    ("published", PUBLISHER),
    ("starred in", CAST),
    ("stars in", CAST),
    ("acted in", CAST),
    ("is in the cast of", CAST),
    ("owns", OWNER),
    ("runs", CEO),
    ("leads", HEAD_OF_GOVERNMENT),
];

/// `how <adjective> is <subject>`.
const HOW: &[(&str, Property)] = &[
    ("tall", HEIGHT),
    ("high", ELEVATION),
    ("long", LENGTH),
    ("wide", WIDTH),
    ("deep", DEPTH),
    ("big", AREA),
    ("large", AREA),
    ("heavy", MASS),
    ("fast", SPEED),
    ("far away", DISTANCE),
    ("far", DISTANCE),
    ("old", AGE),
    ("hot", TEMPERATURE),
];

/// `when was <subject> <verb>`.
const WHEN: &[(&str, Property)] = &[
    ("born", BORN),
    ("die", DIED),
    ("died", DIED),
    ("founded", FOUNDED),
    ("established", FOUNDED),
    ("formed", FOUNDED),
    ("created", FOUNDED),
    ("started", FOUNDED),
    ("built", OPENED),
    ("constructed", OPENED),
    ("completed", OPENED),
    ("opened", OPENED),
    ("released", RELEASED),
    ("published", RELEASED),
    ("written", RELEASED),
    ("come out", RELEASED),
    ("came out", RELEASED),
    ("made", RELEASED),
    ("discovered", DISCOVERED),
    ("invented", FOUNDED),
];

/// A question answered online.
#[derive(Debug, Clone, PartialEq)]
pub enum Question {
    Fact { property: Property, subject: String },
    About(String),
    Define(String),
    Translate { text: String, to: &'static str },
}

/// A question, and whether the words plainly ask it (Search → All's card) rather than only
/// might.
#[derive(Debug, Clone, PartialEq)]
pub struct Read {
    pub question: Question,
    pub strong: bool,
}

fn subject(text: &str) -> Option<String> {
    let text = text.trim().trim_end_matches(['?', '.', '!']).trim();
    let text = text.strip_suffix("'s").unwrap_or(text);
    let text = without_article(text).trim();
    (!text.is_empty() && text.chars().count() <= 120).then(|| text.to_owned())
}

fn fact(property: Property, subject_text: &str, strong: bool) -> Option<Read> {
    Some(Read {
        question: Question::Fact {
            property,
            subject: subject(subject_text)?,
        },
        strong,
    })
}

fn of_property(name: &str) -> Option<Property> {
    OF.iter()
        .find(|(known, _)| *known == name)
        .map(|(_, property)| *property)
}

/// A translation asked for.
fn translation(lower: &str, original: &str) -> Option<Read> {
    // Keep the words to translate as typed.
    let typed = |text: &str| -> String {
        let at = original.to_lowercase().find(text).unwrap_or(0);
        original
            .get(at..at + text.len())
            .map_or_else(|| text.to_owned(), str::to_owned)
    };
    let (rest, asked) = strip_prefix_words(
        lower,
        &[
            "how do you say",
            "how do i say",
            "how to say",
            "how would you say",
            "how does one say",
            "what is the word for",
            "translate",
        ],
    );
    if asked {
        let (text, language) = match split_last(rest, &["to", "into", "in"]) {
            Some((text, _, language)) => (text, languages::tag_of(language)?),
            None => return None,
        };
        let typed_text = typed(text);
        let (text, _) = unquoted(&typed_text);
        let text = text.trim();
        return (!text.is_empty() && text.chars().count() <= 500).then(|| Read {
            question: Question::Translate {
                text: text.to_owned(),
                to: language,
            },
            strong: true,
        });
    }
    // `"good morning" in French`; `good morning in French` only might be.
    let (text, _, language) = split_last(lower, &["in", "to"])?;
    let language = languages::tag_of(language)?;
    let typed_text = typed(text);
    let (unquoted_text, quoted) = unquoted(&typed_text);
    let words = unquoted_text.split_whitespace().count();
    if words == 0 || words > 8 {
        return None;
    }
    Some(Read {
        question: Question::Translate {
            text: unquoted_text.to_owned(),
            to: language,
        },
        strong: quoted,
    })
}

fn definition(lower: &str) -> Option<Read> {
    let (rest, asked) = strip_prefix_words(
        lower,
        &[
            "what is the definition of",
            "what's the definition of",
            "what is the meaning of",
            "what's the meaning of",
            "definition of",
            "meaning of",
            "synonyms for",
            "synonyms of",
            "synonym for",
            "synonym of",
            "define",
        ],
    );
    let word = if asked {
        rest
    } else if let Some(word) = lower
        .strip_prefix("what does ")
        .and_then(|rest| rest.strip_suffix(" mean"))
    {
        word
    } else {
        let (word, ended) = strip_suffix_words(lower, &["meaning", "definition", "synonyms"]);
        if !ended {
            return None;
        }
        word
    };
    let (word, _) = unquoted(word);
    let word = word.trim();
    let words = word.split_whitespace().count();
    (1..=4).contains(&words).then(|| Read {
        question: Question::Define(word.to_owned()),
        strong: true,
    })
}

/// Facts: `population of Japan`, `Japan's population`, `who directed Spirited Away`, `how tall
/// is the Eiffel Tower`, `when was Einstein born`, `where was Mozart born`.
fn fact_question(lower: &str) -> Option<Read> {
    let lead_ins = [
        "what is the",
        "what's the",
        "whats the",
        "what was the",
        "what are the",
        "what were the",
        "who is the",
        "who's the",
        "who was the",
        "who are the",
        "who were the",
        "when is the",
        "when was the",
        "where is the",
        "how much is the",
        "how many",
        "tell me the",
    ];
    let (rest, asked) = strip_prefix_words(lower, &lead_ins);
    // `the` alone doesn't ask anything (`The Age of Adaline`).
    let rest = without_article(rest);
    // `<property> of <subject>`: without a question's words, only facts no title is likely to
    // be named after (`Children of Men`, `Age of Empires` and `Father of the Bride` are titles).
    if let Some((name, _, subject_text)) = split_first(rest, &["of", "for"])
        && let Some(property) = of_property(name)
    {
        let short = subject_text.split_whitespace().count() <= 6;
        return fact(
            property,
            subject_text,
            asked || (short && PLAINLY_ASKED.contains(&name)),
        );
    }
    // `<subject>'s <property>`.
    if let Some((subject_text, name)) = rest.split_once("'s ").or_else(|| rest.split_once("’s "))
        && let Some(property) = of_property(name.trim())
    {
        return fact(property, subject_text, true);
    }
    // `who directed <subject>`.
    if let Some(after) = lower.strip_prefix("who ") {
        for (verb, property) in WHO {
            if let Some(subject_text) = after
                .strip_prefix(verb)
                .filter(|rest| rest.starts_with(' '))
            {
                return fact(*property, subject_text, true);
            }
        }
        // `who is <subject> married to`.
        if let Some(rest) = after
            .strip_prefix("is ")
            .or_else(|| after.strip_prefix("was "))
            .and_then(|rest| rest.strip_suffix(" married to"))
        {
            return fact(SPOUSE, rest, true);
        }
    }
    // `how tall is <subject>`; `how much does <subject> weigh`.
    if let Some(after) = lower.strip_prefix("how ") {
        for (adjective, property) in HOW {
            if let Some(rest) = after.strip_prefix(adjective)
                && let Some(subject_text) = rest
                    .strip_prefix(" is ")
                    .or_else(|| rest.strip_prefix(" are "))
                    .or_else(|| rest.strip_prefix(" was "))
            {
                let subject_text = strip_suffix_words(
                    subject_text,
                    &["from earth", "from the earth", "now", "today"],
                )
                .0;
                return fact(*property, subject_text, true);
            }
        }
        if let Some(subject_text) = after
            .strip_prefix("much does ")
            .and_then(|rest| rest.strip_suffix(" weigh"))
        {
            return fact(MASS, subject_text, true);
        }
        // `how many people live in <subject>`.
        for (lead, property) in [
            ("many people live in ", POPULATION),
            ("many people are in ", POPULATION),
            ("many people are there in ", POPULATION),
            ("many inhabitants does ", POPULATION),
            ("many employees does ", EMPLOYEES),
            ("many episodes does ", EPISODES),
            ("many episodes of ", EPISODES),
            ("many episodes are in ", EPISODES),
            ("many seasons does ", SEASONS),
            ("many seasons of ", SEASONS),
            ("many floors does ", FLOORS),
            ("many children does ", CHILDREN),
        ] {
            if let Some(rest) = after.strip_prefix(lead) {
                let rest = strip_suffix_words(rest, &["have", "has"]).0;
                return fact(property, rest, true);
            }
        }
    }
    // `when was <subject> born`, `when did <subject> die`.
    for lead in [
        "when was ",
        "when were ",
        "when did ",
        "when is ",
        "what year was ",
        "what year did ",
    ] {
        if let Some(rest) = lower.strip_prefix(lead) {
            for (verb, property) in WHEN {
                if let Some(subject_text) = rest
                    .strip_suffix(verb)
                    .filter(|subject_text| subject_text.ends_with(' '))
                {
                    return fact(*property, subject_text, true);
                }
            }
        }
    }
    // `where was <subject> born`, `where did <subject> die`.
    if let Some(rest) = lower
        .strip_prefix("where was ")
        .or_else(|| lower.strip_prefix("where were "))
        .and_then(|rest| rest.strip_suffix(" born"))
    {
        return fact(BIRTHPLACE, rest, true);
    }
    if let Some(rest) = lower
        .strip_prefix("where did ")
        .and_then(|rest| rest.strip_suffix(" die"))
    {
        return fact(DEATHPLACE, rest, true);
    }
    // `what language is spoken in <subject>`, `what currency does <subject> use`.
    for (lead, property) in [
        ("what language is spoken in ", LANGUAGES),
        ("what languages are spoken in ", LANGUAGES),
        ("what language do they speak in ", LANGUAGES),
        ("what currency is used in ", CURRENCY),
        ("what is the currency in ", CURRENCY),
        ("what country is ", COUNTRY),
        ("what genre is ", GENRE),
        ("what time zone is ", TIME_ZONE),
    ] {
        if let Some(rest) = lower.strip_prefix(lead) {
            let rest = strip_suffix_words(rest, &["in", "from"]).0;
            return fact(property, rest, true);
        }
    }
    if let Some(rest) = lower
        .strip_prefix("what currency does ")
        .and_then(|rest| rest.strip_suffix(" use"))
    {
        return fact(CURRENCY, rest, true);
    }
    // `<subject> <property>`, typed as keywords (`japan population`): only might be one.
    if !asked {
        let words: Vec<&str> = lower.split(' ').collect();
        for take in [3, 2, 1] {
            if words.len() > take {
                let name = words[words.len() - take..].join(" ");
                if let Some(property) = of_property(&name) {
                    return fact(property, &words[..words.len() - take].join(" "), false);
                }
            }
        }
    }
    None
}

/// What something is: `what is a black hole`, `who is Ada Lovelace`, `tell me about Mars`.
fn about(lower: &str) -> Option<Read> {
    let (rest, asked) = strip_prefix_words(
        lower,
        &[
            "what is",
            "what's",
            "whats",
            "what are",
            "what was",
            "what were",
            "who is",
            "who's",
            "who was",
            "who are",
            "who were",
            "tell me about",
            "explain",
            "where is",
            "where's",
            "where are",
            "information about",
            "info about",
            "facts about",
        ],
    );
    if !asked {
        return None;
    }
    let rest = strip_suffix_words(rest, &["located", "exactly"]).0;
    let topic = subject(rest)?;
    Some(Read {
        question: Question::About(topic),
        strong: true,
    })
}

/// The question the words ask online, if they ask one. `original` keeps the words' case (the
/// text to translate), `lower` is the same words in lower case.
pub fn read(lower: &str, original: &str) -> Option<Read> {
    definition(lower)
        .or_else(|| translation(lower, original))
        .or_else(|| fact_question(lower))
        .or_else(|| about(lower))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asked(query: &str) -> Option<(Question, bool)> {
        let lower = query.to_lowercase();
        read(&lower, query).map(|read| (read.question, read.strong))
    }

    fn fact_of(query: &str) -> (&'static str, String, bool) {
        match asked(query) {
            Some((Question::Fact { property, subject }, strong)) => {
                (property.label, subject, strong)
            }
            other => panic!("{query}: {other:?}"),
        }
    }

    #[test]
    fn facts_are_read() {
        assert_eq!(
            fact_of("population of Japan"),
            ("Population", "japan".to_owned(), true)
        );
        assert_eq!(fact_of("What is the population of Japan?").0, "Population");
        assert_eq!(fact_of("height of Mount Everest").1, "mount everest");
        assert_eq!(
            fact_of("who directed Spirited Away"),
            ("Director", "spirited away".to_owned(), true)
        );
        assert_eq!(fact_of("who wrote 1984").1, "1984");
        assert_eq!(
            fact_of("how tall is the Eiffel Tower"),
            ("Height", "eiffel tower".to_owned(), true)
        );
        assert_eq!(fact_of("when was Einstein born").0, "Date of birth");
        assert_eq!(fact_of("where was Mozart born").0, "Place of birth");
        assert_eq!(fact_of("how old is Taylor Swift").0, "Age");
        assert_eq!(fact_of("Japan's population").0, "Population");
        assert_eq!(fact_of("capital of France").1, "france");
        assert_eq!(fact_of("how many people live in Tokyo").0, "Population");
        assert_eq!(
            fact_of("what language is spoken in Brazil").0,
            "Official languages"
        );
        let (label, subject, strong) = fact_of("japan population");
        assert_eq!(
            (label, subject.as_str(), strong),
            ("Population", "japan", false)
        );
    }

    #[test]
    fn other_questions_are_read() {
        assert_eq!(
            asked("define serendipity"),
            Some((Question::Define("serendipity".to_owned()), true))
        );
        assert_eq!(
            asked("what does ephemeral mean"),
            Some((Question::Define("ephemeral".to_owned()), true))
        );
        assert_eq!(
            asked("translate Good morning to French"),
            Some((
                Question::Translate {
                    text: "Good morning".to_owned(),
                    to: "fr"
                },
                true
            ))
        );
        assert_eq!(
            asked("how do you say thank you in Japanese"),
            Some((
                Question::Translate {
                    text: "thank you".to_owned(),
                    to: "ja"
                },
                true
            ))
        );
        assert_eq!(
            asked("\"cat\" in German").map(|(_, strong)| strong),
            Some(true)
        );
        assert_eq!(
            asked("love in french").map(|(_, strong)| strong),
            Some(false)
        );
        assert_eq!(
            asked("what is a black hole"),
            Some((Question::About("black hole".to_owned()), true))
        );
        assert_eq!(
            asked("who is Ada Lovelace"),
            Some((Question::About("ada lovelace".to_owned()), true))
        );
    }

    #[test]
    fn titles_are_not_questions() {
        for title in [
            "Blue",
            "1984",
            "The Matrix",
            "Spirited Away",
            "Lost in Translation",
            "Midnight in Paris",
            "Sleepless in Seattle",
            "An American in Paris",
            "Mean Girls",
            "Children of Men",
            "Age of Empires",
            "The Age of Adaline",
            "Father of the Bride",
            "Speed of Sound",
            "King of the Hill",
            "Seasons of Love",
            "Mother of Pearl",
            "Queen of the Damned",
        ] {
            let lower = title.to_lowercase();
            assert!(
                read(&lower, title).is_none_or(|read| !read.strong),
                "{title}"
            );
        }
        // Words that start like a question, about something: still questions.
        assert!(asked("What Is Love").is_some());
    }
}
