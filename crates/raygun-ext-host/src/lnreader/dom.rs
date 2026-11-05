//! The documents LNReader plugins query through cheerio and htmlparser2 (`prelude.js`): HTML
//! parsed into `scraper` trees that stay on this side, their nodes handed to the script as
//! numbers. Selectors are Raygun's Jsoup-compatible engine, which takes the jQuery extensions
//! cheerio's plugins lean on (`:contains`, `:eq`, `:has`, `:not`, `:first`...).

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use ego_tree::{NodeId, NodeRef, Tree};
use scraper::{ElementRef, Html, Node};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::shosetsu::dom::{Place, copy, new_element};
use crate::shosetsu::select::Selector;

/// Documents kept at once, and the HTML they were parsed from; past either, the oldest go first.
/// Plugins parse snippets in loops (`$('<p>…</p>')`, `load(item)`), so the count is generous;
/// whole pages weigh in by their size.
const MAX_DOCUMENTS: usize = 1024;
const MAX_DOCUMENT_BYTES: usize = 24 * 1024 * 1024;
/// Selectors kept parsed.
const MAX_SELECTORS: usize = 256;
/// Nodes of one document a handle can name.
const NODES_PER_DOCUMENT: u64 = 1 << 24;

struct Document {
    html: Html,
    /// Handles' nodes, by handle.
    ids: Vec<NodeId>,
    handles: HashMap<NodeId, u32>,
    /// The length of the HTML it was parsed from.
    bytes: usize,
}

impl Document {
    fn handle(&mut self, number: u64, id: NodeId) -> u64 {
        let index = *self.handles.entry(id).or_insert_with(|| {
            self.ids.push(id);
            (self.ids.len() - 1) as u32
        });
        number * NODES_PER_DOCUMENT + u64::from(index)
    }
}

/// One extension's documents.
#[derive(Default)]
pub(crate) struct Documents {
    documents: RefCell<VecDeque<(u64, Document)>>,
    next: Cell<u64>,
    selectors: RefCell<HashMap<String, Rc<Selector>>>,
}

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    html: String,
    #[serde(default)]
    nodes: Vec<f64>,
    #[serde(default)]
    selector: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    place: String,
    #[serde(default)]
    all: bool,
    #[serde(default)]
    forward: bool,
    /// Siblings of any kind (text nodes too), as a node's `next` and `prev` see them.
    #[serde(default)]
    any: bool,
    #[serde(default)]
    text: String,
}

/// A node's text content: every text node under it, as it is (cheerio's `text()`).
fn text_content(node: NodeRef<'_, Node>) -> String {
    match node.value() {
        Node::Text(text) => text.to_string(),
        Node::Comment(_) => String::new(),
        _ => node
            .descendants()
            .filter_map(|descendant| match descendant.value() {
                Node::Text(text) => Some(&**text),
                _ => None,
            })
            .collect(),
    }
}

fn inner_html(node: NodeRef<'_, Node>) -> String {
    match node.value() {
        Node::Text(text) => escape(text),
        _ => match ElementRef::wrap(node) {
            Some(element) => element.inner_html(),
            None => node.children().map(outer_html).collect(),
        },
    }
}

fn outer_html(node: NodeRef<'_, Node>) -> String {
    match node.value() {
        Node::Text(text) => escape(text),
        Node::Comment(comment) => format!("<!--{}-->", &**comment),
        _ => match ElementRef::wrap(node) {
            Some(element) => element.html(),
            None => node.children().map(outer_html).collect(),
        },
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// What the script sees of a node: cheerio's (domhandler's) `type`, `name` and `attribs`, and
/// a text node's `data`.
fn info(node: NodeRef<'_, Node>) -> Value {
    match node.value() {
        Node::Element(element) => {
            let attribs: serde_json::Map<String, Value> = element
                .attrs()
                .map(|(name, value)| (name.to_owned(), json!(value)))
                .collect();
            let kind = match element.name() {
                "script" => "script",
                "style" => "style",
                _ => "tag",
            };
            json!({ "type": kind, "name": element.name(), "attribs": attribs })
        }
        Node::Text(text) => json!({ "type": "text", "data": &**text }),
        Node::Comment(comment) => json!({ "type": "comment", "data": &**comment }),
        Node::Document | Node::Fragment => json!({ "type": "root", "name": "root" }),
        _ => json!({ "type": "directive" }),
    }
}

fn is_element(node: &NodeRef<'_, Node>) -> bool {
    node.value().is_element()
}

/// Elements Jsoup puts a space between when reading text (its `Tag.isBlock`).
const BLOCKS: [&str; 62] = [
    "html",
    "head",
    "body",
    "frameset",
    "script",
    "noscript",
    "style",
    "meta",
    "link",
    "title",
    "frame",
    "noframes",
    "section",
    "nav",
    "aside",
    "hgroup",
    "header",
    "footer",
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "pre",
    "div",
    "blockquote",
    "hr",
    "address",
    "figure",
    "figcaption",
    "form",
    "fieldset",
    "ins",
    "del",
    "dl",
    "dt",
    "dd",
    "li",
    "table",
    "caption",
    "thead",
    "tfoot",
    "tbody",
    "colgroup",
    "col",
    "tr",
    "th",
    "td",
    "video",
    "audio",
    "canvas",
    "details",
    "menu",
    "plaintext",
    "template",
    "article",
    "main",
    "center",
];

fn element_name<'a>(node: &NodeRef<'a, Node>) -> Option<&'a str> {
    match node.value() {
        Node::Element(element) => Some(element.name()),
        _ => None,
    }
}

