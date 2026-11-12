//! Units: the names people type made into fend's (`fluid ounces` → `floz`, `degrees Celsius` →
//! `°C`), units written as answers show them (`km / h` → `km/h`, `m^2` → `m²`), and the units a
//! result can be toggled to, by what it measures.

/// Which system a unit belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum System {
    Metric,
    Imperial,
    Neither,
}

impl System {
    pub fn name(self) -> &'static str {
        match self {
            Self::Metric => "metric",
            Self::Imperial => "imperial",
            Self::Neither => "",
        }
    }
}

/// A unit as fend reads it, as answers show it, and its system.
pub type Unit = (&'static str, &'static str, System);

use System::{Imperial as I, Metric as M, Neither as N};

/// Units of one quantity, in the order the toggle shows them. The first unit converts anything
/// of that quantity, which is how a result's quantity is found.
pub struct Quantity {
    pub units: &'static [Unit],
}

pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        units: &[
            ("m", "m", M),
            ("mm", "mm", M),
            ("cm", "cm", M),
            ("km", "km", M),
            ("inch", "in", I),
            ("ft", "ft", I),
            ("yd", "yd", I),
            ("mi", "mi", I),
            ("nmi", "nmi", N),
        ],
    },
    Quantity {
        units: &[
            ("kg", "kg", M),
            ("mg", "mg", M),
            ("g", "g", M),
            ("tonne", "t", M),
            ("oz", "oz", I),
            ("lb", "lb", I),
            ("stone", "st", I),
            ("short_ton", "US ton", I),
        ],
    },
    Quantity {
        units: &[("K", "K", M), ("°C", "°C", M), ("°F", "°F", I)],
    },
    Quantity {
        units: &[
            ("l", "L", M),
            ("ml", "mL", M),
            ("m^3", "m³", M),
            ("tsp", "tsp", I),
            ("tbsp", "tbsp", I),
            ("floz", "fl oz", I),
            ("cup", "cup", I),
            ("pint", "pt", I),
            ("quart", "qt", I),
            ("gallon", "gal", I),
        ],
    },
    Quantity {
        units: &[
            ("m^2", "m²", M),
            ("cm^2", "cm²", M),
            ("hectare", "ha", M),
            ("km^2", "km²", M),
            ("ft^2", "ft²", I),
            ("yd^2", "yd²", I),
            ("acre", "acre", I),
            ("mi^2", "mi²", I),
        ],
    },
    Quantity {
        units: &[
            ("m/s", "m/s", M),
            ("km/h", "km/h", M),
            ("mph", "mph", I),
            ("ft/s", "ft/s", I),
            ("knot", "kn", N),
        ],
    },
    Quantity {
        units: &[
            ("s", "s", N),
            ("ms", "ms", N),
            ("min", "min", N),
            ("hour", "h", N),
            ("day", "days", N),
            ("week", "weeks", N),
            ("month", "months", N),
            ("year", "years", N),
        ],
    },
    Quantity {
        units: &[
            ("bytes", "B", N),
            ("bits", "bit", N),
            ("kB", "kB", N),
            ("MB", "MB", N),
            ("GB", "GB", N),
            ("TB", "TB", N),
            ("KiB", "KiB", N),
            ("MiB", "MiB", N),
            ("GiB", "GiB", N),
            ("TiB", "TiB", N),
        ],
    },
    Quantity {
        units: &[
            ("J", "J", M),
            ("kJ", "kJ", M),
            ("cal", "cal", N),
            ("kcal", "kcal", N),
            ("Wh", "Wh", M),
            ("kWh", "kWh", M),
            ("eV", "eV", N),
            ("btu", "BTU", I),
        ],
    },
    Quantity {
        units: &[
            ("W", "W", M),
            ("kW", "kW", M),
            ("MW", "MW", M),
            ("hp", "hp", I),
        ],
    },
    Quantity {
        units: &[
            ("Pa", "Pa", M),
            ("kPa", "kPa", M),
            ("bar", "bar", M),
            ("atm", "atm", N),
            ("psi", "psi", I),
            ("mmHg", "mmHg", N),
        ],
    },
    Quantity {
        units: &[
            ("rad", "rad", N),
            ("deg", "°", N),
            ("turn", "turns", N),
            ("gradian", "grad", N),
        ],
    },
    Quantity {
        units: &[
            ("Hz", "Hz", N),
            ("kHz", "kHz", N),
            ("MHz", "MHz", N),
            ("GHz", "GHz", N),
            ("rpm", "rpm", N),
        ],
    },
    Quantity {
        units: &[("N", "N", M), ("kN", "kN", M), ("lbf", "lbf", I)],
    },
];

