//! Time zones by the names people use: cities and countries (`Tokyo`, `New York`, `Japan`),
//! abbreviations (`PST`, `CET`), and offsets (`UTC+5:30`). The IANA database comes with
//! chrono-tz.

use chrono::{DateTime, FixedOffset, Offset, TimeZone, Utc};
use chrono_tz::Tz;

/// A zone found by name: a zone of the database or a fixed offset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Zone {
    Named(Tz),
    Fixed(FixedOffset),
}

impl Zone {
    /// The offset from UTC at `at`.
    pub fn offset_at(self, at: DateTime<Utc>) -> FixedOffset {
        match self {
            Self::Named(tz) => tz.offset_from_utc_datetime(&at.naive_utc()).fix(),
            Self::Fixed(offset) => offset,
        }
    }

    /// The zone's name: `Asia/Tokyo`, or `UTC+05:30`.
    pub fn name(self) -> String {
        match self {
            Self::Named(tz) => tz.name().to_owned(),
            Self::Fixed(offset) => offset_text(offset.local_minus_utc()),
        }
    }

    /// The abbreviation in use at `at` (`JST`), when the database gives one in letters.
    pub fn abbreviation(self, at: DateTime<Utc>) -> Option<String> {
        match self {
            Self::Named(tz) => {
                let shown = tz.offset_from_utc_datetime(&at.naive_utc()).to_string();
                shown
                    .chars()
                    .all(|char| char.is_ascii_alphabetic())
                    .then_some(shown)
            }
            Self::Fixed(_) => None,
        }
    }
}

/// `UTC+05:30`, `UTC−03:00`, `UTC`.
pub fn offset_text(seconds: i32) -> String {
    if seconds == 0 {
        return "UTC".to_owned();
    }
    let sign = if seconds < 0 { '−' } else { '+' };
    let minutes = seconds.unsigned_abs() / 60;
    format!("UTC{sign}{:02}:{:02}", minutes / 60, minutes % 60)
}

/// Abbreviations, with the zone they're said for (as people use them: `EST` for New York's
/// time all year).
const ABBREVIATIONS: &[(&str, &str)] = &[
    ("utc", "UTC"),
    ("gmt", "Etc/GMT"),
    ("z", "UTC"),
    ("zulu", "UTC"),
    ("est", "America/New_York"),
    ("edt", "America/New_York"),
    ("et", "America/New_York"),
    ("eastern", "America/New_York"),
    ("eastern time", "America/New_York"),
    ("cst", "America/Chicago"),
    ("cdt", "America/Chicago"),
    ("ct", "America/Chicago"),
    ("central", "America/Chicago"),
    ("central time", "America/Chicago"),
    ("mst", "America/Denver"),
    ("mdt", "America/Denver"),
    ("mt", "America/Denver"),
    ("mountain time", "America/Denver"),
    ("pst", "America/Los_Angeles"),
    ("pdt", "America/Los_Angeles"),
    ("pt", "America/Los_Angeles"),
    ("pacific", "America/Los_Angeles"),
    ("pacific time", "America/Los_Angeles"),
    ("akst", "America/Anchorage"),
    ("hst", "Pacific/Honolulu"),
    ("ast", "America/Halifax"),
    ("nst", "America/St_Johns"),
    ("bst", "Europe/London"),
    ("ist", "Asia/Kolkata"),
    ("wet", "Europe/Lisbon"),
    ("west", "Europe/Lisbon"),
    ("cet", "Europe/Paris"),
    ("cest", "Europe/Paris"),
    ("eet", "Europe/Athens"),
    ("eest", "Europe/Athens"),
    ("msk", "Europe/Moscow"),
    ("gst", "Asia/Dubai"),
    ("pkt", "Asia/Karachi"),
    ("ict", "Asia/Bangkok"),
    ("wib", "Asia/Jakarta"),
    ("sgt", "Asia/Singapore"),
    ("hkt", "Asia/Hong_Kong"),
    ("pht", "Asia/Manila"),
    ("jst", "Asia/Tokyo"),
    ("kst", "Asia/Seoul"),
    ("awst", "Australia/Perth"),
    ("acst", "Australia/Adelaide"),
    ("aest", "Australia/Sydney"),
    ("aedt", "Australia/Sydney"),
    ("nzst", "Pacific/Auckland"),
    ("nzdt", "Pacific/Auckland"),
    ("sast", "Africa/Johannesburg"),
    ("cat", "Africa/Maputo"),
    ("eat", "Africa/Nairobi"),
    ("wat", "Africa/Lagos"),
    ("brt", "America/Sao_Paulo"),
    ("art", "America/Argentina/Buenos_Aires"),
];

