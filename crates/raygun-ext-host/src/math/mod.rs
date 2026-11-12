//! Formulas in book text. Sources write them as MathML (EPUB 3, MediaWiki, KaTeX's and MathJax's
//! accessible copies) or TeX (`$…$`, `$$…$$`, `\(…\)`, `\[…\]`, MathJax's `math/tex` scripts,
//! `<span class="math">`). Both become a [`Formula`]: MathML the webview draws itself (MathML
//! Core, in Chromium since 109), TeX converted by `pulldown-latex` first. What reaches the app is
//! rebuilt from an allowlist of MathML elements and attributes, so no source markup gets through:
//! no scripts, links, event handlers or styles beyond a few safe properties. Each formula also
//! carries its text (the TeX, or a linear form such as `x² + 1`) and its words for reading aloud.

mod speech;
#[cfg(test)]
mod tests;

use ego_tree::NodeRef;
use scraper::Node;
use serde::{Deserialize, Serialize};

/// The largest formula kept, as MathML; a bigger one is left as its text.
const MAX_MATHML_BYTES: usize = 64 * 1024;
/// How deep a formula's elements may nest.
const MAX_DEPTH: u32 = 64;
const MATHML_NS: &str = "http://www.w3.org/1998/Math/MathML";

/// A formula.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Formula {
    /// `<math>…</math>` with MathML elements and attributes only.
    pub mathml: String,
    /// The formula as text: its TeX, or a linear form of its MathML.
    pub text: String,
    /// The formula in words, for reading aloud.
    pub speech: String,
    /// Set on a line of its own.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub display: bool,
}

/// A MathML element or text, rebuilt from the allowlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MathNode {
    Element {
        name: &'static str,
        attributes: Vec<(&'static str, String)>,
        children: Vec<MathNode>,
    },
    Text(String),
}

impl MathNode {
    fn element(name: &'static str, children: Vec<MathNode>) -> Self {
        Self::Element {
            name,
            attributes: Vec::new(),
            children,
        }
    }

    pub(crate) fn name(&self) -> Option<&'static str> {
        match self {
            Self::Element { name, .. } => Some(name),
            Self::Text(_) => None,
        }
    }

    pub(crate) fn children(&self) -> &[MathNode] {
        match self {
            Self::Element { children, .. } => children,
            Self::Text(_) => &[],
        }
    }

    pub(crate) fn attribute(&self, wanted: &str) -> Option<&str> {
        match self {
            Self::Element { attributes, .. } => attributes
                .iter()
                .find(|(name, _)| *name == wanted)
                .map(|(_, value)| value.as_str()),
            Self::Text(_) => None,
        }
    }

    /// The text inside, all of it.
    pub(crate) fn text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Element { children, .. } => children.iter().map(Self::text).collect(),
        }
    }

    /// The element children only.
    pub(crate) fn elements(&self) -> impl Iterator<Item = &MathNode> {
        self.children()
            .iter()
            .filter(|child| matches!(child, Self::Element { .. }))
    }
}

/// Elements kept; others give way to what they hold.
const ELEMENTS: &[&str] = &[
    "math",
    "semantics",
    "annotation",
    "mrow",
    "mi",
    "mn",
    "mo",
    "ms",
    "mtext",
    "mspace",
    "mfrac",
    "msqrt",
    "mroot",
    "msub",
    "msup",
    "msubsup",
    "munder",
    "mover",
    "munderover",
    "mmultiscripts",
    "mprescripts",
    "none",
    "mtable",
    "mtr",
    "mtd",
    "mstyle",
    "mpadded",
    "mphantom",
    "merror",
    "menclose",
];

/// Elements whose text is drawn.
const TOKENS: &[&str] = &["mi", "mn", "mo", "ms", "mtext", "annotation"];

/// Attributes kept on any element, with what their values may be.
const ATTRIBUTES: &[&str] = &[
    "display",
    "displaystyle",
    "scriptlevel",
    "mathvariant",
    "mathsize",
    "mathcolor",
    "mathbackground",
    "dir",
    "form",
    "fence",
    "separator",
    "stretchy",
    "symmetric",
    "largeop",
    "movablelimits",
    "accent",
    "accentunder",
    "lspace",
    "rspace",
    "minsize",
    "maxsize",
    "linethickness",
    "width",
    "height",
    "depth",
    "voffset",
    "notation",
    "columnalign",
    "rowalign",
    "columnspacing",
    "rowspacing",
    "columnlines",
    "rowlines",
    "columnspan",
    "rowspan",
    "frame",
    "framespacing",
    "encoding",
    "class",
    "style",
];

