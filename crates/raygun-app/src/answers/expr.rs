//! Arithmetic as typed in Search, read into a tree: it says whether the words are a calculation
//! at all (`2+2`, `sqrt(2)*3`, `5!`) rather than a title (`1984`, `Blue`), and how to write the
//! calculation as a formula ("Input interpretation"). fend computes the value.

/// Functions the reader knows, with how many arguments they take.
const FUNCTIONS: &[&str] = &[
    "sqrt", "cbrt", "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "ln",
    "log", "log2", "log10", "exp", "abs", "floor", "ceil", "round",
];
/// Named numbers.
const CONSTANTS: &[&str] = &["pi", "e", "phi", "tau"];

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(String),
    Word(String),
    Symbol(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    /// Written without a sign: `2pi`, `3(4+5)`.
    Juxtaposed,
    Div,
    Pow,
    Mod,
}

/// A calculation.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Digits as written (thousands separators left out), maybe `1e10` or `0xff`.
    Number(String),
    Constant(&'static str),
    Neg(Box<Expr>),
    Binary(Op, Box<Expr>, Box<Expr>),
    Factorial(Box<Expr>),
    Percent(Box<Expr>),
    Call(&'static str, Box<Expr>),
}

fn tokens(text: &str) -> Option<Vec<Token>> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let char = chars[at];
        if char.is_whitespace() {
            at += 1;
            continue;
        }
        if char.is_ascii_digit()
            || (char == '.' && chars.get(at + 1).is_some_and(char::is_ascii_digit))
        {
            let start = at;
            // 0x…, 0b…, 0o…
            if char == '0'
                && let Some(base) = chars.get(at + 1).copied()
                && matches!(base, 'x' | 'b' | 'o')
                && chars
                    .get(at + 2)
                    .is_some_and(|next| next.is_ascii_hexdigit())
            {
                at += 2;
                while chars.get(at).is_some_and(char::is_ascii_hexdigit) {
                    at += 1;
                }
                out.push(Token::Number(chars[start..at].iter().collect()));
                continue;
            }
            let mut digits = String::new();
            while at < chars.len() {
                let next = chars[at];
                if next.is_ascii_digit() || next == '.' {
                    digits.push(next);
                    at += 1;
                } else if next == ','
                    && chars
                        .get(at + 1..at + 4)
                        .is_some_and(|group| group.iter().all(char::is_ascii_digit))
                    && !chars.get(at + 4).is_some_and(char::is_ascii_digit)
                {
                    // A thousands separator: `1,000`.
                    at += 1;
                } else if matches!(next, 'e' | 'E')
                    && (chars.get(at + 1).is_some_and(char::is_ascii_digit)
                        || (matches!(chars.get(at + 1), Some('-' | '+'))
                            && chars.get(at + 2).is_some_and(char::is_ascii_digit)))
                {
                    digits.push('e');
                    at += 1;
                    if matches!(chars[at], '-' | '+') {
                        digits.push(chars[at]);
                        at += 1;
                    }
                } else {
                    break;
                }
            }
            if digits.matches('.').count() > 1 {
                return None;
            }
            out.push(Token::Number(digits));
            continue;
        }
        if char.is_alphabetic() {
            let start = at;
            while chars
                .get(at)
                .is_some_and(|next| next.is_alphanumeric() || *next == '_')
            {
                at += 1;
            }
            out.push(Token::Word(
                chars[start..at].iter().collect::<String>().to_lowercase(),
            ));
            continue;
        }
        if char == '*' && chars.get(at + 1) == Some(&'*') {
            out.push(Token::Symbol('^'));
            at += 2;
            continue;
        }
        if matches!(char, '+' | '-' | '*' | '/' | '^' | '(' | ')' | '!' | '%') {
            out.push(Token::Symbol(char));
            at += 1;
            continue;
        }
        return None;
    }
    Some(out)
}

struct Reader {
    tokens: Vec<Token>,
    at: usize,
}