/// Places without a zone of their own name, and countries (by their capital's or largest
/// city's zone).
const PLACES: &[(&str, &str)] = &[
    ("san francisco", "America/Los_Angeles"),
    ("seattle", "America/Los_Angeles"),
    ("san diego", "America/Los_Angeles"),
    ("las vegas", "America/Los_Angeles"),
    ("portland", "America/Los_Angeles"),
    ("silicon valley", "America/Los_Angeles"),
    ("hollywood", "America/Los_Angeles"),
    ("california", "America/Los_Angeles"),
    ("washington dc", "America/New_York"),
    ("washington d.c", "America/New_York"),
    ("washington, d.c", "America/New_York"),
    ("boston", "America/New_York"),
    ("miami", "America/New_York"),
    ("atlanta", "America/New_York"),
    ("philadelphia", "America/New_York"),
    ("nyc", "America/New_York"),
    ("florida", "America/New_York"),
    ("dallas", "America/Chicago"),
    ("houston", "America/Chicago"),
    ("austin", "America/Chicago"),
    ("texas", "America/Chicago"),
    ("new orleans", "America/Chicago"),
    ("salt lake city", "America/Denver"),
    ("montreal", "America/Toronto"),
    ("ottawa", "America/Toronto"),
    ("quebec", "America/Toronto"),
    ("calgary", "America/Edmonton"),
    ("rio de janeiro", "America/Sao_Paulo"),
    ("rio", "America/Sao_Paulo"),
    ("brasilia", "America/Sao_Paulo"),
    ("buenos aires", "America/Argentina/Buenos_Aires"),
    ("beijing", "Asia/Shanghai"),
    ("shenzhen", "Asia/Shanghai"),
    ("guangzhou", "Asia/Shanghai"),
    ("mumbai", "Asia/Kolkata"),
    ("bombay", "Asia/Kolkata"),
    ("delhi", "Asia/Kolkata"),
    ("new delhi", "Asia/Kolkata"),
    ("bangalore", "Asia/Kolkata"),
    ("bengaluru", "Asia/Kolkata"),
    ("hyderabad", "Asia/Kolkata"),
    ("chennai", "Asia/Kolkata"),
    ("osaka", "Asia/Tokyo"),
    ("kyoto", "Asia/Tokyo"),
    ("busan", "Asia/Seoul"),
    ("hanoi", "Asia/Bangkok"),
    ("abu dhabi", "Asia/Dubai"),
    ("doha", "Asia/Qatar"),
    ("mecca", "Asia/Riyadh"),
    ("tel aviv", "Asia/Jerusalem"),
    ("lahore", "Asia/Karachi"),
    ("islamabad", "Asia/Karachi"),
    ("munich", "Europe/Berlin"),
    ("frankfurt", "Europe/Berlin"),
    ("hamburg", "Europe/Berlin"),
    ("cologne", "Europe/Berlin"),
    ("barcelona", "Europe/Madrid"),
    ("seville", "Europe/Madrid"),
    ("milan", "Europe/Rome"),
    ("venice", "Europe/Rome"),
    ("florence", "Europe/Rome"),
    ("naples", "Europe/Rome"),
    ("geneva", "Europe/Zurich"),
    ("lyon", "Europe/Paris"),
    ("marseille", "Europe/Paris"),
    ("edinburgh", "Europe/London"),
    ("manchester", "Europe/London"),
    ("glasgow", "Europe/London"),
    ("liverpool", "Europe/London"),
    ("saint petersburg", "Europe/Moscow"),
    ("st petersburg", "Europe/Moscow"),
    ("krakow", "Europe/Warsaw"),
    ("porto", "Europe/Lisbon"),
    ("rotterdam", "Europe/Amsterdam"),
    ("antwerp", "Europe/Brussels"),
    ("cape town", "Africa/Johannesburg"),
    ("melbourne", "Australia/Melbourne"),
    ("canberra", "Australia/Sydney"),
    ("wellington", "Pacific/Auckland"),
    ("hawaii", "Pacific/Honolulu"),
    ("alaska", "America/Anchorage"),
    ("japan", "Asia/Tokyo"),
    ("china", "Asia/Shanghai"),
    ("india", "Asia/Kolkata"),
    ("south korea", "Asia/Seoul"),
    ("korea", "Asia/Seoul"),
    ("north korea", "Asia/Pyongyang"),
    ("taiwan", "Asia/Taipei"),
    ("thailand", "Asia/Bangkok"),
    ("vietnam", "Asia/Ho_Chi_Minh"),
    ("philippines", "Asia/Manila"),
    ("indonesia", "Asia/Jakarta"),
    ("malaysia", "Asia/Kuala_Lumpur"),
    ("pakistan", "Asia/Karachi"),
    ("bangladesh", "Asia/Dhaka"),
    ("nepal", "Asia/Kathmandu"),
    ("sri lanka", "Asia/Colombo"),
    ("iran", "Asia/Tehran"),
    ("iraq", "Asia/Baghdad"),
    ("israel", "Asia/Jerusalem"),
    ("saudi arabia", "Asia/Riyadh"),
    ("uae", "Asia/Dubai"),
    ("united arab emirates", "Asia/Dubai"),
    ("qatar", "Asia/Qatar"),
    ("turkey", "Europe/Istanbul"),
    ("egypt", "Africa/Cairo"),
    ("nigeria", "Africa/Lagos"),
    ("kenya", "Africa/Nairobi"),
    ("ethiopia", "Africa/Addis_Ababa"),
    ("morocco", "Africa/Casablanca"),
    ("south africa", "Africa/Johannesburg"),
    ("uk", "Europe/London"),
    ("united kingdom", "Europe/London"),
    ("england", "Europe/London"),
    ("britain", "Europe/London"),
    ("great britain", "Europe/London"),
    ("scotland", "Europe/London"),
    ("wales", "Europe/London"),
    ("ireland", "Europe/Dublin"),
    ("france", "Europe/Paris"),
    ("germany", "Europe/Berlin"),
    ("spain", "Europe/Madrid"),
    ("portugal", "Europe/Lisbon"),
    ("italy", "Europe/Rome"),
    ("netherlands", "Europe/Amsterdam"),
    ("holland", "Europe/Amsterdam"),
    ("belgium", "Europe/Brussels"),
    ("switzerland", "Europe/Zurich"),
    ("austria", "Europe/Vienna"),
    ("poland", "Europe/Warsaw"),
    ("czechia", "Europe/Prague"),
    ("czech republic", "Europe/Prague"),
    ("hungary", "Europe/Budapest"),
    ("romania", "Europe/Bucharest"),
    ("bulgaria", "Europe/Sofia"),
    ("greece", "Europe/Athens"),
    ("sweden", "Europe/Stockholm"),
    ("norway", "Europe/Oslo"),
    ("denmark", "Europe/Copenhagen"),
    ("finland", "Europe/Helsinki"),
    ("iceland", "Atlantic/Reykjavik"),
    ("ukraine", "Europe/Kyiv"),
    ("russia", "Europe/Moscow"),
    ("usa", "America/New_York"),
    ("us", "America/New_York"),
    ("united states", "America/New_York"),
    ("america", "America/New_York"),
    ("canada", "America/Toronto"),
    ("mexico", "America/Mexico_City"),
    ("brazil", "America/Sao_Paulo"),
    ("argentina", "America/Argentina/Buenos_Aires"),
    ("chile", "America/Santiago"),
    ("colombia", "America/Bogota"),
    ("peru", "America/Lima"),
    ("venezuela", "America/Caracas"),
    ("australia", "Australia/Sydney"),
    ("new zealand", "Pacific/Auckland"),
];