/// CSS properties a formula's `style` may set (the TeX converter's colours and spacing).
const STYLE_PROPERTIES: &[&str] = &[
    "color",
    "background-color",
    "border-color",
    "margin-left",
    "margin-right",
    "padding-left",
    "padding-right",
    "height",
    "width",
];

/// An allowed attribute's value, checked: words, numbers, lengths and colours only.
fn allowed_value(name: &str, value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() > 200 {
        return None;
    }
    match name {
        "class" => {
            let ours = |token: &str| {
                token == "small"
                    || ["menv-", "cell-", "mop-"].iter().any(|prefix| {
                        token.strip_prefix(prefix).is_some_and(|rest| {
                            !rest.is_empty()
                                && rest
                                    .chars()
                                    .all(|char| char.is_ascii_lowercase() || char == '-')
                        })
                    })
            };
            let kept: Vec<&str> = value
                .split_whitespace()
                .filter(|token| ours(token))
                .collect();
            (!kept.is_empty()).then(|| kept.join(" "))
        }
        "style" => {
            let kept: Vec<String> = value
                .split(';')
                .filter_map(|declaration| {
                    let (property, setting) = declaration.split_once(':')?;
                    let property = property.trim().to_ascii_lowercase();
                    let setting = setting.trim();
                    let plain = !setting.is_empty()
                        && setting
                            .chars()
                            .all(|char| char.is_ascii_alphanumeric() || " .,#%()-+".contains(char))
                        && !setting.to_ascii_lowercase().contains("url")
                        && !setting.to_ascii_lowercase().contains("var");
                    (STYLE_PROPERTIES.contains(&property.as_str()) && plain)
                        .then(|| format!("{property}: {setting}"))
                })
                .collect();
            (!kept.is_empty()).then(|| kept.join("; "))
        }
        "encoding" => value
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || "/-+.".contains(char))
            .then(|| value.to_owned()),
        _ => value
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || " .,#%+-".contains(char))
            .then(|| value.to_owned()),
    }
}

/// An element's name without a namespace prefix (`m:math` in EPUB 2), in lower case.
fn local_name(name: &str) -> String {
    name.rsplit(':').next().unwrap_or(name).to_ascii_lowercase()
}

fn interned(name: &str) -> Option<&'static str> {
    ELEMENTS.iter().copied().find(|known| *known == name)
}

/// Whether `node` is a `<math>` element (any prefix).
pub fn is_math(node: NodeRef<'_, Node>) -> bool {
    matches!(node.value(), Node::Element(element) if local_name(element.name()) == "math")
}

impl Formula {
    /// A formula from a `<math>` element of a parsed page: MathML, or TeX written inside it
    /// (MediaWiki's `<math>x^2</math>`).
    pub fn from_element(math: NodeRef<'_, Node>) -> Option<Self> {
        let Node::Element(element) = math.value() else {
            return None;
        };
        let display =
            element.attr("display") == Some("block") || element.attr("mode") == Some("display");
        let has_elements = math
            .children()
            .any(|child| matches!(child.value(), Node::Element(_)));
        if !has_elements {
            let tex: String = math
                .descendants()
                .filter_map(|node| node.value().as_text().map(|text| String::from(&**text)))
                .collect();
            return Self::from_tex(&tex, display);
        }
        let mut children = Vec::new();
        rebuild_children(math, &mut children, 0);
        Self::from_tree(children, display)
    }

    /// A formula from MathML markup (`<script type="math/mml">`).
    pub fn from_markup(markup: &str) -> Option<Self> {
        let document = scraper::Html::parse_fragment(markup);
        let math = document
            .tree
            .root()
            .descendants()
            .find(|node| is_math(*node))?;
        Self::from_element(math)
    }

