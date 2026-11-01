//! A Jsoup-compatible selector engine (M32) over `scraper`'s tree. Shosetsu scripts were written
//! against Jsoup, whose selectors go beyond CSS: `:contains`, `:containsOwn`, `:matches`,
//! `:eq/:lt/:gt`, `:has` with any selector inside, and relative selectors such as `> p`.
//! Matching runs right to left with backtracking, like a browser's engine.

use ego_tree::{NodeId, NodeRef};
use regex::Regex;
use scraper::Node;

/// Parsed selector: alternatives separated by commas.
#[derive(Debug, Clone)]
pub struct Selector {
    alternatives: Vec<Complex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Combinator {
    Descendant,
    Child,
    Adjacent,
    Sibling,
}

/// Compounds joined by combinators, left to right. `leading` is set for relative selectors
/// (`> p`), whose left side is the element the selection starts from.
#[derive(Debug, Clone)]
struct Complex {
    leading: Option<Combinator>,
    compounds: Vec<Compound>,
    /// `combinators[i]` joins `compounds[i]` and `compounds[i + 1]`.
    combinators: Vec<Combinator>,
}

#[derive(Debug, Clone, Default)]
struct Compound {
    tag: Option<String>,
    conditions: Vec<Condition>,
}

#[derive(Debug, Clone)]
enum AttrOp {
    Exists,
    Equals(String),
    Prefix(String),
    Suffix(String),
    Contains(String),
    Word(String),
    Dash(String),
    NotEquals(String),
}

#[derive(Debug, Clone)]
enum Condition {
    Id(String),
    Class(String),
    Attr(String, AttrOp),
    Has(Box<Selector>),
    Not(Box<Selector>),
    Contains(String),
    ContainsOwn(String),
    Matches(Regex),
    MatchesOwn(Regex),
    Eq(usize),
    Lt(usize),
    Gt(usize),
    /// `an+b`, counted among elements (or same-type elements), from the start or end.
    Nth {
        a: i64,
        b: i64,
        of_type: bool,
        from_end: bool,
    },
    Only {
        of_type: bool,
    },
    Empty,
    Root,
}

impl Selector {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut parser = Parser {
            chars: text.chars().collect(),
            position: 0,
        };
        let selector = parser.selector(false)?;
        parser.skip_whitespace();
        if parser.position < parser.chars.len() {
            return Err(format!(
                "unexpected {:?} in selector {text:?}",
                parser.chars[parser.position]
            ));
        }
        Ok(selector)
    }

    /// Elements that match: `scope` itself and those under it, in document order (Jsoup's
    /// `select` includes the element it starts from).
    pub fn select(&self, scope: NodeRef<'_, Node>) -> Vec<NodeId> {
        scope
            .descendants()
            .filter(|node| node.value().is_element() && self.matches(*node, scope))
            .map(|node| node.id())
            .collect()
    }

    /// The first match, `scope` included.
    pub fn select_first(&self, scope: NodeRef<'_, Node>) -> Option<NodeId> {
        scope
            .descendants()
            .find(|node| node.value().is_element() && self.matches(*node, scope))
            .map(|node| node.id())
    }

    /// Whether `element` matches, with relative alternatives anchored at `scope`.
    pub fn matches(&self, element: NodeRef<'_, Node>, scope: NodeRef<'_, Node>) -> bool {
        self.alternatives
            .iter()
            .any(|complex| complex.matches(element, scope))
    }
}

impl Complex {
    fn matches(&self, element: NodeRef<'_, Node>, scope: NodeRef<'_, Node>) -> bool {
        self.matches_from(self.compounds.len() - 1, element, scope)
    }

    /// Whether `element` matches compound `index` and everything to its left.
    fn matches_from(
        &self,
        index: usize,
        element: NodeRef<'_, Node>,
        scope: NodeRef<'_, Node>,
    ) -> bool {
        if !self.compounds[index].matches(element, scope) {
            return false;
        }
        if index == 0 {
            return match self.leading {
                None => true,
                Some(combinator) => {
                    related(combinator, element, scope, |candidate| candidate == scope)
                }
            };
        }
        related(self.combinators[index - 1], element, scope, |candidate| {
            self.matches_from(index - 1, candidate, scope)
        })
    }
}

