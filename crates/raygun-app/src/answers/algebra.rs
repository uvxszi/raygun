//! Algebra with polynomials in one variable: equations solved (`solve 2x + 3 = 7`,
//! `x^2 - 5x + 6 = 0`), and polynomials expanded, factored, differentiated and integrated.
//! Coefficients are exact fractions; quadratics are solved exactly (surds and complex roots
//! included), higher degrees numerically.

use std::cmp::Ordering;
use std::fmt;

use raygun_model::{AnswerKind, AnswerRow};

use super::numbers::show_f64;
use super::{Found, blank, math_text};

/// The highest power a polynomial may reach.
const MAX_DEGREE: usize = 8;

/// An exact fraction, kept in lowest terms with a positive denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Q {
    num: i128,
    den: i128,
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl Q {
    pub const ZERO: Self = Self { num: 0, den: 1 };
    pub const ONE: Self = Self { num: 1, den: 1 };

    pub fn new(num: i128, den: i128) -> Option<Self> {
        if den == 0 {
            return None;
        }
        let divisor = gcd(num, den).max(1);
        let sign = if den < 0 { -1 } else { 1 };
        Some(Self {
            num: sign * num / divisor,
            den: sign * den / divisor,
        })
    }

    pub fn int(value: i128) -> Self {
        Self { num: value, den: 1 }
    }

    /// `2.5` as 5/2.
    fn parse(digits: &str) -> Option<Self> {
        let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        if fraction.len() > 12 || whole.len() > 18 {
            return None;
        }
        let scale = 10i128.checked_pow(fraction.len() as u32)?;
        let whole: i128 = if whole.is_empty() {
            0
        } else {
            whole.parse().ok()?
        };
        let fraction: i128 = if fraction.is_empty() {
            0
        } else {
            fraction.parse().ok()?
        };
        Self::new(whole.checked_mul(scale)?.checked_add(fraction)?, scale)
    }

    pub fn is_zero(self) -> bool {
        self.num == 0
    }

    fn add(self, other: Self) -> Option<Self> {
        Self::new(
            self.num
                .checked_mul(other.den)?
                .checked_add(other.num.checked_mul(self.den)?)?,
            self.den.checked_mul(other.den)?,
        )
    }

    fn neg(self) -> Self {
        Self {
            num: -self.num,
            den: self.den,
        }
    }

    fn sub(self, other: Self) -> Option<Self> {
        self.add(other.neg())
    }

    fn mul(self, other: Self) -> Option<Self> {
        Self::new(
            self.num.checked_mul(other.num)?,
            self.den.checked_mul(other.den)?,
        )
    }

    fn div(self, other: Self) -> Option<Self> {
        if other.is_zero() {
            return None;
        }
        Self::new(
            self.num.checked_mul(other.den)?,
            self.den.checked_mul(other.num)?,
        )
    }

    fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    fn signum(self) -> i128 {
        self.num.signum()
    }

    fn abs(self) -> Self {
        Self {
            num: self.num.abs(),
            den: self.den,
        }
    }

    /// TeX: `\frac{3}{4}`, `-2`.
    fn tex(self) -> String {
        if self.den == 1 {
            self.num.to_string()
        } else if self.num < 0 {
            format!("-\\frac{{{}}}{{{}}}", -self.num, self.den)
        } else {
            format!("\\frac{{{}}}{{{}}}", self.num, self.den)
        }
    }
}

impl fmt::Display for Q {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(formatter, "{}", self.num)
        } else {
            write!(formatter, "{}/{}", self.num, self.den)
        }
    }
}

/// The largest square dividing `n`: `n = k² m` with `m` square-free; `(k, m)`.
fn square_part(n: i128) -> Option<(i128, i128)> {
    if n <= 0 || n > 1_000_000_000_000_000 {
        return None;
    }
    let mut outside = 1i128;
    let mut inside = n;
    let mut factor = 2i128;
    while factor * factor <= inside {
        while inside % (factor * factor) == 0 {
            inside /= factor * factor;
            outside *= factor;
        }
        factor += 1;
    }
    Some((outside, inside))
}