    /// A formula from TeX; `None` when it isn't TeX the converter reads.
    pub fn from_tex(tex: &str, display: bool) -> Option<Self> {
        let tex = tex.trim();
        if tex.is_empty() || tex.len() > MAX_MATHML_BYTES / 4 {
            return None;
        }
        // MediaWiki writes `{\displaystyle …}` around its formulas.
        let tex = strip_displaystyle(tex);
        let storage = pulldown_latex::Storage::new();
        let events: Vec<_> = pulldown_latex::Parser::new(tex, &storage).collect();
        if events.iter().any(Result::is_err) {
            return None;
        }
        let config = pulldown_latex::RenderConfig {
            display_mode: if display {
                pulldown_latex::config::DisplayMode::Block
            } else {
                pulldown_latex::config::DisplayMode::Inline
            },
            ..pulldown_latex::RenderConfig::default()
        };
        let mut markup = String::new();
        pulldown_latex::push_mathml(&mut markup, events.into_iter(), config).ok()?;
        // pulldown-latex writes the spaces of `\ `, `~` and `\text{ }` as the text "&nbsp;",
        // which its writer escapes again: read them as the no-break spaces they are.
        let markup = markup.replace("&amp;nbsp;", "&#160;");
        let mut formula = Self::from_markup(&markup)?;
        formula.display = display;
        formula.text = tex.split_whitespace().collect::<Vec<_>>().join(" ");
        Some(formula)
    }

    /// A formula from rebuilt children of `<math>`.
    fn from_tree(children: Vec<MathNode>, display: bool) -> Option<Self> {
        // A semantics wrapper's TeX annotation is the formula's text when there is one.
        let tex = tex_annotation(&children);
        let mut attributes = vec![("xmlns", MATHML_NS.to_owned())];
        if display {
            attributes.push(("display", "block".to_owned()));
        }
        let root = MathNode::Element {
            name: "math",
            attributes,
            children,
        };
        let text = tex
            .map(|tex| strip_displaystyle(&tex).to_owned())
            .unwrap_or_else(|| speech::linear(&root));
        let speech = speech::words(&root);
        if text.trim().is_empty() && speech.trim().is_empty() {
            return None;
        }
        let mut mathml = String::new();
        write(&root, &mut mathml);
        if mathml.len() > MAX_MATHML_BYTES {
            return None;
        }
        Some(Self {
            mathml,
            text: text.split_whitespace().collect::<Vec<_>>().join(" "),
            speech,
            display,
        })
    }
}

/// `{\displaystyle x}` → `x`.
fn strip_displaystyle(tex: &str) -> &str {
    tex.strip_prefix("{\\displaystyle")
        .or_else(|| tex.strip_prefix("{\\textstyle"))
        .and_then(|rest| rest.strip_suffix('}'))
        .map_or(tex, str::trim)
}

fn tex_annotation(children: &[MathNode]) -> Option<String> {
    children.iter().find_map(|child| match child {
        MathNode::Element {
            name: "semantics",
            children,
            ..
        } => children.iter().find_map(|inner| {
            let tex = inner.name() == Some("annotation")
                && inner
                    .attribute("encoding")
                    .is_some_and(|encoding| encoding.contains("tex"));
            tex.then(|| inner.text().trim().to_owned())
                .filter(|text| !text.is_empty())
        }),
        _ => None,
    })
}

/// The allowed contents of `node`, appended to `out`.
fn rebuild_children(node: NodeRef<'_, Node>, out: &mut Vec<MathNode>, depth: u32) {
    if depth > MAX_DEPTH {
        return;
    }
    let token = match node.value() {
        Node::Element(element) => TOKENS.contains(&local_name(element.name()).as_str()),
        _ => false,
    };
    for child in node.children() {
        match child.value() {
            Node::Text(text) => {
                if token {
                    out.push(MathNode::Text(String::from(&**text)));
                } else if !text.trim().is_empty() {
                    // Words loose in a row are drawn as text.
                    out.push(MathNode::element(
                        "mtext",
                        vec![MathNode::Text(text.trim().to_owned())],
                    ));
                }
            }
            Node::Element(element) => {
                let name = local_name(element.name());
                match name.as_str() {
                    "script" | "style" | "annotation-xml" => {}
                    "mfenced" => out.push(fenced(child, element, depth)),
                    "maction" => {
                        // The first of its choices, as shown before any action.
                        if let Some(first) = child
                            .children()
                            .find(|inner| matches!(inner.value(), Node::Element(_)))
                        {
                            let mut kept = Vec::new();
                            rebuild_one(first, &mut kept, depth + 1);
                            out.extend(kept);
                        }
                    }
                    "mlabeledtr" => {
                        // A numbered row: the number (its first cell) goes.
                        let mut cells = Vec::new();
                        for cell in child
                            .children()
                            .filter(|inner| matches!(inner.value(), Node::Element(_)))
                            .skip(1)
                        {
                            rebuild_one(cell, &mut cells, depth + 1);
                        }
                        out.push(MathNode::element("mtr", cells));
                    }
                    _ => rebuild_one(child, out, depth),
                }
            }
            _ => {}
        }
    }
}