/// Whether some element related to `element` by `combinator` (as its left side) passes `test`.
/// Ancestors are looked at up to `scope` (included), as Jsoup does.
fn related<'a>(
    combinator: Combinator,
    element: NodeRef<'a, Node>,
    scope: NodeRef<'a, Node>,
    mut test: impl FnMut(NodeRef<'a, Node>) -> bool,
) -> bool {
    match combinator {
        Combinator::Child => element.parent().is_some_and(&mut test),
        Combinator::Descendant => {
            for ancestor in element.ancestors() {
                if test(ancestor) {
                    return true;
                }
                if ancestor == scope {
                    break;
                }
            }
            false
        }
        Combinator::Adjacent => previous_elements(element).next().is_some_and(&mut test),
        Combinator::Sibling => previous_elements(element).any(test),
    }
}

fn previous_elements<'a>(element: NodeRef<'a, Node>) -> impl Iterator<Item = NodeRef<'a, Node>> {
    element
        .prev_siblings()
        .filter(|sibling| sibling.value().is_element())
}

impl Compound {
    fn matches(&self, node: NodeRef<'_, Node>, scope: NodeRef<'_, Node>) -> bool {
        let Some(element) = node.value().as_element() else {
            return false;
        };
        if let Some(tag) = &self.tag
            && !element.name().eq_ignore_ascii_case(tag)
        {
            return false;
        }
        self.conditions
            .iter()
            .all(|condition| condition.matches(node, element, scope))
    }
}

impl Condition {
    fn matches(
        &self,
        node: NodeRef<'_, Node>,
        element: &scraper::node::Element,
        scope: NodeRef<'_, Node>,
    ) -> bool {
        match self {
            Self::Id(id) => element.id() == Some(id.as_str()),
            Self::Class(class) => element.classes().any(|candidate| candidate == class),
            Self::Attr(name, op) => {
                let value = element
                    .attrs()
                    .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
                    .map(|(_, value)| value);
                match (op, value) {
                    (AttrOp::Exists, value) => value.is_some(),
                    (AttrOp::NotEquals(expected), value) => value != Some(expected.as_str()),
                    (_, None) => false,
                    (AttrOp::Equals(expected), Some(value)) => value.eq_ignore_ascii_case(expected),
                    (AttrOp::Prefix(expected), Some(value)) => {
                        !expected.is_empty() && lower(value).starts_with(&lower(expected))
                    }
                    (AttrOp::Suffix(expected), Some(value)) => {
                        !expected.is_empty() && lower(value).ends_with(&lower(expected))
                    }
                    (AttrOp::Contains(expected), Some(value)) => {
                        !expected.is_empty() && lower(value).contains(&lower(expected))
                    }
                    (AttrOp::Word(expected), Some(value)) => value
                        .split_whitespace()
                        .any(|word| word.eq_ignore_ascii_case(expected)),
                    (AttrOp::Dash(expected), Some(value)) => {
                        value.eq_ignore_ascii_case(expected)
                            || lower(value).starts_with(&format!("{}-", lower(expected)))
                    }
                }
            }
            // Jsoup's :has looks at descendants, or at whatever a relative selector names.
            Self::Has(inner) => {
                let siblings = inner.alternatives.iter().any(|complex| {
                    matches!(
                        complex.leading,
                        Some(Combinator::Adjacent | Combinator::Sibling)
                    )
                });
                let mut candidates: Box<dyn Iterator<Item = NodeRef<'_, Node>>> =
                    Box::new(node.descendants().skip(1));
                if siblings {
                    candidates = Box::new(candidates.chain(node.next_siblings()));
                }
                candidates.any(|candidate| {
                    candidate.value().is_element() && inner.matches(candidate, node)
                })
            }
            Self::Not(inner) => !inner.matches(node, scope),
            Self::Contains(text) => lower(&element_text(node)).contains(&lower(text)),
            Self::ContainsOwn(text) => lower(&own_text(node)).contains(&lower(text)),
            Self::Matches(pattern) => pattern.is_match(&element_text(node)),
            Self::MatchesOwn(pattern) => pattern.is_match(&own_text(node)),
            Self::Eq(index) => sibling_index(node) == *index,
            Self::Lt(index) => sibling_index(node) < *index,
            Self::Gt(index) => sibling_index(node) > *index,
            Self::Nth {
                a,
                b,
                of_type,
                from_end,
            } => {
                let position = position_among(node, element.name(), *of_type, *from_end) as i64;
                if *a == 0 {
                    position == *b
                } else {
                    let steps = position - b;
                    steps % a == 0 && steps / a >= 0
                }
            }
            Self::Only { of_type } => {
                position_among(node, element.name(), *of_type, false) == 1
                    && position_among(node, element.name(), *of_type, true) == 1
            }
            Self::Empty => node.children().all(|child| match child.value() {
                Node::Element(_) => false,
                Node::Text(text) => text.is_empty(),
                _ => true,
            }),
            Self::Root => node
                .parent()
                .is_none_or(|parent| !parent.value().is_element()),
        }
    }
}

