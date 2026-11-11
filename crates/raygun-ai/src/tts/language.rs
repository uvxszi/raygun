//! Which language a book is written in, read from its text, so a voice that speaks it reads it:
//! the script first, then (for the Latin, Cyrillic, Arabic and Devanagari scripts) the letters
//! and the most common words each language has. Only the book's language is guessed this way,
//! from a whole chapter (a book that names its language is read in it); a passage changes the
//! voice only when it's long and clearly in another language ([`foreign_passage`]).

/// How much of a text is looked at.
const SAMPLE_CHARS: usize = 4_000;
/// A language's score must beat the next one's by this much (in tenths) to be the guess.
const MARGIN_TENTHS: usize = 11;
/// A passage in the book's script needs this many letters to be read in another language…
const PASSAGE_LETTERS: usize = 150;
/// …and one in another script this many.
const PASSAGE_SCRIPT_LETTERS: usize = 40;

/// The language `text` is most likely in, as a BCP 47 primary tag (`en`, `de`, `zh`), or nothing
/// when it's too short or unclear to tell.
pub fn guess(text: &str) -> Option<&'static str> {
    let sample: String = text.chars().take(SAMPLE_CHARS).collect();
    let found = detect(&sample)?;
    match found.script {
        Script::Devanagari | Script::Cyrillic | Script::Latin => {
            found.best(MARGIN_TENTHS, 4).map(|(tag, _)| tag)
        }
        _ => found.single,
    }
}

/// The language of a passage (a paragraph) that is clearly not `book`'s: long enough, and in
/// another script or scoring well above the book's language. Quotations, names and short lines
/// stay in the book's language.
pub fn foreign_passage(text: &str, book: &str) -> Option<&'static str> {
    let book = primary(book);
    let sample: String = text.chars().take(SAMPLE_CHARS).collect();
    let found = detect(&sample)?;
    if Some(found.script) != script_of_language(&book) {
        // Chinese and Japanese share Han characters: a line of them isn't a change of language.
        let east_asian =
            |script: Option<Script>| matches!(script, Some(Script::Han | Script::Kana));
        if east_asian(Some(found.script)) && east_asian(script_of_language(&book)) {
            return None;
        }
        let tag = match found.script {
            Script::Devanagari | Script::Cyrillic | Script::Latin => {
                found.best(15, 3).map(|(tag, _)| tag)
            }
            _ => found.single,
        }?;
        return (found.count >= PASSAGE_SCRIPT_LETTERS
            && found.count * 10 >= found.letters * 8
            && tag != book)
            .then_some(tag);
    }
    if found.count < PASSAGE_LETTERS {
        return None;
    }
    let (tag, top) = found.best(15, 6)?;
    let book_score = found.score_of(&book);
    (tag != book && top >= book_score * 2).then_some(tag)
}

/// A language tag's primary language, lower case (`pt-BR` → `pt`), Norwegian's as `nb`.
fn primary(tag: &str) -> String {
    let primary = tag
        .split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match primary.as_str() {
        "no" | "nn" => "nb".to_owned(),
        _ => primary,
    }
}

/// What a sample is written in: its main script, how many of its letters are in it, and each
/// language's score (for scripts several languages share) or the one language (for the rest).
struct Detection {
    script: Script,
    /// Letters in the main script.
    count: usize,
    /// Letters in any script.
    letters: usize,
    single: Option<&'static str>,
    scores: Vec<(&'static str, usize)>,
}

impl Detection {
    /// The language scoring highest, when it scores at least `least` and beats the next by
    /// `margin_tenths` tenths.
    fn best(&self, margin_tenths: usize, least: usize) -> Option<(&'static str, usize)> {
        let (tag, top) = *self.scores.first()?;
        let second = self.scores.get(1).map_or(0, |(_, score)| *score);
        (top >= least && top * 10 >= second * margin_tenths).then_some((tag, top))
    }

