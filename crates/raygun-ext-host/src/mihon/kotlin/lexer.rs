//! Kotlin's tokens, as far as extension sources use them: names, numbers, strings with templates
//! (`"$baseUrl/manga"`, `"${page + 1}"`, raw `"""…"""`), characters, operators and the line
//! breaks that end statements. Comments are dropped.

/// A piece of a string literal: text, or a template's expression (its tokens).
#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Text(String),
    Code(Vec<Token>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Name(String),
    /// A number as written, without `_`, its suffix (`f`, `u`) kept off.
    Int(i64),
    /// A whole number written with `L` (`16777619L`, `0xFFL`): a `Long` whatever its size.
    Long(i64),
    Float(f64),
    Str(Vec<Part>),
    Char(char),
    /// Operators and punctuation: `(`, `?.`, `?:`, `!!`, `::`, `->`, `..`, `==`…
    Op(&'static str),
    Newline,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: Kind,
    /// Line, from 1, for error messages.
    pub line: u32,
}

const OPS: [&str; 41] = [
    "===", "!==", "...", "..<", "?.", "?:", "!!", "::", "->", "..", "==", "!=", "<=", ">=", "&&",
    "||", "+=", "-=", "*=", "/=", "%=", "++", "--", "(", ")", "{", "}", "[", "]", "<", ">", ",",
    ".", ";", ":", "=", "+", "-", "*", "/", "%",
];
const MORE_OPS: [&str; 4] = ["!", "?", "@", "&"];

pub fn lex(source: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut lexer = Lexer {
        chars: &chars,
        at: 0,
        line: 1,
    };
    lexer.tokens(false)
}

struct Lexer<'a> {
    chars: &'a [char],
    at: usize,
    line: u32,
}