fn lower(text: &str) -> String {
    text.to_lowercase()
}

/// Index among sibling elements, from 0 (Jsoup's `elementSiblingIndex`).
fn sibling_index(node: NodeRef<'_, Node>) -> usize {
    previous_elements(node).count()
}

/// 1-based position among sibling elements (or those of the same tag), from either end.
fn position_among(node: NodeRef<'_, Node>, tag: &str, of_type: bool, from_end: bool) -> usize {
    let same = |sibling: &NodeRef<'_, Node>| {
        sibling
            .value()
            .as_element()
            .is_some_and(|element| !of_type || element.name() == tag)
    };
    1 + if from_end {
        node.next_siblings().filter(same).count()
    } else {
        node.prev_siblings().filter(same).count()
    }
}

/// Elements that end a run of text: their text gets a space around it in `text()`.
fn is_block(tag: &str) -> bool {
    matches!(
        tag,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "body"
            | "br"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hr"
            | "html"
            | "li"
            | "main"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "td"
            | "th"
            | "tr"
            | "ul"
    )
}

/// Jsoup's `text()`: all descendant text, whitespace collapsed, blocks and `<br>` separated.
pub fn element_text(node: NodeRef<'_, Node>) -> String {
    let mut raw = String::new();
    for edge in node.traverse() {
        match edge {
            ego_tree::iter::Edge::Open(child) => match child.value() {
                Node::Text(text) => raw.push_str(text),
                Node::Element(element) if is_block(element.name()) => raw.push(' '),
                _ => {}
            },
            ego_tree::iter::Edge::Close(child) => {
                if child
                    .value()
                    .as_element()
                    .is_some_and(|element| is_block(element.name()))
                {
                    raw.push(' ');
                }
            }
        }
    }
    normalize(&raw)
}

/// Jsoup's `ownText()`: the element's own text children only.
pub fn own_text(node: NodeRef<'_, Node>) -> String {
    let raw: String = node
        .children()
        .filter_map(|child| match child.value() {
            Node::Text(text) => Some(text.to_string()),
            Node::Element(element) if element.name() == "br" => Some(" ".to_owned()),
            _ => None,
        })
        .collect();
    normalize(&raw)
}