fn ends_with_space(text: &str) -> bool {
    text.is_empty() || text.ends_with(' ')
}

/// Appends `text` with its whitespace collapsed to single spaces, as Jsoup does.
fn append_normalised(out: &mut String, text: &str) {
    let mut space = ends_with_space(out);
    for char in text.chars() {
        // Unicode's white space, the no-break space among it, as Jsoup's normalising takes.
        if char.is_whitespace() {
            if !space {
                out.push(' ');
                space = true;
            }
        } else {
            out.push(char);
            space = false;
        }
    }
}

/// Jsoup's `Element.text()`: every text inside, whitespace collapsed, a space where a block or a
/// `<br>` breaks the text; scripts and styles left out.
fn jsoup_text(node: NodeRef<'_, Node>) -> String {
    let mut out = String::new();
    for edge in node.traverse() {
        match edge {
            ego_tree::iter::Edge::Open(current) => match current.value() {
                Node::Text(text) => {
                    let hidden = current.ancestors().any(|ancestor| {
                        matches!(
                            element_name(&ancestor),
                            Some("script" | "style" | "template")
                        )
                    });
                    if !hidden {
                        append_normalised(&mut out, text);
                    }
                }
                Node::Element(element) => {
                    let name = element.name();
                    if !out.is_empty()
                        && (BLOCKS.contains(&name) || name == "br")
                        && !ends_with_space(&out)
                    {
                        out.push(' ');
                    }
                }
                _ => {}
            },
            ego_tree::iter::Edge::Close(current) => {
                if let Some(name) = element_name(&current)
                    && BLOCKS.contains(&name)
                    && current
                        .next_sibling()
                        .is_some_and(|next| next.value().is_text())
                    && !ends_with_space(&out)
                {
                    out.push(' ');
                }
            }
        }
    }
    out.trim().to_owned()
}

/// Jsoup's `ownText()`: the element's own text nodes, a space for a `<br>`.
fn jsoup_own_text(node: NodeRef<'_, Node>) -> String {
    let mut out = String::new();
    for child in node.children() {
        match child.value() {
            Node::Text(text) => append_normalised(&mut out, text),
            Node::Element(element) if element.name() == "br" && !ends_with_space(&out) => {
                out.push(' ');
            }
            _ => {}
        }
    }
    out.trim().to_owned()
}

/// Jsoup's `data()`: a script's or a style's contents as written.
fn jsoup_data(node: NodeRef<'_, Node>) -> String {
    let own = matches!(element_name(&node), Some("script" | "style"));
    node.descendants()
        .filter(|descendant| {
            descendant.value().is_text()
                && (own
                    || descendant.ancestors().any(|ancestor| {
                        matches!(element_name(&ancestor), Some("script" | "style"))
                    }))
        })
        .filter_map(|descendant| match descendant.value() {
            Node::Text(text) => Some(text.to_string()),
            _ => None,
        })
        .collect()
}

/// The nodes `html` parses into, as a fragment: inside its wrapping `<html>`.
fn fragment_nodes(fragment: &Html) -> Vec<NodeRef<'_, Node>> {
    let root = fragment.tree.root();
    let source = root
        .children()
        .find(|child| child.value().is_element())
        .unwrap_or(root);
    source.children().collect()
}

