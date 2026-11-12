//! Named constants of mathematics and physics, with their values (CODATA 2018, as NIST lists
//! them; the SI's defining constants are exact).

use raygun_model::{AnswerCredit, AnswerKind, AnswerRow, AnswerText};

use super::{Found, blank, math_text};

pub struct Constant {
    pub names: &'static [&'static str],
    /// Shown as the interpretation, with its symbol.
    pub name: &'static str,
    pub symbol_tex: &'static str,
    pub value: &'static str,
    pub value_tex: &'static str,
    /// The value is exact by definition, or a mathematical constant.
    pub exact: bool,
    /// Where the value comes from.
    pub physical: bool,
}

pub const CONSTANTS: &[Constant] = &[
    Constant {
        names: &["pi", "π", "the number pi", "value of pi"],
        name: "Pi",
        symbol_tex: "\\pi",
        value: "3.14159265358979323846264338327950288…",
        value_tex: "3.14159265358979323846\\ldots",
        exact: false,
        physical: false,
    },
    Constant {
        names: &[
            "euler's number",
            "eulers number",
            "euler number",
            "napier's constant",
            "the number e",
        ],
        name: "Euler's number",
        symbol_tex: "e",
        value: "2.71828182845904523536028747135266249…",
        value_tex: "2.71828182845904523536\\ldots",
        exact: false,
        physical: false,
    },
    Constant {
        names: &["golden ratio", "the golden ratio", "golden mean", "phi"],
        name: "Golden ratio",
        symbol_tex: "\\varphi = \\frac{1 + \\sqrt{5}}{2}",
        value: "1.61803398874989484820458683436563811…",
        value_tex: "1.61803398874989484820\\ldots",
        exact: false,
        physical: false,
    },
    Constant {
        names: &["tau", "the number tau"],
        name: "Tau",
        symbol_tex: "\\tau = 2\\pi",
        value: "6.28318530717958647692528676655900576…",
        value_tex: "6.28318530717958647692\\ldots",
        exact: false,
        physical: false,
    },
    Constant {
        names: &[
            "euler-mascheroni constant",
            "euler mascheroni constant",
            "euler's constant",
        ],
        name: "Euler–Mascheroni constant",
        symbol_tex: "\\gamma",
        value: "0.57721566490153286060651209008240243…",
        value_tex: "0.57721566490153286060\\ldots",
        exact: false,
        physical: false,
    },
    Constant {
        names: &[
            "speed of light",
            "the speed of light",
            "speed of light in vacuum",
            "speed of light in a vacuum",
            "light speed",
        ],
        name: "Speed of light in vacuum",
        symbol_tex: "c",
        value: "299,792,458 m/s",
        value_tex: "299{,}792{,}458\\ \\mathrm{m/s}",
        exact: true,
        physical: true,
    },
    Constant {
        names: &[
            "gravitational constant",
            "newtonian constant of gravitation",
            "newton's gravitational constant",
            "big g",
        ],
        name: "Newtonian constant of gravitation",
        symbol_tex: "G",
        value: "6.67430 × 10⁻¹¹ m³ kg⁻¹ s⁻²",
        value_tex: "6.67430 \\times 10^{-11}\\ \\mathrm{m^{3}\\,kg^{-1}\\,s^{-2}}",
        exact: false,
        physical: true,
    },
    Constant {
        names: &["planck constant", "planck's constant", "plancks constant"],
        name: "Planck constant",
        symbol_tex: "h",
        value: "6.62607015 × 10⁻³⁴ J·s",
        value_tex: "6.62607015 \\times 10^{-34}\\ \\mathrm{J\\,s}",
        exact: true,
        physical: true,
    },
    Constant {
        names: &["reduced planck constant", "dirac constant", "h bar", "hbar"],
        name: "Reduced Planck constant",
        symbol_tex: "\\hbar = \\frac{h}{2\\pi}",
        value: "1.054571817… × 10⁻³⁴ J·s",
        value_tex: "1.054571817\\ldots \\times 10^{-34}\\ \\mathrm{J\\,s}",
        exact: true,
        physical: true,
    },
    Constant {
        names: &[
            "avogadro constant",
            "avogadro's constant",
            "avogadro number",
            "avogadro's number",
            "avogadros number",
        ],
        name: "Avogadro constant",
        symbol_tex: "N_{A}",
        value: "6.02214076 × 10²³ mol⁻¹",
        value_tex: "6.02214076 \\times 10^{23}\\ \\mathrm{mol^{-1}}",
        exact: true,
        physical: true,
    },
    Constant {
        names: &["boltzmann constant", "boltzmann's constant"],
        name: "Boltzmann constant",
        symbol_tex: "k_{B}",
        value: "1.380649 × 10⁻²³ J/K",
        value_tex: "1.380649 \\times 10^{-23}\\ \\mathrm{J/K}",
        exact: true,
        physical: true,
    },
    Constant {
        names: &[
            "elementary charge",
            "charge of an electron",
            "electron charge",
            "charge of electron",
        ],
        name: "Elementary charge",
        symbol_tex: "e",
        value: "1.602176634 × 10⁻¹⁹ C",
        value_tex: "1.602176634 \\times 10^{-19}\\ \\mathrm{C}",
        exact: true,
        physical: true,
    },
    Constant {
        names: &[
            "electron mass",
            "mass of an electron",
            "mass of electron",
            "electron rest mass",
        ],
        name: "Electron mass",
        symbol_tex: "m_{e}",
        value: "9.1093837015 × 10⁻³¹ kg",
        value_tex: "9.1093837015 \\times 10^{-31}\\ \\mathrm{kg}",
        exact: false,
        physical: true,
    },
    Constant {
        names: &["proton mass", "mass of a proton", "mass of proton"],
        name: "Proton mass",
        symbol_tex: "m_{p}",
        value: "1.67262192369 × 10⁻²⁷ kg",
        value_tex: "1.67262192369 \\times 10^{-27}\\ \\mathrm{kg}",
        exact: false,
        physical: true,
    },
    Constant {
        names: &["neutron mass", "mass of a neutron", "mass of neutron"],
        name: "Neutron mass",
        symbol_tex: "m_{n}",
        value: "1.67492749804 × 10⁻²⁷ kg",
        value_tex: "1.67492749804 \\times 10^{-27}\\ \\mathrm{kg}",
        exact: false,
        physical: true,
    },
    Constant {
        names: &[
            "gas constant",
            "molar gas constant",
            "ideal gas constant",
            "universal gas constant",
        ],
        name: "Molar gas constant",
        symbol_tex: "R",
        value: "8.314462618… J mol⁻¹ K⁻¹",
        value_tex: "8.314462618\\ldots\\ \\mathrm{J\\,mol^{-1}\\,K^{-1}}",
        exact: true,
        physical: true,
    },
    Constant {
        names: &[
            "standard gravity",
            "acceleration due to gravity",
            "acceleration of gravity",
            "gravitational acceleration",
            "standard acceleration of gravity",
            "g force",
            "earth gravity",
        ],
        name: "Standard acceleration of gravity",
        symbol_tex: "g_{n}",
        value: "9.80665 m/s²",
        value_tex: "9.80665\\ \\mathrm{m/s^{2}}",
        exact: true,
        physical: true,
    },
    Constant {
        names: &[
            "vacuum permittivity",
            "permittivity of free space",
            "electric constant",
        ],
        name: "Vacuum electric permittivity",
        symbol_tex: "\\varepsilon_{0}",
        value: "8.8541878128 × 10⁻¹² F/m",
        value_tex: "8.8541878128 \\times 10^{-12}\\ \\mathrm{F/m}",
        exact: false,
        physical: true,
    },
    Constant {
        names: &[
            "vacuum permeability",
            "permeability of free space",
            "magnetic constant",
        ],
        name: "Vacuum magnetic permeability",
        symbol_tex: "\\mu_{0}",
        value: "1.25663706212 × 10⁻⁶ N/A²",
        value_tex: "1.25663706212 \\times 10^{-6}\\ \\mathrm{N/A^{2}}",
        exact: false,
        physical: true,
    },
    Constant {
        names: &["fine-structure constant", "fine structure constant"],
        name: "Fine-structure constant",
        symbol_tex: "\\alpha",
        value: "7.2973525693 × 10⁻³ ≈ 1/137.035999084",
        value_tex: "7.2973525693 \\times 10^{-3} \\approx \\frac{1}{137.035999084}",
        exact: false,
        physical: true,
    },
    Constant {
        names: &["stefan-boltzmann constant", "stefan boltzmann constant"],
        name: "Stefan–Boltzmann constant",
        symbol_tex: "\\sigma",
        value: "5.670374419… × 10⁻⁸ W m⁻² K⁻⁴",
        value_tex: "5.670374419\\ldots \\times 10^{-8}\\ \\mathrm{W\\,m^{-2}\\,K^{-4}}",
        exact: true,
        physical: true,
    },
    Constant {
        names: &["faraday constant", "faraday's constant"],
        name: "Faraday constant",
        symbol_tex: "F",
        value: "96,485.33212… C/mol",
        value_tex: "96{,}485.33212\\ldots\\ \\mathrm{C/mol}",
        exact: true,
        physical: true,
    },
    Constant {
        names: &["rydberg constant"],
        name: "Rydberg constant",
        symbol_tex: "R_{\\infty}",
        value: "10,973,731.568160 m⁻¹",
        value_tex: "10{,}973{,}731.568160\\ \\mathrm{m^{-1}}",
        exact: false,
        physical: true,
    },
    Constant {
        names: &["absolute zero"],
        name: "Absolute zero",
        symbol_tex: "0\\ \\mathrm{K}",
        value: "−273.15 °C = −459.67 °F",
        value_tex: "-273.15\\ {}^{\\circ}\\mathrm{C} = -459.67\\ {}^{\\circ}\\mathrm{F}",
        exact: true,
        physical: true,
    },
    Constant {
        names: &["astronomical unit"],
        name: "Astronomical unit",
        symbol_tex: "\\mathrm{au}",
        value: "149,597,870,700 m",
        value_tex: "149{,}597{,}870{,}700\\ \\mathrm{m}",
        exact: true,
        physical: true,
    },
    Constant {
        names: &["light year", "light-year", "a light year", "one light year"],
        name: "Light-year",
        symbol_tex: "\\mathrm{ly}",
        value: "9,460,730,472,580,800 m",
        value_tex: "9{,}460{,}730{,}472{,}580{,}800\\ \\mathrm{m}",
        exact: true,
        physical: true,
    },
];