/// `utc+5`, `gmt-3`, `utc +05:30`.
fn fixed_offset(name: &str) -> Option<FixedOffset> {
    let compact: String = name.chars().filter(|char| !char.is_whitespace()).collect();
    let rest = compact
        .strip_prefix("utc")
        .or_else(|| compact.strip_prefix("gmt"))?;
    if rest.is_empty() {
        return FixedOffset::east_opt(0);
    }
    let (sign, rest) = match rest.strip_prefix('+') {
        Some(rest) => (1, rest),
        None => (
            -1,
            rest.strip_prefix('-').or_else(|| rest.strip_prefix('−'))?,
        ),
    };
    let (hours, minutes) = match rest.split_once(':') {
        Some((hours, minutes)) => (hours.parse::<i32>().ok()?, minutes.parse::<i32>().ok()?),
        None if rest.len() == 4 => (rest[..2].parse().ok()?, rest[2..].parse().ok()?),
        None => (rest.parse().ok()?, 0),
    };
    if hours > 14 || minutes >= 60 {
        return None;
    }
    FixedOffset::east_opt(sign * (hours * 3600 + minutes * 60))
}

/// The place's name as a zone name's last part: `new york` → `New_York`.
fn zone_by_city(name: &str) -> Option<Tz> {
    let wanted = name.replace(' ', "_");
    chrono_tz::TZ_VARIANTS.iter().copied().find(|tz| {
        let full = tz.name();
        !full.starts_with("Etc/")
            && full.contains('/')
            && full
                .rsplit('/')
                .next()
                .is_some_and(|city| city.eq_ignore_ascii_case(&wanted))
    })
}

