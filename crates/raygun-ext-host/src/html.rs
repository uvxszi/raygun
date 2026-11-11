//! `raygun.html.select`: CSS selection over HTML, backed by the `scraper` crate.

use std::collections::BTreeMap;

use scraper::{Html, Selector};
use serde::Serialize;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Element {
    /// All descendant text, concatenated.
    pub text: String,
    /// Outer HTML, so the element can be selected into again.
    pub html: String,
    pub inner: String,
    pub attrs: BTreeMap<String, String>,
}

/// Elements matching `selector`. `fragment` parses `html` as a snippet (an element selected
/// earlier) rather than a whole document.
pub fn select(html: &str, selector: &str, fragment: bool) -> Result<Vec<Element>, String> {
    check_selector(selector)?;
    let selector =
        Selector::parse(selector).map_err(|error| format!("invalid selector: {error}"))?;
    let document = if fragment {
        Html::parse_fragment(html)
    } else {
        Html::parse_document(html)
    };
    Ok(document
        .select(&selector)
        .map(|element| Element {
            text: element.text().collect(),
            html: element.html(),
            inner: element.inner_html(),
            attrs: element
                .value()
                .attrs()
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .collect(),
        })
        .collect())
}

/// The longest selector a script may ask for.
const MAX_SELECTOR_LEN: usize = 4096;
/// How deep a selector's parentheses may nest (`:not(:has(:is(…)))`).
const MAX_SELECTOR_NESTING: usize = 16;

/// Refuses selectors no page needs whose parsing or matching recurses with their size: their
/// parsers and matchers go one call deeper per nested group or compound, and a deep enough one
/// would overflow the stack, which ends the whole app rather than the call.
pub(crate) fn check_selector(selector: &str) -> Result<(), String> {
    if selector.len() > MAX_SELECTOR_LEN {
        return Err(format!(
            "invalid selector: longer than {MAX_SELECTOR_LEN} characters"
        ));
    }
    let mut depth = 0usize;
    for character in selector.chars() {
        match character {
            '(' | '[' => {
                depth += 1;
                if depth > MAX_SELECTOR_NESTING {
                    return Err(format!(
                        "invalid selector: nested deeper than {MAX_SELECTOR_NESTING} levels"
                    ));
                }
            }
            ')' | ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_nested_without_end_are_refused_not_followed() {
        let deep = format!("{}a{}", ":not(".repeat(10_000), ")".repeat(10_000));
        assert!(select("<a></a>", &deep, true).is_err());
        let long = "a + ".repeat(5_000) + "a";
        assert!(select("<a></a>", &long, true).is_err());
        assert_eq!(
            select("<p><a>x</a></p>", "p > a:not(.b)", true)
                .unwrap()
                .len(),
            1
        );
    }
}
