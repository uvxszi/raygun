//! Years as Raygun shows them.

/// A span of years as it's shown: a year with nothing after its dash (a show still running, an
/// artist still active: "2024–", "1980-", "1980 - ") is the year alone, and anything else
/// ("2019–2023", "1999", "S1 E2 · Pilot") stays as it is, trimmed. Sources write the open dash
/// (Stremio addons for running shows, music databases for active artists).
pub fn year_range_label(text: &str) -> String {
    let text = text.trim();
    let open = text.trim_end_matches(|character: char| {
        character.is_whitespace() || matches!(character, '-' | '–' | '—')
    });
    if open.len() < text.len() && open.len() == 4 && open.bytes().all(|byte| byte.is_ascii_digit())
    {
        open.to_owned()
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::year_range_label;

    #[test]
    fn a_span_still_open_shows_its_first_year_alone() {
        for open in [
            "2024-",
            "2024–",
            "2024 - ",
            " 2024 – ",
            "2024—",
            "1980–",
            "1980 -",
        ] {
            assert_eq!(year_range_label(open), &open.trim()[..4], "{open:?}");
        }
    }

    #[test]
    fn closed_spans_and_other_text_stay_as_they_are() {
        assert_eq!(year_range_label("1980–2003"), "1980–2003");
        assert_eq!(year_range_label("2019–2023"), "2019–2023");
        assert_eq!(year_range_label("1998-1999"), "1998-1999");
        assert_eq!(year_range_label(" 1999 "), "1999");
        // Only a year loses its dash: other lines (an episode's) stay as they are.
        assert_eq!(year_range_label("S1 E2 · Pilot"), "S1 E2 · Pilot");
        assert_eq!(year_range_label("Part 2 -"), "Part 2 -");
        assert_eq!(year_range_label("19801–"), "19801–");
        assert_eq!(year_range_label("–2003"), "–2003");
        assert_eq!(year_range_label(""), "");
    }
}