/// The zone `name` stands for, with the place as it's shown.
pub fn find(name: &str) -> Option<(Zone, String)> {
    let lower = name.trim().to_lowercase();
    let lower = lower.trim_end_matches('.');
    let (lower, _) = super::text::strip_suffix_words(lower, &["time zone", "timezone", "time"]);
    let lower = super::text::without_article(lower).trim();
    if lower.is_empty() || lower.len() > 60 {
        return None;
    }
    let shown = name.trim().to_owned();
    if let Some(offset) = fixed_offset(lower) {
        let zone = if offset.local_minus_utc() == 0 {
            Zone::Named(Tz::UTC)
        } else {
            Zone::Fixed(offset)
        };
        return Some((zone, offset_text(offset.local_minus_utc())));
    }
    for (abbreviation, zone) in ABBREVIATIONS {
        if *abbreviation == lower {
            let tz: Tz = zone.parse().ok()?;
            return Some((Zone::Named(tz), lower.to_uppercase()));
        }
    }
    for (place, zone) in PLACES {
        if *place == lower {
            let tz: Tz = zone.parse().ok()?;
            return Some((Zone::Named(tz), super::text::capitalized(lower)));
        }
    }
    if let Some(tz) = zone_by_city(lower) {
        return Some((Zone::Named(tz), shown));
    }
    // A full zone name: `Europe/Paris`.
    let tz: Tz = name.trim().parse().ok()?;
    Some((Zone::Named(tz), shown))
}

/// This device's zone: its IANA name, else its offset now.
pub fn device(name: Option<&str>, offset_minutes: i32) -> Zone {
    name.and_then(|name| name.parse::<Tz>().ok())
        .map(Zone::Named)
        .or_else(|| FixedOffset::east_opt(offset_minutes * 60).map(Zone::Fixed))
        .unwrap_or(Zone::Named(Tz::UTC))
}

/// The time at `at` in `zone`.
pub fn local(zone: Zone, at: DateTime<Utc>) -> DateTime<FixedOffset> {
    at.with_timezone(&zone.offset_at(at))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(name: &str) -> Option<String> {
        find(name).map(|(zone, _)| zone.name())
    }

    #[test]
    fn places_have_zones() {
        assert_eq!(zone("Tokyo").as_deref(), Some("Asia/Tokyo"));
        assert_eq!(zone("new york").as_deref(), Some("America/New_York"));
        assert_eq!(zone("Sao Paulo").as_deref(), Some("America/Sao_Paulo"));
        assert_eq!(
            zone("San Francisco").as_deref(),
            Some("America/Los_Angeles")
        );
        assert_eq!(zone("Japan").as_deref(), Some("Asia/Tokyo"));
        assert_eq!(zone("the UK").as_deref(), Some("Europe/London"));
        assert_eq!(zone("PST").as_deref(), Some("America/Los_Angeles"));
        assert_eq!(zone("london time").as_deref(), Some("Europe/London"));
        assert_eq!(zone("Europe/Paris").as_deref(), Some("Europe/Paris"));
        assert_eq!(zone("UTC+5:30").as_deref(), Some("UTC+05:30"));
        assert_eq!(zone("gmt-3").as_deref(), Some("UTC−03:00"));
        assert_eq!(zone("utc").as_deref(), Some("UTC"));
        assert_eq!(zone("Blue"), None);
        assert_eq!(zone("the moon"), None);
    }

    #[test]
    fn offsets_follow_daylight_saving() {
        let tokyo = find("tokyo").unwrap().0;
        let winter = Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
        let summer = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        assert_eq!(tokyo.offset_at(winter).local_minus_utc(), 9 * 3600);
        let york = find("new york").unwrap().0;
        assert_eq!(york.offset_at(winter).local_minus_utc(), -5 * 3600);
        assert_eq!(york.offset_at(summer).local_minus_utc(), -4 * 3600);
        assert_eq!(york.abbreviation(summer).as_deref(), Some("EDT"));
    }
}
