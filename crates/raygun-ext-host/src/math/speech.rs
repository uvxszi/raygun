//! A formula as text: a linear form for copying, searching and plain-text exports (`x² + 1`,
//! `(a+b)/c`), and words for reading aloud ("x squared plus 1", "a plus b, over c"). Both are read
//! from the rebuilt MathML; letters set in the mathematical alphabets (𝑥, 𝐀) read as plain ones.

use super::MathNode;

/// The formula in one line of text.
pub(super) fn linear(node: &MathNode) -> String {
    let text = linear_of(node);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The formula in words.
pub(super) fn words(node: &MathNode) -> String {
    let mut out = Vec::new();
    speak(node, &mut out);
    let text = out.join(" ");
    let text = text.replace(" ,", ",");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn linear_of(node: &MathNode) -> String {
    let MathNode::Element { name, .. } = node else {
        return plain(&node.text());
    };
    let parts: Vec<&MathNode> = node.elements().collect();
    let part = |index: usize| {
        parts
            .get(index)
            .map(|child| linear_of(child))
            .unwrap_or_default()
    };
    match *name {
        "mi" | "mn" | "mtext" => plain(&node.text()),
        "ms" => format!("\"{}\"", plain(&node.text())),
        "mo" => {
            let operator = plain(&node.text());
            let operator = operator.trim();
            if is_spaced_operator(operator) {
                format!(" {operator} ")
            } else if operator == "," || operator == ";" {
                format!("{operator} ")
            } else {
                operator.to_owned()
            }
        }
        "mspace" => " ".to_owned(),
        "mphantom" | "annotation" | "none" | "mprescripts" => String::new(),
        "semantics" => part(0),
        "mfrac" => format!("{}/{}", grouped(&part(0)), grouped(&part(1))),
        "msqrt" => format!("√{}", grouped(&joined(&parts))),
        "mroot" => {
            let index = part(1);
            match index.as_str() {
                "3" => format!("∛{}", grouped(&part(0))),
                "4" => format!("∜{}", grouped(&part(0))),
                _ => format!("root({index}, {})", part(0)),
            }
        }
        "msup" => format!("{}{}", part(0), raised(&part(1))),
        "msub" => format!("{}{}", part(0), lowered(&part(1))),
        "msubsup" => format!("{}{}{}", part(0), lowered(&part(1)), raised(&part(2))),
        "munder" => format!("{}{}", part(0), lowered(&part(1))),
        "mover" => {
            let over = part(1);
            if accent_word(over.trim()).is_some() {
                format!("{}{}", part(0), over.trim())
            } else {
                format!("{}{}", part(0), raised(&over))
            }
        }
        "munderover" => format!("{}{}{}", part(0), lowered(&part(1)), raised(&part(2))),
        "mtable" => {
            let rows: Vec<String> = parts
                .iter()
                .map(|row| {
                    row.elements()
                        .map(linear_of)
                        .map(|cell| cell.trim().to_owned())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .collect();
            format!("[{}]", rows.join("; "))
        }
        _ => joined(&parts),
    }
}

fn joined(parts: &[&MathNode]) -> String {
    parts.iter().map(|child| linear_of(child)).collect()
}

/// A part that's more than one symbol, in brackets.
fn grouped(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= 1
        || text
            .chars()
            .all(|char| char.is_alphanumeric() || char == '.')
        || (text.starts_with('(') && text.ends_with(')'))
    {
        text.to_owned()
    } else {
        format!("({text})")
    }
}

const SUPERSCRIPTS: [(char, char); 15] = [
    ('0', '⁰'),
    ('1', '¹'),
    ('2', '²'),
    ('3', '³'),
    ('4', '⁴'),
    ('5', '⁵'),
    ('6', '⁶'),
    ('7', '⁷'),
    ('8', '⁸'),
    ('9', '⁹'),
    ('+', '⁺'),
    ('-', '⁻'),
    ('−', '⁻'),
    ('n', 'ⁿ'),
    ('i', 'ⁱ'),
];
const SUBSCRIPTS: [(char, char); 13] = [
    ('0', '₀'),
    ('1', '₁'),
    ('2', '₂'),
    ('3', '₃'),
    ('4', '₄'),
    ('5', '₅'),
    ('6', '₆'),
    ('7', '₇'),
    ('8', '₈'),
    ('9', '₉'),
    ('+', '₊'),
    ('-', '₋'),
    ('−', '₋'),
];

fn mapped(text: &str, table: &[(char, char)]) -> Option<String> {
    text.chars()
        .map(|char| {
            table
                .iter()
                .find(|(from, _)| *from == char)
                .map(|(_, to)| *to)
        })
        .collect()
}

/// Text in superscript characters, when there's one for each.
pub(crate) fn superscript(text: &str) -> Option<String> {
    (!text.is_empty())
        .then(|| mapped(text, &SUPERSCRIPTS))
        .flatten()
}

/// Text in subscript characters, when there's one for each.
pub(crate) fn subscript(text: &str) -> Option<String> {
    (!text.is_empty())
        .then(|| mapped(text, &SUBSCRIPTS))
        .flatten()
}

/// A superscript: in raised characters when it has them all, else after `^`.
pub(crate) fn raised(text: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }
    if text.chars().all(|char| "′″‴'".contains(char)) {
        return text.to_owned();
    }
    mapped(text, &SUPERSCRIPTS).unwrap_or_else(|| format!("^{}", grouped(text)))
}

/// A subscript: in lowered characters when it has them all, else after `_`.
pub(crate) fn lowered(text: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }
    mapped(text, &SUBSCRIPTS).unwrap_or_else(|| format!("_{}", grouped(text)))
}

fn is_spaced_operator(operator: &str) -> bool {
    matches!(
        operator,
        "=" | "+"
            | "-"
            | "−"
            | "×"
            | "÷"
            | "<"
            | ">"
            | "≤"
            | "≥"
            | "≠"
            | "≈"
            | "±"
            | "∓"
            | "→"
            | "←"
            | "⇒"
            | "⇔"
            | "↦"
            | "∈"
            | "∉"
            | "⊂"
            | "⊆"
            | "∪"
            | "∩"
            | "≡"
            | "∝"
            | "∧"
            | "∨"
    )
}

/// Letters from the mathematical alphabets (𝑥, 𝐀, 𝔸, 𝟏) as plain ones.
pub(crate) fn plain(text: &str) -> String {
    text.chars().map(plain_char).collect()
}

fn plain_char(char: char) -> char {
    const GREEK: [char; 58] = [
        'Α', 'Β', 'Γ', 'Δ', 'Ε', 'Ζ', 'Η', 'Θ', 'Ι', 'Κ', 'Λ', 'Μ', 'Ν', 'Ξ', 'Ο', 'Π', 'Ρ', 'ϴ',
        'Σ', 'Τ', 'Υ', 'Φ', 'Χ', 'Ψ', 'Ω', '∇', 'α', 'β', 'γ', 'δ', 'ε', 'ζ', 'η', 'θ', 'ι', 'κ',
        'λ', 'μ', 'ν', 'ξ', 'ο', 'π', 'ρ', 'ς', 'σ', 'τ', 'υ', 'φ', 'χ', 'ψ', 'ω', '∂', 'ϵ', 'ϑ',
        'ϰ', 'ϕ', 'ϱ', 'ϖ',
    ];
    let code = u32::from(char);
    match code {
        0x1D400..=0x1D6A3 => {
            let offset = (code - 0x1D400) % 52;
            let base = if offset < 26 { 'A' } else { 'a' };
            char::from_u32(u32::from(base) + offset % 26).unwrap_or(char)
        }
        0x1D6A4 => 'ı',
        0x1D6A5 => 'ȷ',
        0x1D6A8..=0x1D7C9 => GREEK[((code - 0x1D6A8) % 58) as usize],
        0x1D7CE..=0x1D7FF => char::from_u32(u32::from('0') + (code - 0x1D7CE) % 10).unwrap_or(char),
        0x210E => 'h',
        _ => char,
    }
}

fn speak(node: &MathNode, out: &mut Vec<String>) {
    let MathNode::Element { name, .. } = node else {
        out.push(spoken_text(&node.text()));
        return;
    };
    let parts: Vec<&MathNode> = node.elements().collect();
    let said = |node: &MathNode| words(node);
    let simple = |node: &MathNode| node.elements().count() <= 1 && node.text().chars().count() <= 3;
    match *name {
        "mi" | "mtext" | "ms" => out.push(spoken_text(&node.text())),
        "mn" => out.push(plain(&node.text())),
        "mo" => {
            let operator = plain(&node.text());
            if let Some(word) = operator_word(operator.trim()) {
                out.push(word.to_owned());
            } else if !operator.trim().is_empty() && !is_bracket(operator.trim()) {
                out.push(spoken_text(&operator));
            }
        }
        "mphantom" | "annotation" | "none" | "mprescripts" | "mspace" => {}
        "semantics" => {
            if let Some(first) = parts.first() {
                speak(first, out);
            }
        }
        "mfrac" => {
            let (top, bottom) = (parts.first(), parts.get(1));
            if let (Some(top), Some(bottom)) = (top, bottom) {
                if simple(top) && simple(bottom) {
                    out.push(format!("{} over {}", said(top), said(bottom)));
                } else {
                    out.push(format!(
                        "the fraction {}, over {}, end fraction",
                        said(top),
                        said(bottom)
                    ));
                }
            }
        }
        "msqrt" => {
            let inside: Vec<String> = parts.iter().map(|part| said(part)).collect();
            out.push(format!("the square root of {}", inside.join(" ")));
            if !parts.iter().all(|part| simple(part)) || parts.len() > 1 {
                out.push(", end root".to_owned());
            }
        }
        "mroot" => {
            if let (Some(base), Some(index)) = (parts.first(), parts.get(1)) {
                let index = said(index);
                let which = match index.as_str() {
                    "2" => "square".to_owned(),
                    "3" => "cube".to_owned(),
                    other => ordinal(other),
                };
                out.push(format!("the {which} root of {}", said(base)));
            }
        }
        "msup" => {
            if let (Some(base), Some(power)) = (parts.first(), parts.get(1)) {
                out.push(said(base));
                out.push(power_words(power));
            }
        }
        "msub" | "munder" => {
            if let (Some(base), Some(under)) = (parts.first(), parts.get(1)) {
                let base_text = plain(&base.text());
                if let Some(big) = large_operator(base_text.trim()) {
                    out.push(format!("{big} over {}", said(under)));
                } else if base_text.trim() == "lim" {
                    out.push(format!("the limit as {}", said(under)));
                } else if *name == "munder" {
                    out.push(said(base));
                    out.push(said(under));
                } else {
                    out.push(format!("{} sub {}", said(base), said(under)));
                }
            }
        }
        "msubsup" | "munderover" => {
            if let (Some(base), Some(under), Some(over)) =
                (parts.first(), parts.get(1), parts.get(2))
            {
                let base_text = plain(&base.text());
                if let Some(big) = large_operator(base_text.trim()) {
                    out.push(format!("{big} from {} to {}, of", said(under), said(over)));
                } else {
                    out.push(format!("{} sub {}", said(base), said(under)));
                    out.push(power_words(over));
                }
            }
        }
        "mover" => {
            if let (Some(base), Some(over)) = (parts.first(), parts.get(1)) {
                out.push(said(base));
                match accent_word(plain(&over.text()).trim()) {
                    Some(accent) => out.push(accent.to_owned()),
                    None => out.push(power_words(over)),
                }
            }
        }
        "mtable" => {
            let rows: Vec<String> = parts
                .iter()
                .map(|row| row.elements().map(words).collect::<Vec<_>>().join(", "))
                .collect();
            out.push(rows.join("; "));
        }
        _ => {
            for part in parts {
                speak(part, out);
            }
        }
    }
}

/// An exponent in words: squared, cubed, prime, or "to the power of …".
fn power_words(power: &MathNode) -> String {
    let text = plain(&power.text());
    match text.trim() {
        "2" => "squared".to_owned(),
        "3" => "cubed".to_owned(),
        "′" | "'" => "prime".to_owned(),
        "″" | "''" => "double prime".to_owned(),
        "T" => "transpose".to_owned(),
        "−1" | "-1" => "inverse".to_owned(),
        _ => {
            let said = words(power);
            if power.elements().count() <= 1 {
                format!("to the power of {said}")
            } else {
                format!("to the power of {said}, end power")
            }
        }
    }
}

fn ordinal(number: &str) -> String {
    match number {
        "1" => "first".to_owned(),
        "2" => "second".to_owned(),
        "3" => "third".to_owned(),
        other if other.ends_with('1') && !other.ends_with("11") => format!("{other}st"),
        other if other.ends_with('2') && !other.ends_with("12") => format!("{other}nd"),
        other if other.ends_with('3') && !other.ends_with("13") => format!("{other}rd"),
        other => format!("{other}th"),
    }
}

fn is_bracket(operator: &str) -> bool {
    matches!(
        operator,
        "(" | ")"
            | "["
            | "]"
            | "{"
            | "}"
            | "⟨"
            | "⟩"
            | "⌊"
            | "⌋"
            | "⌈"
            | "⌉"
            | "|"
            | "‖"
            | "\u{2061}"
            | "\u{2062}"
            | "\u{2063}"
    )
}

fn large_operator(symbol: &str) -> Option<&'static str> {
    Some(match symbol {
        "∑" => "the sum",
        "∏" => "the product",
        "∫" => "the integral",
        "∬" => "the double integral",
        "∭" => "the triple integral",
        "∮" => "the contour integral",
        "⋃" => "the union",
        "⋂" => "the intersection",
        _ => return None,
    })
}

fn accent_word(accent: &str) -> Option<&'static str> {
    Some(match accent {
        "^" | "ˆ" | "\u{302}" => "hat",
        "¯" | "‾" | "ˉ" | "_" | "\u{304}" | "\u{305}" => "bar",
        "~" | "˜" | "\u{303}" => "tilde",
        "˙" | "." | "\u{307}" => "dot",
        "¨" | "\u{308}" => "double dot",
        "→" | "\u{20D7}" => "vector",
        _ => return None,
    })
}