/// A polynomial: coefficients from the constant up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Poly(Vec<Q>);

impl Poly {
    fn constant(value: Q) -> Self {
        Self(vec![value])
    }

    fn variable() -> Self {
        Self(vec![Q::ZERO, Q::ONE])
    }

    fn trimmed(mut self) -> Self {
        while self.0.len() > 1 && self.0.last().is_some_and(|last| last.is_zero()) {
            self.0.pop();
        }
        if self.0.is_empty() {
            self.0.push(Q::ZERO);
        }
        self
    }

    pub fn degree(&self) -> usize {
        self.0.len() - 1
    }

    fn coefficient(&self, power: usize) -> Q {
        self.0.get(power).copied().unwrap_or(Q::ZERO)
    }

    fn add(&self, other: &Self) -> Option<Self> {
        let length = self.0.len().max(other.0.len());
        let mut out = Vec::with_capacity(length);
        for power in 0..length {
            out.push(self.coefficient(power).add(other.coefficient(power))?);
        }
        Some(Self(out).trimmed())
    }

    fn neg(&self) -> Self {
        Self(self.0.iter().map(|coefficient| coefficient.neg()).collect())
    }

    fn sub(&self, other: &Self) -> Option<Self> {
        self.add(&other.neg())
    }

    fn mul(&self, other: &Self) -> Option<Self> {
        if self.degree() + other.degree() > MAX_DEGREE {
            return None;
        }
        let mut out = vec![Q::ZERO; self.0.len() + other.0.len() - 1];
        for (i, a) in self.0.iter().enumerate() {
            for (j, b) in other.0.iter().enumerate() {
                out[i + j] = out[i + j].add(a.mul(*b)?)?;
            }
        }
        Some(Self(out).trimmed())
    }

    fn pow(&self, power: u32) -> Option<Self> {
        let mut out = Self::constant(Q::ONE);
        for _ in 0..power {
            out = out.mul(self)?;
        }
        Some(out)
    }

    fn derivative(&self) -> Option<Self> {
        if self.degree() == 0 {
            return Some(Self::constant(Q::ZERO));
        }
        let mut out = Vec::with_capacity(self.0.len() - 1);
        for (power, coefficient) in self.0.iter().enumerate().skip(1) {
            out.push(coefficient.mul(Q::int(power as i128))?);
        }
        Some(Self(out).trimmed())
    }

    fn integral(&self) -> Option<Self> {
        let mut out = vec![Q::ZERO];
        for (power, coefficient) in self.0.iter().enumerate() {
            out.push(coefficient.div(Q::int(power as i128 + 1))?);
        }
        Some(Self(out).trimmed())
    }

    /// TeX in descending powers: `3x^{2} - \frac{1}{2}x + 1`.
    pub fn tex(&self, variable: char) -> String {
        let mut out = String::new();
        for power in (0..self.0.len()).rev() {
            let coefficient = self.0[power];
            if coefficient.is_zero() && !(power == 0 && out.is_empty()) {
                continue;
            }
            let negative = coefficient.signum() < 0;
            if out.is_empty() {
                if negative {
                    out.push('-');
                }
            } else {
                out.push_str(if negative { " - " } else { " + " });
            }
            let size = coefficient.abs();
            if power == 0 || size != Q::ONE {
                out.push_str(&size.tex());
            }
            match power {
                0 => {}
                1 => out.push(variable),
                _ => out.push_str(&format!("{variable}^{{{power}}}")),
            }
        }
        out
    }

    /// As text: `3x^2 - 1/2x + 1`.
    pub fn text(&self, variable: char) -> String {
        let mut out = String::new();
        for power in (0..self.0.len()).rev() {
            let coefficient = self.0[power];
            if coefficient.is_zero() && !(power == 0 && out.is_empty()) {
                continue;
            }
            let negative = coefficient.signum() < 0;
            if out.is_empty() {
                if negative {
                    out.push('-');
                }
            } else {
                out.push_str(if negative { " - " } else { " + " });
            }
            let size = coefficient.abs();
            if power == 0 || size != Q::ONE {
                out.push_str(&size.to_string());
            }
            match power {
                0 => {}
                1 => out.push(variable),
                _ => out.push_str(&format!("{variable}^{power}")),
            }
        }
        out
    }

