//! XPath for Miru's `queryXPath`, which Miru answers with Dart's `xpath_selector`. Scripts use
//! location paths over HTML: `//div[@id="playlist1"]/ul/li`, `//a/@href`, `//div/script[1]`.
//! This is XPath 1.0's location paths with the usual axes, predicates (positions, comparisons,
//! `and`/`or`) and the string functions scripts reach for; no arithmetic or variables.

use std::collections::HashMap;

use ego_tree::{NodeId, NodeRef};
use scraper::{Html, Node};

use super::dom::text_content;

/// Most steps and predicates one expression may hold, so a hostile one can't run long.
const MAX_TOKENS: usize = 512;

/// One result: a node of the document, or an attribute of an element.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Node(NodeId),
    Attr {
        owner: NodeId,
        name: String,
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Slash,
    DoubleSlash,
    Open,
    Close,
    OpenParen,
    CloseParen,
    At,
    Comma,
    Pipe,
    Dot,
    DotDot,
    Axis(String),
    Name(String),
    Literal(String),
    Number(f64),
    Op(&'static str),
}

fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        if c.is_whitespace() {
            index += 1;
            continue;
        }
        let next = chars.get(index + 1).copied();
        let token = match c {
            '/' if next == Some('/') => {
                index += 1;
                Token::DoubleSlash
            }
            '/' => Token::Slash,
            '[' => Token::Open,
            ']' => Token::Close,
            '(' => Token::OpenParen,
            ')' => Token::CloseParen,
            '@' => Token::At,
            ',' => Token::Comma,
            '|' => Token::Pipe,
            '.' if next == Some('.') => {
                index += 1;
                Token::DotDot
            }
            '.' if !next.is_some_and(|c| c.is_ascii_digit()) => Token::Dot,
            '=' => Token::Op("="),
            '!' if next == Some('=') => {
                index += 1;
                Token::Op("!=")
            }
            '<' | '>' => {
                let op = match (c, next) {
                    ('<', Some('=')) => "<=",
                    ('>', Some('=')) => ">=",
                    ('<', _) => "<",
                    _ => ">",
                };
                if op.len() == 2 {
                    index += 1;
                }
                Token::Op(op)
            }
            '"' | '\'' => {
                let end = chars[index + 1..]
                    .iter()
                    .position(|candidate| *candidate == c)
                    .ok_or("unclosed string in XPath")?;
                let literal: String = chars[index + 1..index + 1 + end].iter().collect();
                index += end + 1;
                Token::Literal(literal)
            }
            '*' => Token::Name("*".into()),
            c if c.is_ascii_digit() || c == '.' => {
                let start = index;
                while chars
                    .get(index + 1)
                    .is_some_and(|c| c.is_ascii_digit() || *c == '.')
                {
                    index += 1;
                }
                let number: String = chars[start..=index].iter().collect();
                Token::Number(
                    number
                        .parse()
                        .map_err(|_| format!("bad number {number:?} in XPath"))?,
                )
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = index;
                while chars.get(index + 1).is_some_and(|c| {
                    c.is_alphanumeric()
                        || matches!(c, '-' | '_' | '.')
                        || (*c == ':' && chars.get(index + 2) != Some(&':'))
                }) {
                    index += 1;
                }
                let name: String = chars[start..=index].iter().collect();
                if chars.get(index + 1) == Some(&':') && chars.get(index + 2) == Some(&':') {
                    index += 2;
                    Token::Axis(name)
                } else {
                    Token::Name(name)
                }
            }
            other => return Err(format!("unexpected {other:?} in XPath")),
        };
        tokens.push(token);
        if tokens.len() > MAX_TOKENS {
            return Err("the XPath is too long".into());
        }
        index += 1;
    }
    Ok(tokens)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Child,
    Descendant,
    DescendantOrSelf,
    SelfNode,
    Parent,
    Ancestor,
    AncestorOrSelf,
    FollowingSibling,
    PrecedingSibling,
    Attribute,
}