/// Collapses whitespace runs to one space and trims.
pub fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct Parser {
    chars: Vec<char>,
    position: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn skip_whitespace(&mut self) -> bool {
        let start = self.position;
        while self.peek().is_some_and(char::is_whitespace) {
            self.position += 1;
        }
        self.position > start
    }

    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    /// A comma-separated list; inside parentheses it stops at `)`.
    fn selector(&mut self, nested: bool) -> Result<Selector, String> {
        let mut alternatives = vec![self.complex()?];
        loop {
            self.skip_whitespace();
            if self.eat(',') {
                alternatives.push(self.complex()?);
            } else if nested && self.peek() == Some(')') || self.peek().is_none() {
                break;
            } else {
                return Err(format!("unexpected {:?} in selector", self.peek()));
            }
        }
        Ok(Selector { alternatives })
    }

    fn combinator(&mut self) -> Option<Combinator> {
        let had_space = self.skip_whitespace();
        let combinator = match self.peek() {
            Some('>') => Some(Combinator::Child),
            Some('+') => Some(Combinator::Adjacent),
            Some('~') => Some(Combinator::Sibling),
            _ => None,
        };
        if let Some(combinator) = combinator {
            self.position += 1;
            self.skip_whitespace();
            return Some(combinator);
        }
        match self.peek() {
            Some(',' | ')') | None => None,
            _ if had_space => Some(Combinator::Descendant),
            _ => None,
        }
    }

    fn complex(&mut self) -> Result<Complex, String> {
        self.skip_whitespace();
        let leading = match self.peek() {
            Some('>') => Some(Combinator::Child),
            Some('+') => Some(Combinator::Adjacent),
            Some('~') => Some(Combinator::Sibling),
            _ => None,
        };
        if leading.is_some() {
            self.position += 1;
            self.skip_whitespace();
        }
        let mut compounds = vec![self.compound()?];
        let mut combinators = Vec::new();
        while let Some(combinator) = self.combinator() {
            combinators.push(combinator);
            compounds.push(self.compound()?);
        }
        Ok(Complex {
            leading,
            compounds,
            combinators,
        })
    }

    fn identifier(&mut self) -> String {
        let mut name = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || matches!(c, '-' | '_') || !c.is_ascii() {
                name.push(c);
                self.position += 1;
            } else if c == '\\' {
                self.position += 1;
                if let Some(escaped) = self.peek() {
                    name.push(escaped);
                    self.position += 1;
                }
            } else {
                break;
            }
        }
        name
    }

    fn compound(&mut self) -> Result<Compound, String> {
        let mut compound = Compound::default();
        let universal = self.eat('*');
        if !universal && self.peek().is_some_and(|c| c.is_alphabetic()) {
            compound.tag = Some(self.identifier());
        }
        loop {
            match self.peek() {
                Some('#') => {
                    self.position += 1;
                    compound.conditions.push(Condition::Id(self.identifier()));
                }
                Some('.') => {
                    self.position += 1;
                    compound
                        .conditions
                        .push(Condition::Class(self.identifier()));
                }
                Some('[') => {
                    self.position += 1;
                    compound.conditions.push(self.attribute()?);
                }
                Some(':') => {
                    self.position += 1;
                    compound.conditions.push(self.pseudo()?);
                }
                _ => break,
            }
        }
        if !universal && compound.tag.is_none() && compound.conditions.is_empty() {
            return Err(format!("expected a selector at {:?}", self.peek()));
        }
        Ok(compound)
    }

    fn quoted_or_bare(&mut self, until: char) -> String {
        self.skip_whitespace();
        let quote = self.peek().filter(|c| matches!(c, '"' | '\''));
        let mut value = String::new();
        if let Some(quote) = quote {
            self.position += 1;
            while let Some(c) = self.peek() {
                self.position += 1;
                if c == quote {
                    break;
                }
                value.push(c);
            }
        } else {
            while let Some(c) = self.peek() {
                if c == until {
                    break;
                }
                value.push(c);
                self.position += 1;
            }
            value = value.trim().to_owned();
        }
        self.skip_whitespace();
        value
    }

    fn attribute(&mut self) -> Result<Condition, String> {
        self.skip_whitespace();
        let name = self.identifier();
        if name.is_empty() {
            return Err("attribute selector without a name".into());
        }
        self.skip_whitespace();
        let op = match self.peek() {
            Some(']') => None,
            Some('=') => {
                self.position += 1;
                Some("=")
            }
            Some(c @ ('^' | '$' | '*' | '~' | '|' | '!')) => {
                self.position += 1;
                if !self.eat('=') {
                    return Err(format!("expected = after {c}"));
                }
                Some(match c {
                    '^' => "^=",
                    '$' => "$=",
                    '*' => "*=",
                    '~' => "~=",
                    '|' => "|=",
                    _ => "!=",
                })
            }
            other => return Err(format!("unexpected {other:?} in attribute selector")),
        };
        let op = match op {
            None => AttrOp::Exists,
            Some(op) => {
                let value = self.quoted_or_bare(']');
                match op {
                    "=" => AttrOp::Equals(value),
                    "^=" => AttrOp::Prefix(value),
                    "$=" => AttrOp::Suffix(value),
                    "*=" => AttrOp::Contains(value),
                    "~=" => AttrOp::Word(value),
                    "|=" => AttrOp::Dash(value),
                    _ => AttrOp::NotEquals(value),
                }
            }
        };
        if !self.eat(']') {
            return Err("unclosed attribute selector".into());
        }
        Ok(Condition::Attr(name.to_ascii_lowercase(), op))
    }

    /// The raw text inside `( … )`, balancing nested parentheses.
    fn argument(&mut self) -> Result<String, String> {
        if !self.eat('(') {
            return Err("expected (".into());
        }
        let mut depth = 1;
        let mut text = String::new();
        while let Some(c) = self.peek() {
            self.position += 1;
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(text);
                    }
                }
                _ => {}
            }
            text.push(c);
        }
        Err("unclosed (".into())
    }

    fn pseudo(&mut self) -> Result<Condition, String> {
        let name = self.identifier().to_ascii_lowercase();
        let unquote = |text: String| {
            let trimmed = text.trim();
            trimmed
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
                .or_else(|| {
                    trimmed
                        .strip_prefix('\'')
                        .and_then(|rest| rest.strip_suffix('\''))
                })
                .unwrap_or(trimmed)
                .to_owned()
        };
        let index = |text: String| {
            text.trim()
                .parse::<usize>()
                .map_err(|_| format!(":{name} needs a number"))
        };
        let regex = |text: String| Regex::new(&unquote(text)).map_err(|error| error.to_string());
        Ok(match name.as_str() {
            "has" => {
                let inner = self.argument()?;
                Condition::Has(Box::new(Selector::parse(&inner)?))
            }
            "not" => {
                let inner = self.argument()?;
                Condition::Not(Box::new(Selector::parse(&inner)?))
            }
            "contains" => Condition::Contains(unquote(self.argument()?)),
            "containsown" => Condition::ContainsOwn(unquote(self.argument()?)),
            "matches" => Condition::Matches(regex(self.argument()?)?),
            "matchesown" => Condition::MatchesOwn(regex(self.argument()?)?),
            "eq" => Condition::Eq(index(self.argument()?)?),
            "lt" => Condition::Lt(index(self.argument()?)?),
            "gt" => Condition::Gt(index(self.argument()?)?),
            "nth-child" | "nth-last-child" | "nth-of-type" | "nth-last-of-type" => {
                let (a, b) = nth(&self.argument()?)?;
                Condition::Nth {
                    a,
                    b,
                    of_type: name.ends_with("of-type"),
                    from_end: name.contains("last"),
                }
            }
            "first-child" => nth_fixed(false, false),
            "last-child" => nth_fixed(false, true),
            "first-of-type" => nth_fixed(true, false),
            "last-of-type" => nth_fixed(true, true),
            "only-child" => Condition::Only { of_type: false },
            "only-of-type" => Condition::Only { of_type: true },
            "empty" => Condition::Empty,
            "root" => Condition::Root,
            other => return Err(format!("unsupported pseudo-class :{other}")),
        })
    }
}