/// One element: kept with its allowed attributes, or replaced by what it holds.
fn rebuild_one(node: NodeRef<'_, Node>, out: &mut Vec<MathNode>, depth: u32) {
    let Node::Element(element) = node.value() else {
        if let Node::Text(text) = node.value()
            && !text.trim().is_empty()
        {
            out.push(MathNode::Text(String::from(&**text)));
        }
        return;
    };
    let name = local_name(element.name());
    let Some(name) = interned(&name) else {
        if !matches!(name.as_str(), "script" | "style" | "annotation-xml") {
            rebuild_children(node, out, depth + 1);
        }
        return;
    };
    let attributes = element
        .attrs()
        .filter_map(|(attribute, value)| {
            let attribute = local_name(attribute);
            let known = ATTRIBUTES
                .iter()
                .copied()
                .find(|known| *known == attribute)?;
            Some((known, allowed_value(known, value)?))
        })
        .collect();
    let mut children = Vec::new();
    rebuild_children(node, &mut children, depth + 1);
    out.push(MathNode::Element {
        name,
        attributes,
        children,
    });
}

/// `<mfenced open="(" close=")" separators=",">a b</mfenced>` (not in MathML Core) as the row
/// it stands for: `(a,b)`.
fn fenced(node: NodeRef<'_, Node>, element: &scraper::node::Element, depth: u32) -> MathNode {
    let open = element.attr("open").unwrap_or("(");
    let close = element.attr("close").unwrap_or(")");
    let separators: Vec<char> = element
        .attr("separators")
        .unwrap_or(",")
        .chars()
        .filter(|char| !char.is_whitespace())
        .collect();
    let mut items = Vec::new();
    for child in node
        .children()
        .filter(|inner| matches!(inner.value(), Node::Element(_)))
    {
        rebuild_one(child, &mut items, depth + 1);
    }
    let operator = |text: &str| MathNode::Element {
        name: "mo",
        attributes: vec![("fence", "true".to_owned())],
        children: vec![MathNode::Text(text.to_owned())],
    };
    let mut row = Vec::new();
    if !open.is_empty() {
        row.push(operator(open));
    }
    let count = items.len();
    for (index, item) in items.into_iter().enumerate() {
        row.push(item);
        if index + 1 < count
            && let Some(separator) = separators.get(index).or(separators.last()).copied()
        {
            row.push(MathNode::element(
                "mo",
                vec![MathNode::Text(separator.to_string())],
            ));
        }
    }
    if !close.is_empty() {
        row.push(operator(close));
    }
    MathNode::element("mrow", row)
}

/// MathML markup of a rebuilt tree, well-formed as XML (an EPUB holds it as it is).
fn write(node: &MathNode, out: &mut String) {
    match node {
        MathNode::Text(text) => escape(text, out),
        MathNode::Element {
            name,
            attributes,
            children,
        } => {
            out.push('<');
            out.push_str(name);
            for (attribute, value) in attributes {
                out.push(' ');
                out.push_str(attribute);
                out.push_str("=\"");
                escape(value, out);
                out.push('"');
            }
            out.push('>');
            for child in children {
                write(child, out);
            }
            out.push_str("</");
            out.push_str(name);
            out.push('>');
        }
    }
}

fn escape(text: &str, out: &mut String) {
    for char in text.chars() {
        match char {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            char if char.is_control() && char != '\n' && char != '\t' => {}
            char => out.push(char),
        }
    }
}