/// Units written as more than one word, or as fend doesn't read them, with fend's name.
const PHRASES: &[(&[&str], &str)] = &[
    (&["fluid", "ounces"], "floz"),
    (&["fluid", "ounce"], "floz"),
    (&["fl", "oz"], "floz"),
    (&["fl.", "oz."], "floz"),
    (&["nautical", "miles"], "nmi"),
    (&["nautical", "mile"], "nmi"),
    (&["light", "years"], "light_years"),
    (&["light", "year"], "light_years"),
    (&["light-years"], "light_years"),
    (&["light-year"], "light_years"),
    (&["metric", "tons"], "tonnes"),
    (&["metric", "ton"], "tonnes"),
    (&["metric", "tonnes"], "tonnes"),
    (&["long", "tons"], "(2240 lb)"),
    (&["long", "ton"], "(2240 lb)"),
    (&["short", "tons"], "short_tons"),
    (&["short", "ton"], "short_tons"),
    (&["tons"], "short_tons"),
    (&["ton"], "short_tons"),
    (&["degrees", "celsius"], "°C"),
    (&["degree", "celsius"], "°C"),
    (&["deg", "c"], "°C"),
    (&["degrees", "c"], "°C"),
    (&["degree", "c"], "°C"),
    (&["degrees", "centigrade"], "°C"),
    (&["centigrade"], "°C"),
    (&["celsius"], "°C"),
    (&["°c"], "°C"),
    (&["degrees", "fahrenheit"], "°F"),
    (&["degree", "fahrenheit"], "°F"),
    (&["deg", "f"], "°F"),
    (&["degrees", "f"], "°F"),
    (&["degree", "f"], "°F"),
    (&["fahrenheit"], "°F"),
    (&["°f"], "°F"),
    (&["degrees", "kelvin"], "K"),
    (&["kelvins"], "K"),
    (&["kelvin"], "K"),
    (&["miles", "per", "hour"], "mph"),
    (&["mile", "per", "hour"], "mph"),
    (&["kilometers", "per", "hour"], "km/h"),
    (&["kilometres", "per", "hour"], "km/h"),
    (&["kilometer", "per", "hour"], "km/h"),
    (&["kilometre", "per", "hour"], "km/h"),
    (&["km", "per", "hour"], "km/h"),
    (&["kmph"], "km/h"),
    (&["meters", "per", "second"], "m/s"),
    (&["metres", "per", "second"], "m/s"),
    (&["meter", "per", "second"], "m/s"),
    (&["metre", "per", "second"], "m/s"),
    (&["feet", "per", "second"], "ft/s"),
    (&["kilowatt", "hours"], "kWh"),
    (&["kilowatt", "hour"], "kWh"),
    (&["kilowatt-hours"], "kWh"),
    (&["kilowatt-hour"], "kWh"),
    (&["watt", "hours"], "Wh"),
    (&["horsepower"], "hp"),
    (&["kilocalories"], "kcal"),
    (&["kilocalorie"], "kcal"),
    (&["megabits"], "Mb"),
    (&["megabit"], "Mb"),
    (&["mbit"], "Mb"),
    (&["mbits"], "Mb"),
    (&["gigabits"], "Gb"),
    (&["gigabit"], "Gb"),
    (&["gbit"], "Gb"),
    (&["kilobits"], "kb"),
    (&["kbit"], "kb"),
    (&["mbps"], "Mbps"),
    (&["gbps"], "Gbps"),
    (&["yrs"], "years"),
    (&["sq", "km"], "km^2"),
    (&["sq", "m"], "m^2"),
    (&["sq", "mi"], "mi^2"),
    (&["sq", "ft"], "ft^2"),
    (&["sq", "in"], "inch^2"),
    (&["sq", "yd"], "yd^2"),
];