    fn value_at(&self, x: f64) -> f64 {
        self.0
            .iter()
            .rev()
            .fold(0.0, |sum, coefficient| sum * x + coefficient.to_f64())
    }
}

// ---- Reading polynomials ----

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(String),
    Variable(char),
    Symbol(char),
}

fn tokens(text: &str) -> Option<(Vec<Token>, Option<char>)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut variable: Option<char> = None;
    let mut at = 0;
    while at < chars.len() {
        let char = chars[at];
        if char.is_whitespace() {
            at += 1;
        } else if char.is_ascii_digit() || char == '.' {
            let start = at;
            while chars
                .get(at)
                .is_some_and(|next| next.is_ascii_digit() || *next == '.')
            {
                at += 1;
            }
            out.push(Token::Number(chars[start..at].iter().collect()));
        } else if char.is_ascii_alphabetic() {
            // One letter at a time, and only one letter in all: `2x`, not `sin`.
            if chars.get(at + 1).is_some_and(char::is_ascii_alphabetic) {
                return None;
            }
            let letter = char.to_ascii_lowercase();
            if matches!(letter, 'e' | 'i') || variable.is_some_and(|known| known != letter) {
                return None;
            }
            variable = Some(letter);
            out.push(Token::Variable(letter));
            at += 1;
        } else if matches!(char, '+' | '-' | '*' | '/' | '^' | '(' | ')') {
            out.push(Token::Symbol(char));
            at += 1;
        } else {
            return None;
        }
    }
    Some((out, variable))
}

struct Reader {
    tokens: Vec<Token>,
    at: usize,
}

impl Reader {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn take(&mut self, symbol: char) -> bool {
        if self.peek() == Some(&Token::Symbol(symbol)) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn sum(&mut self, depth: u32) -> Option<Poly> {
        if depth > 30 {
            return None;
        }
        let mut total = self.product(depth)?;
        loop {
            if self.take('+') {
                total = total.add(&self.product(depth)?)?;
            } else if self.take('-') {
                total = total.sub(&self.product(depth)?)?;
            } else {
                return Some(total);
            }
        }
    }

    fn product(&mut self, depth: u32) -> Option<Poly> {
        let mut total = self.signed(depth)?;
        loop {
            if self.take('*') {
                total = total.mul(&self.signed(depth)?)?;
            } else if self.take('/') {
                let divisor = self.signed(depth)?;
                if divisor.degree() != 0 || divisor.0[0].is_zero() {
                    return None;
                }
                let inverse = Q::ONE.div(divisor.0[0])?;
                total = total.mul(&Poly::constant(inverse))?;
            } else if matches!(
                self.peek(),
                Some(Token::Number(_) | Token::Variable(_) | Token::Symbol('('))
            ) {
                // `2x`, `3(x + 1)`, `x(x - 1)`.
                total = total.mul(&self.power(depth)?)?;
            } else {
                return Some(total);
            }
        }
    }

    fn signed(&mut self, depth: u32) -> Option<Poly> {
        if self.take('-') {
            return Some(self.signed(depth + 1)?.neg());
        }
        if self.take('+') {
            return self.signed(depth + 1);
        }
        self.power(depth)
    }

    fn power(&mut self, depth: u32) -> Option<Poly> {
        let base = self.atom(depth)?;
        if self.take('^') {
            let Some(Token::Number(digits)) = self.peek().cloned() else {
                return None;
            };
            self.at += 1;
            let power: u32 = digits.parse().ok()?;
            if power as usize > MAX_DEGREE {
                return None;
            }
            return base.pow(power);
        }
        Some(base)
    }