impl Documents {
    fn selector(&self, text: &str) -> Result<Rc<Selector>, String> {
        if let Some(found) = self.selectors.borrow().get(text) {
            return Ok(found.clone());
        }
        let parsed = Rc::new(Selector::parse(text).map_err(|error| format!("{text:?}: {error}"))?);
        let mut selectors = self.selectors.borrow_mut();
        if selectors.len() >= MAX_SELECTORS {
            selectors.clear();
        }
        selectors.insert(text.to_owned(), parsed.clone());
        Ok(parsed)
    }

    /// Keeps a parsed document; returns its number.
    fn keep(&self, html: Html, bytes: usize) -> u64 {
        let number = self.next.get();
        self.next.set(number + 1);
        let mut documents = self.documents.borrow_mut();
        let mut kept: usize = documents.iter().map(|(_, document)| document.bytes).sum();
        while !documents.is_empty()
            && (documents.len() >= MAX_DOCUMENTS || kept + bytes > MAX_DOCUMENT_BYTES)
        {
            if let Some((_, oldest)) = documents.pop_front() {
                kept -= oldest.bytes;
            }
        }
        documents.push_back((
            number,
            Document {
                html,
                ids: Vec::new(),
                handles: HashMap::new(),
                bytes,
            },
        ));
        number
    }

    /// Runs `work` on the document `handle` belongs to, with the node it names.
    fn with<T>(
        &self,
        handle: f64,
        work: impl FnOnce(&mut Document, u64, NodeId) -> T,
    ) -> Result<T, String> {
        let handle = handle as u64;
        let (number, index) = (handle / NODES_PER_DOCUMENT, handle % NODES_PER_DOCUMENT);
        let mut documents = self.documents.borrow_mut();
        let document = documents
            .iter_mut()
            .find(|(known, _)| *known == number)
            .map(|(_, document)| document)
            .ok_or("this page was let go: parse it again with cheerio.load")?;
        let id = *document
            .ids
            .get(index as usize)
            .ok_or("this node doesn't belong to its page")?;
        Ok(work(document, number, id))
    }