/// Raised text as plain text: in superscript characters when it has them all (`x²`), else as it is.
pub fn raised_text(text: &str) -> String {
    speech::superscript(text.trim()).unwrap_or_else(|| text.to_owned())
}

/// Lowered text as plain text: in subscript characters when it has them all (`H₂O`), else as it is.
pub fn lowered_text(text: &str) -> String {
    speech::subscript(text.trim()).unwrap_or_else(|| text.to_owned())
}

/// A piece of running text: words, or TeX between delimiters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextPiece<'a> {
    Text(&'a str),
    Tex {
        tex: &'a str,
        display: bool,
        /// The TeX with its delimiters, as written.
        source: &'a str,
    },
}

/// Running text split at TeX delimiters: `$$…$$` and `\[…\]` on lines of their own, `\(…\)`
/// and `$…$` in the line. A single `$` follows Pandoc's rule so prices stay words: the opening
/// one has no space after it, the closing one none before it and no digit after it, and what's
/// between looks like math.
pub fn split_tex(text: &str) -> Vec<TextPiece<'_>> {
    let mut pieces = Vec::new();
    let mut plain_from = 0;
    let mut at = 0;
    let bytes = text.as_bytes();
    while at < bytes.len() {
        // An escaped dollar is a dollar.
        if bytes[at] == b'\\' && bytes.get(at + 1) == Some(&b'$') {
            at += 2;
            continue;
        }
        let found = match bytes[at] {
            b'\\' => match bytes.get(at + 1) {
                Some(b'(') => closing(text, at + 2, "\\)").map(|end| (at + 2, end, end + 2, false)),
                Some(b'[') => closing(text, at + 2, "\\]").map(|end| (at + 2, end, end + 2, true)),
                _ => None,
            },
            b'$' if bytes.get(at + 1) == Some(&b'$') => {
                closing(text, at + 2, "$$").map(|end| (at + 2, end, end + 2, true))
            }
            b'$' => single_dollar(text, at).map(|end| (at + 1, end, end + 1, false)),
            _ => None,
        };
        match found {
            Some((start, end, after, display)) if !text[start..end].trim().is_empty() => {
                if plain_from < at {
                    pieces.push(TextPiece::Text(&text[plain_from..at]));
                }
                pieces.push(TextPiece::Tex {
                    tex: &text[start..end],
                    display,
                    source: &text[at..after],
                });
                at = after;
                plain_from = after;
            }
            _ => at += 1,
        }
    }
    if plain_from < text.len() {
        pieces.push(TextPiece::Text(&text[plain_from..]));
    }
    pieces
}

/// Where `close` next appears from `from`.
fn closing(text: &str, from: usize, close: &str) -> Option<usize> {
    text.get(from..)?.find(close).map(|offset| from + offset)
}

/// The closing `$` of an inline formula opened at `open`, by Pandoc's rule.
fn single_dollar(text: &str, open: usize) -> Option<usize> {
    let after_open = text[open + 1..].chars().next()?;
    if after_open.is_whitespace() || after_open == '$' {
        return None;
    }
    let mut search = open + 1;
    while let Some(offset) = text[search..].find('$') {
        let close = search + offset;
        let before = text[..close].chars().next_back()?;
        let next = text[close + 1..].chars().next();
        if before == '\\' {
            search = close + 1;
            continue;
        }
        if before.is_whitespace() || next.is_some_and(|char| char.is_ascii_digit()) {
            return None;
        }
        let inside = &text[open + 1..close];
        return looks_like_math(inside).then_some(close);
    }
    None
}

/// What's between single dollars reads as math: a command, a script, a brace or an operator,
/// or a short name such as `x` or `f(x)`.
fn looks_like_math(inside: &str) -> bool {
    if inside.contains('\n') || inside.len() > 300 {
        return false;
    }
    inside.contains(['\\', '^', '_', '{', '}', '=', '+', '<', '>', '|'])
        || (inside.chars().count() <= 6
            && inside.chars().next().is_some_and(char::is_alphabetic)
            && inside
                .chars()
                .all(|char| char.is_alphanumeric() || "()',".contains(char)))
}