    fn atom(&mut self, depth: u32) -> Option<Poly> {
        match self.peek()?.clone() {
            Token::Number(digits) => {
                self.at += 1;
                Some(Poly::constant(Q::parse(&digits)?))
            }
            Token::Variable(_) => {
                self.at += 1;
                Some(Poly::variable())
            }
            Token::Symbol('(') => {
                self.at += 1;
                let inner = self.sum(depth + 1)?;
                self.take(')').then_some(inner)
            }
            Token::Symbol(_) => None,
        }
    }
}

/// A polynomial and its variable, when `text` is one.
pub fn polynomial(text: &str) -> Option<(Poly, Option<char>)> {
    let (tokens, variable) = tokens(text)?;
    if tokens.is_empty() {
        return None;
    }
    let mut reader = Reader { tokens, at: 0 };
    let poly = reader.sum(0)?;
    (reader.at == reader.tokens.len()).then_some((poly, variable))
}

// ---- Solving ----

/// One root of an equation.
#[derive(Debug, Clone, PartialEq)]
enum Root {
    Exact(Q),
    /// `a ± b√m` (`m` square-free; `i` when complex).
    Surd {
        a: Q,
        b: Q,
        m: i128,
        complex: bool,
    },
    Numeric {
        re: f64,
        im: f64,
    },
}

/// What an equation's solutions are.
#[derive(Debug, Clone, PartialEq)]
enum Solutions {
    Every,
    None,
    Roots(Vec<Root>),
}

fn solve(poly: &Poly) -> Option<Solutions> {
    match poly.degree() {
        0 => Some(if poly.0[0].is_zero() {
            Solutions::Every
        } else {
            Solutions::None
        }),
        1 => Some(Solutions::Roots(vec![Root::Exact(
            poly.0[0].neg().div(poly.0[1])?,
        )])),
        2 => {
            let (c, b, a) = (poly.0[0], poly.0[1], poly.0[2]);
            let discriminant = b.mul(b)?.sub(Q::int(4).mul(a)?.mul(c)?)?;
            let two_a = Q::int(2).mul(a)?;
            let centre = b.neg().div(two_a)?;
            if discriminant.is_zero() {
                return Some(Solutions::Roots(vec![Root::Exact(centre)]));
            }
            let complex = discriminant.signum() < 0;
            // √(p/q) = √(p·q)/q.
            let size = discriminant.abs();
            let (outside, inside) = square_part(size.num.checked_mul(size.den)?)?;
            let spread = Q::new(outside, size.den)?.div(two_a.abs())?;
            if inside == 1 && !complex {
                let mut roots = vec![
                    Root::Exact(centre.sub(spread)?),
                    Root::Exact(centre.add(spread)?),
                ];
                roots.sort_by(|left, right| match (left, right) {
                    (Root::Exact(left), Root::Exact(right)) => left
                        .to_f64()
                        .partial_cmp(&right.to_f64())
                        .unwrap_or(Ordering::Equal),
                    _ => Ordering::Equal,
                });
                return Some(Solutions::Roots(roots));
            }
            Some(Solutions::Roots(vec![Root::Surd {
                a: centre,
                b: spread,
                m: inside,
                complex,
            }]))
        }
        _ => Some(Solutions::Roots(numeric_roots(poly)?)),
    }
}

/// Roots of a polynomial by the Durand–Kerner method, real ones first in order.
fn numeric_roots(poly: &Poly) -> Option<Vec<Root>> {
    let degree = poly.degree();
    let lead = poly.0[degree].to_f64();
    let coefficients: Vec<f64> = poly
        .0
        .iter()
        .map(|coefficient| coefficient.to_f64() / lead)
        .collect();
    let value = |re: f64, im: f64| -> (f64, f64) {
        // Horner's rule with complex numbers.
        let mut sum = (0.0, 0.0);
        for coefficient in coefficients.iter().rev() {
            sum = (
                sum.0 * re - sum.1 * im + coefficient,
                sum.0 * im + sum.1 * re,
            );
        }
        sum
    };
    let mut roots: Vec<(f64, f64)> = (0..degree)
        .map(|k| {
            let angle = 0.4 + 2.0 * std::f64::consts::PI * k as f64 / degree as f64;
            (0.9 * angle.cos(), 0.9 * angle.sin())
        })
        .collect();
    for _ in 0..500 {
        let mut largest_step: f64 = 0.0;
        for i in 0..degree {
            let (re, im) = roots[i];
            let (top_re, top_im) = value(re, im);
            let mut bottom = (1.0, 0.0);
            for (j, other) in roots.iter().enumerate() {
                if i == j {
                    continue;
                }
                let difference = (re - other.0, im - other.1);
                bottom = (
                    bottom.0 * difference.0 - bottom.1 * difference.1,
                    bottom.0 * difference.1 + bottom.1 * difference.0,
                );
            }
            let size = bottom.0 * bottom.0 + bottom.1 * bottom.1;
            if size == 0.0 {
                return None;
            }
            let step = (
                (top_re * bottom.0 + top_im * bottom.1) / size,
                (top_im * bottom.0 - top_re * bottom.1) / size,
            );
            roots[i] = (re - step.0, im - step.1);
            largest_step = largest_step.max(step.0.abs() + step.1.abs());
        }
        if largest_step < 1e-14 {
            break;
        }
    }
    let scale = roots
        .iter()
        .map(|(re, im)| re.abs() + im.abs())
        .fold(1.0, f64::max);
    let mut found: Vec<Root> = roots
        .into_iter()
        .filter(|(re, im)| re.is_finite() && im.is_finite())
        .map(|(re, im)| Root::Numeric {
            re: if re.abs() < 1e-12 * scale { 0.0 } else { re },
            im: if im.abs() < 1e-9 * scale { 0.0 } else { im },
        })
        .collect();
    found.sort_by(|left, right| match (left, right) {
        (Root::Numeric { re: a, im: x }, Root::Numeric { re: b, im: y }) => (x.abs() > 0.0)
            .cmp(&(y.abs() > 0.0))
            .then(a.partial_cmp(b).unwrap_or(Ordering::Equal)),
        _ => Ordering::Equal,
    });
    (found.len() == degree).then_some(found)
}

fn root_tex(root: &Root, variable: char) -> String {
    match root {
        Root::Exact(value) => format!("{variable} = {}", value.tex()),
        Root::Surd { a, b, m, complex } => {
            let unit = if *complex { "i" } else { "" };
            let root = if *m == 1 {
                String::new()
            } else {
                format!("\\sqrt{{{m}}}")
            };
            let core = format!("{root}{unit}");
            let core = if core.is_empty() {
                "1".to_owned()
            } else {
                core
            };
            let spread = if *b == Q::ONE {
                core
            } else if b.den == 1 {
                format!("{}{core}", b.num)
            } else {
                let top = if b.num == 1 {
                    core
                } else {
                    format!("{}{core}", b.num)
                };
                format!("\\frac{{{top}}}{{{}}}", b.den)
            };
            if a.is_zero() {
                format!("{variable} = \\pm {spread}")
            } else {
                format!("{variable} = {} \\pm {spread}", a.tex())
            }
        }
        Root::Numeric { re, im } => format!("{variable} \\approx {}", complex_text(*re, *im).1),
    }
}

/// A complex number as text and TeX, 10 significant digits.
fn complex_text(re: f64, im: f64) -> (String, String) {
    let real = show_f64(re, 10);
    if im == 0.0 {
        return (real.text, real.tex);
    }
    let imaginary = show_f64(im.abs(), 10);
    let sign = if im < 0.0 { "-" } else { "+" };
    if re == 0.0 {
        let sign = if im < 0.0 { "-" } else { "" };
        return (
            format!("{sign}{}i", imaginary.text),
            format!("{sign}{}\\,i", imaginary.tex),
        );
    }
    (
        format!("{} {sign} {}i", real.text, imaginary.text),
        format!("{} {sign} {}\\,i", real.tex, imaginary.tex),
    )
}

fn root_text(root: &Root, variable: char) -> String {
    match root {
        Root::Exact(value) => format!("{variable} = {value}"),
        Root::Surd { a, b, m, complex } => {
            let unit = if *complex { "i" } else { "" };
            let root = if *m == 1 {
                String::new()
            } else {
                format!("√{m}")
            };
            let core = format!("{root}{unit}");
            let spread = if *b == Q::ONE {
                core
            } else if root.is_empty() {
                format!("{b}{unit}")
            } else {
                format!("{b}·{core}")
            };
            if a.is_zero() {
                format!("{variable} = ±{spread}")
            } else {
                format!("{variable} = {a} ± {spread}")
            }
        }
        Root::Numeric { re, im } => format!("{variable} ≈ {}", complex_text(*re, *im).0),
    }
}

/// TeX written as plain text: `\left(x - 2\right)^{2}` → `(x - 2)^2`.
fn plain(tex: &str) -> String {
    tex.replace("\\left(", "(")
        .replace("\\right)", ")")
        .replace("^{", "^")
        .replace('}', "")
}

/// Decimal values of roots that aren't whole fractions already.
fn root_decimals(root: &Root, variable: char) -> Vec<String> {
    match root {
        Root::Exact(value) if value.den != 1 => {
            vec![format!(
                "{variable} ≈ {}",
                show_f64(value.to_f64(), 10).text
            )]
        }
        Root::Surd { a, b, m, complex } => {
            let spread = b.to_f64() * (*m as f64).sqrt();
            if *complex {
                vec![
                    format!("{variable} ≈ {}", complex_text(a.to_f64(), -spread).0),
                    format!("{variable} ≈ {}", complex_text(a.to_f64(), spread).0),
                ]
            } else {
                vec![
                    format!("{variable} ≈ {}", show_f64(a.to_f64() - spread, 10).text),
                    format!("{variable} ≈ {}", show_f64(a.to_f64() + spread, 10).text),
                ]
            }
        }
        _ => Vec::new(),
    }
}

/// What's asked of a polynomial.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Task {
    Solve,
    Expand,
    Factor,
    Derive,
    Integrate,
}

