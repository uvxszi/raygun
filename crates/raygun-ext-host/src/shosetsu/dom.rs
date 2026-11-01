//! Jsoup-like documents for Shosetsu scripts (M32): `Document`, `Element`, `Elements` and text
//! nodes as Lua userdata over one shared, mutable `scraper` tree. Method names and behaviour
//! follow Jsoup, which Shosetsu's kotlin-lib (GPL-3.0) hands to scripts through LuaJ.

use std::cell::RefCell;
use std::rc::Rc;

use ego_tree::{NodeId, NodeRef, Tree};
use mlua::{Function, Lua, MetaMethod, Table, UserData, UserDataMethods, Value};
use scraper::{ElementRef, Html, Node};

use super::select::{Selector, element_text, normalize, own_text};

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

#[derive(Clone, Copy)]
enum Place {
    Before,
    After,
    Prepend,
    Append,
}

/// Copies `source` (from another tree) into `tree` at `place` relative to `anchor`.
fn copy(tree: &mut Tree<Node>, source: NodeRef<'_, Node>, anchor: NodeId, place: Place) {
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
            this.select(&format!("[class~={class:?}]"))
        });
        methods.add_method("text", |_, this, ()| Ok(this.text()));
        methods.add_method("ownText", |_, this, ()| {
            Ok(this.with(|node| match node.value() {
                Node::Text(text) => normalize(text),
                _ => own_text(node),
            }))
        });
        for name in ["wholeText", "getWholeText", "data"] {
            methods.add_method(name, |_, this, ()| {
                Ok(this.with(|node| match node.value() {
                    Node::Text(text) => text.to_string(),
                    _ => node
                        .descendants()
                        .filter_map(|child| child.value().as_text().map(|text| text.to_string()))
                        .collect(),
                }))
            });
        }
        methods.add_method("isBlank", |_, this, ()| Ok(this.text().is_empty()));
        methods.add_method("hasText", |_, this, ()| Ok(!this.text().is_empty()));
        // Jsoup's attr(name, value) sets; this tree's attributes are read-only, so it's ignored.
        methods.add_method("attr", |_, this, (name, value): (String, Option<Value>)| {
            Ok(value.is_none().then(|| this.attr(&name)))
        });
        methods.add_method("absUrl", |_, this, name: String| Ok(this.abs_url(&name)));
        methods.add_method("hasAttr", |_, this, name: String| Ok(this.has_attr(&name)));
        methods.add_method("removeAttr", |_, this, _name: String| Ok(this.clone()));
        methods.add_method("id", |_, this, ()| Ok(this.attr("id")));
        methods.add_method("className", |_, this, ()| Ok(this.attr("class")));
        methods.add_method("hasClass", |_, this, class: String| {
            Ok(this
                .attr("class")
                .split_whitespace()
                .any(|candidate| candidate.eq_ignore_ascii_case(&class)))
        });
        methods.add_method("tagName", |_, this, ()| Ok(this.tag()));
        methods.add_method("nodeName", |_, this, ()| Ok(this.tag()));
        methods.add_method("normalName", |_, this, ()| Ok(this.tag()));
        methods.add_method("html", |_, this, ()| Ok(this.inner_html()));
        methods.add_method("outerHtml", |_, this, ()| Ok(this.outer_html()));
        methods.add_method("remove", |_, this, ()| {
            this.remove();
            Ok(())
        });
        methods.add_method("before", |_, this, html: String| {
            this.insert(&html, Place::Before);
            Ok(this.clone())
        });
        methods.add_method("after", |_, this, html: String| {
            this.insert(&html, Place::After);
            Ok(this.clone())
        });
        methods.add_method("prepend", |_, this, html: String| {
            this.insert(&html, Place::Prepend);
            Ok(this.clone())
        });
        methods.add_method("append", |_, this, html: String| {
            this.insert(&html, Place::Append);
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
    }
}

impl LuaElements {
    fn nodes(&self) -> impl Iterator<Item = LuaNode> + '_ {
        self.ids.iter().map(|id| LuaNode {
            doc: self.doc.clone(),
            id: *id,
        })
    }
}

impl UserData for LuaElements {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("size", |_, this, ()| Ok(this.ids.len()));
        methods.add_method("isEmpty", |_, this, ()| Ok(this.ids.is_empty()));
        methods.add_method("get", |_, this, index: usize| Ok(this.nodes().nth(index)));
        methods.add_method("first", |_, this, ()| Ok(this.nodes().next()));
        methods.add_method("last", |_, this, ()| Ok(this.nodes().last()));
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
        methods.add_method("attr", |_, this, name: String| {
            Ok(this
                .nodes()
                .map(|node| node.attr(&name))
                .find(|value| !value.is_empty())
                .unwrap_or_default())
        });
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
        methods.add_method("html", |_, this, ()| {
            Ok(this
                .nodes()
                .map(|node| node.inner_html())
                .collect::<Vec<_>>()
                .join("\n"))
        });
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
            let mut ids = Vec::new();
            for id in &this.ids {
                if let Some(node) = html.tree.get(*id) {
                    for found in selector.select(node) {
                        if !ids.contains(&found) {
                            ids.push(found);
                        }
                    }
                }
            }
            Ok(LuaElements {
                doc: this.doc.clone(),
                ids,
            })
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
        methods.add_method("removeAttr", |_, this, _name: String| Ok(this.clone()));
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