/// Lengths that `square`, `cubic`, `squared` and `cubed` can go with, as fend names them.
const LENGTHS: &[(&str, &str)] = &[
    ("millimeters", "mm"),
    ("millimetres", "mm"),
    ("millimeter", "mm"),
    ("millimetre", "mm"),
    ("mm", "mm"),
    ("centimeters", "cm"),
    ("centimetres", "cm"),
    ("centimeter", "cm"),
    ("centimetre", "cm"),
    ("cm", "cm"),
    ("meters", "m"),
    ("metres", "m"),
    ("meter", "m"),
    ("metre", "m"),
    ("m", "m"),
    ("kilometers", "km"),
    ("kilometres", "km"),
    ("kilometer", "km"),
    ("kilometre", "km"),
    ("km", "km"),
    ("inches", "inch"),
    ("inch", "inch"),
    ("in", "inch"),
    ("feet", "ft"),
    ("foot", "ft"),
    ("ft", "ft"),
    ("yards", "yd"),
    ("yard", "yd"),
    ("yd", "yd"),
    ("miles", "mi"),
    ("mile", "mi"),
    ("mi", "mi"),
];

fn length(word: &str) -> Option<&'static str> {
    LENGTHS
        .iter()
        .find(|(name, _)| *name == word)
        .map(|(_, fend)| *fend)
}

/// Words that convert, as fend reads them.
pub const CONVERTS: &[&str] = &["to", "in", "into", "as"];

/// `text` with units as fend names them. `in` after a number and before a conversion word is
/// inches (`5 in to cm`), and a lone `c` or `f` is a temperature when the other side is one.
pub fn for_fend(text: &str) -> String {
    let words: Vec<&str> = text.split(' ').filter(|word| !word.is_empty()).collect();
    let lower: Vec<String> = words.iter().map(|word| word.to_lowercase()).collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut at = 0;
    'words: while at < words.len() {
        // `square feet`, `cubic meters`, `meters squared`.
        if matches!(lower[at].as_str(), "square" | "sq" | "cubic" | "cu")
            && let Some(unit) = lower.get(at + 1).and_then(|word| length(word))
        {
            let power = if lower[at].starts_with("sq") { 2 } else { 3 };
            out.push(format!("{unit}^{power}"));
            at += 2;
            continue;
        }
        if let Some(unit) = length(&lower[at])
            && let Some(power) = lower.get(at + 1).and_then(|word| match word.as_str() {
                "squared" => Some(2),
                "cubed" => Some(3),
                _ => None,
            })
        {
            out.push(format!("{unit}^{power}"));
            at += 2;
            continue;
        }
        for (phrase, fend) in PHRASES {
            let end = at + phrase.len();
            if end <= lower.len()
                && lower[at..end]
                    .iter()
                    .zip(phrase.iter())
                    .all(|(a, b)| a == b)
            {
                out.push((*fend).to_owned());
                at = end;
                continue 'words;
            }
        }
        // `5 in to cm`: inches, not a conversion.
        if lower[at] == "in"
            && at > 0
            && out.last().is_some_and(|last| last.parse::<f64>().is_ok())
            && lower
                .get(at + 1)
                .is_some_and(|next| CONVERTS.contains(&next.as_str()))
        {
            out.push("inch".to_owned());
            at += 1;
            continue;
        }
        out.push(if lower[at] == "into" {
            "to".to_owned()
        } else {
            words[at].to_owned()
        });
        at += 1;
    }
    let joined = out.join(" ");
    temperatures_named(&joined)
}