const TASKS: &[(&str, Task)] = &[
    ("solve for x", Task::Solve),
    ("solve", Task::Solve),
    ("roots of", Task::Solve),
    ("zeros of", Task::Solve),
    ("zeroes of", Task::Solve),
    ("expand", Task::Expand),
    ("simplify", Task::Expand),
    ("factorise", Task::Factor),
    ("factorize", Task::Factor),
    ("factor", Task::Factor),
    ("derivative of", Task::Derive),
    ("the derivative of", Task::Derive),
    ("differentiate", Task::Derive),
    ("d/dx", Task::Derive),
    ("integral of", Task::Integrate),
    ("the integral of", Task::Integrate),
    ("antiderivative of", Task::Integrate),
    ("integrate", Task::Integrate),
];

fn factored_tex(poly: &Poly, variable: char) -> Option<String> {
    if poly.degree() != 2 {
        return None;
    }
    let Solutions::Roots(roots) = solve(poly)? else {
        return None;
    };
    let lead = poly.0[2];
    let factor = |root: Q| -> String {
        // (x - p/q) written as (qx - p).
        let (num, den) = (root.num, root.den);
        let scale = if den == 1 {
            String::new()
        } else {
            den.to_string()
        };
        if num == 0 {
            format!("{scale}{variable}")
        } else if num < 0 {
            format!("\\left({scale}{variable} + {}\\right)", -num)
        } else {
            format!("\\left({scale}{variable} - {num}\\right)")
        }
    };
    let (first, second) = match roots.as_slice() {
        [Root::Exact(root)] => (*root, *root),
        [Root::Exact(first), Root::Exact(second)] => (*first, *second),
        _ => return None,
    };
    // The leading coefficient left over once the denominators moved inside.
    let left = lead.div(Q::int(first.den).mul(Q::int(second.den))?)?;
    let prefix = if left == Q::ONE {
        String::new()
    } else if left == Q::ONE.neg() {
        "-".to_owned()
    } else {
        left.tex()
    };
    if first == second {
        let inner = factor(first);
        return Some(format!("{prefix}{inner}^{{2}}"));
    }
    Some(format!("{prefix}{}{}", factor(first), factor(second)))
}