impl Axis {
    fn parse(name: &str) -> Result<Self, String> {
        Ok(match name {
            "child" => Self::Child,
            "descendant" => Self::Descendant,
            "descendant-or-self" => Self::DescendantOrSelf,
            "self" => Self::SelfNode,
            "parent" => Self::Parent,
            "ancestor" => Self::Ancestor,
            "ancestor-or-self" => Self::AncestorOrSelf,
            "following-sibling" => Self::FollowingSibling,
            "preceding-sibling" => Self::PrecedingSibling,
            "attribute" => Self::Attribute,
            other => return Err(format!("the XPath axis {other} isn't supported")),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Test {
    /// An element (or attribute) name; `*` is any.
    Name(String),
    Text,
    Node,
}

#[derive(Debug, Clone)]
struct Step {
    axis: Axis,
    test: Test,
    predicates: Vec<Expr>,
}

#[derive(Debug, Clone)]
enum Expr {
    Or(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Compare(&'static str, Box<Expr>, Box<Expr>),
    Union(Vec<Expr>),
    /// A location path, absolute or from the context node.
    Path {
        absolute: bool,
        steps: Vec<Step>,
    },
    /// `( expr )[predicate]`.
    Filter(Box<Expr>, Vec<Expr>),
    Literal(String),
    Number(f64),
    Call(String, Vec<Expr>),
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn eat(&mut self, token: &Token) -> bool {
        if self.peek() == Some(token) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, token: &Token) -> Result<(), String> {
        if self.eat(token) {
            Ok(())
        } else {
            Err(format!("expected {token:?} in XPath"))
        }
    }

    fn word(&mut self, word: &str) -> bool {
        if matches!(self.peek(), Some(Token::Name(name)) if name == word) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn or(&mut self) -> Result<Expr, String> {
        let mut left = self.and()?;
        while self.word("or") {
            left = Expr::Or(Box::new(left), Box::new(self.and()?));
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Expr, String> {
        let mut left = self.compare()?;
        while self.word("and") {
            left = Expr::And(Box::new(left), Box::new(self.compare()?));
        }
        Ok(left)
    }

    fn compare(&mut self) -> Result<Expr, String> {
        let mut left = self.union()?;
        while let Some(Token::Op(op)) = self.peek().cloned() {
            self.position += 1;
            left = Expr::Compare(op, Box::new(left), Box::new(self.union()?));
        }
        Ok(left)
    }

    fn union(&mut self) -> Result<Expr, String> {
        let mut paths = vec![self.path()?];
        while self.eat(&Token::Pipe) {
            paths.push(self.path()?);
        }
        Ok(if paths.len() == 1 {
            paths.remove(0)
        } else {
            Expr::Union(paths)
        })
    }

    fn path(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Some(Token::Literal(text)) => {
                let text = text.clone();
                self.position += 1;
                Ok(Expr::Literal(text))
            }
            Some(Token::Number(number)) => {
                let number = *number;
                self.position += 1;
                Ok(Expr::Number(number))
            }
            Some(Token::OpenParen) => {
                self.position += 1;
                let inner = self.or()?;
                self.expect(&Token::CloseParen)?;
                let predicates = self.predicates()?;
                Ok(Expr::Filter(Box::new(inner), predicates))
            }
            Some(Token::Name(name))
                if self.tokens.get(self.position + 1) == Some(&Token::OpenParen)
                    && !matches!(name.as_str(), "text" | "node" | "comment") =>
            {
                let name = name.clone();
                self.position += 2;
                let mut args = Vec::new();
                if !self.eat(&Token::CloseParen) {
                    loop {
                        args.push(self.or()?);
                        if self.eat(&Token::CloseParen) {
                            break;
                        }
                        self.expect(&Token::Comma)?;
                    }
                }
                Ok(Expr::Call(name, args))
            }
            _ => self.location(),
        }
    }

    fn location(&mut self) -> Result<Expr, String> {
        let mut steps = Vec::new();
        let absolute = match self.peek() {
            Some(Token::Slash) => {
                self.position += 1;
                // `/` alone is the document.
                if !self.starts_step() {
                    return Ok(Expr::Path {
                        absolute: true,
                        steps,
                    });
                }
                true
            }
            Some(Token::DoubleSlash) => {
                self.position += 1;
                steps.push(descendants());
                true
            }
            _ => false,
        };
        steps.push(self.step()?);
        loop {
            if self.eat(&Token::Slash) {
                steps.push(self.step()?);
            } else if self.eat(&Token::DoubleSlash) {
                steps.push(descendants());
                steps.push(self.step()?);
            } else {
                break;
            }
        }
        Ok(Expr::Path { absolute, steps })
    }

    fn starts_step(&self) -> bool {
        matches!(
            self.peek(),
            Some(Token::Name(_) | Token::At | Token::Dot | Token::DotDot | Token::Axis(_))
        )
    }

    fn step(&mut self) -> Result<Step, String> {
        if self.eat(&Token::Dot) {
            return Ok(Step {
                axis: Axis::SelfNode,
                test: Test::Node,
                predicates: self.predicates()?,
            });
        }
        if self.eat(&Token::DotDot) {
            return Ok(Step {
                axis: Axis::Parent,
                test: Test::Node,
                predicates: self.predicates()?,
            });
        }
        let axis = match self.peek().cloned() {
            Some(Token::At) => {
                self.position += 1;
                Axis::Attribute
            }
            Some(Token::Axis(name)) => {
                self.position += 1;
                Axis::parse(&name)?
            }
            _ => Axis::Child,
        };
        let test = match self.peek().cloned() {
            Some(Token::Name(name)) => {
                self.position += 1;
                if self.eat(&Token::OpenParen) {
                    self.expect(&Token::CloseParen)?;
                    match name.as_str() {
                        "text" => Test::Text,
                        "node" => Test::Node,
                        other => return Err(format!("the XPath test {other}() isn't supported")),
                    }
                } else {
                    Test::Name(name.to_ascii_lowercase())
                }
            }
            other => return Err(format!("expected a step in XPath, found {other:?}")),
        };
        Ok(Step {
            axis,
            test,
            predicates: self.predicates()?,
        })
    }

    fn predicates(&mut self) -> Result<Vec<Expr>, String> {
        let mut predicates = Vec::new();
        while self.eat(&Token::Open) {
            predicates.push(self.or()?);
            self.expect(&Token::Close)?;
        }
        Ok(predicates)
    }
}

fn descendants() -> Step {
    Step {
        axis: Axis::DescendantOrSelf,
        test: Test::Node,
        predicates: Vec::new(),
    }
}

fn parse(text: &str) -> Result<Expr, String> {
    let mut parser = Parser {
        tokens: tokenize(text)?,
        position: 0,
    };
    if parser.tokens.is_empty() {
        return Err("empty XPath".into());
    }
    let expr = parser.or()?;
    if parser.position < parser.tokens.len() {
        return Err(format!(
            "unexpected {:?} in XPath {text:?}",
            parser.tokens[parser.position]
        ));
    }
    Ok(expr)
}

#[derive(Debug, Clone)]
enum Value {
    Nodes(Vec<Item>),
    Text(String),
    Number(f64),
    Boolean(bool),
}

struct Context<'a> {
    html: &'a Html,
    /// Every node's place in document order.
    order: HashMap<NodeId, usize>,
}

/// Where an evaluation stands: the node, and its position among the nodes being filtered.
struct Focus {
    item: Item,
    position: usize,
    size: usize,
}

impl Context<'_> {
    fn node(&self, id: NodeId) -> Option<NodeRef<'_, Node>> {
        self.html.tree.get(id)
    }

    fn string_of(&self, item: &Item) -> String {
        match item {
            Item::Attr { value, .. } => value.clone(),
            Item::Node(id) => self.node(*id).map(text_content).unwrap_or_default(),
        }
    }

    fn sort(&self, items: &mut Vec<Item>) {
        let key = |item: &Item| match item {
            Item::Node(id) => (
                self.order.get(id).copied().unwrap_or(usize::MAX),
                0,
                String::new(),
            ),
            Item::Attr { owner, name, .. } => (
                self.order.get(owner).copied().unwrap_or(usize::MAX),
                1,
                name.clone(),
            ),
        };
        items.sort_by_key(key);
        items.dedup();
    }

    fn matches(&self, node: NodeRef<'_, Node>, test: &Test) -> bool {
        match test {
            Test::Node => true,
            Test::Text => node.value().is_text(),
            Test::Name(name) => node
                .value()
                .as_element()
                .is_some_and(|element| name == "*" || element.name().eq_ignore_ascii_case(name)),
        }
    }

    /// The items `step`'s axis reaches from `item`, in the axis's own order.
    fn axis(&self, item: &Item, step: &Step) -> Vec<Item> {
        let Item::Node(id) = item else {
            // Attributes have only themselves and their element.
            return match (step.axis, item) {
                (Axis::SelfNode, _) => vec![item.clone()],
                (
                    Axis::Parent | Axis::Ancestor | Axis::AncestorOrSelf,
                    Item::Attr { owner, .. },
                ) => {
                    vec![Item::Node(*owner)]
                }
                _ => Vec::new(),
            };
        };
        let Some(node) = self.node(*id) else {
            return Vec::new();
        };
        let nodes: Vec<NodeRef<'_, Node>> = match step.axis {
            Axis::Attribute => {
                let Some(element) = node.value().as_element() else {
                    return Vec::new();
                };
                return element
                    .attrs()
                    .filter(|(name, _)| match &step.test {
                        Test::Name(wanted) => wanted == "*" || name.eq_ignore_ascii_case(wanted),
                        Test::Node => true,
                        Test::Text => false,
                    })
                    .map(|(name, value)| Item::Attr {
                        owner: *id,
                        name: name.to_owned(),
                        value: value.to_owned(),
                    })
                    .collect();
            }
            Axis::Child => node.children().collect(),
            Axis::Descendant => node.descendants().skip(1).collect(),
            Axis::DescendantOrSelf => node.descendants().collect(),
            Axis::SelfNode => vec![node],
            Axis::Parent => node.parent().into_iter().collect(),
            Axis::Ancestor => node.ancestors().collect(),
            Axis::AncestorOrSelf => std::iter::once(node).chain(node.ancestors()).collect(),
            Axis::FollowingSibling => node.next_siblings().collect(),
            Axis::PrecedingSibling => node.prev_siblings().collect(),
        };
        nodes
            .into_iter()
            .filter(|candidate| self.matches(*candidate, &step.test))
            .map(|candidate| Item::Node(candidate.id()))
            .collect()
    }

    fn filter(&self, items: Vec<Item>, predicates: &[Expr]) -> Result<Vec<Item>, String> {
        let mut items = items;
        for predicate in predicates {
            let size = items.len();
            let mut kept = Vec::new();
            for (index, item) in items.into_iter().enumerate() {
                let focus = Focus {
                    item: item.clone(),
                    position: index + 1,
                    size,
                };
                let keep = match self.eval(predicate, &focus)? {
                    Value::Number(number) => number == (index + 1) as f64,
                    other => self.boolean(&other),
                };
                if keep {
                    kept.push(item);
                }
            }
            items = kept;
        }
        Ok(items)
    }

    fn path(&self, absolute: bool, steps: &[Step], focus: &Focus) -> Result<Vec<Item>, String> {
        let mut current = vec![if absolute {
            Item::Node(self.html.tree.root().id())
        } else {
            focus.item.clone()
        }];
        for step in steps {
            let mut next = Vec::new();
            for item in &current {
                // Each axis lists its nodes nearest first, so positions in the reverse axes
                // (ancestors, preceding siblings) count back from the context node.
                let reached = self.axis(item, step);
                next.extend(self.filter(reached, &step.predicates)?);
            }
            self.sort(&mut next);
            current = next;
        }
        Ok(current)
    }

    fn boolean(&self, value: &Value) -> bool {
        match value {
            Value::Nodes(items) => !items.is_empty(),
            Value::Text(text) => !text.is_empty(),
            Value::Number(number) => *number != 0.0 && !number.is_nan(),
            Value::Boolean(flag) => *flag,
        }
    }

    fn text(&self, value: &Value) -> String {
        match value {
            Value::Nodes(items) => items
                .first()
                .map(|item| self.string_of(item))
                .unwrap_or_default(),
            Value::Text(text) => text.clone(),
            Value::Number(number) => {
                if number.fract() == 0.0 && number.is_finite() {
                    format!("{number:.0}")
                } else {
                    number.to_string()
                }
            }
            Value::Boolean(flag) => flag.to_string(),
        }
    }

    fn number(&self, value: &Value) -> f64 {
        match value {
            Value::Number(number) => *number,
            Value::Boolean(flag) => f64::from(u8::from(*flag)),
            other => self.text(other).trim().parse().unwrap_or(f64::NAN),
        }
    }

    /// XPath 1.0's comparisons: a node set compares through each of its nodes' values.
    fn compare(&self, op: &str, left: &Value, right: &Value) -> bool {
        match (left, right) {
            (Value::Nodes(a), Value::Nodes(b)) => a.iter().any(|left| {
                let left = Value::Text(self.string_of(left));
                b.iter()
                    .any(|right| self.compare_atoms(op, &left, &Value::Text(self.string_of(right))))
            }),
            (Value::Nodes(items), other) => self.compare_set(op, items, other, false),
            (other, Value::Nodes(items)) => self.compare_set(op, items, other, true),
            _ => self.compare_atoms(op, left, right),
        }
    }

    fn compare_set(&self, op: &str, items: &[Item], other: &Value, flipped: bool) -> bool {
        let ordered = |a: &Value, b: &Value| {
            if flipped {
                self.compare_atoms(op, b, a)
            } else {
                self.compare_atoms(op, a, b)
            }
        };
        if let Value::Boolean(_) = other {
            return ordered(&Value::Boolean(!items.is_empty()), other);
        }
        items.iter().any(|item| {
            let value = self.string_of(item);
            let value = match other {
                Value::Number(_) => Value::Number(value.trim().parse().unwrap_or(f64::NAN)),
                _ => Value::Text(value),
            };
            ordered(&value, other)
        })
    }

    fn compare_atoms(&self, op: &str, left: &Value, right: &Value) -> bool {
        if matches!(op, "=" | "!=") {
            let equal = if matches!(left, Value::Boolean(_)) || matches!(right, Value::Boolean(_)) {
                self.boolean(left) == self.boolean(right)
            } else if matches!(left, Value::Number(_)) || matches!(right, Value::Number(_)) {
                self.number(left) == self.number(right)
            } else {
                self.text(left) == self.text(right)
            };
            return equal == (op == "=");
        }
        compare_numbers(op, self.number(left), self.number(right))
    }

    fn eval(&self, expr: &Expr, focus: &Focus) -> Result<Value, String> {
        Ok(match expr {
            Expr::Or(left, right) => Value::Boolean(
                self.boolean(&self.eval(left, focus)?) || self.boolean(&self.eval(right, focus)?),
            ),
            Expr::And(left, right) => Value::Boolean(
                self.boolean(&self.eval(left, focus)?) && self.boolean(&self.eval(right, focus)?),
            ),
            Expr::Compare(op, left, right) => {
                let (left, right) = (self.eval(left, focus)?, self.eval(right, focus)?);
                Value::Boolean(self.compare(op, &left, &right))
            }
            Expr::Union(paths) => {
                let mut items = Vec::new();
                for path in paths {
                    if let Value::Nodes(found) = self.eval(path, focus)? {
                        items.extend(found);
                    }
                }
                self.sort(&mut items);
                Value::Nodes(items)
            }
            Expr::Path { absolute, steps } => Value::Nodes(self.path(*absolute, steps, focus)?),
            Expr::Filter(inner, predicates) => match self.eval(inner, focus)? {
                Value::Nodes(items) => Value::Nodes(self.filter(items, predicates)?),
                other if predicates.is_empty() => other,
                _ => return Err("only node sets can be filtered in XPath".into()),
            },
            Expr::Literal(text) => Value::Text(text.clone()),
            Expr::Number(number) => Value::Number(*number),
            Expr::Call(name, args) => self.call(name, args, focus)?,
        })
    }

    fn call(&self, name: &str, args: &[Expr], focus: &Focus) -> Result<Value, String> {
        let values = args
            .iter()
            .map(|arg| self.eval(arg, focus))
            .collect::<Result<Vec<_>, _>>()?;
        let text = |index: usize| {
            values
                .get(index)
                .map_or_else(|| self.string_of(&focus.item), |value| self.text(value))
        };
        Ok(match (name, values.len()) {
            ("position", 0) => Value::Number(focus.position as f64),
            ("last", 0) => Value::Number(focus.size as f64),
            ("count", 1) => match &values[0] {
                Value::Nodes(items) => Value::Number(items.len() as f64),
                _ => return Err("count() needs a node set".into()),
            },
            ("not", 1) => Value::Boolean(!self.boolean(&values[0])),
            ("true", 0) => Value::Boolean(true),
            ("false", 0) => Value::Boolean(false),
            ("string", 0 | 1) => Value::Text(text(0)),
            ("contains", 2) => Value::Boolean(text(0).contains(&text(1))),
            ("starts-with", 2) => Value::Boolean(text(0).starts_with(&text(1))),
            ("ends-with", 2) => Value::Boolean(text(0).ends_with(&text(1))),
            ("normalize-space", 0 | 1) => {
                Value::Text(text(0).split_whitespace().collect::<Vec<_>>().join(" "))
            }
            ("string-length", 0 | 1) => Value::Number(text(0).chars().count() as f64),
            ("concat", count) if count >= 2 => {
                Value::Text((0..count).map(text).collect::<String>())
            }
            ("lower-case", 1) => Value::Text(text(0).to_lowercase()),
            ("upper-case", 1) => Value::Text(text(0).to_uppercase()),
            ("name" | "local-name", 0) => Value::Text(match &focus.item {
                Item::Attr { name, .. } => name.clone(),
                Item::Node(id) => self
                    .node(*id)
                    .and_then(|node| {
                        node.value()
                            .as_element()
                            .map(|element| element.name().to_owned())
                    })
                    .unwrap_or_default(),
            }),
            ("number", 0 | 1) => Value::Number(values.first().map_or_else(
                || {
                    self.string_of(&focus.item)
                        .trim()
                        .parse()
                        .unwrap_or(f64::NAN)
                },
                |value| self.number(value),
            )),
            (other, count) => {
                return Err(format!(
                    "the XPath function {other}() with {count} arguments isn't supported"
                ));
            }
        })
    }
}

fn compare_numbers(op: &str, a: f64, b: f64) -> bool {
    match op {
        "=" => a == b,
        "!=" => a != b,
        "<" => a < b,
        "<=" => a <= b,
        ">" => a > b,
        _ => a >= b,
    }
}

/// What `expression` selects in `html`, in document order. A result that isn't a node set
/// (`count(//a)`) comes back as one attribute-like item holding its text.
pub fn evaluate(html: &Html, expression: &str) -> Result<Vec<Item>, String> {
    let expr = parse(expression)?;
    let order = html
        .tree
        .root()
        .descendants()
        .enumerate()
        .map(|(index, node)| (node.id(), index))
        .collect();
    let context = Context { html, order };
    let root = html.tree.root().id();
    let focus = Focus {
        item: Item::Node(root),
        position: 1,
        size: 1,
    };
    Ok(match context.eval(&expr, &focus)? {
        Value::Nodes(items) => items,
        other => vec![Item::Attr {
            owner: root,
            name: String::new(),
            value: context.text(&other),
        }],
    })
}

/// The nodes of a result: elements, text nodes, and the elements of attributes.
pub fn nodes(items: &[Item]) -> Vec<NodeId> {
    let mut nodes = Vec::new();
    for item in items {
        let id = match item {
            Item::Node(id) => *id,
            Item::Attr { owner, .. } => *owner,
        };
        if !nodes.contains(&id) {
            nodes.push(id);
        }
    }
    nodes
}

/// The texts a result holds: attribute values and text nodes, as `xpath_selector`'s `attrs`.
pub fn attributes(html: &Html, items: &[Item]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Attr { value, .. } => Some(value.clone()),
            Item::Node(id) => html
                .tree
                .get(*id)
                .and_then(|node| node.value().as_text().map(|text| text.to_string())),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<html><body>
      <div class="myui-content__detail"><h1>Title <i>here</i></h1></div>
      <img class="lazyload" data-original="https://img.example/c.jpg">
      <ul id="searchList">
        <li><h4 class="title"><a href="/v/1">One</a></h4><div class="thumb"><a data-original="/p1.jpg" href="/v/1"></a></div></li>
        <li><h4 class="title"><a href="/v/2">Two</a></h4></li>
      </ul>
      <div class="player-box-main"><script>var player_aaaa={"url":"x"}</script><script>other()</script></div>
      <p lang="en">a</p><p>b</p><p>c</p>
    </body></html>"#;

    fn run(expression: &str) -> (Vec<String>, Vec<String>) {
        let html = Html::parse_document(PAGE);
        let items = evaluate(&html, expression).unwrap();
        let texts = nodes(&items)
            .into_iter()
            .map(|id| text_content(html.tree.get(id).unwrap()))
            .collect();
        (texts, attributes(&html, &items))
    }

    #[test]
    fn selects_what_miru_scripts_ask_for() {
        assert_eq!(
            run(r#"//div[@class="myui-content__detail"]/h1"#).0,
            ["Title here"]
        );
        assert_eq!(
            run(r#"//img[@class="lazyload"]/@data-original"#).1,
            ["https://img.example/c.jpg"]
        );
        assert_eq!(run(r#"//ul[@id="searchList"]/li"#).0.len(), 2);
        assert_eq!(run(r#"//h4[@class="title"]/a/@href"#).1, ["/v/1", "/v/2"]);
        assert_eq!(
            run(r#"//div[@class="thumb"]/a/@data-original"#).1,
            ["/p1.jpg"]
        );
        assert_eq!(
            run(r#"//div[@class="player-box-main"]/script[1]"#).0,
            [r#"var player_aaaa={"url":"x"}"#]
        );
        // A class must match whole, as in XPath.
        assert!(run(r#"//img[@class="lazy"]"#).0.is_empty());
    }

    #[test]
    fn predicates_axes_and_functions() {
        assert_eq!(run("//p[last()]").0, ["c"]);
        assert_eq!(run("//p[position() > 1]").0, ["b", "c"]);
        assert_eq!(run("//p[@lang]").0, ["a"]);
        assert_eq!(run("//p[not(@lang)][1]").0, ["b"]);
        assert_eq!(run("//li[contains(., 'Tw')]/h4/a").0, ["Two"]);
        assert_eq!(
            run("//a[starts-with(@href, '/v/') and text()='One']").0,
            ["One"]
        );
        assert_eq!(run("//h4/a/text()").1, ["One", "Two"]);
        assert_eq!(run("//a[text()='Two']/../../h4/a").0, ["Two"]);
        assert_eq!(run("(//p)[2]").0, ["b"]);
        assert_eq!(run("//p[1] | //h1").0, ["Title here", "a"]);
        assert_eq!(run("//i/ancestor::div[1]/h1").0, ["Title here"]);
        assert_eq!(run("//p[2]/following-sibling::p").0, ["c"]);
        assert_eq!(run("//p[3]/preceding-sibling::p[1]").0, ["b"]);
        assert_eq!(run("count(//li)").1, ["2"]);
        assert_eq!(run("//*[@id='searchList']/li[2]//a").0, ["Two"]);
    }

    #[test]
    fn rejects_broken_expressions() {
        let html = Html::parse_document(PAGE);
        for broken in ["", "//div[", "//a/@", "//p[unknown()]", "//p['x]"] {
            assert!(evaluate(&html, broken).is_err(), "{broken}");
        }
    }
}