impl Reader {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn take_symbol(&mut self, symbol: char) -> bool {
        if self.peek() == Some(&Token::Symbol(symbol)) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn take_word(&mut self, word: &str) -> bool {
        if matches!(self.peek(), Some(Token::Word(found)) if found == word) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expr(&mut self, depth: u32) -> Option<Expr> {
        if depth > 40 {
            return None;
        }
        let mut left = self.term(depth)?;
        loop {
            if self.take_symbol('+') {
                left = Expr::Binary(Op::Add, Box::new(left), Box::new(self.term(depth)?));
            } else if self.take_symbol('-') {
                left = Expr::Binary(Op::Sub, Box::new(left), Box::new(self.term(depth)?));
            } else {
                return Some(left);
            }
        }
    }

    /// Whether the next token can start a factor written right after another (`2pi`).
    fn starts_juxtaposed(&self) -> bool {
        match self.peek() {
            Some(Token::Symbol('(')) => true,
            Some(Token::Word(word)) => {
                CONSTANTS.contains(&word.as_str()) || FUNCTIONS.contains(&word.as_str())
            }
            _ => false,
        }
    }

    fn term(&mut self, depth: u32) -> Option<Expr> {
        let mut left = self.unary(depth)?;
        loop {
            if self.take_symbol('*') {
                left = Expr::Binary(Op::Mul, Box::new(left), Box::new(self.unary(depth)?));
            } else if self.take_symbol('/') {
                left = Expr::Binary(Op::Div, Box::new(left), Box::new(self.unary(depth)?));
            } else if self.take_word("mod") {
                left = Expr::Binary(Op::Mod, Box::new(left), Box::new(self.unary(depth)?));
            } else if matches!(left, Expr::Percent(_)) && self.take_word("of") {
                // `10% of 50`.
                left = Expr::Binary(Op::Mul, Box::new(left), Box::new(self.unary(depth)?));
            } else if self.starts_juxtaposed() {
                left = Expr::Binary(Op::Juxtaposed, Box::new(left), Box::new(self.power(depth)?));
            } else {
                return Some(left);
            }
        }
    }

    fn unary(&mut self, depth: u32) -> Option<Expr> {
        if self.take_symbol('-') {
            return Some(Expr::Neg(Box::new(self.unary(depth + 1)?)));
        }
        if self.take_symbol('+') {
            return self.unary(depth + 1);
        }
        self.power(depth)
    }

    fn power(&mut self, depth: u32) -> Option<Expr> {
        let base = self.postfix(depth)?;
        if self.take_symbol('^') {
            let exponent = self.unary(depth + 1)?;
            return Some(Expr::Binary(Op::Pow, Box::new(base), Box::new(exponent)));
        }
        Some(base)
    }

    fn postfix(&mut self, depth: u32) -> Option<Expr> {
        let mut atom = self.atom(depth)?;
        loop {
            if self.take_symbol('!') {
                atom = Expr::Factorial(Box::new(atom));
            } else if self.take_symbol('%') {
                atom = Expr::Percent(Box::new(atom));
            } else {
                return Some(atom);
            }
        }
    }

    fn atom(&mut self, depth: u32) -> Option<Expr> {
        match self.peek()?.clone() {
            Token::Number(digits) => {
                self.at += 1;
                Some(Expr::Number(digits))
            }
            Token::Symbol('(') => {
                self.at += 1;
                let inner = self.expr(depth + 1)?;
                self.take_symbol(')').then_some(inner)
            }
            Token::Word(word) => {
                self.at += 1;
                if let Some(constant) = CONSTANTS.iter().find(|known| **known == word) {
                    return Some(Expr::Constant(constant));
                }
                let function = FUNCTIONS.iter().find(|known| **known == word)?;
                // `sqrt(2)`, or `sqrt 2`.
                let argument = if self.take_symbol('(') {
                    let inner = self.expr(depth + 1)?;
                    if !self.take_symbol(')') {
                        return None;
                    }
                    inner
                } else {
                    self.power(depth + 1)?
                };
                Some(Expr::Call(function, Box::new(argument)))
            }
            Token::Symbol(_) => None,
        }
    }
}

/// The calculation `text` is, when all of it reads as one.
pub fn parse(text: &str) -> Option<Expr> {
    let tokens = tokens(text)?;
    if tokens.is_empty() {
        return None;
    }
    let mut reader = Reader { tokens, at: 0 };
    let expr = reader.expr(0)?;
    (reader.at == reader.tokens.len()).then_some(expr)
}

impl Expr {
    /// Something is computed: an operation, a function, a factorial or a percentage, not just a
    /// number or a name.
    pub fn computes(&self) -> bool {
        match self {
            Self::Number(_) | Self::Constant(_) => false,
            Self::Neg(inner) => inner.computes(),
            Self::Binary(..) | Self::Factorial(_) | Self::Call(..) => true,
            Self::Percent(inner) => inner.computes(),
        }
    }