    /// Maps each node to more nodes (with their document), the results once each in order.
    fn gather(
        &self,
        nodes: &[f64],
        mut each: impl FnMut(NodeRef<'_, Node>) -> Vec<NodeId>,
    ) -> Result<Vec<u64>, String> {
        let mut found = Vec::new();
        for handle in nodes {
            let more = self.with(*handle, |document, number, id| {
                let ids = document
                    .html
                    .tree
                    .get(id)
                    .map(&mut each)
                    .unwrap_or_default();
                ids.into_iter()
                    .map(|id| document.handle(number, id))
                    .collect::<Vec<_>>()
            })?;
            for handle in more {
                if !found.contains(&handle) {
                    found.push(handle);
                }
            }
        }
        Ok(found)
    }

    fn read<T>(&self, handle: f64, read: impl FnOnce(NodeRef<'_, Node>) -> T) -> Result<T, String> {
        self.with(handle, |document, _, id| {
            document.html.tree.get(id).map(read)
        })?
        .ok_or_else(|| "the node is gone".to_owned())
    }

    fn edit(
        &self,
        nodes: &[f64],
        mut edit: impl FnMut(&mut Tree<Node>, NodeId),
    ) -> Result<(), String> {
        for handle in nodes {
            self.with(*handle, |document, _, id| edit(&mut document.html.tree, id))?;
        }
        Ok(())
    }

    /// Changes an element's attributes; the element is built anew (scraper caches its id and
    /// classes).
    fn set_attribute(tree: &mut Tree<Node>, id: NodeId, name: &str, value: Option<&str>) {
        let Some(mut node) = tree.get_mut(id) else {
            return;
        };
        let Node::Element(element) = node.value() else {
            return;
        };
        let tag = element.name().to_owned();
        let mut attributes: Vec<(String, String)> = element
            .attrs()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect();
        let name = name.to_ascii_lowercase();
        match value {
            Some(value) => match attributes.iter_mut().find(|(key, _)| *key == name) {
                Some((_, existing)) => value.clone_into(existing),
                None => attributes.push((name, value.to_owned())),
            },
            None => attributes.retain(|(key, _)| *key != name),
        }
        *node.value() = new_element(&tag, attributes);
    }

    fn insert(tree: &mut Tree<Node>, id: NodeId, html: &str, place: Place) {
        let fragment = Html::parse_fragment(html);
        let nodes = fragment_nodes(&fragment);
        match place {
            Place::Before | Place::Append => {
                for node in nodes {
                    copy(tree, node, id, place);
                }
            }
            // In reverse: each lands right after (or first inside) the anchor.
            Place::After | Place::Prepend => {
                for node in nodes.into_iter().rev() {
                    copy(tree, node, id, place);
                }
            }
        }
    }

    /// A copy of `source`'s subtree, not yet in the page.
    fn orphan_copy(tree: &mut Tree<Node>, source: NodeId) -> Option<NodeId> {
        fn copy_children(tree: &mut Tree<Node>, from: NodeId, to: NodeId) {
            let children: Vec<(NodeId, Node)> = tree
                .get(from)
                .map(|node| {
                    node.children()
                        .map(|child| (child.id(), child.value().clone()))
                        .collect()
                })
                .unwrap_or_default();
            for (child, value) in children {
                let copied = tree.get_mut(to).map(|mut parent| parent.append(value).id());
                if let Some(copied) = copied {
                    copy_children(tree, child, copied);
                }
            }
        }
        let value = tree.get(source)?.value().clone();
        let id = tree.orphan(value).id();
        copy_children(tree, source, id);
        Some(id)
    }

    /// One operation the prelude asks for (`op`), with its arguments as JSON.
    pub fn run(&self, op: &str, args: &str) -> Result<Value, String> {
        let args: Args =
            serde_json::from_str(args).map_err(|error| format!("invalid arguments: {error}"))?;
        let nodes = &args.nodes;
        Ok(match op {
            "load" => {
                let number = self.keep(Html::parse_document(&args.html), args.html.len());
                let mut documents = self.documents.borrow_mut();
                let document = &mut documents.back_mut().expect("just kept").1;
                let root = document.html.tree.root().id();
                json!(document.handle(number, root))
            }
            // Nodes made from HTML (`$('<p>…</p>')`), in a page of their own.
            "parse" => {
                let number = self.keep(Html::parse_fragment(&args.html), args.html.len());
                let mut documents = self.documents.borrow_mut();
                let document = &mut documents.back_mut().expect("just kept").1;
                let ids: Vec<NodeId> = fragment_nodes(&document.html)
                    .iter()
                    .map(NodeRef::id)
                    .collect();
                json!(
                    ids.into_iter()
                        .map(|id| document.handle(number, id))
                        .collect::<Vec<_>>()
                )
            }
            "select" => {
                let selector = self.selector(&args.selector)?;
                json!(self.gather(nodes, |node| selector.select(node))?)
            }
            "filter" => {
                let selector = self.selector(&args.selector)?;
                let mut kept = Vec::new();
                for handle in nodes {
                    let matches = self.read(*handle, |node| {
                        let root = node.ancestors().last().unwrap_or(node);
                        node.value().is_element() && selector.matches(node, root)
                    })?;
                    if matches {
                        kept.push(*handle);
                    }
                }
                json!(kept)
            }
            "info" => {
                let mut infos = Vec::new();
                for handle in nodes {
                    infos.push(self.read(*handle, info)?);
                }
                json!(infos)
            }
            "children" => {
                let all = args.all;
                json!(self.gather(nodes, |node| {
                    node.children()
                        .filter(|child| all || is_element(child))
                        .map(|child| child.id())
                        .collect()
                })?)
            }
            "parent" => json!(self.gather(nodes, |node| {
                node.parent()
                    .filter(is_element)
                    .map(|parent| parent.id())
                    .into_iter()
                    .collect()
            })?),
            "parents" => json!(self.gather(nodes, |node| {
                node.ancestors()
                    .filter(is_element)
                    .map(|ancestor| ancestor.id())
                    .collect()
            })?),
            // One sibling (`all` off) or every one, forward or back; elements only unless `any`.
            "siblings" => {
                let (all, forward, any) = (args.all, args.forward, args.any);
                json!(self.gather(nodes, |node| {
                    let siblings: Box<dyn Iterator<Item = NodeRef<'_, Node>>> = if forward {
                        Box::new(node.next_siblings())
                    } else {
                        Box::new(node.prev_siblings())
                    };
                    let kept = siblings
                        .filter(|sibling| any || is_element(sibling))
                        .map(|sibling| sibling.id());
                    if all {
                        kept.collect()
                    } else {
                        kept.take(1).collect()
                    }
                })?)
            }
            "text" => {
                let mut text = String::new();
                for handle in nodes {
                    text.push_str(&self.read(*handle, text_content)?);
                }
                json!(text)
            }
            // Jsoup's `text()`, `ownText()` and `data()`, for the Mihon runtime.
            "jsoupText" => {
                let mut texts = Vec::new();
                for handle in nodes {
                    texts.push(self.read(*handle, jsoup_text)?);
                }
                json!(texts)
            }
            "ownText" => match nodes.first() {
                Some(handle) => json!(self.read(*handle, jsoup_own_text)?),
                None => json!(""),
            },
            "data" => match nodes.first() {
                Some(handle) => json!(self.read(*handle, jsoup_data)?),
                None => json!(""),
            },
            "html" => match nodes.first() {
                Some(handle) => json!(self.read(*handle, |node| match node.value() {
                    Node::Document => outer_html(node),
                    _ => inner_html(node),
                })?),
                None => Value::Null,
            },
            "outer" => {
                let mut html = String::new();
                for handle in nodes {
                    html.push_str(&self.read(*handle, outer_html)?);
                }
                json!(html)
            }
            "setAttr" => {
                self.edit(nodes, |tree, id| {
                    Self::set_attribute(tree, id, &args.name, args.value.as_deref());
                })?;
                Value::Null
            }
            "remove" => {
                self.edit(nodes, |tree, id| {
                    if let Some(mut node) = tree.get_mut(id) {
                        node.detach();
                    }
                })?;
                Value::Null
            }
            "empty" | "setHtml" | "setText" => {
                self.edit(nodes, |tree, id| {
                    let children: Vec<NodeId> = tree
                        .get(id)
                        .map(|node| node.children().map(|child| child.id()).collect())
                        .unwrap_or_default();
                    for child in children {
                        if let Some(mut child) = tree.get_mut(child) {
                            child.detach();
                        }
                    }
                    match op {
                        "setHtml" => Self::insert(tree, id, &args.html, Place::Append),
                        "setText" => {
                            if let Some(mut node) = tree.get_mut(id) {
                                node.append(Node::Text(scraper::node::Text {
                                    text: args.text.as_str().into(),
                                }));
                            }
                        }
                        _ => {}
                    }
                })?;
                Value::Null
            }
            "insert" | "replace" => {
                let place = match (op, args.place.as_str()) {
                    ("replace", _) | (_, "before") => Place::Before,
                    (_, "after") => Place::After,
                    (_, "prepend") => Place::Prepend,
                    _ => Place::Append,
                };
                self.edit(nodes, |tree, id| {
                    Self::insert(tree, id, &args.html, place);
                    if op == "replace"
                        && let Some(mut node) = tree.get_mut(id)
                    {
                        node.detach();
                    }
                })?;
                Value::Null
            }
            "clone" => {
                let mut copies = Vec::new();
                for handle in nodes {
                    let copy = self.with(*handle, |document, number, id| {
                        Self::orphan_copy(&mut document.html.tree, id)
                            .map(|copied| document.handle(number, copied))
                    })?;
                    copies.extend(copy);
                }
                json!(copies)
            }
            // htmlparser2's events for a page, from its parsed tree.
            "tokens" => {
                let html = Html::parse_document(&args.html);
                let mut events = Vec::new();
                for edge in html.tree.root().traverse() {
                    match edge {
                        ego_tree::iter::Edge::Open(node) => match node.value() {
                            Node::Element(element) => {
                                let attribs: serde_json::Map<String, Value> = element
                                    .attrs()
                                    .map(|(name, value)| (name.to_owned(), json!(value)))
                                    .collect();
                                events.push(json!(["open", element.name(), attribs]));
                            }
                            Node::Text(text) => events.push(json!(["text", &**text])),
                            _ => {}
                        },
                        ego_tree::iter::Edge::Close(node) => {
                            if let Node::Element(element) = node.value() {
                                events.push(json!(["close", element.name()]));
                            }
                        }
                    }
                }
                json!(events)
            }
            other => return Err(format!("unknown DOM operation {other}")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(documents: &Documents, op: &str, args: Value) -> Value {
        documents.run(op, &args.to_string()).unwrap()
    }

    #[test]
    fn selects_reads_and_edits_through_handles() {
        let documents = Documents::default();
        let root = run(
            &documents,
            "load",
            json!({ "html": "<ul><li class='a'>One <b>1</b></li><li>Two</li></ul><p id='x'>Text &amp; more</p>" }),
        );
        let items = run(
            &documents,
            "select",
            json!({ "nodes": [root], "selector": "li" }),
        );
        assert_eq!(items.as_array().unwrap().len(), 2);
        assert_eq!(
            run(&documents, "text", json!({ "nodes": items })),
            "One 1Two"
        );
        let first = run(
            &documents,
            "filter",
            json!({ "nodes": items, "selector": ".a" }),
        );
        assert_eq!(first.as_array().unwrap().len(), 1);
        assert_eq!(
            run(&documents, "html", json!({ "nodes": first })),
            "One <b>1</b>"
        );
        let info = run(&documents, "info", json!({ "nodes": first }));
        assert_eq!(info[0]["attribs"]["class"], "a");
        let contains = run(
            &documents,
            "select",
            json!({ "nodes": [root], "selector": "li:contains(Two)" }),
        );
        assert_eq!(contains.as_array().unwrap().len(), 1);
        let next = run(
            &documents,
            "siblings",
            json!({ "nodes": first, "forward": true }),
        );
        assert_eq!(run(&documents, "text", json!({ "nodes": next })), "Two");
        let parent = run(&documents, "parent", json!({ "nodes": first }));
        assert_eq!(
            run(&documents, "info", json!({ "nodes": parent }))[0]["name"],
            "ul"
        );

        // Edits.
        let bold = run(
            &documents,
            "select",
            json!({ "nodes": first, "selector": "b" }),
        );
        run(&documents, "remove", json!({ "nodes": bold }));
        run(
            &documents,
            "setAttr",
            json!({ "nodes": first, "name": "data-n", "value": "1" }),
        );
        run(
            &documents,
            "insert",
            json!({ "nodes": first, "html": "<i>!</i>", "place": "append" }),
        );
        assert_eq!(
            run(&documents, "outer", json!({ "nodes": first })),
            "<li class=\"a\" data-n=\"1\">One <i>!</i></li>"
        );
        let paragraph = run(
            &documents,
            "select",
            json!({ "nodes": [root], "selector": "#x" }),
        );
        run(
            &documents,
            "replace",
            json!({ "nodes": paragraph, "html": "<div>New</div>" }),
        );
        assert!(
            run(&documents, "html", json!({ "nodes": [root] }))
                .as_str()
                .unwrap()
                .contains("<div>New</div>")
        );
        let copies = run(&documents, "clone", json!({ "nodes": first }));
        run(&documents, "empty", json!({ "nodes": copies }));
        assert_eq!(
            run(&documents, "text", json!({ "nodes": first })),
            "One !",
            "a copy changes alone"
        );
    }

    #[test]
    fn snippets_parsed_in_a_loop_leave_the_page_alone() {
        let documents = Documents::default();
        let root = run(
            &documents,
            "load",
            json!({ "html": "<p class='keep'>Still here</p>" }),
        );
        for index in 0..200 {
            run(
                &documents,
                "parse",
                json!({ "html": format!("<span>{index}</span>") }),
            );
        }
        let kept = run(
            &documents,
            "select",
            json!({ "nodes": [root], "selector": ".keep" }),
        );
        assert_eq!(
            run(&documents, "text", json!({ "nodes": kept })),
            "Still here"
        );
    }

    #[test]
    fn tokens_follow_the_page() {
        let documents = Documents::default();
        let events = run(
            &documents,
            "tokens",
            json!({ "html": "<div class='c'>Hi<br>there</div>" }),
        );
        let names: Vec<String> = events
            .as_array()
            .unwrap()
            .iter()
            .map(|event| {
                format!(
                    "{}:{}",
                    event[0].as_str().unwrap(),
                    event[1].as_str().unwrap_or("")
                )
            })
            .collect();
        assert!(names.contains(&"open:div".to_owned()));
        assert!(names.contains(&"text:Hi".to_owned()));
        assert!(names.contains(&"close:br".to_owned()));
        let made = run(
            &documents,
            "parse",
            json!({ "html": "<a href='/x'>A</a><span>B</span>" }),
        );
        assert_eq!(made.as_array().unwrap().len(), 2);
        assert!(
            documents
                .run(
                    "select",
                    &json!({ "nodes": [99_999_999_999.0_f64], "selector": "a" }).to_string()
                )
                .is_err()
        );
    }
}