impl Lexer<'_> {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).copied()
    }

    fn starts(&self, text: &str) -> bool {
        text.chars()
            .enumerate()
            .all(|(index, char)| self.peek(index) == Some(char))
    }

    fn error(&self, message: &str) -> String {
        format!("line {}: {message}", self.line)
    }

    /// Tokens up to the end, or (in a template) up to the `}` that closes it.
    fn tokens(&mut self, template: bool) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        let mut depth = 0usize;
        while let Some(char) = self.peek(0) {
            let line = self.line;
            let push = |tokens: &mut Vec<Token>, kind| tokens.push(Token { kind, line });
            match char {
                '\n' => {
                    self.at += 1;
                    self.line += 1;
                    push(&mut tokens, Kind::Newline);
                }
                c if c.is_whitespace() => self.at += 1,
                '/' if self.peek(1) == Some('/') => {
                    while self.peek(0).is_some_and(|c| c != '\n') {
                        self.at += 1;
                    }
                }
                '/' if self.peek(1) == Some('*') => self.block_comment()?,
                '"' => {
                    let parts = self.string(1)?;
                    push(&mut tokens, Kind::Str(parts));
                }
                // A multi-dollar string (`$$"""query ($id: ID)"""`): templates take as many `$`.
                '$' if {
                    let mut ahead = 0;
                    while self.peek(ahead) == Some('$') {
                        ahead += 1;
                    }
                    self.peek(ahead) == Some('"')
                } =>
                {
                    let mut dollars = 0;
                    while self.peek(0) == Some('$') {
                        dollars += 1;
                        self.at += 1;
                    }
                    let parts = self.string(dollars)?;
                    push(&mut tokens, Kind::Str(parts));
                }
                '\'' => {
                    let value = self.character()?;
                    push(&mut tokens, Kind::Char(value));
                }
                '`' => {
                    self.at += 1;
                    let start = self.at;
                    while self.peek(0).is_some_and(|c| c != '`' && c != '\n') {
                        self.at += 1;
                    }
                    let name: String = self.chars[start..self.at].iter().collect();
                    self.at += 1;
                    push(&mut tokens, Kind::Name(name));
                }
                c if c.is_alphabetic() || c == '_' => {
                    let start = self.at;
                    while self
                        .peek(0)
                        .is_some_and(|c| c.is_alphanumeric() || c == '_')
                    {
                        self.at += 1;
                    }
                    let name: String = self.chars[start..self.at].iter().collect();
                    push(&mut tokens, Kind::Name(name));
                }
                c if c.is_ascii_digit() => {
                    let kind = self.number()?;
                    push(&mut tokens, kind);
                }
                '}' if template && depth == 0 => {
                    self.at += 1;
                    return Ok(tokens);
                }
                // A nullable type's reference (`String?::isNullOrBlank`), not `?:` and `:`.
                '?' if self.starts("?::") => {
                    self.at += 1;
                    push(&mut tokens, Kind::Op("?"));
                }
                _ => {
                    let op = OPS
                        .iter()
                        .chain(MORE_OPS.iter())
                        .find(|op| self.starts(op))
                        .copied()
                        .ok_or_else(|| self.error(&format!("unexpected character {char:?}")))?;
                    self.at += op.chars().count();
                    match op {
                        "{" => depth += 1,
                        "}" => depth = depth.saturating_sub(1),
                        _ => {}
                    }
                    push(&mut tokens, Kind::Op(op));
                }
            }
        }
        if template {
            return Err(self.error("a string template isn't closed"));
        }
        Ok(tokens)
    }

    fn block_comment(&mut self) -> Result<(), String> {
        self.at += 2;
        let mut depth = 1;
        while depth > 0 {
            match (self.peek(0), self.peek(1)) {
                (Some('/'), Some('*')) => {
                    depth += 1;
                    self.at += 2;
                }
                (Some('*'), Some('/')) => {
                    depth -= 1;
                    self.at += 2;
                }
                (Some(char), _) => {
                    if char == '\n' {
                        self.line += 1;
                    }
                    self.at += 1;
                }
                (None, _) => return Err(self.error("a comment isn't closed")),
            }
        }
        Ok(())
    }

    fn number(&mut self) -> Result<Kind, String> {
        let start = self.at;
        if self.starts("0x") || self.starts("0X") {
            self.at += 2;
            let digits_start = self.at;
            while self
                .peek(0)
                .is_some_and(|c| c.is_ascii_hexdigit() || c == '_')
            {
                self.at += 1;
            }
            let digits: String = self.chars[digits_start..self.at]
                .iter()
                .filter(|c| **c != '_')
                .collect();
            let mut long = false;
            if self.peek(0).is_some_and(|c| c == 'u' || c == 'U') {
                self.at += 1;
            }
            if self.peek(0) == Some('L') {
                self.at += 1;
                long = true;
            }
            let value = u64::from_str_radix(&digits, 16)
                .map_err(|_| self.error("invalid hexadecimal number"))?;
            return Ok(if long {
                Kind::Long(value as i64)
            } else {
                Kind::Int(value as i64)
            });
        }
        let mut float = false;
        while let Some(char) = self.peek(0) {
            if char.is_ascii_digit() || char == '_' {
                self.at += 1;
            } else if char == '.' && !float && self.peek(1).is_some_and(|c| c.is_ascii_digit()) {
                float = true;
                self.at += 1;
            } else if (char == 'e' || char == 'E')
                && self
                    .peek(1)
                    .is_some_and(|c| c.is_ascii_digit() || c == '-' || c == '+')
            {
                float = true;
                self.at += 2;
            } else {
                break;
            }
        }
        let text: String = self.chars[start..self.at]
            .iter()
            .filter(|c| **c != '_')
            .collect();
        let mut long = false;
        match self.peek(0) {
            Some('f' | 'F' | 'd' | 'D') => {
                self.at += 1;
                float = true;
            }
            Some('L') => {
                self.at += 1;
                long = true;
            }
            Some('u' | 'U') => {
                self.at += 1;
                if self.peek(0) == Some('L') {
                    self.at += 1;
                    long = true;
                }
            }
            _ => {}
        }
        if float {
            text.parse()
                .map(Kind::Float)
                .map_err(|_| self.error("invalid number"))
        } else {
            let make = if long { Kind::Long } else { Kind::Int };
            text.parse::<i64>()
                .map(make)
                .or_else(|_| text.parse::<u64>().map(|value| make(value as i64)))
                .map_err(|_| self.error("invalid number"))
        }
    }

    fn escape(&mut self) -> Result<char, String> {
        self.at += 1;
        let char = self
            .peek(0)
            .ok_or_else(|| self.error("a string isn't closed"))?;
        self.at += 1;
        Ok(match char {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            'b' => '\u{8}',
            '0' => '\0',
            'u' => {
                let read = |lexer: &mut Self| -> Result<u32, String> {
                    let hex: String = (0..4).filter_map(|offset| lexer.peek(offset)).collect();
                    lexer.at += 4;
                    u32::from_str_radix(&hex, 16).map_err(|_| lexer.error("invalid \\u escape"))
                };
                let code = read(self)?;
                // A surrogate pair, written as two escapes (`🔒`).
                if (0xD800..0xDC00).contains(&code) && self.starts("\\u") {
                    self.at += 2;
                    let low = read(self)?;
                    let joined =
                        0x10000 + ((code - 0xD800) << 10) + (low.wrapping_sub(0xDC00) & 0x3FF);
                    char::from_u32(joined).unwrap_or('\u{fffd}')
                } else {
                    char::from_u32(code).unwrap_or('\u{fffd}')
                }
            }
            other => other,
        })
    }

    fn character(&mut self) -> Result<char, String> {
        self.at += 1;
        let value = match self.peek(0) {
            Some('\\') => self.escape()?,
            Some(char) => {
                self.at += 1;
                char
            }
            None => return Err(self.error("a character isn't closed")),
        };
        if self.peek(0) != Some('\'') {
            return Err(self.error("a character literal holds more than one character"));
        }
        self.at += 1;
        Ok(value)
    }

    /// A template at a `$`: `${expression}` or `$name`, after the text before it.
    fn template(&mut self, parts: &mut Vec<Part>, text: &mut String) -> Result<(), String> {
        if !text.is_empty() {
            parts.push(Part::Text(std::mem::take(text)));
        }
        if self.peek(1) == Some('{') {
            self.at += 2;
            let code = self.tokens(true)?;
            parts.push(Part::Code(code));
            return Ok(());
        }
        self.at += 1;
        let start = self.at;
        while self
            .peek(0)
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            self.at += 1;
        }
        let name: String = self.chars[start..self.at].iter().collect();
        parts.push(Part::Code(vec![Token {
            kind: Kind::Name(name),
            line: self.line,
        }]));
        Ok(())
    }

    /// A string literal, plain or raw, as its parts; a template takes `dollars` dollar signs.
    fn string(&mut self, dollars: usize) -> Result<Vec<Part>, String> {
        let raw = self.starts("\"\"\"");
        self.at += if raw { 3 } else { 1 };
        let mut parts = Vec::new();
        let mut text = String::new();
        loop {
            let Some(char) = self.peek(0) else {
                return Err(self.error("a string isn't closed"));
            };
            if raw && self.starts("\"\"\"") {
                // A raw string may end with more quotes than three: they belong to it.
                let mut quotes = 3;
                while self.peek(quotes) == Some('"') {
                    quotes += 1;
                }
                for _ in 3..quotes {
                    text.push('"');
                }
                self.at += quotes;
                break;
            }
            match char {
                '"' if !raw => {
                    self.at += 1;
                    break;
                }
                '\n' if !raw => return Err(self.error("a string isn't closed")),
                '\\' if !raw => text.push(self.escape()?),
                // In a multi-dollar string a template takes its count of dollars; fewer are text,
                // and so are any before them.
                '$' if dollars > 1 => {
                    let mut run = 0;
                    while self.peek(run) == Some('$') {
                        run += 1;
                    }
                    let after = self.peek(run);
                    let template = run >= dollars
                        && after.is_some_and(|c| c == '{' || c.is_alphabetic() || c == '_');
                    let literal = if template { run - dollars } else { run };
                    for _ in 0..literal {
                        text.push('$');
                    }
                    self.at += literal;
                    if template {
                        // Leave one `$` for the plain template arms below to read.
                        self.at += dollars - 1;
                        self.template(&mut parts, &mut text)?;
                    }
                }
                '$' if self
                    .peek(1)
                    .is_some_and(|c| c == '{' || c.is_alphabetic() || c == '_') =>
                {
                    self.template(&mut parts, &mut text)?;
                }
                _ => {
                    if char == '\n' {
                        self.line += 1;
                    }
                    text.push(char);
                    self.at += 1;
                }
            }
        }
        if !text.is_empty() || parts.is_empty() {
            parts.push(Part::Text(text));
        }
        Ok(parts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<Kind> {
        lex(source)
            .unwrap()
            .into_iter()
            .map(|token| token.kind)
            .collect()
    }

    #[test]
    fn reads_templates_numbers_and_operators() {
        let tokens = kinds(r#"val url = "$baseUrl/page/${page + 1}" // comment"#);
        assert_eq!(tokens[0], Kind::Name("val".into()));
        assert_eq!(tokens[2], Kind::Op("="));
        let Kind::Str(parts) = &tokens[3] else {
            panic!("a string");
        };
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[1], Part::Text("/page/".into()));
        assert!(matches!(&parts[2], Part::Code(code) if code.len() == 3));
        assert_eq!(tokens.len(), 4);
        assert_eq!(
            kinds("1_000L 0x1F 1.5f a?.b ?: c!!"),
            [
                Kind::Long(1000),
                Kind::Int(31),
                Kind::Float(1.5),
                Kind::Name("a".into()),
                Kind::Op("?."),
                Kind::Name("b".into()),
                Kind::Op("?:"),
                Kind::Name("c".into()),
                Kind::Op("!!"),
            ]
        );
        let raw = kinds("\"\"\"a\\d+\"b\"\"\"\"");
        assert_eq!(raw, [Kind::Str(vec![Part::Text("a\\d+\"b\"".into())])]);
        assert!(lex("\"open").is_err());
        assert_eq!(kinds("/* a /* b */ c */ x"), [Kind::Name("x".into())]);
    }
}
