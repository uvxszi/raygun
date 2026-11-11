//! A document's outline (its bookmarks): titles with the page each leads to, read from the
//! catalog's `/Outlines` tree. Destinations may be explicit (`[page /XYZ …]`), named (looked up in
//! the `/Names /Dests` tree or the older `/Dests` dictionary) or a GoTo action's.

use std::collections::{HashMap, HashSet};

use hayro::hayro_syntax::Pdf;
use hayro::hayro_syntax::object::{Array, Dict, MaybeRef, Name, Object, ObjectIdentifier};

/// Bookmarks read at most, and how deep.
const MAX_ENTRIES: usize = 5_000;
const MAX_LEVEL: u32 = 8;
/// How deep a name tree is followed.
const MAX_TREE_DEPTH: u32 = 16;

/// One bookmark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineEntry {
    pub title: String,
    /// The page it leads to, from 0.
    pub page: usize,
    /// 0 for the top level.
    pub level: u32,
}

struct Reader<'a> {
    catalog: Dict<'a>,
    pages: HashMap<ObjectIdentifier, usize>,
    page_count: usize,
    seen: HashSet<ObjectIdentifier>,
    entries: Vec<OutlineEntry>,
}

pub(super) fn read(pdf: &Pdf) -> Vec<OutlineEntry> {
    let xref = pdf.xref();
    let Some(catalog) = xref.get::<Dict<'_>>(xref.root_id()) else {
        return Vec::new();
    };
    let Some(root) = catalog.get::<Dict<'_>>(b"Outlines") else {
        return Vec::new();
    };
    let pages = pdf
        .pages()
        .iter()
        .enumerate()
        .filter_map(|(index, page)| Some((page.raw().obj_id()?, index)))
        .collect();
    let mut reader = Reader {
        catalog,
        pages,
        page_count: pdf.pages().len(),
        seen: HashSet::new(),
        entries: Vec::new(),
    };
    reader.items(root.get::<Dict<'_>>(b"First"), 0);
    reader.entries
}

impl<'a> Reader<'a> {
    /// A run of sibling bookmarks from `first`, each followed by its children.
    fn items(&mut self, first: Option<Dict<'a>>, level: u32) {
        let mut item = first;
        while let Some(dict) = item {
            if self.entries.len() >= MAX_ENTRIES {
                return;
            }
            // A loop in the tree (a broken file) ends it.
            if let Some(id) = dict.obj_id()
                && !self.seen.insert(id)
            {
                return;
            }
            let title = dict
                .get::<hayro::hayro_syntax::object::String<'_>>(b"Title")
                .map(|title| super::text_string(title.as_bytes()))
                .map(|title| title.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|title| !title.is_empty());
            if let (Some(title), Some(page)) = (title, self.destination_of(&dict)) {
                self.entries.push(OutlineEntry { title, page, level });
            }
            if level < MAX_LEVEL {
                self.items(dict.get::<Dict<'_>>(b"First"), level + 1);
            }
            item = dict.get::<Dict<'_>>(b"Next");
        }
    }

    /// The page a bookmark leads to: its `/Dest`, or its GoTo action's `/D`.
    fn destination_of(&self, item: &Dict<'a>) -> Option<usize> {
        self.destination(item, b"Dest").or_else(|| {
            let action = item.get::<Dict<'_>>(b"A")?;
            let goto = action
                .get::<Name<'_>>(b"S")
                .is_some_and(|kind| kind.as_ref() == b"GoTo");
            goto.then(|| self.destination(&action, b"D")).flatten()
        })
    }

    /// The destination under `key`: an array, or a name or string to look up.
    fn destination(&self, dict: &Dict<'a>, key: &[u8]) -> Option<usize> {
        if let Some(array) = dict.get::<Array<'_>>(key) {
            return self.page_of(&array);
        }
        let name: Vec<u8> = match dict.get::<Name<'_>>(key) {
            Some(name) => name.as_ref().to_vec(),
            None => dict
                .get::<hayro::hayro_syntax::object::String<'_>>(key)?
                .as_bytes()
                .to_vec(),
        };
        self.named(&name)
    }

    /// The page of an explicit destination: its first item, a page or (in other documents'
    /// destinations) a page number.
    fn page_of(&self, array: &Array<'a>) -> Option<usize> {
        match array.raw_iter().next()? {
            MaybeRef::Ref(reference) => self.pages.get(&reference.into()).copied(),
            MaybeRef::NotRef(Object::Number(number)) => {
                let index = number.as_f64();
                (index >= 0.0 && (index as usize) < self.page_count).then_some(index as usize)
            }
            MaybeRef::NotRef(Object::Dict(page)) => {
                page.obj_id().and_then(|id| self.pages.get(&id).copied())
            }
            MaybeRef::NotRef(_) => None,
        }
    }

    /// A named destination: in the `/Names /Dests` tree, else the catalog's `/Dests`.
    fn named(&self, name: &[u8]) -> Option<usize> {
        let in_tree = self
            .catalog
            .get::<Dict<'_>>(b"Names")
            .and_then(|names| names.get::<Dict<'_>>(b"Dests"))
            .and_then(|tree| find_in_tree(&tree, name, 0));
        let value = in_tree.or_else(|| {
            self.catalog
                .get::<Dict<'_>>(b"Dests")?
                .get::<Object<'_>>(name)
        })?;
        match value {
            Object::Array(array) => self.page_of(&array),
            Object::Dict(dict) => dict
                .get::<Array<'_>>(b"D")
                .and_then(|array| self.page_of(&array)),
            _ => None,
        }
    }
}

/// `name`'s value in a name tree.
fn find_in_tree<'a>(node: &Dict<'a>, name: &[u8], depth: u32) -> Option<Object<'a>> {
    if depth > MAX_TREE_DEPTH {
        return None;
    }
    if let Some(names) = node.get::<Array<'_>>(b"Names") {
        let mut items = names.iter::<Object<'_>>();
        while let (Some(key), Some(value)) = (items.next(), items.next()) {
            if let Object::String(key) = key
                && key.as_bytes() == name
            {
                return Some(value);
            }
        }
    }
    let kids = node.get::<Array<'_>>(b"Kids")?;
    for kid in kids.iter::<Dict<'_>>() {
        // Each kid says which names it holds; others are skipped.
        if let Some(limits) = kid.get::<Array<'_>>(b"Limits") {
            let bounds: Vec<Vec<u8>> = limits
                .iter::<hayro::hayro_syntax::object::String<'_>>()
                .map(|bound| bound.as_bytes().to_vec())
                .collect();
            if let [low, high] = bounds.as_slice()
                && (name < low.as_slice() || name > high.as_slice())
            {
                continue;
            }
        }
        if let Some(found) = find_in_tree(&kid, name, depth + 1) {
            return Some(found);
        }
    }
    None
}