    fn score_of(&self, tag: &str) -> usize {
        self.scores
            .iter()
            .find(|(language, _)| *language == tag)
            .map_or(0, |(_, score)| *score)
    }
}

fn detect(sample: &str) -> Option<Detection> {
    let mut scripts = [0usize; Script::COUNT];
    for c in sample.chars() {
        if let Some(script) = script_of(c) {
            scripts[script as usize] += 1;
        }
    }
    let letters = scripts.iter().sum();
    let (script, count) = scripts
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| **count)
        .map(|(index, count)| (Script::ALL[index], *count))?;
    if count < 8 {
        return None;
    }
    let single = match script {
        Script::Han => {
            // Japanese mixes kana into its Han characters; Chinese has none.
            let kana = scripts[Script::Kana as usize];
            Some(if kana * 10 > count { "ja" } else { "zh" })
        }
        Script::Kana => Some("ja"),
        Script::Hangul => Some("ko"),
        Script::Greek => Some("el"),
        Script::Hebrew => Some("he"),
        Script::Thai => Some("th"),
        Script::Georgian => Some("ka"),
        Script::Armenian => Some("hy"),
        Script::Bengali => Some("bn"),
        Script::Tamil => Some("ta"),
        Script::Malayalam => Some("ml"),
        Script::Arabic => Some(arabic(sample)),
        Script::Devanagari | Script::Cyrillic | Script::Latin => None,
    };
    let scores = match script {
        Script::Devanagari => scores(sample, DEVANAGARI, count),
        Script::Cyrillic => scores(sample, CYRILLIC, count),
        Script::Latin => scores(sample, LATIN, count),
        _ => Vec::new(),
    };
    Some(Detection {
        script,
        count,
        letters,
        single,
        scores,
    })
}