fn operator_word(operator: &str) -> Option<&'static str> {
    Some(match operator {
        "=" => "equals",
        "+" => "plus",
        "-" | "−" => "minus",
        "×" | "·" | "⋅" | "*" | "∗" => "times",
        "÷" | "/" | "∕" => "divided by",
        "<" => "is less than",
        ">" => "is greater than",
        "≤" | "⩽" => "is less than or equal to",
        "≥" | "⩾" => "is greater than or equal to",
        "≠" => "is not equal to",
        "≈" => "is approximately equal to",
        "≡" => "is equivalent to",
        "∝" => "is proportional to",
        "∼" => "is similar to",
        "±" => "plus or minus",
        "∓" => "minus or plus",
        "→" => "tends to",
        "↦" => "maps to",
        "⇒" => "implies",
        "⇔" => "if and only if",
        "∞" => "infinity",
        "∂" => "partial",
        "∇" => "del",
        "∈" => "in",
        "∉" => "not in",
        "⊂" => "subset of",
        "⊆" => "subset of or equal to",
        "∪" => "union",
        "∩" => "intersection",
        "∀" => "for all",
        "∃" => "there exists",
        "¬" => "not",
        "∧" => "and",
        "∨" => "or",
        "′" => "prime",
        "!" => "factorial",
        "%" => "percent",
        "°" => "degrees",
        "…" | "⋯" | "⋮" | "⋱" => "dot dot dot",
        "∘" => "composed with",
        "⊥" => "perpendicular to",
        "∥" => "parallel to",
        "∠" => "angle",
        "√" => "the square root of",
        "," => ",",
        _ => large_operator(operator)?,
    })
}

