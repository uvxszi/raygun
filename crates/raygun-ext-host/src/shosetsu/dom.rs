//! Jsoup-like documents for Shosetsu scripts (M32): `Document`, `Element`, `Elements` and text
//! nodes as Lua userdata over one shared, mutable `scraper` tree. Method names and behaviour
//! follow Jsoup, which Shosetsu's kotlin-lib (GPL-3.0) hands to scripts through LuaJ.

use std::cell::RefCell;
use std::rc::Rc;

use ego_tree::{NodeId, NodeRef, Tree};
use html5ever::{Attribute, LocalName, Namespace, QualName};
use mlua::{AnyUserData, Function, Lua, MetaMethod, Table, UserData, UserDataMethods, Value};
use scraper::{ElementRef, Html, Node};

use super::select::{Selector, element_text, normalize, own_text};

const HTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";

/// One parsed document, shared by every node handed out from it.
pub struct Doc {
    html: RefCell<Html>,
    /// Where the document came from, for `attr("abs:href")`.
    base: Option<url::Url>,
}

pub type DocRef = Rc<Doc>;

/// A node of a document: the document itself, an element or a text node.
#[derive(Clone)]
pub struct LuaNode {
    doc: DocRef,
    id: NodeId,
}

/// Jsoup's `Elements`: a list of nodes, with list methods and bulk element methods.
#[derive(Clone)]
pub struct LuaElements {
    doc: DocRef,
    ids: Vec<NodeId>,
}

pub fn parse_document(text: &str, base: Option<url::Url>) -> LuaNode {
    let html = Html::parse_document(text);
    let id = html.tree.root().id();
    LuaNode {
        doc: Rc::new(Doc {
            html: RefCell::new(html),
            base,
        }),
        id,
    }
}

fn selector(text: &str) -> mlua::Result<Selector> {
    Selector::parse(text).map_err(mlua::Error::runtime)
}

/// A tag name for `appendElement`: letters, digits and dashes only, so it can't inject markup.
fn element_tag(tag: &str) -> mlua::Result<&str> {
    if !tag.is_empty() && tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        Ok(tag)
    } else {
        Err(mlua::Error::runtime(format!("{tag:?} isn't a tag name")))
    }
}

/// Text as HTML that reads back as the same text.
fn escape_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// A new HTML element with these attributes.
pub(crate) fn new_element(tag: &str, attributes: Vec<(String, String)>) -> Node {
    let name = QualName::new(
        None,
        Namespace::from(HTML_NAMESPACE),
        LocalName::from(tag.to_ascii_lowercase()),
    );
    let attributes = attributes
        .into_iter()
        .map(|(name, value)| Attribute {
            name: QualName::new(None, Namespace::from(""), LocalName::from(name)),
            value: value.into(),
        })
        .collect();
    Node::Element(scraper::node::Element::new(name, attributes))
}

/// Jsoup's `wholeText()`: every text node as it is, with a line break for each `<br>`.
fn whole_text(node: NodeRef<'_, Node>) -> String {
    let mut text = String::new();
    for descendant in node.descendants() {
        match descendant.value() {
            Node::Text(part) if !super::select::is_data(descendant) => text.push_str(part),
            Node::Element(element) if element.name() == "br" => text.push('\n'),
            _ => {}
        }
    }
    text
}

/// Jsoup's `data()`: what `<script>` and `<style>` elements (and comments) hold, not text.
fn data_of(node: NodeRef<'_, Node>) -> String {
    let mut data = String::new();
    for descendant in node.descendants() {
        match descendant.value() {
            Node::Comment(comment) => data.push_str(comment),
            Node::Text(part) if super::select::is_data(descendant) => data.push_str(part),
            _ => {}
        }
    }
    data
}