fn nth_fixed(of_type: bool, from_end: bool) -> Condition {
    Condition::Nth {
        a: 0,
        b: 1,
        of_type,
        from_end,
    }
}

/// `an+b`, `odd`, `even` or a number.
fn nth(text: &str) -> Result<(i64, i64), String> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let text = text.to_ascii_lowercase();
    match text.as_str() {
        "odd" => return Ok((2, 1)),
        "even" => return Ok((2, 0)),
        _ => {}
    }
    let invalid = || format!("invalid nth expression {text:?}");
    match text.split_once('n') {
        None => Ok((0, text.parse().map_err(|_| invalid())?)),
        Some((a, b)) => {
            let a = match a {
                "" | "+" => 1,
                "-" => -1,
                a => a.parse().map_err(|_| invalid())?,
            };
            let b = if b.is_empty() {
                0
            } else {
                b.parse().map_err(|_| invalid())?
            };
            Ok((a, b))
        }
    }
}

#[cfg(test)]
mod tests {
    use scraper::Html;

    use super::*;

    const PAGE: &str = r#"<html><body>
        <div class="info" id="main">
          <div><h3>Author:</h3><a href="/author/yamada">Yamada</a></div>
          <div><h3>Status:</h3><a href="/status/ongoing">Ongoing</a></div>
          <li><h3>Alternative names</h3>Sousou no Frieren</li>
        </div>
        <ul class="list"><li>one</li><li class="x">two</li><li>three</li><li>four</li></ul>
        <p>First <b>bold</b> para</p><p data-id="7" lang="en-US">Second</p>
        <span></span>
    </body></html>"#;