    /// Integers joined by `-` with nothing else, or one division of two small integers: more
    /// likely a phone number, a formation, a range of years or a date (`1-800-273-8255`,
    /// `4-3-3`, `1939-1945`, `9/11`) than a calculation someone wants done.
    pub fn looks_like_a_code(&self, text: &str) -> bool {
        let integer = |expr: &Self| matches!(expr, Self::Number(digits) if digits.bytes().all(|byte| byte.is_ascii_digit()));
        let hyphens_only = !text.contains(' ')
            && text.split('-').count() >= 3
            && text
                .split('-')
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()));
        if hyphens_only {
            return true;
        }
        match self {
            Self::Binary(Op::Div, left, right) if integer(left) && integer(right) => {
                let small = |expr: &Self| match expr {
                    Self::Number(digits) => digits.parse::<u32>().is_ok_and(|number| number <= 31),
                    _ => false,
                };
                small(left) && small(right)
            }
            Self::Binary(Op::Sub, left, right) if integer(left) && integer(right) => {
                let year = |expr: &Self| match expr {
                    Self::Number(digits) => {
                        digits.len() == 4
                            && digits
                                .parse::<u32>()
                                .is_ok_and(|year| (1000..=2200).contains(&year))
                    }
                    _ => false,
                };
                year(left) && year(right)
            }
            _ => false,
        }
    }

    fn atomic(&self) -> bool {
        match self {
            Self::Number(digits) => !digits.contains('e'),
            Self::Constant(_) | Self::Call(..) => true,
            Self::Factorial(_) | Self::Percent(_) => true,
            _ => false,
        }
    }

    /// The calculation as TeX.
    pub fn tex(&self) -> String {
        match self {
            Self::Number(digits) => number_tex(digits),
            Self::Constant(name) => match *name {
                "pi" => "\\pi".to_owned(),
                "phi" => "\\varphi".to_owned(),
                "tau" => "\\tau".to_owned(),
                other => other.to_owned(),
            },
            Self::Neg(inner) => {
                if matches!(**inner, Self::Binary(Op::Add | Op::Sub, ..)) {
                    format!("-\\left({}\\right)", inner.tex())
                } else {
                    format!("-{}", inner.tex())
                }
            }
            Self::Factorial(inner) => format!("{}!", wrapped(inner)),
            Self::Percent(inner) => format!("{}\\%", wrapped(inner)),
            Self::Call(name, argument) => call_tex(name, argument),
            Self::Binary(op, left, right) => {
                let sum = |expr: &Self| matches!(expr, Self::Binary(Op::Add | Op::Sub, ..));
                let side = |expr: &Self| {
                    if sum(expr) {
                        format!("\\left({}\\right)", expr.tex())
                    } else {
                        expr.tex()
                    }
                };
                match op {
                    Op::Add => format!("{} + {}", left.tex(), right.tex()),
                    Op::Sub => format!(
                        "{} - {}",
                        left.tex(),
                        if sum(right) || matches!(**right, Self::Neg(_)) {
                            format!("\\left({}\\right)", right.tex())
                        } else {
                            right.tex()
                        }
                    ),
                    Op::Mul => format!("{} \\times {}", side(left), side(right)),
                    Op::Juxtaposed => format!("{} {}", side(left), side(right)),
                    Op::Div => format!("\\frac{{{}}}{{{}}}", left.tex(), right.tex()),
                    Op::Mod => format!("{} \\bmod {}", side(left), side(right)),
                    Op::Pow => format!("{{{}}}^{{{}}}", wrapped(left), right.tex()),
                }
            }
        }
    }
}

/// An operand in parentheses unless it stands alone.
fn wrapped(expr: &Expr) -> String {
    if expr.atomic() {
        expr.tex()
    } else {
        format!("\\left({}\\right)", expr.tex())
    }
}