/// An equation or a task on a polynomial, from `lower`.
pub fn answer(lower: &str) -> Option<Found> {
    let mut task: Option<Task> = None;
    let mut rest = lower.trim();
    for (name, found) in TASKS {
        if let Some(after) = rest.strip_prefix(name)
            && (after.starts_with(' ') || after.starts_with('('))
        {
            task = Some(*found);
            rest = after.trim();
            break;
        }
    }
    let rest = rest.strip_suffix(" for x").unwrap_or(rest);
    if let Some((left, right)) = rest.split_once('=') {
        if right.contains('=')
            || matches!(
                task,
                Some(Task::Expand | Task::Factor | Task::Derive | Task::Integrate)
            )
        {
            return None;
        }
        let (left, left_variable) = polynomial(left)?;
        let (right, right_variable) = polynomial(right)?;
        let variable = match (left_variable, right_variable) {
            (Some(a), Some(b)) if a != b => return None,
            (Some(a), _) | (_, Some(a)) => a,
            (None, None) => return None,
        };
        let equation = left.sub(&right)?;
        let interpretation = (
            format!(
                "solve {} = {} for {variable}",
                left.text(variable),
                right.text(variable)
            ),
            format!(
                "\\text{{solve }} {} = {}",
                left.tex(variable),
                right.tex(variable)
            ),
        );
        return solved(equation, variable, interpretation);
    }
    let task = task?;
    let (poly, variable) = polynomial(rest)?;
    let variable = variable.unwrap_or('x');
    let mut found = blank(AnswerKind::Algebra);
    let (result_text, result_tex, verb) = match task {
        Task::Solve => {
            let interpretation = (
                format!("solve {} = 0 for {variable}", poly.text(variable)),
                format!("\\text{{solve }} {} = 0", poly.tex(variable)),
            );
            return solved(poly, variable, interpretation);
        }
        Task::Expand => (poly.text(variable), poly.tex(variable), "expand"),
        Task::Factor => (
            poly.text(variable),
            factored_tex(&poly, variable)?,
            "factor",
        ),
        Task::Derive => {
            let derivative = poly.derivative()?;
            (
                derivative.text(variable),
                derivative.tex(variable),
                "derivative",
            )
        }
        Task::Integrate => {
            let integral = poly.integral()?;
            (
                format!("{} + C", integral.text(variable)),
                format!("{} + C", integral.tex(variable)),
                "integral",
            )
        }
    };
    let interpretation_tex = match task {
        Task::Derive => format!(
            "\\frac{{d}}{{d{variable}}}\\left({}\\right)",
            poly.tex(variable)
        ),
        Task::Integrate => format!("\\int {}\\, d{variable}", poly.tex(variable)),
        _ => format!("\\text{{{verb} }} {}", poly.tex(variable)),
    };
    found.interpretation = Some(math_text(
        format!("{verb} {}", poly.text(variable)),
        &interpretation_tex,
    ));
    let result_text = if task == Task::Factor {
        plain(&result_tex)
    } else {
        result_text
    };
    found.copy = result_text.clone();
    found.result = math_text(result_text, &result_tex);
    Some(Found {
        answer: found,
        strong: true,
    })
}