    fn texts(html: &Html, selector: &str) -> Vec<String> {
        let selector = Selector::parse(selector).unwrap();
        selector
            .select(html.tree.root())
            .into_iter()
            .map(|id| element_text(html.tree.get(id).unwrap()))
            .collect()
    }

    #[test]
    fn matches_css_basics() {
        let html = Html::parse_document(PAGE);
        assert_eq!(texts(&html, "ul.list > li.x"), ["two"]);
        assert_eq!(texts(&html, "#main a"), ["Yamada", "Ongoing"]);
        assert_eq!(texts(&html, "li.x + li"), ["three"]);
        assert_eq!(texts(&html, "li.x ~ li"), ["three", "four"]);
        assert_eq!(texts(&html, "a[href^=/status]"), ["Ongoing"]);
        assert_eq!(
            texts(&html, r#"a[href*="author"], p[data-id=7]"#),
            ["Yamada", "Second"]
        );
        assert_eq!(texts(&html, "p[lang|=en]"), ["Second"]);
        assert_eq!(texts(&html, "ul li:nth-child(2n)"), ["two", "four"]);
        assert_eq!(texts(&html, "ul li:last-child"), ["four"]);
        assert_eq!(texts(&html, "ul li:not(.x):lt(2)"), ["one"]);
        assert_eq!(texts(&html, "ul li:eq(2)"), ["three"]);
        assert_eq!(texts(&html, "body > span:empty").len(), 1);
    }

    #[test]
    fn matches_jsoup_pseudos() {
        let html = Html::parse_document(PAGE);
        assert_eq!(
            texts(&html, ".info > div:has(h3:containsOwn(tatus)) > a"),
            ["Ongoing"]
        );
        assert_eq!(
            texts(&html, ".info > li:has(h3:contains(lternative))"),
            ["Alternative names Sousou no Frieren"]
        );
        assert_eq!(texts(&html, "p:contains(BOLD)"), ["First bold para"]);
        assert_eq!(
            texts(&html, "p:containsOwn(bold)").len(),
            0,
            "bold is in a child"
        );
        assert_eq!(texts(&html, r"li:matches(^t\w+)"), ["two", "three"]);
        assert_eq!(
            texts(&html, "div:has(> a[href*=author])"),
            ["Author: Yamada"]
        );
    }

    #[test]
    fn relative_selectors_start_at_the_scope() {
        let html = Html::parse_document(PAGE);
        let list = Selector::parse("ul")
            .unwrap()
            .select_first(html.tree.root())
            .unwrap();
        let list = html.tree.get(list).unwrap();
        let children = Selector::parse("> li").unwrap().select(list);
        assert_eq!(children.len(), 4);
        let info = Selector::parse("#main")
            .unwrap()
            .select_first(html.tree.root())
            .unwrap();
        assert_eq!(
            Selector::parse("> a")
                .unwrap()
                .select(html.tree.get(info).unwrap())
                .len(),
            0,
            "links are grandchildren"
        );
        let link = Selector::parse("a")
            .unwrap()
            .select_first(html.tree.root())
            .unwrap();
        assert_eq!(
            Selector::parse("a")
                .unwrap()
                .select_first(html.tree.get(link).unwrap()),
            Some(link),
            "an element matches its own select, as in Jsoup"
        );
    }

    #[test]
    fn text_follows_jsoup() {
        let html = Html::parse_fragment("<div>  Hello<br>world <p>para</p>tail </div>");
        let div = Selector::parse("div")
            .unwrap()
            .select_first(html.tree.root())
            .unwrap();
        let div = html.tree.get(div).unwrap();
        assert_eq!(element_text(div), "Hello world para tail");
        assert_eq!(own_text(div), "Hello world tail");
    }

    #[test]
    fn rejects_broken_selectors() {
        for broken in ["div[", "p:nth-child(x)", "a:unknown", "div >", ":has(p", ""] {
            assert!(Selector::parse(broken).is_err(), "{broken}");
        }
    }
}