fn number_tex(digits: &str) -> String {
    if let Some((mantissa, exponent)) = digits.split_once('e')
        && !digits.starts_with("0x")
    {
        let exponent = exponent.trim_start_matches('+');
        return format!("{mantissa} \\times 10^{{{exponent}}}");
    }
    if let Some(hex) = digits.strip_prefix("0x") {
        return format!("\\mathrm{{{hex}}}_{{16}}");
    }
    if let Some(binary) = digits.strip_prefix("0b") {
        return format!("{binary}_{{2}}");
    }
    if let Some(octal) = digits.strip_prefix("0o") {
        return format!("{octal}_{{8}}");
    }
    digits.to_owned()
}

fn call_tex(name: &str, argument: &Expr) -> String {
    let inner = argument.tex();
    match name {
        "sqrt" => format!("\\sqrt{{{inner}}}"),
        "cbrt" => format!("\\sqrt[3]{{{inner}}}"),
        "abs" => format!("\\left|{inner}\\right|"),
        "floor" => format!("\\left\\lfloor {inner} \\right\\rfloor"),
        "ceil" => format!("\\left\\lceil {inner} \\right\\rceil"),
        "asin" => format!("\\arcsin\\left({inner}\\right)"),
        "acos" => format!("\\arccos\\left({inner}\\right)"),
        "atan" => format!("\\arctan\\left({inner}\\right)"),
        "log" | "log10" => format!("\\log_{{10}}\\left({inner}\\right)"),
        "log2" => format!("\\log_{{2}}\\left({inner}\\right)"),
        "round" => format!("\\operatorname{{round}}\\left({inner}\\right)"),
        // \sin, \cos, \tan, \sinh, \cosh, \tanh, \ln, \exp.
        other => format!("\\{other}\\left({inner}\\right)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tex(text: &str) -> String {
        parse(text).unwrap().tex()
    }

    #[test]
    fn calculations_are_read() {
        assert_eq!(tex("2+2*3"), "2 + 2 \\times 3");
        assert_eq!(tex("(1+2)*3"), "\\left(1 + 2\\right) \\times 3");
        assert_eq!(tex("sqrt(2)*3"), "\\sqrt{2} \\times 3");
        assert_eq!(tex("2pi"), "2 \\pi");
        assert_eq!(tex("2^10"), "{2}^{10}");
        assert_eq!(tex("2**8"), "{2}^{8}");
        assert_eq!(tex("1/3 + 1/4"), "\\frac{1}{3} + \\frac{1}{4}");
        assert_eq!(tex("5!"), "5!");
        assert_eq!(tex("10% of 50"), "10\\% \\times 50");
        assert_eq!(tex("-(2+3)"), "-\\left(2 + 3\\right)");
        assert_eq!(tex("10 mod 3"), "10 \\bmod 3");
        assert_eq!(tex("sin(pi/2)"), "\\sin\\left(\\frac{\\pi}{2}\\right)");
        assert_eq!(tex("1,000 + 1"), "1000 + 1");
        assert_eq!(tex("6.02e23 * 2"), "6.02 \\times 10^{23} \\times 2");
        assert_eq!(tex("0xff + 1"), "\\mathrm{ff}_{16} + 1");
    }

    #[test]
    fn titles_are_not_calculations() {
        for title in [
            "Blue",
            "The Matrix",
            "Catch-22",
            "Maroon 5",
            "U2",
            "Sum 41",
            "8 Mile",
        ] {
            assert!(parse(title).is_none(), "{title}");
        }
        for number in ["1984", "300", "-5", "pi"] {
            assert!(!parse(number).unwrap().computes(), "{number}");
        }
        assert!(parse("2+2").unwrap().computes());
        assert!(parse("sqrt 16").unwrap().computes());
    }

    #[test]
    fn codes_are_not_calculations() {
        let code = |text: &str| parse(text).unwrap().looks_like_a_code(text);
        assert!(code("1-800-273-8255"));
        assert!(code("4-3-3"));
        assert!(code("1939-1945"));
        assert!(code("9/11"));
        assert!(code("24/7"));
        assert!(!code("50/50"));
        assert!(!code("1/3 + 1/4"));
        assert!(!code("2 - 1"));
        assert!(!code("100-7"));
    }
}