/// A lone `c` or `f` in a conversion with a temperature on its other side means degrees.
fn temperatures_named(text: &str) -> String {
    let words: Vec<&str> = text.split(' ').collect();
    let temperature = |word: &str| matches!(word, "°C" | "°F" | "K");
    let any = words.iter().any(|word| temperature(word));
    let lone = words
        .iter()
        .any(|word| matches!(*word, "c" | "f" | "C" | "F"));
    if !lone {
        return text.to_owned();
    }
    let converts = words.iter().any(|word| CONVERTS.contains(word));
    if !(any || converts) {
        return text.to_owned();
    }
    words
        .iter()
        .enumerate()
        .map(|(at, word)| {
            let after_number = at > 0 && words[at - 1].parse::<f64>().is_ok();
            let after_conversion = at > 0 && CONVERTS.contains(&words[at - 1]);
            match *word {
                "c" | "C" if after_number || after_conversion => "°C",
                "f" | "F" if after_number || after_conversion => "°F",
                other => other,
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A unit as fend writes it, as answers show it: `km / h` → `km/h`, `m^2` → `m²`,
/// `light_years` → `light years`.
pub fn pretty(unit: &str) -> String {
    let mut out = unit.trim().replace(" / ", "/").replace('_', " ");
    for (power, sign) in [
        ("^-1", "⁻¹"),
        ("^-2", "⁻²"),
        ("^-3", "⁻³"),
        ("^2", "²"),
        ("^3", "³"),
        ("^4", "⁴"),
    ] {
        out = out.replace(power, sign);
    }
    out
}

/// A unit as TeX, upright: `km/h` → `\mathrm{km/h}`.
pub fn tex(unit: &str) -> String {
    let unit = pretty(unit);
    if unit.is_empty() {
        return String::new();
    }
    let mut tex = String::new();
    for char in unit.chars() {
        match char {
            '²' => tex.push_str("^{2}"),
            '³' => tex.push_str("^{3}"),
            '⁴' => tex.push_str("^{4}"),
            '⁻' => tex.push_str("^{-"),
            '¹' => tex.push_str("1}"),
            '°' => tex.push_str("{}^{\\circ}"),
            '%' => tex.push_str("\\%"),
            ' ' => tex.push_str("\\ "),
            '$' | '#' | '&' | '_' | '{' | '}' => {}
            other => tex.push(other),
        }
    }
    // `⁻²` became `^{-` followed by `^{2}`: `^{-2}`.
    let tex = tex.replace("^{-^{2}", "^{-2}").replace("^{-^{3}", "^{-3}");
    format!("\\mathrm{{{tex}}}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_are_named_as_fend_reads_them() {
        assert_eq!(for_fend("5 fl oz to ml"), "5 floz to ml");
        assert_eq!(for_fend("1 nautical mile to km"), "1 nmi to km");
        assert_eq!(
            for_fend("100 degrees Fahrenheit in Celsius"),
            "100 °F in °C"
        );
        assert_eq!(for_fend("20 c to f"), "20 °C to °F");
        assert_eq!(for_fend("3 square feet to square meters"), "3 ft^2 to m^2");
        assert_eq!(
            for_fend("2 meters squared in feet squared"),
            "2 m^2 in ft^2"
        );
        assert_eq!(for_fend("60 miles per hour into km/h"), "60 mph to km/h");
        assert_eq!(for_fend("5 in to cm"), "5 inch to cm");
        assert_eq!(for_fend("1 ton in kg"), "1 short_tons in kg");
        assert_eq!(for_fend("100 Mbps to MB/s"), "100 Mbps to MB/s");
        assert_eq!(for_fend("2 cups"), "2 cups", "nothing to change");
        assert_eq!(
            for_fend("50 Cent"),
            "50 Cent",
            "a lone c needs a temperature beside it"
        );
    }

    #[test]
    fn units_are_written_as_answers_show_them() {
        assert_eq!(pretty("km / h"), "km/h");
        assert_eq!(pretty("m^2"), "m²");
        assert_eq!(pretty("m / s^2"), "m/s²");
        assert_eq!(pretty("mol^-1"), "mol⁻¹");
        assert_eq!(pretty("light_years"), "light years");
        assert_eq!(tex("km / h"), "\\mathrm{km/h}");
        assert_eq!(tex("m^2"), "\\mathrm{m^{2}}");
        assert_eq!(tex("mol^-1"), "\\mathrm{mol^{-1}}");
        assert_eq!(tex("°C"), "\\mathrm{{}^{\\circ}C}");
    }
}
