//! Which language a book is written in, read from its text, so a voice that speaks it reads it:
//! the script first, then (for the Latin, Cyrillic, Arabic and Devanagari scripts) the letters
//! and the most common short words each language has.

/// How much of a text is looked at.
const SAMPLE_CHARS: usize = 4_000;

/// The language `text` is most likely in, as a BCP 47 primary tag (`en`, `de`, `zh`), or nothing
/// when it's too short or unclear to tell.
pub fn guess(text: &str) -> Option<&'static str> {
    let sample: String = text.chars().take(SAMPLE_CHARS).collect();
    let mut scripts = [0usize; Script::COUNT];
    for c in sample.chars() {
        if let Some(script) = script_of(c) {
            scripts[script as usize] += 1;
        }
    }
    let (script, count) = scripts
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| **count)
        .map(|(index, count)| (Script::ALL[index], *count))?;
    if count < 8 {
        return None;
    }
    let words = words(&sample);
    match script {
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
        Script::Arabic => Some(arabic(&sample)),
        Script::Devanagari => best(&words, &sample, DEVANAGARI),
        Script::Cyrillic => best(&words, &sample, CYRILLIC),
        Script::Latin => best(&words, &sample, LATIN),
    }
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
}

const LATIN: &[Marks] = &[
    Marks {
        tag: "en",
        words: &[
            "the", "and", "of", "to", "is", "was", "that", "he", "she", "it", "with", "you", "for",
            "not", "his", "her",
        ],
        letters: "",
    },
    Marks {
        tag: "de",
        words: &[
            "der", "die", "und", "das", "ist", "nicht", "ein", "eine", "ich", "sie", "er", "mit",
            "den", "auch", "sich", "war",
        ],
        letters: "äöüß",
    },
    Marks {
        tag: "fr",
        words: &[
            "le", "la", "les", "et", "est", "un", "une", "des", "du", "je", "il", "elle", "pas",
            "que", "qui", "dans",
        ],
        letters: "œêûîç",
    },
    Marks {
        tag: "es",
        words: &[
            "el", "la", "los", "las", "y", "es", "un", "una", "que", "de", "en", "por", "con",
            "pero", "del", "se",
        ],
        letters: "ñ¿¡",
    },
    Marks {
        tag: "it",
        words: &[
            "il", "la", "e", "di", "che", "è", "un", "una", "non", "per", "sono", "gli", "della",
            "anche", "ma", "lo",
        ],
        letters: "",
    },
    Marks {
        tag: "pt",
        words: &[
            "o", "a", "os", "as", "e", "de", "que", "não", "um", "uma", "em", "com", "para", "ele",
            "ela", "do",
        ],
        letters: "ãõ",
    },
    Marks {
        tag: "nl",
        words: &[
            "de", "het", "een", "en", "van", "is", "niet", "ik", "je", "dat", "zijn", "op", "met",
            "maar", "ook", "hij",
        ],
        letters: "ĳ",
    },
    Marks {
        tag: "pl",
        words: &[
            "i", "w", "nie", "się", "na", "to", "że", "jest", "jak", "do", "ale", "z", "co", "tak",
            "jego", "był",
        ],
        letters: "ąęłśźżń",
    },
    Marks {
        tag: "sv",
        words: &[
            "och", "att", "det", "är", "en", "jag", "inte", "på", "som", "med", "har", "för",
            "hon", "han", "var", "av",
        ],
        letters: "",
    },
    Marks {
        tag: "da",
        words: &[
            "og", "at", "det", "er", "en", "jeg", "ikke", "af", "på", "som", "med", "har", "hun",
            "han", "var", "hvad", "efter", "mig", "nu", "blev",
        ],
        letters: "",
    },
    Marks {
        tag: "nb",
        words: &[
            "og", "at", "det", "er", "en", "jeg", "ikke", "av", "på", "som", "med", "har", "hun",
            "han", "var", "hva", "etter", "meg", "nå", "ble",
        ],
        letters: "",
    },
    Marks {
        tag: "fi",
        words: &[
            "ja", "on", "ei", "että", "se", "hän", "oli", "mutta", "kun", "niin", "minä", "mitä",
            "joka", "sen", "kuin", "ole",
        ],
        letters: "",
    },
    Marks {
        tag: "cs",
        words: &[
            "a", "je", "se", "na", "že", "to", "v", "jsem", "ale", "jak", "tak", "by", "byl",
            "jsou", "co", "není",
        ],
        letters: "řů",
    },
    Marks {
        tag: "sk",
        words: &[
            "a", "je", "sa", "na", "že", "to", "v", "som", "ale", "ako", "tak", "by", "bol", "sú",
            "čo", "nie",
        ],
        letters: "ľĺŕô",
    },
    Marks {
        tag: "sl",
        words: &[
            "in", "je", "se", "na", "da", "to", "v", "sem", "ki", "pa", "ne", "so", "bil", "za",
            "kot", "ali",
        ],
        letters: "",
    },
    Marks {
        tag: "hu",
        words: &[
            "a", "az", "és", "hogy", "nem", "is", "egy", "van", "meg", "de", "már", "volt", "csak",
            "még", "ez", "mint",
        ],
        letters: "őű",
    },
    Marks {
        tag: "ro",
        words: &[
            "și", "în", "de", "la", "nu", "că", "pe", "cu", "o", "un", "este", "mai", "dar", "ce",
            "sunt", "fost",
        ],
        letters: "șță",
    },
    Marks {
        tag: "tr",
        words: &[
            "ve", "bir", "bu", "da", "de", "ne", "için", "ile", "ama", "çok", "o", "gibi", "daha",
            "var", "değil", "ben",
        ],
        letters: "ğış",
    },
    Marks {
        tag: "ca",
        words: &[
            "el", "la", "els", "les", "i", "és", "un", "una", "que", "de", "per", "amb", "no",
            "però", "del", "va",
        ],
        letters: "ŀ",
    },
    Marks {
        tag: "vi",
        words: &[
            "và", "là", "của", "có", "không", "người", "một", "những", "được", "cho", "trong",
            "đã", "với", "này", "tôi", "anh",
        ],
        letters: "ạảấầẩậắằẳặẹẻẽếềểệỉịọỏốồổộớờởợụủứừửựỳỵỷỹđơư",
    },
    Marks {
        tag: "lv",
        words: &[
            "un", "ir", "ka", "ar", "uz", "no", "par", "kas", "bet", "tas", "to", "es", "viņš",
            "viņa", "bija", "arī",
        ],
        letters: "āēīūģķļņ",
    },
    Marks {
        tag: "id",
        words: &[
            "dan", "yang", "di", "itu", "dengan", "tidak", "ini", "dari", "untuk", "akan", "pada",
            "ada", "saya", "dia", "ke", "juga",
        ],
        letters: "",
    },
    Marks {
        tag: "sw",
        words: &[
            "na", "ya", "wa", "kwa", "ni", "za", "katika", "la", "kuwa", "hiyo", "huo", "lakini",
            "sana", "hii", "kama", "yake",
        ],
        letters: "",
    },
    Marks {
        tag: "cy",
        words: &[
            "y", "yn", "a", "ac", "i", "o", "ei", "mae", "roedd", "yr", "ar", "fy", "ond", "wedi",
            "gyda", "hi",
        ],
        letters: "ŵŷ",
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
    },
    Marks {
        tag: "uk",
        words: &[
            "і", "в", "не", "на", "що", "він", "я", "з", "це", "як", "вона", "але", "його", "був",
            "та", "так",
        ],
        letters: "іїєґ",
    },
    Marks {
        tag: "bg",
        words: &[
            "и", "в", "не", "на", "че", "той", "аз", "с", "това", "как", "тя", "но", "да", "се",
            "е", "беше",
        ],
        letters: "",
    },
    Marks {
        tag: "sr",
        words: &[
            "и", "у", "не", "на", "да", "је", "се", "он", "она", "али", "то", "са", "био", "што",
            "као", "из",
        ],
        letters: "ђјљњћџ",
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
    },
    Marks {
        tag: "be",
        words: &[
            "і", "ў", "не", "на", "што", "ён", "я", "з", "гэта", "як", "яна", "але", "яго", "быў",
            "усе", "так",
        ],
        letters: "ўі",
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
    },
];

/// Lower-case words of the sample.
fn words(sample: &str) -> Vec<String> {
    sample
        .split(|c: char| !(c.is_alphanumeric() || c == '\u{0901}' || c == '\u{094D}' || is_mark(c)))
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Combining marks (Devanagari's vowel signs and the like) belong to the word they're in.
fn is_mark(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x0900..=0x0903 | 0x093A..=0x094F | 0x0951..=0x0957 | 0x0962..=0x0963)
}

/// The language whose words and letters the sample has most of, when it stands out.
fn best(words: &[String], sample: &str, languages: &[Marks]) -> Option<&'static str> {
    let lower = sample.to_lowercase();
    let mut scores: Vec<(&'static str, usize)> = languages
        .iter()
        .map(|marks| {
            let common = words
                .iter()
                .filter(|word| marks.words.contains(&word.as_str()))
                .count();
            let letters = lower.chars().filter(|c| marks.letters.contains(*c)).count();
            (marks.tag, common + letters)
        })
        .collect();
    scores.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    let (tag, top) = scores[0];
    let second = scores.get(1).map_or(0, |(_, score)| *score);
    (top >= 3 && top * 10 >= second * 11).then_some(tag)
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
