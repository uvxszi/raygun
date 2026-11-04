//! Miru's HTML helpers (miru-app's `extension_service.dart`, AGPL-3.0): every call parses the
//! HTML it's given as a whole document, as Dart's `html` package does, and answers with text:
//! the first match's text, inner or outer HTML, an attribute, every match's outer HTML, or the
//! document without some elements. Selectors run on Raygun's Jsoup-compatible engine
//! (`shosetsu::select`), a superset of what Dart's `html` package understands. One difference is
//! on purpose: Dart matches `:nth-child(n)` against a node index that counts text nodes, while
//! the scripts using it mean the CSS meaning, which this engine follows.
//!
//! Scripts ask many questions of one page (a title, a cover, a description, the episodes), so
//! recently parsed documents are kept for the next question.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use ego_tree::NodeRef;
use scraper::{ElementRef, Html, Node};

use crate::shosetsu::select::Selector;

/// How many parsed documents are kept, and the largest kept.
const CACHED_DOCUMENTS: usize = 4;
const MAX_CACHED_BYTES: usize = 2 * 1024 * 1024;

/// What a query answers with (`fun` in Miru's `querySelector` message).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Text,
    Inner,
    Outer,
}

impl Part {
    /// Miru's names; anything else is the outer HTML.
    pub fn parse(name: &str) -> Self {
        match name {
            "text" => Self::Text,
            "innerHTML" => Self::Inner,
            _ => Self::Outer,
        }
    }
}

/// Parsed documents, by their source text.
#[derive(Default)]
pub struct Documents {
    cache: RefCell<VecDeque<(String, Rc<Html>)>>,
}

/// The DOM's `textContent`: every text node under `node`, as written (Dart's `Element.text`).
pub fn text_content(node: NodeRef<'_, Node>) -> String {
    match node.value() {
        Node::Text(text) => text.to_string(),
        _ => node
            .descendants()
            .filter_map(|child| child.value().as_text().map(|text| &**text))
            .collect(),
    }
}

pub fn outer_html(node: NodeRef<'_, Node>) -> String {
    match node.value() {
        Node::Text(text) => text.to_string(),
        _ => ElementRef::wrap(node)
            .map(|element| element.html())
            .unwrap_or_default(),
    }
}

fn inner_html(node: NodeRef<'_, Node>) -> String {
    match node.value() {
        Node::Text(text) => text.to_string(),
        _ => ElementRef::wrap(node)
            .map(|element| element.inner_html())
            .unwrap_or_default(),
    }
}

fn selector(text: &str) -> Result<Selector, String> {
    Selector::parse(text).map_err(|error| format!("invalid selector: {error}"))
}

impl Documents {
    /// `content` parsed as a document, from the cache when it was parsed lately.
    pub fn parse(&self, content: &str) -> Rc<Html> {
        let mut cache = self.cache.borrow_mut();
        if let Some(position) = cache.iter().position(|(source, _)| source == content) {
            let (source, html) = cache.remove(position).expect("found above");
            cache.push_front((source, html.clone()));
            return html;
        }
        let html = Rc::new(Html::parse_document(content));
        if content.len() <= MAX_CACHED_BYTES {
            cache.push_front((content.to_owned(), html.clone()));
            cache.truncate(CACHED_DOCUMENTS);
        }
        html
    }

    /// The first match's text or HTML; empty when nothing matches.
    pub fn query(&self, content: &str, css: &str, part: Part) -> Result<String, String> {
        let selector = selector(css)?;
        let html = self.parse(content);
        let root = html.tree.root();
        Ok(selector
            .select_first(root)
            .and_then(|id| html.tree.get(id))
            .map(|node| match part {
                Part::Text => text_content(node),
                Part::Inner => inner_html(node),
                Part::Outer => outer_html(node),
            })
            .unwrap_or_default())
    }

    /// Every match's outer HTML, in document order.
    pub fn query_all(&self, content: &str, css: &str) -> Result<Vec<String>, String> {
        let selector = selector(css)?;
        let html = self.parse(content);
        Ok(selector
            .select(html.tree.root())
            .into_iter()
            .filter_map(|id| html.tree.get(id))
            .map(outer_html)
            .collect())
    }

