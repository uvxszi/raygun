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