/// Letters and symbols as words: Greek letters by name.
fn spoken_text(text: &str) -> String {
    let text = plain(text);
    let mut out = Vec::new();
    for char in text.chars() {
        match greek_name(char) {
            Some(name) => out.push(name.to_owned()),
            None => match out.last_mut() {
                Some(last) if !last.contains(' ') && greek_from(last).is_none() => last.push(char),
                _ => out.push(char.to_string()),
            },
        }
    }
    out.join(" ")
}

fn greek_from(word: &str) -> Option<char> {
    GREEK_NAMES
        .iter()
        .find(|(_, name)| *name == word)
        .map(|(char, _)| *char)
}

const GREEK_NAMES: [(char, &str); 40] = [
    ('α', "alpha"),
    ('β', "beta"),
    ('γ', "gamma"),
    ('δ', "delta"),
    ('ε', "epsilon"),
    ('ϵ', "epsilon"),
    ('ζ', "zeta"),
    ('η', "eta"),
    ('θ', "theta"),
    ('ϑ', "theta"),
    ('ι', "iota"),
    ('κ', "kappa"),
    ('λ', "lambda"),
    ('μ', "mu"),
    ('ν', "nu"),
    ('ξ', "xi"),
    ('π', "pi"),
    ('ρ', "rho"),
    ('σ', "sigma"),
    ('ς', "sigma"),
    ('τ', "tau"),
    ('υ', "upsilon"),
    ('φ', "phi"),
    ('ϕ', "phi"),
    ('χ', "chi"),
    ('ψ', "psi"),
    ('ω', "omega"),
    ('Γ', "capital gamma"),
    ('Δ', "capital delta"),
    ('Θ', "capital theta"),
    ('Λ', "capital lambda"),
    ('Ξ', "capital xi"),
    ('Π', "capital pi"),
    ('Σ', "capital sigma"),
    ('Υ', "capital upsilon"),
    ('Φ', "capital phi"),
    ('Ψ', "capital psi"),
    ('Ω', "capital omega"),
    ('ℏ', "h bar"),
    ('∅', "the empty set"),
];

fn greek_name(char: char) -> Option<&'static str> {
    GREEK_NAMES
        .iter()
        .find(|(letter, _)| *letter == char)
        .map(|(_, name)| *name)
}