/// The script a language is written in, for the languages guesses name.
fn script_of_language(tag: &str) -> Option<Script> {
    Some(match tag {
        "ru" | "uk" | "bg" | "sr" | "kk" | "be" | "mk" => Script::Cyrillic,
        "el" => Script::Greek,
        "ar" | "fa" | "ur" => Script::Arabic,
        "he" | "yi" => Script::Hebrew,
        "hi" | "ne" | "mr" => Script::Devanagari,
        "bn" => Script::Bengali,
        "ta" => Script::Tamil,
        "ml" => Script::Malayalam,
        "th" => Script::Thai,
        "ka" => Script::Georgian,
        "hy" => Script::Armenian,
        "ko" => Script::Hangul,
        "ja" => Script::Kana,
        "zh" => Script::Han,
        "" => return None,
        _ => Script::Latin,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Script {
    Latin,
    Cyrillic,
    Greek,
    Arabic,
    Hebrew,
    Devanagari,
    Bengali,
    Tamil,
    Malayalam,
    Thai,
    Georgian,
    Armenian,
    Hangul,
    Kana,
    Han,
}

impl Script {
    const COUNT: usize = 15;
    const ALL: [Script; Self::COUNT] = [
        Self::Latin,
        Self::Cyrillic,
        Self::Greek,
        Self::Arabic,
        Self::Hebrew,
        Self::Devanagari,
        Self::Bengali,
        Self::Tamil,
        Self::Malayalam,
        Self::Thai,
        Self::Georgian,
        Self::Armenian,
        Self::Hangul,
        Self::Kana,
        Self::Han,
    ];
}

fn script_of(c: char) -> Option<Script> {
    if !c.is_alphabetic() {
        return None;
    }
    Some(match c as u32 {
        0x0041..=0x024F | 0x1E00..=0x1EFF => Script::Latin,
        0x0370..=0x03FF | 0x1F00..=0x1FFF => Script::Greek,
        0x0400..=0x052F => Script::Cyrillic,
        0x0530..=0x058F => Script::Armenian,
        0x0590..=0x05FF => Script::Hebrew,
        0x0600..=0x06FF | 0x0750..=0x077F | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => Script::Arabic,
        0x0900..=0x097F => Script::Devanagari,
        0x0980..=0x09FF => Script::Bengali,
        0x0B80..=0x0BFF => Script::Tamil,
        0x0D00..=0x0D7F => Script::Malayalam,
        0x0E00..=0x0E7F => Script::Thai,
        0x10A0..=0x10FF => Script::Georgian,
        0x1100..=0x11FF | 0xAC00..=0xD7AF | 0x3130..=0x318F => Script::Hangul,
        0x3040..=0x30FF | 0x31F0..=0x31FF => Script::Kana,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF => Script::Han,
        _ => return None,
    })
}

/// Arabic letters: Persian and Urdu add letters of their own.
fn arabic(sample: &str) -> &'static str {
    let count = |letters: &str| sample.chars().filter(|c| letters.contains(*c)).count();
    let urdu = count("ٹڈڑںےۓ");
    let persian = count("پچژگ");
    if urdu > 2 {
        "ur"
    } else if persian > 2 {
        "fa"
    } else {
        "ar"
    }
}

/// A language's marks: its most common short words, and letters only it (of these) uses.
struct Marks {
    tag: &'static str,
    words: &'static [&'static str],
    letters: &'static str,
    /// Its letters are in nearly every sentence: a long text without any isn't in it.
    required: bool,
}

impl Marks {
    const fn new(tag: &'static str, words: &'static [&'static str], letters: &'static str) -> Self {
        Self {
            tag,
            words,
            letters,
            required: false,
        }
    }
}

const LATIN: &[Marks] = &[
    Marks::new(
        "en",
        &[
            "the", "and", "of", "to", "is", "was", "that", "he", "she", "it", "with", "you", "for",
            "not", "his", "her", "had", "have", "but", "they", "this", "from", "said", "what",
            "were", "been", "him", "would", "there", "which", "when", "are", "all", "my", "we",
        ],
        "",
    ),
    Marks {
        tag: "de",
        words: &[
            "der", "die", "und", "das", "ist", "nicht", "ein", "eine", "ich", "sie", "er", "mit",
            "den", "auch", "sich", "war",
        ],
        letters: "äöüß",
        required: false,
    },
    Marks {
        tag: "fr",
        words: &[
            "le", "la", "les", "et", "est", "un", "une", "des", "du", "je", "il", "elle", "pas",
            "que", "qui", "dans",
        ],
        letters: "œêûîç",
        required: false,
    },
    Marks {
        tag: "es",
        words: &[
            "el", "la", "los", "las", "y", "es", "un", "una", "que", "de", "en", "por", "con",
            "pero", "del", "se",
        ],
        letters: "ñ¿¡",
        required: false,
    },
    Marks {
        tag: "it",
        words: &[
            "il", "la", "e", "di", "che", "è", "un", "una", "non", "per", "sono", "gli", "della",
            "anche", "ma", "lo",
        ],
        letters: "",
        required: false,
    },
    Marks {
        tag: "pt",
        words: &[
            "o", "a", "os", "as", "e", "de", "que", "não", "um", "uma", "em", "com", "para", "ele",
            "ela", "do",
        ],
        letters: "ãõ",
        required: false,
    },
    Marks {
        tag: "nl",
        words: &[
            "de", "het", "een", "en", "van", "is", "niet", "ik", "je", "dat", "zijn", "op", "met",
            "maar", "ook", "hij",
        ],
        letters: "ĳ",
        required: false,
    },
    Marks {
        tag: "pl",
        words: &[
            "i", "w", "nie", "się", "na", "to", "że", "jest", "jak", "do", "ale", "z", "co", "tak",
            "jego", "był",
        ],
        letters: "ąęłśźżńć",
        required: true,
    },
    Marks {
        tag: "sv",
        words: &[
            "och", "att", "det", "är", "en", "jag", "inte", "på", "som", "med", "har", "för",
            "hon", "han", "var", "av",
        ],
        letters: "",
        required: false,
    },
    Marks {
        tag: "da",
        words: &[
            "og", "at", "det", "er", "en", "jeg", "ikke", "af", "på", "som", "med", "har", "hun",
            "han", "var", "hvad", "efter", "mig", "nu", "blev",
        ],
        letters: "",
        required: false,
    },
    Marks {
        tag: "nb",
        words: &[
            "og", "at", "det", "er", "en", "jeg", "ikke", "av", "på", "som", "med", "har", "hun",
            "han", "var", "hva", "etter", "meg", "nå", "ble",
        ],
        letters: "",
        required: false,
    },
    Marks {
        tag: "fi",
        words: &[
            "ja", "on", "ei", "että", "se", "hän", "oli", "mutta", "kun", "niin", "minä", "mitä",
            "joka", "sen", "kuin", "ole",
        ],
        letters: "",
        required: false,
    },
    Marks {
        tag: "cs",
        words: &[
            "a", "je", "se", "na", "že", "to", "v", "jsem", "ale", "jak", "tak", "by", "byl",
            "jsou", "co", "není",
        ],
        letters: "řůě",
        required: true,
    },
    Marks {
        tag: "sk",
        words: &[
            "a", "je", "sa", "na", "že", "to", "v", "som", "ale", "ako", "tak", "by", "bol", "sú",
            "čo", "nie",
        ],
        letters: "ľĺŕô",
        required: false,
    },
    Marks {
        tag: "sl",
        words: &[
            "in", "je", "se", "na", "da", "to", "v", "sem", "ki", "pa", "ne", "so", "bil", "za",
            "kot", "ali",
        ],
        letters: "",
        required: false,
    },
    Marks {
        tag: "hu",
        words: &[
            "a", "az", "és", "hogy", "nem", "is", "egy", "van", "meg", "de", "már", "volt", "csak",
            "még", "ez", "mint",
        ],
        letters: "őű",
        required: false,
    },
    Marks {
        tag: "ro",
        words: &[
            "și", "în", "de", "la", "nu", "că", "pe", "cu", "o", "un", "este", "mai", "dar", "ce",
            "sunt", "fost",
        ],
        letters: "șțăşţ",
        required: true,
    },
    Marks {
        tag: "tr",
        words: &[
            "ve", "bir", "bu", "da", "de", "ne", "için", "ile", "ama", "çok", "o", "gibi", "daha",
            "var", "değil", "ben",
        ],
        letters: "ğış",
        required: true,
    },
    Marks {
        tag: "ca",
        words: &[
            "el", "la", "els", "les", "i", "és", "un", "una", "que", "de", "per", "amb", "no",
            "però", "del", "va",
        ],
        letters: "ŀ",
        required: false,
    },
    Marks {
        tag: "vi",
        words: &[
            "và", "là", "của", "có", "không", "người", "một", "những", "được", "cho", "trong",
            "đã", "với", "này", "tôi", "anh",
        ],
        letters: "ạảấầẩậắằẳặẹẻẽếềểệỉịọỏốồổộớờởợụủứừửựỳỵỷỹđơư",
        required: true,
    },
    Marks {
        tag: "lv",
        words: &[
            "un", "ir", "ka", "ar", "uz", "no", "par", "kas", "bet", "tas", "to", "es", "viņš",
            "viņa", "bija", "arī",
        ],
        letters: "āēīūģķļņ",
        required: true,
    },
    Marks {
        tag: "id",
        words: &[
            "dan", "yang", "di", "itu", "dengan", "tidak", "ini", "dari", "untuk", "akan", "pada",
            "ada", "saya", "dia", "ke", "juga",
        ],
        letters: "",
        required: false,
    },
    Marks {
        tag: "sw",
        words: &[
            "na", "ya", "wa", "kwa", "ni", "za", "katika", "la", "kuwa", "hiyo", "huo", "lakini",
            "sana", "hii", "kama", "yake",
        ],
        letters: "",
        required: false,
    },
    Marks {
        tag: "cy",
        words: &[
            "y", "yn", "a", "ac", "i", "o", "ei", "mae", "roedd", "yr", "ar", "fy", "ond", "wedi",
            "gyda", "hi",
        ],
        letters: "ŵŷ",
        required: false,
    },
];

const CYRILLIC: &[Marks] = &[
    Marks {
        tag: "ru",
        words: &[
            "и", "в", "не", "на", "что", "он", "я", "с", "это", "как", "она", "но", "его", "был",
            "все", "так",
        ],
        letters: "ыэъё",
        required: false,
    },
    Marks {
        tag: "uk",
        words: &[
            "і", "в", "не", "на", "що", "він", "я", "з", "це", "як", "вона", "але", "його", "був",
            "та", "так",
        ],
        letters: "іїєґ",
        required: false,
    },
    Marks {
        tag: "bg",
        words: &[
            "и", "в", "не", "на", "че", "той", "аз", "с", "това", "как", "тя", "но", "да", "се",
            "е", "беше",
        ],
        letters: "",
        required: false,
    },
    Marks {
        tag: "sr",
        words: &[
            "и", "у", "не", "на", "да", "је", "се", "он", "она", "али", "то", "са", "био", "што",
            "као", "из",
        ],
        letters: "ђјљњћџ",
        required: false,
    },
    Marks {
        tag: "kk",
        words: &[
            "және",
            "бұл",
            "мен",
            "деп",
            "бір",
            "оның",
            "ол",
            "да",
            "де",
            "үшін",
            "болды",
            "еді",
            "сол",
            "қалай",
            "енді",
            "емес",
        ],
        letters: "әғқңөұүһ",
        required: false,
    },
    Marks {
        tag: "be",
        words: &[
            "і", "ў", "не", "на", "што", "ён", "я", "з", "гэта", "як", "яна", "але", "яго", "быў",
            "усе", "так",
        ],
        letters: "ўі",
        required: false,
    },
];

const DEVANAGARI: &[Marks] = &[
    Marks {
        tag: "hi",
        words: &[
            "है",
            "के",
            "में",
            "की",
            "और",
            "से",
            "को",
            "का",
            "यह",
            "नहीं",
            "हैं",
            "था",
            "पर",
            "भी",
            "वह",
            "तो",
        ],
        letters: "",
        required: false,
    },
    Marks {
        tag: "ne",
        words: &[
            "छ",
            "को",
            "मा",
            "र",
            "पनि",
            "यो",
            "हो",
            "थियो",
            "छन्",
            "गर्न",
            "भने",
            "लाई",
            "त्यो",
            "हुन्छ",
            "थिए",
            "नै",
        ],
        letters: "",
        required: false,
    },
    Marks {
        tag: "mr",
        words: &[
            "आहे",
            "आणि",
            "या",
            "हे",
            "ते",
            "की",
            "होते",
            "त्या",
            "मी",
            "तो",
            "नाही",
            "एक",
            "पण",
            "त्याने",
            "होता",
            "आहेत",
        ],
        letters: "ळ",
        required: false,
    },
];

/// The words of the sample, as written.
fn words(sample: &str) -> Vec<&str> {
    sample
        .split(|c: char| !(c.is_alphanumeric() || c == '\u{0901}' || c == '\u{094D}' || is_mark(c)))
        .filter(|word| !word.is_empty())
        .collect()
}

/// Combining marks (Devanagari's vowel signs and the like) belong to the word they're in.
fn is_mark(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x0900..=0x0903 | 0x093A..=0x094F | 0x0951..=0x0957 | 0x0962..=0x0963)
}

/// Each language's score in the sample (its common words and own letters), highest first. In
/// Latin script, one-letter words count for nobody (Polish "i", Spanish "y", English "I"…), but
/// a capital "I" alone is English's pronoun; and a language whose letters are in nearly every
/// sentence scores nothing in a long sample without them.
fn scores(sample: &str, languages: &[Marks], letters: usize) -> Vec<(&'static str, usize)> {
    let words = words(sample);
    let latin = std::ptr::eq(languages, LATIN);
    let lower: Vec<String> = words
        .iter()
        .filter(|word| !(latin && word.chars().count() == 1))
        .map(|word| word.to_lowercase())
        .collect();
    let pronouns = words.iter().filter(|word| **word == "I").count();
    let small = sample.to_lowercase();
    let mut scores: Vec<(&'static str, usize)> = languages
        .iter()
        .map(|marks| {
            let common = lower
                .iter()
                .filter(|word| marks.words.contains(&word.as_str()))
                .count();
            let own = small.chars().filter(|c| marks.letters.contains(*c)).count();
            if marks.required && own == 0 && letters >= 200 {
                return (marks.tag, 0);
            }
            let pronoun = if latin && marks.tag == "en" {
                pronouns
            } else {
                0
            };
            (marks.tag, common + own + pronoun)
        })
        .collect();
    scores.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    scores
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_languages_apart() {
        let cases = [
            (
                "It was the best of times, it was the worst of times, and he knew that she was not with him.",
                "en",
            ),
            (
                "Es war einmal ein König, der hatte eine Tochter, und die war schöner als die Sonne. Er wusste nicht, was er tun sollte.",
                "de",
            ),
            (
                "Il était une fois un roi qui avait une fille, et elle était plus belle que le soleil. Je ne sais pas ce qu'il fait dans la maison.",
                "fr",
            ),
            (
                "En un lugar de la Mancha, de cuyo nombre no quiero acordarme, no ha mucho tiempo que vivía un hidalgo de los de lanza en astillero.",
                "es",
            ),
            (
                "Nel mezzo del cammin di nostra vita mi ritrovai per una selva oscura, ché la diritta via era smarrita. Non è facile, ma è così.",
                "it",
            ),
            (
                "Era uma vez um rei que não tinha filhos, e a rainha estava muito triste com isso. Ele não sabia o que fazer para a ajudar.",
                "pt",
            ),
            (
                "Het was een koude dag en de wind blies over het land. Ik weet niet wat hij met de man van het dorp heeft gedaan.",
                "nl",
            ),
            (
                "Był sobie król, który miał córkę. Nie wiem, co się z nim stało, ale to jest bardzo dziwne i tak jak zawsze.",
                "pl",
            ),
            (
                "Он сказал, что не знает, как это было. Она была на улице, и всё было так тихо, но его не было.",
                "ru",
            ),
            (
                "Він сказав, що не знає, як це було. Вона була на вулиці, і все було так тихо, але його не було.",
                "uk",
            ),
            (
                "从前有一个国王，他有一个美丽的女儿。她每天都在花园里散步。",
                "zh",
            ),
            (
                "むかしむかし、ある村におじいさんとおばあさんが住んでいました。おじいさんは山へしばかりに行きました。",
                "ja",
            ),
        ];
        for (text, language) in cases {
            assert_eq!(guess(text), Some(language), "{text}");
        }
    }

    #[test]
    fn says_nothing_about_too_little_text() {
        assert_eq!(guess("OK"), None);
        assert_eq!(guess("1234 5678 !!"), None);
    }

    #[test]
    fn reads_english_dialogue_as_english() {
        // "I", "to" and "do" are Polish words too; quoted speech is full of them.
        let dialogue = "“I don't know,” I said. “I want to go to the shop.” “Do you?” \
                        “No, I do not. I do it so I can go.” I told her to do it anyway.";
        assert_eq!(guess(dialogue), Some("en"));
        let quoted = "“Is it you?” “It is I.” “Do you know who I am?” “I do.” “So do I.” \
                      “No!” “Go!” “Do it.” “I will.” “I know.” “I am.” “I go to it.”";
        assert_ne!(guess(quoted), Some("pl"));
    }

    #[test]
    fn finds_long_passages_in_another_language() {
        let french = "Il était une fois un roi qui avait une fille, et elle était plus belle que \
                      le soleil. Je ne sais pas ce qu'il fait dans la maison, mais il ne veut pas \
                      que nous le sachions, et la reine est très triste depuis des années.";
        assert_eq!(foreign_passage(french, "en"), Some("fr"));
        assert_eq!(foreign_passage(french, "fr-FR"), None);
        // A quotation, a name or a short line stays in the book's language.
        let quoting = "He looked at her and said that she was not with him, and the old man \
                       answered “je ne sais pas” as he always did, and they went home to have \
                       their supper with the children and the dog that had been waiting.";
        assert_eq!(foreign_passage(quoting, "en"), None);
        assert_eq!(foreign_passage("Je ne sais pas ce qu'il fait.", "en"), None);
        // Another script: a shorter passage is clear enough.
        let russian = "Я помню чудное мгновенье: передо мной явилась ты, как мимолётное виденье.";
        assert_eq!(foreign_passage(russian, "en"), Some("ru"));
        assert_eq!(
            foreign_passage("He said “да” and left the room with the others.", "en"),
            None
        );
        // Kanji in a Japanese book aren't Chinese.
        assert_eq!(
            foreign_passage("国王国王国王国王国王国王国王国王国王国王", "ja"),
            None
        );
    }

    #[test]
    fn tells_danish_from_norwegian() {
        assert_eq!(
            guess(
                "Jeg ved ikke hvad han har gjort efter det, men nu er det for sent for mig og hun blev hjemme."
            ),
            Some("da")
        );
        assert_eq!(
            guess(
                "Jeg vet ikke hva han har gjort etter det, men nå er det for sent for meg og hun ble hjemme."
            ),
            Some("nb")
        );
    }
}