    /// An attribute of the first match; `None` when nothing matches or it has no such attribute.
    pub fn attribute(
        &self,
        content: &str,
        css: &str,
        name: &str,
    ) -> Result<Option<String>, String> {
        let selector = selector(css)?;
        let html = self.parse(content);
        Ok(selector
            .select_first(html.tree.root())
            .and_then(|id| html.tree.get(id))
            .and_then(|node| {
                node.value()
                    .as_element()
                    .and_then(|element| element.attr(name))
            })
            .map(str::to_owned))
    }

    /// The document without the elements `css` matches, as HTML (Miru's `removeSelector`).
    pub fn remove(&self, content: &str, css: &str) -> Result<String, String> {
        let selector = selector(css)?;
        // A fresh parse: the cached one stays as it was.
        let mut html = Html::parse_document(content);
        let found = selector.select(html.tree.root());
        for id in found {
            if let Some(mut node) = html.tree.get_mut(id) {
                node.detach();
            }
        }
        Ok(html.html())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<html><head><title>T</title></head><body>
        <ul class="list">
          <li class="item"><a href="/a">  Alpha <b>One</b></a></li>
          <li class="item"><a href="/b" data-x="2">Beta</a></li>
        </ul>
        <div class="ad">buy</div><p>Line one<br>line two</p>
    </body></html>"#;

    #[test]
    fn answers_like_miru() {
        let documents = Documents::default();
        // Text is the DOM's textContent: whitespace kept, nothing added for <br>.
        assert_eq!(
            documents.query(PAGE, "li a", Part::Text).unwrap(),
            "  Alpha One"
        );
        assert_eq!(
            documents.query(PAGE, "p", Part::Text).unwrap(),
            "Line oneline two"
        );
        assert_eq!(
            documents.query(PAGE, "li a", Part::Inner).unwrap(),
            "  Alpha <b>One</b>"
        );
        assert_eq!(
            documents
                .query(PAGE, "li:nth-child(2) a", Part::Outer)
                .unwrap(),
            r#"<a href="/b" data-x="2">Beta</a>"#
        );
        assert_eq!(documents.query(PAGE, ".missing", Part::Text).unwrap(), "");
        let items = documents.query_all(PAGE, "ul.list > li").unwrap();
        assert_eq!(items.len(), 2);
        assert!(items[1].starts_with(r#"<li class="item">"#));
        assert_eq!(
            documents.attribute(PAGE, "a", "href").unwrap().as_deref(),
            Some("/a")
        );
        assert_eq!(documents.attribute(PAGE, "a", "data-x").unwrap(), None);
        assert_eq!(documents.attribute(PAGE, "nav", "href").unwrap(), None);
        let cleaned = documents.remove(PAGE, ".ad, b").unwrap();
        assert!(!cleaned.contains("buy") && !cleaned.contains("<b>"));
        assert!(cleaned.contains("Beta"));
        assert!(documents.query(PAGE, "li[", Part::Text).is_err());
    }

    #[test]
    fn a_fragment_is_parsed_as_a_document() {
        let documents = Documents::default();
        // As in Dart's `parse`: table rows outside a table lose their row and cell tags.
        let row = r#"<tr><td class="title"><a href="/x">X</a></td></tr>"#;
        assert_eq!(documents.query(row, ".title a", Part::Text).unwrap(), "");
        assert_eq!(documents.query(row, "a", Part::Text).unwrap(), "X");
    }

    #[test]
    fn recent_documents_are_reused() {
        let documents = Documents::default();
        let first = documents.parse(PAGE);
        let again = documents.parse(PAGE);
        assert!(Rc::ptr_eq(&first, &again));
        for index in 0..CACHED_DOCUMENTS {
            documents.parse(&format!("<p>{index}</p>"));
        }
        assert!(!Rc::ptr_eq(&first, &documents.parse(PAGE)), "pushed out");
    }
}