fn solved(equation: Poly, variable: char, interpretation: (String, String)) -> Option<Found> {
    if equation.degree() > MAX_DEGREE {
        return None;
    }
    let solutions = solve(&equation)?;
    let mut found = blank(AnswerKind::Algebra);
    found.interpretation = Some(math_text(interpretation.0, &interpretation.1));
    match solutions {
        Solutions::Every => {
            found.result = super::AnswerText::plain(format!("true for every {variable}"));
            found.copy = found.result.text.clone();
        }
        Solutions::None => {
            found.result = super::AnswerText::plain("no solution");
            found.copy = found.result.text.clone();
        }
        Solutions::Roots(roots) => {
            let text = roots
                .iter()
                .map(|root| root_text(root, variable))
                .collect::<Vec<_>>()
                .join(", ");
            let tex = roots
                .iter()
                .map(|root| root_tex(root, variable))
                .collect::<Vec<_>>()
                .join(",\\quad ");
            found.approximate = roots
                .iter()
                .any(|root| matches!(root, Root::Numeric { .. }));
            found.copy = text.clone();
            found.result = math_text(text, &tex);
            let decimals: Vec<String> = roots
                .iter()
                .flat_map(|root| root_decimals(root, variable))
                .collect();
            if !decimals.is_empty() {
                found.rows.push(AnswerRow {
                    key: "decimal".to_owned(),
                    label: "Decimal approximation".to_owned(),
                    value: super::AnswerText::plain(decimals.join(", ")),
                });
            }
            if let Some(factored) = factored_tex(&equation, variable) {
                found.rows.push(AnswerRow {
                    key: "factored".to_owned(),
                    label: "Factored".to_owned(),
                    value: math_text(
                        format!("{} = 0", equation.text(variable)),
                        &format!("{factored} = 0"),
                    ),
                });
            }
            // A check that the roots found make it zero.
            debug_assert!(roots.iter().all(|root| match root {
                Root::Exact(value) => equation.value_at(value.to_f64()).abs() < 1e-6,
                _ => true,
            }));
        }
    }
    Some(Found {
        answer: found,
        strong: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(query: &str) -> String {
        answer(query).unwrap().answer.result.text
    }

    #[test]
    fn linear_equations_are_solved_exactly() {
        assert_eq!(result("solve 2x + 3 = 7"), "x = 2");
        assert_eq!(result("2x+3=7"), "x = 2");
        assert_eq!(result("3y = 1"), "y = 1/3");
        assert_eq!(result("x/2 + 1 = 4"), "x = 6");
        assert_eq!(result("2(x - 1) = x"), "x = 2");
        assert_eq!(result("x = x"), "true for every x");
        assert_eq!(result("x + 1 = x"), "no solution");
    }

    #[test]
    fn quadratics_are_solved_exactly() {
        assert_eq!(result("x^2 - 5x + 6 = 0"), "x = 2, x = 3");
        assert_eq!(result("solve x^2 = 2"), "x = ±√2");
        assert_eq!(result("x^2 + 2x - 1 = 0"), "x = -1 ± √2");
        assert_eq!(result("x^2 + 1 = 0"), "x = ±i");
        assert_eq!(result("x^2 + 2x + 5 = 0"), "x = -1 ± 2i");
        assert_eq!(result("x^2 - 4x + 4 = 0"), "x = 2");
        assert_eq!(result("2x^2 - 8 = 0"), "x = -2, x = 2");
        let surd = answer("x^2 - 2 = 0").unwrap().answer;
        assert!(surd.result.math.is_some());
        assert_eq!(surd.rows[0].value.text, "x ≈ -1.414213562, x ≈ 1.414213562");
    }

    #[test]
    fn higher_degrees_are_solved_numerically() {
        let cubic = answer("x^3 - 6x^2 + 11x - 6 = 0").unwrap().answer;
        assert!(cubic.approximate);
        assert_eq!(cubic.result.text, "x ≈ 1, x ≈ 2, x ≈ 3");
    }

    #[test]
    fn polynomials_are_worked_on() {
        assert_eq!(result("expand (x+1)^2"), "x^2 + 2x + 1");
        assert_eq!(result("derivative of x^3 + 2x"), "3x^2 + 2");
        assert_eq!(result("integrate 3x^2"), "x^3 + C");
        assert_eq!(result("factor x^2 - 5x + 6"), "(x - 2)(x - 3)");
        assert_eq!(result("factor 2x^2 - x - 1"), "(2x + 1)(x - 1)");
    }

    #[test]
    fn other_words_are_not_algebra() {
        for words in [
            "x",
            "solve",
            "Blue",
            "1984",
            "2+2",
            "solve the mystery",
            "x y = 3",
            "sin x = 1",
        ] {
            assert!(answer(words).is_none(), "{words}");
        }
    }
}