/// Words that come before a constant's name: `what is the speed of light`, `value of pi`.
const LEAD_INS: &[&str] = &[
    "what is the value of",
    "what's the value of",
    "the value of",
    "value of",
    "what is the",
    "what's the",
    "whats the",
    "what is",
    "what's",
    "whats",
];

/// The constant `lower` names, when it names one.
pub fn find(lower: &str) -> Option<&'static Constant> {
    let (rest, _) = super::text::strip_prefix_words(lower, LEAD_INS);
    let rest = rest.trim();
    CONSTANTS
        .iter()
        .find(|constant| constant.names.contains(&rest))
}

/// A constant asked for by name. A one-word name (`pi`, `tau`) is also a title, so only a name
/// of more words, or one asked about (`what is pi`), counts as a question.
pub fn answer(lower: &str) -> Option<Found> {
    let constant = find(lower)?;
    let asked = super::text::strip_prefix_words(lower, LEAD_INS).1;
    let strong = asked || lower.trim().contains(' ');
    let mut found = blank(AnswerKind::Constant);
    found.interpretation = Some(math_text(
        format!("{} ({})", constant.name, plain_symbol(constant.symbol_tex)),
        &format!(
            "\\text{{{}}}\\ {}",
            constant.name.replace('–', "-"),
            constant.symbol_tex
        ),
    ));
    found.result = math_text(constant.value.to_owned(), constant.value_tex);
    found.approximate = !constant.exact;
    found.copy = constant.value.replace(['…', ','], "");
    if constant.physical {
        found.credits.push(AnswerCredit {
            name: "CODATA 2018 (NIST)".to_owned(),
            url: Some("https://physics.nist.gov/cuu/Constants/".to_owned()),
            license: None,
            license_url: None,
        });
        found.rows.push(AnswerRow {
            key: if constant.exact {
                "exactByDefinition"
            } else {
                "measured"
            }
            .to_owned(),
            label: if constant.exact {
                "Exact by definition"
            } else {
                "Measured value"
            }
            .to_owned(),
            value: AnswerText::plain(if constant.exact {
                "SI 2019"
            } else {
                "CODATA 2018"
            }),
        });
    }
    Some(Found {
        answer: found,
        strong,
    })
}

fn plain_symbol(tex: &str) -> String {
    let first = tex.split(" = ").next().unwrap_or(tex);
    first
        .replace("\\pi", "π")
        .replace("\\varphi", "φ")
        .replace("\\tau", "τ")
        .replace("\\gamma", "γ")
        .replace("\\hbar", "ħ")
        .replace("\\varepsilon", "ε")
        .replace("\\mu", "μ")
        .replace("\\alpha", "α")
        .replace("\\sigma", "σ")
        .replace("\\infty", "∞")
        .replace("\\mathrm", "")
        .replace("\\ ", " ")
        .replace(['{', '}', '_'], "")
}