impl LuaNode {
    fn with<T>(&self, read: impl FnOnce(NodeRef<'_, Node>) -> T) -> T {
        let html = self.doc.html.borrow();
        let node = html.tree.get(self.id).expect("node ids stay valid");
        read(node)
    }

    fn node(&self, id: NodeId) -> Self {
        Self {
            doc: self.doc.clone(),
            id,
        }
    }

    fn list(&self, ids: Vec<NodeId>) -> LuaElements {
        LuaElements {
            doc: self.doc.clone(),
            ids,
        }
    }

    pub fn select(&self, css: &str) -> mlua::Result<LuaElements> {
        let selector = selector(css)?;
        Ok(self.list(self.with(|node| selector.select(node))))
    }

    pub fn select_first(&self, css: &str) -> mlua::Result<Option<LuaNode>> {
        let selector = selector(css)?;
        Ok(self
            .with(|node| selector.select_first(node))
            .map(|id| self.node(id)))
    }

    pub fn text(&self) -> String {
        self.with(|node| match node.value() {
            Node::Text(text) => normalize(text),
            _ => element_text(node),
        })
    }

    pub fn attr(&self, name: &str) -> String {
        if let Some(name) = name.strip_prefix("abs:") {
            return self.abs_url(name);
        }
        self.with(|node| {
            node.value()
                .as_element()
                .and_then(|element| {
                    element
                        .attrs()
                        .find(|(key, _)| key.eq_ignore_ascii_case(name))
                        .map(|(_, value)| value.to_owned())
                })
                .unwrap_or_default()
        })
    }

    fn abs_url(&self, name: &str) -> String {
        let value = self.attr(name);
        if value.is_empty() {
            return value;
        }
        match &self.doc.base {
            Some(base) => base
                .join(&value)
                .map(|url| url.to_string())
                .unwrap_or_default(),
            None => url::Url::parse(&value)
                .map(|url| url.to_string())
                .unwrap_or_default(),
        }
    }

    fn has_attr(&self, name: &str) -> bool {
        self.with(|node| {
            node.value().as_element().is_some_and(|element| {
                element
                    .attrs()
                    .any(|(key, _)| key.eq_ignore_ascii_case(name))
            })
        })
    }

    fn tag(&self) -> String {
        self.with(|node| match node.value() {
            Node::Element(element) => element.name().to_owned(),
            Node::Text(_) => "#text".to_owned(),
            Node::Document | Node::Fragment => "#document".to_owned(),
            Node::Comment(_) => "#comment".to_owned(),
            _ => "#node".to_owned(),
        })
    }

    pub fn inner_html(&self) -> String {
        self.with(|node| match node.value() {
            Node::Text(text) => text.to_string(),
            _ => ElementRef::wrap(node).map_or_else(
                || {
                    node.children()
                        .filter_map(ElementRef::wrap)
                        .map(|el| el.html())
                        .collect()
                },
                |element| element.inner_html(),
            ),
        })
    }

    pub fn outer_html(&self) -> String {
        self.with(|node| match node.value() {
            // Jsoup escapes a text node's HTML, so it reads back as the same text.
            Node::Text(text) if !super::select::is_data(node) => escape_text(text),
            Node::Text(text) => text.to_string(),
            _ => ElementRef::wrap(node).map_or_else(
                || {
                    node.children()
                        .filter_map(ElementRef::wrap)
                        .map(|el| el.html())
                        .collect()
                },
                |element| element.html(),
            ),
        })
    }

    fn element_children(&self) -> Vec<NodeId> {
        self.with(|node| {
            node.children()
                .filter(|child| child.value().is_element())
                .map(|child| child.id())
                .collect()
        })
    }

    fn remove(&self) {
        if let Some(mut node) = self.doc.html.borrow_mut().tree.get_mut(self.id) {
            node.detach();
        }
    }

    /// Changes this element's attributes (Jsoup's `attr(name, value)`, `removeAttr`, classes).
    /// The element is built anew: scraper caches its id and classes.
    fn edit_attributes(&self, change: impl FnOnce(&mut Vec<(String, String)>)) {
        let mut doc = self.doc.html.borrow_mut();
        let Some(mut node) = doc.tree.get_mut(self.id) else {
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
        change(&mut attributes);
        *node.value() = new_element(&tag, attributes);
    }

    fn set_attr(&self, name: &str, value: &str) {
        let name = name.to_ascii_lowercase();
        self.edit_attributes(|attributes| {
            match attributes
                .iter_mut()
                .find(|(key, _)| key.eq_ignore_ascii_case(&name))
            {
                Some((_, existing)) => value.clone_into(existing),
                None => attributes.push((name, value.to_owned())),
            }
        });
    }

    fn remove_attr(&self, name: &str) {
        self.edit_attributes(|attributes| {
            attributes.retain(|(key, _)| !key.eq_ignore_ascii_case(name));
        });
    }

    /// Adds (`Some(true)`), removes (`Some(false)`) or toggles (`None`) a class.
    fn change_class(&self, class: &str, add: Option<bool>) {
        let mut classes: Vec<String> = self
            .attr("class")
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        let present = classes
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(class));
        match (add, present) {
            (Some(true) | None, false) => classes.push(class.to_owned()),
            (Some(false) | None, true) => {
                classes.retain(|existing| !existing.eq_ignore_ascii_case(class));
            }
            _ => return,
        }
        self.set_attr("class", &classes.join(" "));
    }

    /// Jsoup's `text(value)`: the element's children become this one text; a text node's own
    /// text changes.
    fn set_text(&self, text: &str) {
        let value = Node::Text(scraper::node::Text { text: text.into() });
        if self.with(|node| node.value().is_text()) {
            if let Some(mut node) = self.doc.html.borrow_mut().tree.get_mut(self.id) {
                *node.value() = value;
            }
            return;
        }
        self.empty();
        if let Some(mut node) = self.doc.html.borrow_mut().tree.get_mut(self.id) {
            node.append(value);
        }
    }

    /// Jsoup's `html(value)`: the element's children become this HTML.
    fn set_html(&self, html: &str) {
        self.empty();
        self.insert(html, Place::Append);
    }

    fn element_ancestors(&self) -> Vec<NodeId> {
        self.with(|node| {
            node.ancestors()
                .filter(|ancestor| ancestor.value().is_element())
                .map(|ancestor| ancestor.id())
                .collect()
        })
    }

    /// Moves `child` (or a copy of it, when it's from another document) into this element,
    /// first or last (Jsoup's `appendChild`/`prependChild`).
    fn adopt(&self, child: &LuaNode, first: bool) -> mlua::Result<()> {
        if Rc::ptr_eq(&self.doc, &child.doc) {
            if child.id == self.id || self.element_ancestors().contains(&child.id) {
                return Err(mlua::Error::runtime(
                    "an element can't be put inside itself",
                ));
            }
            let mut doc = self.doc.html.borrow_mut();
            if let Some(mut node) = doc.tree.get_mut(self.id) {
                if first {
                    node.prepend_id(child.id);
                } else {
                    node.append_id(child.id);
                }
            }
            return Ok(());
        }
        let source = child.doc.html.borrow();
        let Some(node) = source.tree.get(child.id) else {
            return Ok(());
        };
        let mut doc = self.doc.html.borrow_mut();
        copy(
            &mut doc.tree,
            node,
            self.id,
            if first { Place::Prepend } else { Place::Append },
        );
        Ok(())
    }

    /// A new element, not yet in the page (Jsoup's `Document.createElement`).
    fn create_element(&self, tag: &str) -> mlua::Result<LuaNode> {
        let tag = element_tag(tag)?;
        let id = self
            .doc
            .html
            .borrow_mut()
            .tree
            .orphan(new_element(tag, Vec::new()))
            .id();
        Ok(self.node(id))
    }

    /// The first or last element among this element's siblings (itself included).
    fn end_sibling(&self, last: bool) -> Option<LuaNode> {
        let id = self.with(|node| {
            let parent = node.parent()?;
            let mut elements = parent.children().filter(|child| child.value().is_element());
            if last {
                elements.last()
            } else {
                elements.next()
            }
            .map(|element| element.id())
        })?;
        Some(self.node(id))
    }

    /// Parses `html` and puts its nodes next to (or inside) this node.
    fn insert(&self, html: &str, place: Place) {
        let fragment = Html::parse_fragment(html);
        let root = fragment.tree.root();
        // A fragment parses into a wrapping <html> element.
        let source = root
            .children()
            .find(|child| child.value().is_element())
            .unwrap_or(root);
        let nodes: Vec<NodeRef<'_, Node>> = source.children().collect();
        let mut doc = self.doc.html.borrow_mut();
        let tree = &mut doc.tree;
        match place {
            Place::Before | Place::Append => {
                for node in nodes {
                    copy(tree, node, self.id, place);
                }
            }
            // Inserted in reverse, each lands right after (or first inside) this node.
            Place::After | Place::Prepend => {
                for node in nodes.into_iter().rev() {
                    copy(tree, node, self.id, place);
                }
            }
        }
    }

    /// Jsoup's `before`/`after` (HTML, or a node, which moves) and `prepend`/`append` (HTML; a
    /// node given there reads as its HTML, as LuaJ turns it into a string).
    fn put(&self, content: &Value, place: Place) -> mlua::Result<()> {
        let html = match content {
            Value::String(text) => text.to_str()?.to_owned(),
            Value::Integer(number) => number.to_string(),
            Value::Number(number) => number.to_string(),
            Value::UserData(data) => {
                if let Ok(node) = data.borrow::<LuaNode>() {
                    let node = node.clone();
                    let moves = matches!(place, Place::Before | Place::After)
                        && Rc::ptr_eq(&node.doc, &self.doc);
                    if !moves {
                        node.outer_html()
                    } else {
                        if node.id == self.id || self.element_ancestors().contains(&node.id) {
                            return Err(mlua::Error::runtime("a node can't be put beside itself"));
                        }
                        let mut doc = self.doc.html.borrow_mut();
                        if let Some(mut anchor) = doc.tree.get_mut(self.id) {
                            if place == Place::Before {
                                anchor.insert_id_before(node.id);
                            } else {
                                anchor.insert_id_after(node.id);
                            }
                        }
                        return Ok(());
                    }
                } else {
                    let elements = data.borrow::<LuaElements>()?;
                    elements
                        .nodes()
                        .map(|node| node.outer_html())
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            }
            other => {
                return Err(mlua::Error::runtime(format!(
                    "expected HTML or a node, got {}",
                    other.type_name()
                )));
            }
        };
        self.insert(&html, place);
        Ok(())
    }

    fn empty(&self) {
        let children: Vec<NodeId> =
            self.with(|node| node.children().map(|child| child.id()).collect());
        let mut doc = self.doc.html.borrow_mut();
        for child in children {
            if let Some(mut child) = doc.tree.get_mut(child) {
                child.detach();
            }
        }
    }

    fn sibling_elements(&self, forward: bool) -> Vec<NodeId> {
        self.with(|node| {
            let siblings: Box<dyn Iterator<Item = NodeRef<'_, Node>>> = if forward {
                Box::new(node.next_siblings())
            } else {
                Box::new(node.prev_siblings())
            };
            siblings
                .filter(|sibling| sibling.value().is_element())
                .map(|sibling| sibling.id())
                .collect()
        })
    }

    /// Calls `visitor.head(node, depth)` on the way down and `visitor.tail(node, depth)` on the
    /// way up, like Jsoup's `traverse`.
    fn traverse(&self, visitor: &Table) -> mlua::Result<()> {
        let head: Option<Function> = visitor.get("head")?;
        let tail: Option<Function> = visitor.get("tail")?;
        let elements_only: bool = visitor.get::<Option<bool>>("elementsOnly")?.unwrap_or(true);
        // Collected first: the callbacks may change the tree.
        let edges: Vec<(NodeId, usize, bool, bool)> = self.with(|node| {
            let mut depth = 0usize;
            node.traverse()
                .map(|edge| match edge {
                    ego_tree::iter::Edge::Open(child) => {
                        let entry = (child.id(), depth, true, child.value().is_element());
                        depth += 1;
                        entry
                    }
                    ego_tree::iter::Edge::Close(child) => {
                        depth -= 1;
                        (child.id(), depth, false, child.value().is_element())
                    }
                })
                .collect()
        });
        for (id, depth, open, is_element) in edges {
            if elements_only && !is_element {
                continue;
            }
            let callback = if open { &head } else { &tail };
            if let Some(callback) = callback {
                callback.call::<()>((self.node(id), depth))?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    Before,
    After,
    Prepend,
    Append,
}

/// Copies `source` (from another tree) into `tree` at `place` relative to `anchor`.
pub(crate) fn copy(tree: &mut Tree<Node>, source: NodeRef<'_, Node>, anchor: NodeId, place: Place) {
    let Some(mut target) = tree.get_mut(anchor) else {
        return;
    };
    let value = source.value().clone();
    let id = match place {
        Place::Before => target.insert_before(value).id(),
        Place::After => target.insert_after(value).id(),
        Place::Prepend => target.prepend(value).id(),
        Place::Append => target.append(value).id(),
    };
    for child in source.children() {
        copy(tree, child, id, Place::Append);
    }
}

impl UserData for LuaNode {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("select", |_, this, css: String| this.select(&css));
        methods.add_method("selectFirst", |_, this, css: String| {
            this.select_first(&css)
        });
        methods.add_method("getElementById", |_, this, id: String| {
            this.select_first(&format!("[id={id:?}]"))
        });
        methods.add_method("getElementsByTag", |_, this, tag: String| this.select(&tag));
        methods.add_method("getElementsByClass", |_, this, class: String| {
            let selector = Selector::class(&class);
            Ok(this.list(this.with(|node| selector.select(node))))
        });
        methods.add_method("selectLast", |_, this, css: String| {
            Ok(this.select(&css)?.nodes().last())
        });
        // Jsoup's `text()` reads; `text(value)` replaces the children with that text.
        methods.add_function(
            "text",
            |lua, (this, value): (AnyUserData, Option<String>)| {
                let node = this.borrow::<LuaNode>()?.clone();
                match value {
                    Some(value) => {
                        node.set_text(&value);
                        Ok(Value::UserData(this))
                    }
                    None => Ok(Value::String(lua.create_string(node.text())?)),
                }
            },
        );
        methods.add_method("ownText", |_, this, ()| {
            Ok(this.with(|node| match node.value() {
                Node::Text(text) => normalize(text),
                _ => own_text(node),
            }))
        });
        for name in ["wholeText", "getWholeText"] {
            methods.add_method(name, |_, this, ()| {
                Ok(this.with(|node| match node.value() {
                    Node::Text(text) => text.to_string(),
                    _ => whole_text(node),
                }))
            });
        }
        methods.add_method("data", |_, this, ()| {
            Ok(this.with(|node| match node.value() {
                Node::Text(text) => text.to_string(),
                _ => data_of(node),
            }))
        });
        methods.add_method("isBlank", |_, this, ()| Ok(this.text().is_empty()));
        methods.add_method("hasText", |_, this, ()| Ok(!this.text().is_empty()));
        // Jsoup's `attr(name)` reads; `attr(name, value)` sets and gives the element back.
        methods.add_function(
            "attr",
            |lua, (this, name, value): (AnyUserData, String, Option<Value>)| {
                let node = this.borrow::<LuaNode>()?.clone();
                match value {
                    None | Some(Value::Nil) => {
                        Ok(Value::String(lua.create_string(node.attr(&name))?))
                    }
                    Some(value) => {
                        let text = match &value {
                            Value::String(text) => text.to_str()?.to_owned(),
                            Value::Boolean(flag) => flag.to_string(),
                            Value::Integer(number) => number.to_string(),
                            Value::Number(number) => number.to_string(),
                            other => {
                                return Err(mlua::Error::runtime(format!(
                                    "an attribute can't be a {}",
                                    other.type_name()
                                )));
                            }
                        };
                        node.set_attr(&name, &text);
                        Ok(Value::UserData(this))
                    }
                }
            },
        );
        methods.add_method("absUrl", |_, this, name: String| Ok(this.abs_url(&name)));
        methods.add_method("hasAttr", |_, this, name: String| Ok(this.has_attr(&name)));
        methods.add_method("removeAttr", |_, this, name: String| {
            this.remove_attr(&name);
            Ok(this.clone())
        });
        for (name, add) in [
            ("addClass", Some(true)),
            ("removeClass", Some(false)),
            ("toggleClass", None),
        ] {
            methods.add_method(name, move |_, this, class: String| {
                this.change_class(&class, add);
                Ok(this.clone())
            });
        }
        methods.add_method("id", |_, this, ()| Ok(this.attr("id")));
        methods.add_method("className", |_, this, ()| Ok(this.attr("class")));
        methods.add_method("classNames", |lua, this, ()| {
            lua.create_sequence_from(
                this.attr("class")
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
            )
        });
        methods.add_method("hasClass", |_, this, class: String| {
            Ok(this
                .attr("class")
                .split_whitespace()
                .any(|candidate| candidate.eq_ignore_ascii_case(&class)))
        });
        methods.add_method("tagName", |_, this, ()| Ok(this.tag()));
        methods.add_method("nodeName", |_, this, ()| Ok(this.tag()));
        methods.add_method("normalName", |_, this, ()| Ok(this.tag()));
        // Jsoup's `html()` reads; `html(value)` replaces the children with that HTML.
        methods.add_function(
            "html",
            |lua, (this, value): (AnyUserData, Option<String>)| {
                let node = this.borrow::<LuaNode>()?.clone();
                match value {
                    Some(value) => {
                        node.set_html(&value);
                        Ok(Value::UserData(this))
                    }
                    None => Ok(Value::String(lua.create_string(node.inner_html())?)),
                }
            },
        );
        methods.add_method("outerHtml", |_, this, ()| Ok(this.outer_html()));
        methods.add_method("appendChild", |_, this, child: AnyUserData| {
            let child = child.borrow::<LuaNode>()?.clone();
            this.adopt(&child, false)?;
            Ok(this.clone())
        });
        methods.add_method("prependChild", |_, this, child: AnyUserData| {
            let child = child.borrow::<LuaNode>()?.clone();
            this.adopt(&child, true)?;
            Ok(this.clone())
        });
        methods.add_method("createElement", |_, this, tag: String| {
            this.create_element(&tag)
        });
        methods.add_method("parents", |_, this, ()| {
            // Nearest first, up to <html>: the document itself isn't an element.
            Ok(this.list(this.element_ancestors()))
        });
        methods.add_method("firstElementSibling", |_, this, ()| {
            Ok(this.end_sibling(false))
        });
        methods.add_method("lastElementSibling", |_, this, ()| {
            Ok(this.end_sibling(true))
        });
        methods.add_method("firstElementChild", |_, this, ()| {
            Ok(this.element_children().first().map(|id| this.node(*id)))
        });
        methods.add_method("lastElementChild", |_, this, ()| {
            Ok(this.element_children().last().map(|id| this.node(*id)))
        });
        methods.add_method("remove", |_, this, ()| {
            this.remove();
            Ok(())
        });
        for (name, place) in [
            ("before", Place::Before),
            ("after", Place::After),
            ("prepend", Place::Prepend),
            ("append", Place::Append),
        ] {
            methods.add_method(name, move |_, this, content: Value| {
                this.put(&content, place)?;
                Ok(this.clone())
            });
        }
        // Jsoup's appendElement/prependElement give back the new element; appendText/prependText
        // add escaped text (M46.9.1: scripts that build pages, e.g. ReadFromNet).
        methods.add_method("appendElement", |_, this, tag: String| {
            this.insert(&format!("<{0}></{0}>", element_tag(&tag)?), Place::Append);
            Ok(this.element_children().last().map(|id| this.node(*id)))
        });
        methods.add_method("prependElement", |_, this, tag: String| {
            this.insert(&format!("<{0}></{0}>", element_tag(&tag)?), Place::Prepend);
            Ok(this.element_children().first().map(|id| this.node(*id)))
        });
        methods.add_method("appendText", |_, this, text: String| {
            this.insert(&escape_text(&text), Place::Append);
            Ok(this.clone())
        });
        methods.add_method("prependText", |_, this, text: String| {
            this.insert(&escape_text(&text), Place::Prepend);
            Ok(this.clone())
        });
        methods.add_method("empty", |_, this, ()| {
            this.empty();
            Ok(this.clone())
        });
        methods.add_method("children", |_, this, ()| {
            Ok(this.list(this.element_children()))
        });
        methods.add_method("childrenSize", |_, this, ()| {
            Ok(this.element_children().len())
        });
        methods.add_method("child", |_, this, index: usize| {
            Ok(this.element_children().get(index).map(|id| this.node(*id)))
        });
        methods.add_method("childNodes", |_, this, ()| {
            Ok(this.list(this.with(|node| node.children().map(|child| child.id()).collect())))
        });
        methods.add_method("childNodeSize", |_, this, ()| {
            Ok(this.with(|node| node.children().count()))
        });
        methods.add_method("childNode", |_, this, index: usize| {
            Ok(this
                .with(|node| node.children().nth(index).map(|child| child.id()))
                .map(|id| this.node(id)))
        });
        methods.add_method("textNodes", |_, this, ()| {
            Ok(this.list(this.with(|node| {
                node.children()
                    .filter(|child| child.value().is_text())
                    .map(|child| child.id())
                    .collect()
            })))
        });
        methods.add_method("parent", |_, this, ()| {
            Ok(this
                .with(|node| {
                    node.parent()
                        .filter(|parent| parent.value().is_element())
                        .map(|parent| parent.id())
                })
                .map(|id| this.node(id)))
        });
        methods.add_method("nextElementSibling", |_, this, ()| {
            Ok(this.sibling_elements(true).first().map(|id| this.node(*id)))
        });
        methods.add_method("previousElementSibling", |_, this, ()| {
            Ok(this
                .sibling_elements(false)
                .first()
                .map(|id| this.node(*id)))
        });
        methods.add_method("nextElementSiblings", |_, this, ()| {
            Ok(this.list(this.sibling_elements(true)))
        });
        methods.add_method("previousElementSiblings", |_, this, ()| {
            Ok(this.list(this.sibling_elements(false)))
        });
        methods.add_method("siblingElements", |_, this, ()| {
            let mut ids = this.sibling_elements(false);
            ids.reverse();
            ids.extend(this.sibling_elements(true));
            Ok(this.list(ids))
        });
        methods.add_method("elementSiblingIndex", |_, this, ()| {
            Ok(this.sibling_elements(false).len())
        });
        methods.add_method("body", |_, this, ()| this.select_first("body"));
        methods.add_method("head", |_, this, ()| this.select_first("head"));
        methods.add_method("title", |_, this, ()| {
            Ok(this
                .select_first("title")?
                .map(|title| title.text())
                .unwrap_or_default())
        });
        methods.add_method("location", |_, this, ()| {
            Ok(this
                .doc
                .base
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default())
        });
        methods.add_method("traverse", |_, this, visitor: Table| {
            this.traverse(&visitor)?;
            Ok(this.clone())
        });
        methods.add_method("is", |_, this, css: String| {
            let selector = selector(&css)?;
            Ok(this.with(|node| selector.matches(node, node)))
        });
        methods.add_meta_method(MetaMethod::ToString, |_, this, ()| Ok(this.outer_html()));
        // The same node, however it was reached (Jsoup compares nodes by identity).
        methods.add_meta_method(MetaMethod::Eq, |_, this, other: AnyUserData| {
            Ok(other
                .borrow::<LuaNode>()
                .is_ok_and(|other| Rc::ptr_eq(&this.doc, &other.doc) && this.id == other.id))
        });
    }
}

impl LuaElements {
    fn nodes(&self) -> impl Iterator<Item = LuaNode> + '_ {
        self.ids.iter().map(|id| LuaNode {
            doc: self.doc.clone(),
            id: *id,
        })
    }

    fn at(&self, index: usize) -> Option<LuaNode> {
        self.ids.get(index).map(|id| LuaNode {
            doc: self.doc.clone(),
            id: *id,
        })
    }

    fn with_ids(&self, ids: Vec<NodeId>) -> Self {
        Self {
            doc: self.doc.clone(),
            ids,
        }
    }

    /// Ids gathered from each node, each once, in order.
    fn gather(&self, mut each: impl FnMut(&LuaNode) -> Vec<NodeId>) -> Self {
        let mut seen = std::collections::HashSet::new();
        let mut ids = Vec::new();
        for node in self.nodes() {
            for id in each(&node) {
                if seen.insert(id) {
                    ids.push(id);
                }
            }
        }
        self.with_ids(ids)
    }
}

impl UserData for LuaElements {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("size", |_, this, ()| Ok(this.ids.len()));
        methods.add_method("isEmpty", |_, this, ()| Ok(this.ids.is_empty()));
        methods.add_method("get", |_, this, index: usize| Ok(this.at(index)));
        methods.add_method("first", |_, this, ()| Ok(this.at(0)));
        methods.add_method("last", |_, this, ()| {
            Ok(this.ids.len().checked_sub(1).and_then(|last| this.at(last)))
        });
        methods.add_method("eq", |_, this, index: usize| {
            Ok(this.with_ids(this.ids.get(index).copied().into_iter().collect()))
        });
        methods.add_method("is", |_, this, css: String| {
            let selector = selector(&css)?;
            Ok(this
                .nodes()
                .any(|node| node.with(|inner| selector.matches(inner, inner))))
        });
        methods.add_method("parents", |_, this, ()| {
            Ok(this.gather(LuaNode::element_ancestors))
        });
        methods.add_method("next", |_, this, ()| {
            Ok(this.gather(|node| node.sibling_elements(true).into_iter().take(1).collect()))
        });
        methods.add_method("prev", |_, this, ()| {
            Ok(this.gather(|node| node.sibling_elements(false).into_iter().take(1).collect()))
        });
        for (name, place) in [
            ("before", Place::Before),
            ("after", Place::After),
            ("prepend", Place::Prepend),
            ("append", Place::Append),
        ] {
            // Elements take HTML only (a node reads as its HTML, as LuaJ makes it a string).
            methods.add_method(name, move |_, this, content: Value| {
                let html = match &content {
                    Value::UserData(data) => match data.borrow::<LuaNode>() {
                        Ok(node) => node.outer_html(),
                        Err(_) => data
                            .borrow::<LuaElements>()?
                            .nodes()
                            .map(|node| node.outer_html())
                            .collect::<Vec<_>>()
                            .join("\n"),
                    },
                    Value::String(text) => text.to_str()?.to_owned(),
                    Value::Integer(number) => number.to_string(),
                    Value::Number(number) => number.to_string(),
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "expected HTML, got {}",
                            other.type_name()
                        )));
                    }
                };
                for node in this.nodes() {
                    node.insert(&html, place);
                }
                Ok(this.clone())
            });
        }
        methods.add_method("empty", |_, this, ()| {
            for node in this.nodes() {
                node.empty();
            }
            Ok(this.clone())
        });
        for (name, add) in [
            ("addClass", Some(true)),
            ("removeClass", Some(false)),
            ("toggleClass", None),
        ] {
            methods.add_method(name, move |_, this, class: String| {
                for node in this.nodes() {
                    node.change_class(&class, add);
                }
                Ok(this.clone())
            });
        }
        methods.add_method("text", |_, this, ()| {
            Ok(this
                .nodes()
                .map(|node| node.text())
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join(" "))
        });
        methods.add_method("eachText", |lua, this, ()| {
            lua.create_sequence_from(
                this.nodes()
                    .map(|node| node.text())
                    .filter(|text| !text.is_empty()),
            )
        });
        // Jsoup: the attribute of the first element that has it; with a value, sets it on all.
        methods.add_function(
            "attr",
            |lua, (this, name, value): (AnyUserData, String, Option<String>)| {
                let elements = this.borrow::<LuaElements>()?.clone();
                match value {
                    Some(value) => {
                        for node in elements.nodes() {
                            node.set_attr(&name, &value);
                        }
                        Ok(Value::UserData(this))
                    }
                    None => {
                        let found = elements
                            .nodes()
                            .find(|node| node.has_attr(name.strip_prefix("abs:").unwrap_or(&name)))
                            .map(|node| node.attr(&name))
                            .unwrap_or_default();
                        Ok(Value::String(lua.create_string(found)?))
                    }
                }
            },
        );
        methods.add_method("eachAttr", |lua, this, name: String| {
            lua.create_sequence_from(
                this.nodes()
                    .map(|node| node.attr(&name))
                    .filter(|value| !value.is_empty()),
            )
        });
        methods.add_method("hasAttr", |_, this, name: String| {
            Ok(this.nodes().any(|node| node.has_attr(&name)))
        });
        methods.add_function(
            "html",
            |lua, (this, value): (AnyUserData, Option<String>)| {
                let elements = this.borrow::<LuaElements>()?.clone();
                match value {
                    Some(value) => {
                        for node in elements.nodes() {
                            node.set_html(&value);
                        }
                        Ok(Value::UserData(this))
                    }
                    None => Ok(Value::String(
                        lua.create_string(
                            elements
                                .nodes()
                                .map(|node| node.inner_html())
                                .collect::<Vec<_>>()
                                .join("\n"),
                        )?,
                    )),
                }
            },
        );
        methods.add_method("outerHtml", |_, this, ()| {
            Ok(this
                .nodes()
                .map(|node| node.outer_html())
                .collect::<Vec<_>>()
                .join("\n"))
        });
        methods.add_method("select", |_, this, css: String| {
            let selector = selector(&css)?;
            let html = this.doc.html.borrow();
            let mut seen = std::collections::HashSet::new();
            let mut ids = Vec::new();
            for id in &this.ids {
                if let Some(node) = html.tree.get(*id) {
                    for found in selector.select(node) {
                        if seen.insert(found) {
                            ids.push(found);
                        }
                    }
                }
            }
            Ok(this.with_ids(ids))
        });
        methods.add_method("selectFirst", |_, this, css: String| {
            for node in this.nodes() {
                if let Some(found) = node.select_first(&css)? {
                    return Ok(Some(found));
                }
            }
            Ok(None)
        });
        methods.add_method("not", |_, this, css: String| {
            let selector = selector(&css)?;
            let html = this.doc.html.borrow();
            let ids = this
                .ids
                .iter()
                .copied()
                .filter(|id| {
                    html.tree
                        .get(*id)
                        .is_some_and(|node| !selector.matches(node, node))
                })
                .collect();
            Ok(LuaElements {
                doc: this.doc.clone(),
                ids,
            })
        });
        methods.add_method("remove", |_, this, ()| {
            for node in this.nodes() {
                node.remove();
            }
            Ok(this.clone())
        });
        methods.add_method("removeAttr", |_, this, name: String| {
            for node in this.nodes() {
                node.remove_attr(&name);
            }
            Ok(this.clone())
        });
        methods.add_meta_method(MetaMethod::Len, |_, this, ()| Ok(this.ids.len()));
        methods.add_meta_method(MetaMethod::ToString, |_, this, ()| {
            Ok(this
                .nodes()
                .map(|node| node.outer_html())
                .collect::<Vec<_>>()
                .join("\n"))
        });
    }
}

/// `Document(html)`: a document parsed from a string.
pub fn install(lua: &Lua, globals: &Table) -> mlua::Result<()> {
    globals.set(
        "Document",
        lua.create_function(|_, html: String| Ok(parse_document(&html, None)))?,
    )?;
    globals.set(
        "_PageOfElem",
        lua.create_function(
            |_,
             (node, _remove_style, custom_style, _keep_scripts): (
                mlua::AnyUserData,
                Option<bool>,
                Option<String>,
                Option<bool>,
            )| {
                let html = match node.borrow::<LuaNode>() {
                    Ok(node) => node.outer_html(),
                    Err(_) => node
                        .borrow::<LuaElements>()?
                        .nodes()
                        .map(|node| node.outer_html())
                        .collect(),
                };
                let style = custom_style
                    .filter(|style| !style.is_empty())
                    .map(|style| format!("<style type=\"text/css\">{style}</style>"))
                    .unwrap_or_default();
                Ok(format!(
                    "<!DOCTYPE html><html><head>{style}</head><body>{html}</body></html>"
                ))
            },
        )?,
    )?;
    Ok(())
}
