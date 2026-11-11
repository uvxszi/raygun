//! A recursive-descent parser for the Kotlin extension sources are written in. It reads files,
//! declarations, statements and expressions with Kotlin's precedence and its line-break rules: a
//! line break ends a statement, except inside parentheses and brackets, after an operator, and
//! before `.`, `?.`, `?:`, `&&`, `||` and `as`.

use super::ast::*;
use super::lexer::{self, Kind, Part, Token};

/// The modifiers declarations take.
const MODIFIERS: [&str; 27] = [
    "abstract",
    "open",
    "override",
    "private",
    "protected",
    "public",
    "internal",
    "final",
    "const",
    "lateinit",
    "suspend",
    "inline",
    "operator",
    "infix",
    "data",
    "sealed",
    "enum",
    "annotation",
    "companion",
    "inner",
    "tailrec",
    "external",
    "value",
    "crossinline",
    "noinline",
    "reified",
    "expect",
];

pub fn parse_file(source: &str) -> Result<File, String> {
    let tokens = lexer::lex(source)?;
    let mut parser = Parser::new(tokens);
    parser.file()
}

/// Parses one expression (for tests and string templates).
pub fn parse_expr(source: &str) -> Result<Expr, String> {
    let tokens = lexer::lex(source)?;
    let mut parser = Parser::new(tokens);
    let expr = parser.expr()?;
    parser.skip_newlines();
    if parser.at < parser.tokens.len() {
        return Err(parser.error("unexpected text after the expression"));
    }
    Ok(expr)
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
    /// Whether line breaks are insignificant here (inside parentheses or brackets).
    nesting: Vec<bool>,
    /// A `{` here isn't a trailing lambda: it opens the class body after `Interface by delegate`.
    no_trailing_lambda: bool,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            at: 0,
            nesting: vec![false],
            no_trailing_lambda: false,
        }
    }

    fn error(&self, message: &str) -> String {
        let line = self
            .tokens
            .get(self.at.min(self.tokens.len().saturating_sub(1)))
            .map_or(0, |token| token.line);
        let near = match self.tokens.get(self.at).map(|token| &token.kind) {
            Some(Kind::Name(name)) => format!(" near {name}"),
            Some(Kind::Op(op)) => format!(" near {op}"),
            _ => String::new(),
        };
        format!("line {line}: {message}{near}")
    }

    fn line(&self) -> u32 {
        self.tokens.get(self.at).map_or(0, |token| token.line)
    }

    fn ignoring_newlines(&self) -> bool {
        self.nesting.last().copied().unwrap_or(false)
    }

    fn skip_newlines(&mut self) {
        while matches!(
            self.tokens.get(self.at),
            Some(Token {
                kind: Kind::Newline,
                ..
            })
        ) {
            self.at += 1;
        }
    }

    /// The next token, past line breaks where they don't count.
    fn peek(&mut self) -> Option<&Kind> {
        if self.ignoring_newlines() {
            self.skip_newlines();
        }
        self.tokens.get(self.at).map(|token| &token.kind)
    }

    /// The token `ahead` places on, past line breaks.
    fn peek_past_newlines(&self, ahead: usize) -> Option<&Kind> {
        self.tokens[self.at..]
            .iter()
            .filter(|token| token.kind != Kind::Newline)
            .nth(ahead)
            .map(|token| &token.kind)
    }

    fn is_op(&mut self, op: &str) -> bool {
        matches!(self.peek(), Some(Kind::Op(found)) if *found == op)
    }

    fn is_name(&mut self, name: &str) -> bool {
        matches!(self.peek(), Some(Kind::Name(found)) if found == name)
    }

    fn eat_op(&mut self, op: &str) -> bool {
        if self.is_op(op) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn eat_name(&mut self, name: &str) -> bool {
        if self.is_name(name) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expect_op(&mut self, op: &str) -> Result<(), String> {
        if self.eat_op(op) {
            Ok(())
        } else {
            Err(self.error(&format!("expected {op}")))
        }
    }

    fn name(&mut self) -> Result<String, String> {
        match self.peek().cloned() {
            Some(Kind::Name(name)) => {
                self.at += 1;
                Ok(name)
            }
            _ => Err(self.error("expected a name")),
        }
    }

    /// A continuation token after line breaks (`.`, `?:`…): the line breaks are skipped.
    fn continues(&mut self, ops: &[&str]) -> Option<&'static str> {
        let found = match self.peek_past_newlines(0) {
            Some(Kind::Op(op)) if ops.contains(op) => Some(*op),
            _ => None,
        }?;
        // `?.` after a line break could start a new statement only in odd code; `.` too.
        self.skip_newlines();
        Some(found)
    }

    fn with_nesting<T>(
        &mut self,
        ignoring: bool,
        parse: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        self.nesting.push(ignoring);
        let outer = std::mem::take(&mut self.no_trailing_lambda);
        let result = parse(self);
        self.no_trailing_lambda = outer;
        self.nesting.pop();
        result
    }

    // ---- Files and declarations ----

    fn file(&mut self) -> Result<File, String> {
        let mut file = File::default();
        loop {
            self.skip_newlines();
            self.skip_file_annotations();
            if self.eat_name("package") {
                file.package = self.dotted()?;
            } else if self.eat_name("import") {
                let full = self.dotted_import()?;
                let alias = if self.eat_name("as") {
                    self.name()?
                } else {
                    full.rsplit('.').next().unwrap_or_default().to_owned()
                };
                file.imports.push((alias, full));
            } else {
                break;
            }
            self.eat_op(";");
        }
        loop {
            self.skip_newlines();
            if self.at >= self.tokens.len() {
                break;
            }
            if self.eat_op(";") {
                continue;
            }
            file.members.push(self.member()?);
        }
        Ok(file)
    }

    fn skip_file_annotations(&mut self) {
        // `@file:Suppress(...)`
        while matches!(
            self.tokens.get(self.at).map(|t| &t.kind),
            Some(Kind::Op("@"))
        ) && matches!(self.tokens.get(self.at + 1).map(|t| &t.kind), Some(Kind::Name(name)) if name == "file")
        {
            let _ = self.annotation();
            self.skip_newlines();
        }
    }

    fn dotted(&mut self) -> Result<String, String> {
        let mut name = self.name()?;
        while self.is_op(".")
            && matches!(
                self.tokens.get(self.at + 1).map(|t| &t.kind),
                Some(Kind::Name(_))
            )
        {
            self.at += 1;
            name.push('.');
            name.push_str(&self.name()?);
        }
        Ok(name)
    }

    fn dotted_import(&mut self) -> Result<String, String> {
        let mut name = self.name()?;
        while self.eat_op(".") {
            name.push('.');
            if self.eat_op("*") {
                name.push('*');
                break;
            }
            name.push_str(&self.name()?);
        }
        Ok(name)
    }

    /// One annotation's name (`@Source`, `@Suppress("x")`, `@field:Json`).
    fn annotation(&mut self) -> Result<String, String> {
        self.expect_op("@")?;
        let mut name = self.name()?;
        if self.eat_op(":") {
            name = self.name()?;
        }
        while self.is_op(".")
            && matches!(
                self.tokens.get(self.at + 1).map(|t| &t.kind),
                Some(Kind::Name(_))
            )
        {
            self.at += 1;
            name.push('.');
            name.push_str(&self.name()?);
        }
        if self.is_op("(")
            && self
                .tokens
                .get(self.at)
                .is_some_and(|t| t.kind == Kind::Op("("))
        {
            let args = self.call_args()?;
            // The JSON name a DTO's property is read by.
            if name == "SerialName"
                && let Some(Arg {
                    value: Expr::Str(parts),
                    ..
                }) = args.first()
                && let [StrPart::Text(text)] = parts.as_slice()
            {
                return Ok(format!("SerialName={text}"));
            }
            // A protobuf field's number (`@ProtoNumber(3)`).
            if name == "ProtoNumber"
                && let Some(Arg {
                    value: Expr::Int(number),
                    ..
                }) = args.first()
            {
                return Ok(format!("ProtoNumber={number}"));
            }
            // How a protobuf number is written (`@ProtoType(ProtoIntegerType.SIGNED)`).
            if name == "ProtoType"
                && let Some(Arg {
                    value: Expr::Member { name: kind, .. },
                    ..
                }) = args.first()
            {
                return Ok(format!("ProtoType={kind}"));
            }
            // The other JSON names it's read by (`@JsonNames("results", "manga_list")`).
            if name == "JsonNames" {
                let names: Vec<&str> = args
                    .iter()
                    .filter_map(|arg| match &arg.value {
                        Expr::Str(parts) => match parts.as_slice() {
                            [StrPart::Text(text)] => Some(text.as_str()),
                            _ => None,
                        },
                        _ => None,
                    })
                    .collect();
                return Ok(format!("JsonNames={}", names.join("\u{1f}")));
            }
        }
        Ok(name)
    }

    fn modifiers(&mut self) -> Result<(Vec<String>, Vec<String>), String> {
        let mut modifiers = Vec::new();
        let mut annotations = Vec::new();
        loop {
            self.skip_newlines();
            if self.is_op("@") {
                annotations.push(self.annotation()?);
                continue;
            }
            let Some(Kind::Name(name)) = self.peek().cloned() else {
                break;
            };
            // Context parameters (`context(source: HttpSource) fun …`): the source the code
            // runs in, kept as `context:<name>` among the modifiers.
            if name == "context"
                && self
                    .tokens
                    .get(self.at + 1)
                    .is_some_and(|t| t.kind == Kind::Op("("))
            {
                self.at += 1;
                for param in self.params()? {
                    if param.name != "_" {
                        modifiers.push(format!("context:{}", param.name));
                    }
                }
                continue;
            }
            // `enum class`, `data class`, `open val`: a modifier is followed by a name (another
            // modifier or the declaration's keyword); `data?.let` is a variable.
            let next = self.tokens.get(self.at + 1).map(|t| &t.kind);
            let is_modifier =
                MODIFIERS.contains(&name.as_str()) && matches!(next, Some(Kind::Name(_)));
            if !is_modifier {
                break;
            }
            self.at += 1;
            modifiers.push(name);
        }
        Ok((modifiers, annotations))
    }

    fn member(&mut self) -> Result<Member, String> {
        let (modifiers, annotations) = self.modifiers()?;
        let line = self.line();
        match self.peek().cloned() {
            Some(Kind::Name(keyword)) => match keyword.as_str() {
                "fun" => {
                    self.at += 1;
                    self.function(modifiers, annotations, line).map(Member::Fun)
                }
                "val" | "var" => {
                    self.at += 1;
                    self.property(modifiers, keyword == "var", line)
                        .map(Member::Prop)
                }
                "class" | "interface" => {
                    self.at += 1;
                    let kind = if keyword == "interface" {
                        ClassKind::Interface
                    } else if modifiers.iter().any(|m| m == "enum") {
                        ClassKind::Enum
                    } else {
                        ClassKind::Class
                    };
                    self.class(kind, modifiers, annotations, line)
                        .map(Member::Class)
                }
                "object" => {
                    self.at += 1;
                    self.class(ClassKind::Object, modifiers, annotations, line)
                        .map(Member::Class)
                }
                "init" => {
                    self.at += 1;
                    self.block().map(Member::Init)
                }
                "typealias" => {
                    self.at += 1;
                    let name = self.name()?;
                    if self.is_op("<") {
                        self.type_params()?;
                    }
                    self.expect_op("=")?;
                    self.skip_newlines();
                    let target = self.ty()?;
                    Ok(Member::TypeAlias(name, target))
                }
                "constructor" => {
                    // A secondary constructor: its parameters, a delegation and a body.
                    self.at += 1;
                    self.params()?;
                    if self.eat_op(":") {
                        self.name()?;
                        self.call_args()?;
                    }
                    let body = if self.is_op("{") {
                        self.block()?
                    } else {
                        Vec::new()
                    };
                    Ok(Member::Init(body))
                }
                _ => Err(self.error("expected a declaration")),
            },
            _ => Err(self.error("expected a declaration")),
        }
    }

    fn function(
        &mut self,
        modifiers: Vec<String>,
        annotations: Vec<String>,
        line: u32,
    ) -> Result<FunDecl, String> {
        let mut modifiers = modifiers;
        // `inline fun <reified T> …`: its reified type parameters, kept as `reified:<name>`
        // among the modifiers.
        if self.is_op("<") {
            for name in self.function_type_params()? {
                modifiers.push(format!("reified:{name}"));
            }
        }
        let (receiver, name) = self.receiver_and_name()?;
        let params = self.params()?;
        let ret = if self.eat_op(":") {
            Some(self.ty()?)
        } else {
            None
        };
        self.where_clause()?;
        let body = self.function_body()?;
        Ok(FunDecl {
            name,
            modifiers,
            annotations,
            receiver,
            params,
            ret,
            body,
            line,
        })
    }

    fn function_body(&mut self) -> Result<Option<Body>, String> {
        if self.eat_op("=") {
            self.skip_newlines();
            Ok(Some(Body::Expr(self.expr()?)))
        } else if self.is_op("{") {
            Ok(Some(Body::Block(self.block()?)))
        } else {
            Ok(None)
        }
    }

    fn where_clause(&mut self) -> Result<(), String> {
        if self.eat_name("where") {
            loop {
                self.name()?;
                self.expect_op(":")?;
                self.ty()?;
                if !self.eat_op(",") {
                    break;
                }
            }
        }
        Ok(())
    }

    /// `name` or `Receiver.name` (`OkHttpClient.Builder.configureClient`).
    fn receiver_and_name(&mut self) -> Result<(Option<Type>, String), String> {
        let mut first = self.ty()?;
        if self.eat_op(".") {
            let name = self.name()?;
            return Ok((Some(first), name));
        }
        // A nullable receiver: `String?.removeLegacyRating`.
        if self.eat_op("?.") {
            first.nullable = true;
            let name = self.name()?;
            return Ok((Some(first), name));
        }
        // A dotted type name read as the receiver: its last part is the name.
        match first.name.rsplit_once('.') {
            Some((receiver, name)) if first.args.is_empty() && !first.nullable => Ok((
                Some(Type {
                    name: receiver.to_owned(),
                    ..Type::default()
                }),
                name.to_owned(),
            )),
            _ if first.args.is_empty() && !first.nullable => Ok((None, first.name)),
            _ => Err(self.error("expected a name after the receiver type")),
        }
    }

    fn property(
        &mut self,
        modifiers: Vec<String>,
        mutable: bool,
        line: u32,
    ) -> Result<PropDecl, String> {
        if self.is_op("<") {
            self.type_params()?;
        }
        let (receiver, name) = self.receiver_and_name()?;
        let ty = if self.eat_op(":") {
            Some(self.ty()?)
        } else {
            None
        };
        self.where_clause()?;
        let mut value = None;
        let mut delegate = None;
        if self.eat_op("=") {
            self.skip_newlines();
            value = Some(self.expr()?);
        } else if self.eat_name("by") {
            self.skip_newlines();
            delegate = Some(self.expr()?);
        }
        // Accessors, on the same line or the next ones.
        let mut getter = None;
        let mut setter = None;
        loop {
            let save = self.at;
            self.skip_newlines();
            let (_, _) = self.modifiers()?;
            if self.eat_name("get") {
                if self.eat_op("(") {
                    self.expect_op(")")?;
                    if self.eat_op(":") {
                        self.ty()?;
                    }
                    getter = self.function_body()?;
                }
                continue;
            }
            if self.eat_name("set") {
                if self.eat_op("(") {
                    let param = self.with_nesting(true, |parser| {
                        let name = parser.name()?;
                        if parser.eat_op(":") {
                            parser.ty()?;
                        }
                        Ok(name)
                    })?;
                    self.expect_op(")")?;
                    setter = self.function_body()?.map(|body| Box::new((param, body)));
                }
                continue;
            }
            self.at = save;
            break;
        }
        Ok(PropDecl {
            name,
            modifiers,
            mutable,
            receiver,
            ty,
            value,
            getter,
            setter,
            delegate,
            line,
        })
    }

    fn class(
        &mut self,
        kind: ClassKind,
        modifiers: Vec<String>,
        annotations: Vec<String>,
        line: u32,
    ) -> Result<ClassDecl, String> {
        // `companion object` may be unnamed; `object : Interface` is anonymous.
        let name = match self.peek() {
            Some(Kind::Name(_)) => self.name()?,
            _ => String::new(),
        };
        let type_params = if self.is_op("<") {
            self.type_params()?
        } else {
            Vec::new()
        };
        // A primary constructor, perhaps with modifiers (`private constructor(...)`).
        let save = self.at;
        let (ctor_modifiers, _) = self.modifiers()?;
        let params = if self.eat_name("constructor") || self.is_op("(") {
            self.params()?
        } else {
            if !ctor_modifiers.is_empty() {
                self.at = save;
            }
            Vec::new()
        };
        let supers = if self.peek_past_newlines(0) == Some(&Kind::Op(":")) {
            self.skip_newlines();
            self.expect_op(":")?;
            self.supers()?
        } else {
            Vec::new()
        };
        self.where_clause()?;
        let (entries, members) = if self.peek_past_newlines(0) == Some(&Kind::Op("{")) {
            self.skip_newlines();
            self.class_body(kind == ClassKind::Enum)?
        } else {
            (Vec::new(), Vec::new())
        };
        Ok(ClassDecl {
            name,
            type_params,
            kind,
            modifiers,
            annotations,
            params,
            supers,
            entries,
            members,
            line,
        })
    }

    fn supers(&mut self) -> Result<Vec<SuperRef>, String> {
        let mut supers = Vec::new();
        loop {
            self.skip_newlines();
            let ty = self.ty()?;
            let args = if self
                .tokens
                .get(self.at)
                .is_some_and(|t| t.kind == Kind::Op("("))
            {
                Some(self.call_args()?)
            } else {
                None
            };
            // `Interface by delegate`, before the class's body.
            let delegate = if self.eat_name("by") {
                let outer = std::mem::replace(&mut self.no_trailing_lambda, true);
                let delegate = self.expr();
                self.no_trailing_lambda = outer;
                Some(delegate?)
            } else {
                None
            };
            supers.push(SuperRef { ty, args, delegate });
            if !self.eat_op(",") {
                break;
            }
        }
        Ok(supers)
    }

    fn class_body(&mut self, is_enum: bool) -> Result<(Vec<EnumEntry>, Vec<Member>), String> {
        self.expect_op("{")?;
        self.with_nesting(false, |parser| {
            let mut entries = Vec::new();
            if is_enum {
                loop {
                    parser.skip_newlines();
                    while parser.is_op("@") {
                        parser.annotation()?;
                        parser.skip_newlines();
                    }
                    let Some(Kind::Name(name)) = parser.peek().cloned() else {
                        break;
                    };
                    parser.at += 1;
                    let args = if parser.is_op("(") {
                        parser.call_args()?
                    } else {
                        Vec::new()
                    };
                    // An entry with a body of its own.
                    if parser.is_op("{") {
                        parser.class_body(false)?;
                    }
                    entries.push(EnumEntry { name, args });
                    parser.skip_newlines();
                    if !parser.eat_op(",") {
                        break;
                    }
                }
                parser.skip_newlines();
                parser.eat_op(";");
            }
            let mut members = Vec::new();
            loop {
                parser.skip_newlines();
                if parser.eat_op("}") {
                    break;
                }
                if parser.eat_op(";") {
                    continue;
                }
                if parser.at >= parser.tokens.len() {
                    return Err(parser.error("a class body isn't closed"));
                }
                members.push(parser.member()?);
            }
            Ok((entries, members))
        })
    }

    /// `<T, out R : Any>`: the names.
    /// A function's type parameters: the names of its `reified` ones.
    fn function_type_params(&mut self) -> Result<Vec<String>, String> {
        self.expect_op("<")?;
        self.with_nesting(true, |parser| {
            let mut names = Vec::new();
            loop {
                let (modifiers, _) = parser.modifiers()?;
                parser.eat_name("in");
                parser.eat_name("out");
                let name = parser.name()?;
                if modifiers.iter().any(|modifier| modifier == "reified") {
                    names.push(name);
                }
                if parser.eat_op(":") {
                    parser.ty()?;
                }
                if !parser.eat_op(",") {
                    break;
                }
            }
            parser.expect_op(">")?;
            Ok(names)
        })
    }

    fn type_params(&mut self) -> Result<Vec<String>, String> {
        self.expect_op("<")?;
        self.with_nesting(true, |parser| {
            let mut names = Vec::new();
            loop {
                let _ = parser.modifiers()?;
                parser.eat_name("in");
                parser.eat_name("out");
                names.push(parser.name()?);
                if parser.eat_op(":") {
                    parser.ty()?;
                }
                if !parser.eat_op(",") {
                    break;
                }
            }
            parser.expect_op(">")?;
            Ok(names)
        })
    }

    fn params(&mut self) -> Result<Vec<Param>, String> {
        self.expect_op("(")?;
        self.with_nesting(true, |parser| {
            let mut params = Vec::new();
            while !parser.eat_op(")") {
                let (_, annotations) = parser.modifiers()?;
                // `private vararg val words: String`, in any order.
                let (mut property, mut vararg) = (false, false);
                while let Some(Kind::Name(word)) = parser.peek().cloned()
                    && matches!(
                        parser.tokens.get(parser.at + 1).map(|t| &t.kind),
                        Some(Kind::Name(_))
                    )
                {
                    match word.as_str() {
                        "val" | "var" => property = true,
                        "vararg" => vararg = true,
                        modifier if MODIFIERS.contains(&modifier) => {}
                        _ => break,
                    }
                    parser.at += 1;
                }
                let name = parser.name()?;
                let ty = if parser.eat_op(":") {
                    parser.ty()?
                } else {
                    Type::default()
                };
                let default = if parser.eat_op("=") {
                    Some(parser.expr()?)
                } else {
                    None
                };
                params.push(Param {
                    name,
                    ty,
                    default,
                    vararg,
                    property,
                    annotations,
                });
                if !parser.eat_op(",") {
                    parser.expect_op(")")?;
                    break;
                }
            }
            Ok(params)
        })
    }

    /// A type: `String`, `List<Pair<String, Int>>?`, `(Int) -> Unit`, `suspend () -> T`,
    /// `Filter.Select<String>`, `Array<out String>`.
    fn ty(&mut self) -> Result<Type, String> {
        while self.is_op("@") {
            self.annotation()?;
            self.skip_newlines();
        }
        self.eat_name("suspend");
        if self.is_op("(") {
            // A function type, or a type in parentheses.
            self.at += 1;
            let inner = self.with_nesting(true, |parser| {
                let mut args = Vec::new();
                while !parser.eat_op(")") {
                    // Named parameters of a function type: `(page: Int) -> Unit`.
                    if matches!(parser.peek(), Some(Kind::Name(_)))
                        && matches!(
                            parser.tokens.get(parser.at + 1).map(|t| &t.kind),
                            Some(Kind::Op(":"))
                        )
                    {
                        parser.at += 2;
                    }
                    args.push(parser.ty()?);
                    if !parser.eat_op(",") {
                        parser.expect_op(")")?;
                        break;
                    }
                }
                Ok(args)
            })?;
            if self.eat_op("->") {
                let ret = self.ty()?;
                let mut args = inner;
                args.push(ret);
                let nullable = false;
                return Ok(Type {
                    name: "()".to_owned(),
                    args,
                    nullable,
                });
            }
            let mut ty = inner.into_iter().next().unwrap_or_default();
            if self.eat_op("?") {
                ty.nullable = true;
            }
            return Ok(ty);
        }
        if self.eat_op("*") {
            return Ok(Type {
                name: "*".to_owned(),
                ..Type::default()
            });
        }
        let mut name = self.name()?;
        let mut args = Vec::new();
        loop {
            if self
                .tokens
                .get(self.at)
                .is_some_and(|t| t.kind == Kind::Op("<"))
            {
                args = self.type_args()?;
            }
            // `Filter.Select`: a nested type. A receiver's function name (`Element.imgAttr`) is
            // split off by the caller.
            if args.is_empty()
                && self
                    .tokens
                    .get(self.at)
                    .is_some_and(|t| t.kind == Kind::Op("."))
                && matches!(
                    self.tokens.get(self.at + 1).map(|t| &t.kind),
                    Some(Kind::Name(_))
                )
            {
                self.at += 1;
                name.push('.');
                name.push_str(&self.name()?);
                continue;
            }
            break;
        }
        let nullable = self
            .tokens
            .get(self.at)
            .is_some_and(|t| t.kind == Kind::Op("?"));
        if nullable {
            self.at += 1;
        }
        // A function type with a receiver: `HttpUrl.Builder.() -> Unit`.
        if self
            .tokens
            .get(self.at)
            .is_some_and(|t| t.kind == Kind::Op("."))
            && self
                .tokens
                .get(self.at + 1)
                .is_some_and(|t| t.kind == Kind::Op("("))
        {
            self.at += 1;
            let mut function = self.ty()?;
            function.args.insert(
                0,
                Type {
                    name,
                    args,
                    nullable,
                },
            );
            // Its first argument is the receiver.
            function.name = RECEIVER_FUNCTION.to_owned();
            return Ok(function);
        }
        Ok(Type {
            name,
            args,
            nullable,
        })
    }

    fn type_args(&mut self) -> Result<Vec<Type>, String> {
        self.expect_op("<")?;
        self.with_nesting(true, |parser| {
            let mut args = Vec::new();
            loop {
                // `List<@Serializable(X::class) Item,>`: annotations and a trailing comma.
                while parser.is_op("@") {
                    parser.annotation()?;
                }
                parser.eat_name("in");
                parser.eat_name("out");
                args.push(parser.ty()?);
                if !parser.eat_op(",") || parser.is_op(">") {
                    break;
                }
            }
            parser.expect_op(">")?;
            Ok(args)
        })
    }

    /// Type arguments of a call (`parseAs<List<X>>()`), when what follows `<` reads as them.
    fn try_call_type_args(&mut self) -> Option<Vec<Type>> {
        if !self
            .tokens
            .get(self.at)
            .is_some_and(|t| t.kind == Kind::Op("<"))
        {
            return None;
        }
        let save = self.at;
        let nesting = self.nesting.len();
        match self.type_args() {
            Ok(args)
                if matches!(
                    self.tokens.get(self.at).map(|t| &t.kind),
                    Some(Kind::Op("(" | "{" | "::" | "." | ")" | "," | "?."))
                        | Some(Kind::Newline)
                        | None
                ) =>
            {
                Some(args)
            }
            _ => {
                self.at = save;
                self.nesting.truncate(nesting);
                None
            }
        }
    }

    // ---- Statements ----

    fn block(&mut self) -> Result<Block, String> {
        self.expect_op("{")?;
        self.with_nesting(false, |parser| parser.statements_until_brace())
    }

    fn statements_until_brace(&mut self) -> Result<Block, String> {
        let mut statements = Vec::new();
        loop {
            self.skip_newlines();
            if self.eat_op("}") {
                return Ok(statements);
            }
            if self.eat_op(";") {
                continue;
            }
            if self.at >= self.tokens.len() {
                return Err(self.error("a block isn't closed"));
            }
            statements.push(self.statement()?);
        }
    }

    fn label(&mut self) -> Option<String> {
        if let (Some(Kind::Name(name)), Some(Kind::Op("@"))) = (
            self.tokens.get(self.at).map(|t| &t.kind),
            self.tokens.get(self.at + 1).map(|t| &t.kind),
        ) {
            let name = name.clone();
            if !matches!(
                name.as_str(),
                "return" | "break" | "continue" | "this" | "super"
            ) {
                self.at += 2;
                return Some(name);
            }
        }
        None
    }

    fn statement(&mut self) -> Result<Stmt, String> {
        while self.is_op("@") {
            self.annotation()?;
            self.skip_newlines();
        }
        let save = self.at;
        let label = self.label();
        match self.peek().cloned() {
            Some(Kind::Name(keyword)) => match keyword.as_str() {
                "val" | "var" => {
                    self.at += 1;
                    let mutable = keyword == "var";
                    let (names, destructured) = if self.eat_op("(") {
                        let names = self.with_nesting(true, |parser| {
                            let mut names = Vec::new();
                            while !parser.eat_op(")") {
                                names.push(parser.name()?);
                                if parser.eat_op(":") {
                                    parser.ty()?;
                                }
                                if !parser.eat_op(",") {
                                    parser.expect_op(")")?;
                                    break;
                                }
                            }
                            Ok(names)
                        })?;
                        (names, true)
                    } else {
                        (vec![self.name()?], false)
                    };
                    let ty = if self.eat_op(":") {
                        Some(self.ty()?)
                    } else {
                        None
                    };
                    let (value, delegate) = if self.eat_op("=") {
                        self.skip_newlines();
                        (Some(self.expr()?), None)
                    } else if self.eat_name("by") {
                        (None, Some(self.expr()?))
                    } else {
                        (None, None)
                    };
                    Ok(Stmt::Val {
                        names,
                        destructured,
                        mutable,
                        ty,
                        value,
                        delegate,
                    })
                }
                "for" => {
                    self.at += 1;
                    self.expect_op("(")?;
                    let (vars, iterable) = self.with_nesting(true, |parser| {
                        let vars = if parser.eat_op("(") {
                            let mut names = Vec::new();
                            while !parser.eat_op(")") {
                                names.push(parser.name()?);
                                if parser.eat_op(":") {
                                    parser.ty()?;
                                }
                                if !parser.eat_op(",") {
                                    parser.expect_op(")")?;
                                    break;
                                }
                            }
                            LambdaParam::Destructured(names)
                        } else {
                            let name = parser.name()?;
                            if parser.eat_op(":") {
                                parser.ty()?;
                            }
                            LambdaParam::Name(name)
                        };
                        if !parser.eat_name("in") {
                            return Err(parser.error("expected in"));
                        }
                        let iterable = parser.expr()?;
                        parser.expect_op(")")?;
                        Ok((vars, iterable))
                    })?;
                    let body = self.loop_body()?;
                    Ok(Stmt::For {
                        label,
                        vars,
                        iterable,
                        body,
                    })
                }
                "while" => {
                    self.at += 1;
                    let condition = self.parenthesized()?;
                    let body = self.loop_body()?;
                    Ok(Stmt::While {
                        label,
                        condition,
                        body,
                        after: false,
                    })
                }
                "do" => {
                    self.at += 1;
                    let body = self.loop_body()?;
                    self.skip_newlines();
                    if !self.eat_name("while") {
                        return Err(self.error("expected while"));
                    }
                    let condition = self.parenthesized()?;
                    Ok(Stmt::While {
                        label,
                        condition,
                        body,
                        after: true,
                    })
                }
                _ if label.is_none() && self.local_declaration() => {
                    self.member().map(|member| Stmt::Decl(Box::new(member)))
                }
                _ => {
                    if label.is_some() {
                        // A labelled expression (`run outer@{ … }` is read by the call).
                        self.at = save;
                    }
                    self.expression_statement()
                }
            },
            _ => {
                if label.is_some() {
                    self.at = save;
                }
                self.expression_statement()
            }
        }
    }

    /// Whether a statement starts a local declaration: modifiers, then `fun`, `class`, `interface`
    /// or a named `object` (`object : X {}` is an expression).
    fn local_declaration(&self) -> bool {
        let mut at = self.at;
        while let Some(Kind::Name(name)) = self.tokens.get(at).map(|t| &t.kind) {
            match name.as_str() {
                "fun" | "class" | "interface" => {
                    return !matches!(
                        self.tokens.get(at + 1).map(|t| &t.kind),
                        Some(Kind::Op("." | "(" | "=" | ":" | "?."))
                    ) || name == "fun";
                }
                "object" => {
                    return matches!(
                        self.tokens.get(at + 1).map(|t| &t.kind),
                        Some(Kind::Name(_))
                    );
                }
                modifier
                    if MODIFIERS.contains(&modifier)
                        && matches!(
                            self.tokens.get(at + 1).map(|t| &t.kind),
                            Some(Kind::Name(_))
                        ) =>
                {
                    at += 1;
                }
                _ => return false,
            }
        }
        false
    }

    fn loop_body(&mut self) -> Result<Block, String> {
        self.skip_newlines();
        if self.is_op("{") {
            self.block()
        } else {
            Ok(vec![self.statement()?])
        }
    }

    fn parenthesized(&mut self) -> Result<Expr, String> {
        self.expect_op("(")?;
        self.with_nesting(true, |parser| {
            let expr = parser.expr()?;
            parser.expect_op(")")?;
            Ok(expr)
        })
    }

    fn expression_statement(&mut self) -> Result<Stmt, String> {
        let target = self.expr()?;
        for op in ["=", "+=", "-=", "*=", "/=", "%="] {
            if self.eat_op(op) {
                self.skip_newlines();
                let value = self.expr()?;
                return Ok(Stmt::Assign {
                    target,
                    op: match op {
                        "=" => "=",
                        "+=" => "+=",
                        "-=" => "-=",
                        "*=" => "*=",
                        "/=" => "/=",
                        _ => "%=",
                    },
                    value,
                });
            }
        }
        Ok(Stmt::Expr(target))
    }

    // ---- Expressions ----

    pub fn expr(&mut self) -> Result<Expr, String> {
        self.disjunction()
    }

    fn binary_level(
        &mut self,
        ops: &[&'static str],
        next: fn(&mut Self) -> Result<Expr, String>,
        newline_before: bool,
    ) -> Result<Expr, String> {
        let mut left = next(self)?;
        loop {
            let op = if newline_before {
                self.continues(ops)
            } else {
                match self.peek() {
                    Some(Kind::Op(op)) if ops.contains(op) => Some(*op),
                    _ => None,
                }
            };
            let Some(op) = op else {
                return Ok(left);
            };
            self.at += 1;
            self.skip_newlines();
            let right = next(self)?;
            left = Expr::Binary {
                op: op.to_owned(),
                left: Box::new(left),
                right: Box::new(right),
            };
        }
    }

    fn disjunction(&mut self) -> Result<Expr, String> {
        self.binary_level(&["||"], Self::conjunction, true)
    }

    fn conjunction(&mut self) -> Result<Expr, String> {
        self.binary_level(&["&&"], Self::equality, true)
    }

    fn equality(&mut self) -> Result<Expr, String> {
        self.binary_level(&["==", "!=", "===", "!=="], Self::comparison, false)
    }

    fn comparison(&mut self) -> Result<Expr, String> {
        let mut left = self.named_check()?;
        loop {
            let op = match self.peek() {
                Some(Kind::Op(op @ ("<" | ">" | "<=" | ">="))) => *op,
                _ => return Ok(left),
            };
            self.at += 1;
            self.skip_newlines();
            let right = self.named_check()?;
            left = Expr::Binary {
                op: op.to_owned(),
                left: Box::new(left),
                right: Box::new(right),
            };
        }
    }

    /// `in`, `!in`, `is`, `!is`.
    fn named_check(&mut self) -> Result<Expr, String> {
        let mut left = self.elvis()?;
        loop {
            let negated = self.is_op("!")
                && matches!(self.tokens.get(self.at + 1).map(|t| &t.kind), Some(Kind::Name(name)) if name == "in" || name == "is");
            if negated {
                self.at += 1;
            }
            if self.eat_name("in") {
                self.skip_newlines();
                let right = self.elvis()?;
                left = Expr::Binary {
                    op: if negated { "!in" } else { "in" }.to_owned(),
                    left: Box::new(left),
                    right: Box::new(right),
                };
            } else if self.eat_name("is") {
                let ty = self.ty()?;
                left = Expr::Is {
                    expr: Box::new(left),
                    ty,
                    negated,
                };
            } else {
                if negated {
                    self.at -= 1;
                }
                return Ok(left);
            }
        }
    }

    fn elvis(&mut self) -> Result<Expr, String> {
        self.binary_level(&["?:"], Self::infix_call, true)
    }

    /// Infix functions: `a to b`, `0 until n`, `x and y`.
    fn infix_call(&mut self) -> Result<Expr, String> {
        let mut left = self.range()?;
        loop {
            let Some(Kind::Name(name)) = self.peek().cloned() else {
                return Ok(left);
            };
            // Any function may be infix (`genres intersect other`); keywords aren't.
            let keyword = matches!(
                name.as_str(),
                "in" | "is"
                    | "as"
                    | "if"
                    | "else"
                    | "when"
                    | "by"
                    | "get"
                    | "set"
                    | "where"
                    | "catch"
                    | "finally"
                    | "while"
                    | "do"
                    | "for"
                    | "return"
                    | "throw"
                    | "break"
                    | "continue"
                    | "val"
                    | "var"
                    | "fun"
                    | "class"
                    | "object"
                    | "interface"
                    | "this"
                    | "super"
                    | "null"
                    | "true"
                    | "false"
                    | "constructor"
                    | "init"
            );
            // The operand may start on the next line (`FLAG_A or\n FLAG_B`).
            let operand_follows = matches!(
                self.tokens[self.at + 1..]
                    .iter()
                    .find(|token| token.kind != Kind::Newline)
                    .map(|t| &t.kind),
                Some(
                    Kind::Name(_)
                        | Kind::Int(_)
                        | Kind::Long(_)
                        | Kind::Float(_)
                        | Kind::Str(_)
                        | Kind::Char(_)
                        | Kind::Op("(" | "-" | "!" | "::" | "{")
                )
            );
            if keyword || !operand_follows || name.chars().next().is_some_and(char::is_uppercase) {
                return Ok(left);
            }
            self.at += 1;
            self.skip_newlines();
            let right = self.range()?;
            left = Expr::Binary {
                op: name,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
    }

    fn range(&mut self) -> Result<Expr, String> {
        self.binary_level(&["..", "..<"], Self::additive, false)
    }

    fn additive(&mut self) -> Result<Expr, String> {
        self.binary_level(&["+", "-"], Self::multiplicative, false)
    }

    fn multiplicative(&mut self) -> Result<Expr, String> {
        self.binary_level(&["*", "/", "%"], Self::type_rhs, false)
    }

    fn type_rhs(&mut self) -> Result<Expr, String> {
        let mut expr = self.prefix()?;
        loop {
            if matches!(self.peek_past_newlines(0), Some(Kind::Name(name)) if name == "as") {
                self.skip_newlines();
                self.at += 1;
                let safe = self.eat_op("?");
                let ty = self.ty()?;
                expr = Expr::As {
                    expr: Box::new(expr),
                    ty,
                    safe,
                };
                continue;
            }
            return Ok(expr);
        }
    }

    fn prefix(&mut self) -> Result<Expr, String> {
        // An annotated expression: `@SuppressLint("…") object : X509TrustManager { … }`.
        while self.is_op("@") {
            self.annotation()?;
            self.skip_newlines();
        }
        for op in ["!", "-", "+", "++", "--"] {
            if self.is_op(op) {
                self.at += 1;
                let expr = self.prefix()?;
                let op = match op {
                    "!" => "!",
                    "-" => "-",
                    "+" => "+",
                    "++" => "++",
                    _ => "--",
                };
                return Ok(Expr::Unary {
                    op,
                    expr: Box::new(expr),
                });
            }
        }
        // A labelled lambda: `outer@{ … }`.
        if let (Some(Kind::Name(_)), Some(Kind::Op("@")), Some(Kind::Op("{"))) = (
            self.tokens.get(self.at).map(|t| &t.kind),
            self.tokens.get(self.at + 1).map(|t| &t.kind),
            self.tokens.get(self.at + 2).map(|t| &t.kind),
        ) {
            let label = self.label();
            let mut lambda = self.lambda()?;
            lambda.label = label;
            return self.postfix(Expr::Lambda(Box::new(lambda)));
        }
        let primary = self.primary()?;
        self.postfix(primary)
    }

    fn postfix(&mut self, mut expr: Expr) -> Result<Expr, String> {
        loop {
            match self.tokens.get(self.at).map(|t| &t.kind) {
                Some(Kind::Op("(")) => {
                    let args = self.call_args()?;
                    let lambda = self.trailing_lambda()?;
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        type_args: Vec::new(),
                        args,
                        lambda,
                    };
                    continue;
                }
                Some(Kind::Op("<")) if is_callable(&expr) => {
                    if let Some(type_args) = self.try_call_type_args() {
                        // `IndexedValue<SManga>::index`: a reference on the type.
                        if self
                            .tokens
                            .get(self.at)
                            .is_some_and(|t| t.kind == Kind::Op("::"))
                        {
                            continue;
                        }
                        let args = if self
                            .tokens
                            .get(self.at)
                            .is_some_and(|t| t.kind == Kind::Op("("))
                        {
                            self.call_args()?
                        } else {
                            Vec::new()
                        };
                        let lambda = self.trailing_lambda()?;
                        expr = Expr::Call {
                            callee: Box::new(expr),
                            type_args,
                            args,
                            lambda,
                        };
                        continue;
                    }
                    return Ok(expr);
                }
                Some(Kind::Op("{")) if is_callable(&expr) && !self.no_trailing_lambda => {
                    let lambda = self.lambda()?;
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        type_args: Vec::new(),
                        args: Vec::new(),
                        lambda: Some(Box::new(lambda)),
                    };
                    continue;
                }
                // A labelled lambda after a name (`run stop@{ … }`, `forEach outer@{ … }`).
                Some(Kind::Name(_))
                    if is_callable(&expr)
                        && matches!(
                            (
                                self.tokens.get(self.at + 1).map(|t| &t.kind),
                                self.tokens.get(self.at + 2).map(|t| &t.kind),
                            ),
                            (Some(Kind::Op("@")), Some(Kind::Op("{")))
                        ) =>
                {
                    let lambda = self.trailing_lambda()?;
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        type_args: Vec::new(),
                        args: Vec::new(),
                        lambda,
                    };
                    continue;
                }
                Some(Kind::Op("[")) => {
                    self.at += 1;
                    let index = self.with_nesting(true, |parser| {
                        let mut index = Vec::new();
                        while !parser.eat_op("]") {
                            index.push(parser.expr()?);
                            if !parser.eat_op(",") {
                                parser.expect_op("]")?;
                                break;
                            }
                        }
                        Ok(index)
                    })?;
                    expr = Expr::Index {
                        target: Box::new(expr),
                        index,
                    };
                    continue;
                }
                Some(Kind::Op("!!")) => {
                    self.at += 1;
                    expr = Expr::NotNull(Box::new(expr));
                    continue;
                }
                // `String?::isNullOrBlank`: the reference is the type's.
                Some(Kind::Op("?"))
                    if self
                        .tokens
                        .get(self.at + 1)
                        .is_some_and(|t| t.kind == Kind::Op("::")) =>
                {
                    self.at += 1;
                    continue;
                }
                Some(Kind::Op(op @ ("++" | "--"))) => {
                    let op = if *op == "++" { "++" } else { "--" };
                    // `a++` only right after the operand on the same line.
                    self.at += 1;
                    expr = Expr::Postfix {
                        op,
                        expr: Box::new(expr),
                    };
                    continue;
                }
                _ => {}
            }
            match self.continues(&[".", "?.", "::"]) {
                Some("::") => {
                    self.at += 1;
                    let name = if self.eat_name("class") {
                        "class".to_owned()
                    } else {
                        self.name()?
                    };
                    expr = Expr::Reference {
                        target: Some(Box::new(expr)),
                        name,
                    };
                }
                Some(op) => {
                    self.at += 1;
                    self.skip_newlines();
                    let name = if self.eat_name("class") {
                        "class".to_owned()
                    } else {
                        self.name()?
                    };
                    expr = Expr::Member {
                        target: Box::new(expr),
                        name,
                        safe: op == "?.",
                    };
                }
                None => return Ok(expr),
            }
        }
    }

    fn trailing_lambda(&mut self) -> Result<Option<Box<Lambda>>, String> {
        if self.no_trailing_lambda {
            return Ok(None);
        }
        let labelled = matches!(
            (
                self.tokens.get(self.at).map(|t| &t.kind),
                self.tokens.get(self.at + 1).map(|t| &t.kind),
                self.tokens.get(self.at + 2).map(|t| &t.kind),
            ),
            (
                Some(Kind::Name(_)),
                Some(Kind::Op("@")),
                Some(Kind::Op("{"))
            )
        );
        if labelled {
            let label = self.label();
            let mut lambda = self.lambda()?;
            lambda.label = label;
            return Ok(Some(Box::new(lambda)));
        }
        if self
            .tokens
            .get(self.at)
            .is_some_and(|t| t.kind == Kind::Op("{"))
        {
            return Ok(Some(Box::new(self.lambda()?)));
        }
        Ok(None)
    }

    fn call_args(&mut self) -> Result<Vec<Arg>, String> {
        self.expect_op("(")?;
        self.with_nesting(true, |parser| {
            let mut args = Vec::new();
            while !parser.eat_op(")") {
                let name = match (
                    parser.peek().cloned(),
                    parser.tokens.get(parser.at + 1).map(|t| t.kind.clone()),
                ) {
                    (Some(Kind::Name(name)), Some(Kind::Op("="))) => {
                        parser.at += 2;
                        Some(name)
                    }
                    _ => None,
                };
                let spread = parser.eat_op("*");
                let value = parser.expr()?;
                args.push(Arg {
                    name,
                    value,
                    spread,
                });
                if !parser.eat_op(",") {
                    parser.expect_op(")")?;
                    break;
                }
            }
            Ok(args)
        })
    }

    fn lambda(&mut self) -> Result<Lambda, String> {
        self.expect_op("{")?;
        self.with_nesting(false, |parser| {
            let save = parser.at;
            let params = parser.lambda_params();
            let (params, implicit) = match params {
                Some(params) => (params, false),
                None => {
                    parser.at = save;
                    (Vec::new(), true)
                }
            };
            let body = parser.statements_until_brace()?;
            Ok(Lambda {
                params,
                implicit,
                body,
                label: None,
            })
        })
    }

    /// A lambda's parameters and its `->`; `None` when it has none.
    fn lambda_params(&mut self) -> Option<Vec<LambdaParam>> {
        self.skip_newlines();
        let mut params = Vec::new();
        if self.eat_op("->") {
            return Some(params);
        }
        loop {
            self.skip_newlines();
            if self.eat_op("(") {
                let mut names = Vec::new();
                loop {
                    let Some(Kind::Name(name)) = self.tokens.get(self.at).map(|t| t.kind.clone())
                    else {
                        return None;
                    };
                    self.at += 1;
                    if self.eat_op(":") {
                        self.ty().ok()?;
                    }
                    names.push(name);
                    if self.eat_op(")") {
                        break;
                    }
                    if !self.eat_op(",") {
                        return None;
                    }
                }
                params.push(LambdaParam::Destructured(names));
            } else {
                let Some(Kind::Name(name)) = self.tokens.get(self.at).map(|t| t.kind.clone())
                else {
                    return None;
                };
                self.at += 1;
                if self.eat_op(":") {
                    self.ty().ok()?;
                }
                params.push(LambdaParam::Name(name));
            }
            self.skip_newlines();
            if self.eat_op("->") {
                return Some(params);
            }
            if !self.eat_op(",") {
                return None;
            }
        }
    }

    fn string(&mut self, parts: Vec<Part>) -> Result<Expr, String> {
        let mut out = Vec::new();
        for part in parts {
            match part {
                Part::Text(text) => out.push(StrPart::Text(text)),
                Part::Code(tokens) => {
                    let mut inner = Parser::new(tokens);
                    inner.nesting = vec![true];
                    let expr = inner.expr()?;
                    out.push(StrPart::Code(expr));
                }
            }
        }
        Ok(Expr::Str(out))
    }

    fn primary(&mut self) -> Result<Expr, String> {
        let Some(token) = self.peek().cloned() else {
            return Err(self.error("expected an expression"));
        };
        match token {
            Kind::Int(value) => {
                self.at += 1;
                Ok(Expr::Int(value))
            }
            Kind::Long(value) => {
                self.at += 1;
                Ok(Expr::Long(value))
            }
            Kind::Float(value) => {
                self.at += 1;
                Ok(Expr::Float(value))
            }
            Kind::Char(value) => {
                self.at += 1;
                Ok(Expr::Char(value))
            }
            Kind::Str(parts) => {
                self.at += 1;
                self.string(parts)
            }
            Kind::Op("(") => self.parenthesized(),
            Kind::Op("{") => Ok(Expr::Lambda(Box::new(self.lambda()?))),
            Kind::Op("::") => {
                self.at += 1;
                let name = self.name()?;
                Ok(Expr::Reference { target: None, name })
            }
            Kind::Op("[") => {
                // Collection literals appear only in annotations.
                Err(self.error("collection literals aren't supported"))
            }
            Kind::Name(name) => {
                self.at += 1;
                match name.as_str() {
                    "true" => Ok(Expr::Bool(true)),
                    "false" => Ok(Expr::Bool(false)),
                    "null" => Ok(Expr::Null),
                    "this" => {
                        let label = if self
                            .tokens
                            .get(self.at)
                            .is_some_and(|t| t.kind == Kind::Op("@"))
                        {
                            self.at += 1;
                            Some(self.name()?)
                        } else {
                            None
                        };
                        Ok(Expr::This(label))
                    }
                    "super" => {
                        if self
                            .tokens
                            .get(self.at)
                            .is_some_and(|t| t.kind == Kind::Op("<"))
                        {
                            self.type_args()?;
                        }
                        if self
                            .tokens
                            .get(self.at)
                            .is_some_and(|t| t.kind == Kind::Op("@"))
                        {
                            self.at += 1;
                            self.name()?;
                        }
                        Ok(Expr::Super)
                    }
                    "if" => self.if_expr(),
                    "when" => self.when_expr(),
                    "try" => self.try_expr(),
                    "throw" => {
                        self.skip_newlines();
                        Ok(Expr::Throw(Box::new(self.expr()?)))
                    }
                    "return" => {
                        let label = if self
                            .tokens
                            .get(self.at)
                            .is_some_and(|t| t.kind == Kind::Op("@"))
                        {
                            self.at += 1;
                            Some(self.name()?)
                        } else {
                            None
                        };
                        let ends = matches!(
                            self.tokens.get(self.at).map(|t| &t.kind),
                            None | Some(Kind::Newline | Kind::Op("}" | ")" | ";" | ","))
                        );
                        let value = if ends {
                            None
                        } else {
                            Some(Box::new(self.expr()?))
                        };
                        Ok(Expr::Return { label, value })
                    }
                    "break" | "continue" => {
                        let label = if self
                            .tokens
                            .get(self.at)
                            .is_some_and(|t| t.kind == Kind::Op("@"))
                        {
                            self.at += 1;
                            Some(self.name()?)
                        } else {
                            None
                        };
                        Ok(if name == "break" {
                            Expr::Break(label)
                        } else {
                            Expr::Continue(label)
                        })
                    }
                    "object" => {
                        let class =
                            self.class(ClassKind::Object, Vec::new(), Vec::new(), self.line())?;
                        Ok(Expr::Object(Box::new(class)))
                    }
                    "fun" => Err(self.error("anonymous functions aren't supported")),
                    _ => Ok(Expr::Name(name)),
                }
            }
            _ => Err(self.error("expected an expression")),
        }
    }

    /// A branch of `if` or `when`: a block, or one statement.
    fn branch(&mut self) -> Result<Block, String> {
        self.skip_newlines();
        if self.is_op("{") && !self.braces_hold_a_lambda() {
            self.block()
        } else {
            Ok(vec![self.statement()?])
        }
    }

    /// Braces where a block could go that start with a lambda's parameters and its arrow
    /// (`SINGLE -> { _ -> "SINGLE" }`) are a lambda, as Kotlin reads them.
    fn braces_hold_a_lambda(&mut self) -> bool {
        let save = self.at;
        let found = self.expect_op("{").is_ok()
            && self
                .with_nesting(false, |parser| Ok(parser.lambda_params().is_some()))
                .unwrap_or(false);
        self.at = save;
        found
    }

    fn if_expr(&mut self) -> Result<Expr, String> {
        let condition = self.parenthesized()?;
        let then = self.branch()?;
        // `else ->` on the next line is the `when` branch this `if` sits in, not its `else`.
        let else_branch = matches!(self.peek_past_newlines(0), Some(Kind::Name(name)) if name == "else")
            && !matches!(self.peek_past_newlines(1), Some(Kind::Op("->")));
        let otherwise = if else_branch && !self.is_statement_end_before_else() {
            self.skip_newlines();
            self.at += 1;
            Some(self.branch()?)
        } else {
            None
        };
        Ok(Expr::If {
            condition: Box::new(condition),
            then,
            otherwise,
        })
    }

    /// `else` on a later line belongs to the `if` unless a `;` came between.
    fn is_statement_end_before_else(&self) -> bool {
        self.tokens
            .get(self.at)
            .is_some_and(|t| t.kind == Kind::Op(";"))
    }

    fn when_expr(&mut self) -> Result<Expr, String> {
        let mut subject = None;
        let mut binding = None;
        if self
            .tokens
            .get(self.at)
            .is_some_and(|t| t.kind == Kind::Op("("))
        {
            self.at += 1;
            let (found, bound) = self.with_nesting(true, |parser| {
                let bound = if parser.eat_name("val") {
                    let name = parser.name()?;
                    if parser.eat_op(":") {
                        parser.ty()?;
                    }
                    parser.expect_op("=")?;
                    Some(name)
                } else {
                    None
                };
                let expr = parser.expr()?;
                parser.expect_op(")")?;
                Ok((expr, bound))
            })?;
            subject = Some(Box::new(found));
            binding = bound;
        }
        self.skip_newlines();
        self.expect_op("{")?;
        let branches = self.with_nesting(false, |parser| {
            let mut branches = Vec::new();
            loop {
                parser.skip_newlines();
                if parser.eat_op("}") {
                    break;
                }
                if parser.eat_op(";") {
                    continue;
                }
                let mut conditions = Vec::new();
                if !parser.eat_name("else") {
                    loop {
                        parser.skip_newlines();
                        let negated = parser.is_op("!")
                            && matches!(parser.tokens.get(parser.at + 1).map(|t| &t.kind), Some(Kind::Name(name)) if name == "is" || name == "in");
                        if negated {
                            parser.at += 1;
                        }
                        if parser.eat_name("is") {
                            conditions.push(WhenCondition::Is {
                                ty: parser.ty()?,
                                negated,
                            });
                        } else if parser.eat_name("in") {
                            conditions.push(WhenCondition::In {
                                range: parser.expr()?,
                                negated,
                            });
                        } else {
                            conditions.push(WhenCondition::Expr(parser.expr()?));
                        }
                        parser.skip_newlines();
                        if !parser.eat_op(",") {
                            break;
                        }
                    }
                }
                parser.skip_newlines();
                let guard = if parser.eat_name("if") {
                    Some(parser.expr()?)
                } else {
                    None
                };
                parser.skip_newlines();
                parser.expect_op("->")?;
                let body = parser.branch()?;
                branches.push(WhenBranch {
                    conditions,
                    guard,
                    body,
                });
            }
            Ok(branches)
        })?;
        Ok(Expr::When {
            subject,
            binding,
            branches,
        })
    }

    fn try_expr(&mut self) -> Result<Expr, String> {
        let body = self.block()?;
        let mut catches = Vec::new();
        let mut finally = None;
        loop {
            if matches!(self.peek_past_newlines(0), Some(Kind::Name(name)) if name == "catch") {
                self.skip_newlines();
                self.at += 1;
                self.expect_op("(")?;
                let (name, ty) = self.with_nesting(true, |parser| {
                    let name = parser.name()?;
                    parser.expect_op(":")?;
                    let ty = parser.ty()?;
                    parser.expect_op(")")?;
                    Ok((name, ty))
                })?;
                let body = self.block()?;
                catches.push(Catch { name, ty, body });
            } else if matches!(self.peek_past_newlines(0), Some(Kind::Name(name)) if name == "finally")
            {
                self.skip_newlines();
                self.at += 1;
                finally = Some(self.block()?);
            } else {
                break;
            }
        }
        Ok(Expr::Try {
            body,
            catches,
            finally,
        })
    }
}

/// Whether an expression can take a call's parentheses, type arguments or trailing lambda.
fn is_callable(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Name(_) | Expr::Member { .. } | Expr::Super | Expr::Call { .. } | Expr::This(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_theme_source() {
        let file = parse_file(
            r#"package eu.kanade.tachiyomi.extension.all.comicsvalley

import eu.kanade.tachiyomi.multisrc.madara.Madara
import java.util.Locale

@Source
abstract class ComicsValley : Madara() {
    override val mangaSubString = "comics-new"
    override val chapterMode = ChapterMode.MangaAjax
    override val chapterDateFormat = DateTimeFormatter.ofPattern(
        "dd/MM/yyyy",
        Locale.ROOT,
    )
    override fun chapterListSelector() = "li.wp-manga-chapter, " +
        "div.chapter"

    override fun chapterFromElement(element: Element, mangaPath: String): SChapter? = super.chapterFromElement(element, mangaPath)?.apply {
        name = element.selectFirst("a + a")!!.text()
    }

    companion object {
        private const val X = "x"
    }
}
"#,
        )
        .unwrap();
        assert_eq!(
            file.package,
            "eu.kanade.tachiyomi.extension.all.comicsvalley"
        );
        assert_eq!(file.imports[0].0, "Madara");
        let Member::Class(class) = &file.members[0] else {
            panic!("a class");
        };
        assert_eq!(class.name, "ComicsValley");
        assert_eq!(class.annotations, ["Source"]);
        assert_eq!(class.supers[0].ty.name, "Madara");
        let names: Vec<&str> = class.members.iter().map(Member::name).collect();
        assert_eq!(
            names,
            [
                "mangaSubString",
                "chapterMode",
                "chapterDateFormat",
                "chapterListSelector",
                "chapterFromElement",
                ""
            ]
        );
        let Member::Fun(fun) = &class.members[3] else {
            panic!("a function");
        };
        assert!(matches!(&fun.body, Some(Body::Expr(Expr::Binary { op, .. })) if op == "+"));
    }

    #[test]
    fn reads_newer_kotlin_and_names_that_look_like_keywords() {
        let file = parse_file(
            r#"
typealias Localized =
    @Serializable(LocalizedSerializer::class)
    Map<String, String>

private inline val String?.timestamp: Long
    get() = this?.length?.toLong() ?: 0L

class Source {
    override fun getFilterList(data: JsonElement?): FilterList {
        val filters = mutableListOf<Filter<*>>(Header("x"))
        data?.parseAs<List<Pair<String, String>>>()?.also { filters += it }
        val rows = data[Column.Group]!!.text()
        val shared = (genres intersect other).size
        val trust = @Suppress("x") object : X509TrustManager {
            override fun check() = Unit
        }
        val query = $$"""query ($id: ID) { a(id: $$id) $${'$'} }"""
        val lock = " 🔒"
        val pages = when (val json = text.parseAs<JsonElement>()) {
            is JsonObject if "i" in json -> 1
            else -> 2
        }
        when (it) {
            is FilterOrder -> if (it.selected != null) {
                url.add(it.selected)
            }
            else -> {}
        }
        return FilterList(filters)
    }

    private fun String?.removeLegacyRating(): String? = when (this) {
        "safe" -> this
        else -> null
    }

    private suspend fun browse(page: Int, params: HttpUrl.Builder.() -> Unit = {}): MangasPage = TODO()

    context(source: Dilar)
    fun toSManga() = SManga.create()
}
"#,
        )
        .unwrap();
        let Member::Class(class) = &file.members[2] else {
            panic!("a class");
        };
        let names: Vec<&str> = class.members.iter().map(Member::name).collect();
        assert_eq!(
            names,
            ["getFilterList", "removeLegacyRating", "browse", "toSManga"]
        );
        let Member::Fun(fun) = &class.members[0] else {
            panic!("a function");
        };
        let Some(Body::Block(body)) = &fun.body else {
            panic!("a body");
        };
        let query = body.iter().find_map(|statement| match statement {
            Stmt::Val {
                names,
                value: Some(Expr::Str(parts)),
                ..
            } if names[0] == "query" => Some(parts.clone()),
            _ => None,
        });
        let parts = query.unwrap();
        assert_eq!(parts[0], StrPart::Text("query ($id: ID) { a(id: ".into()));
        assert!(matches!(&parts[1], StrPart::Code(Expr::Name(name)) if name == "id"));
        let lock = body.iter().find_map(|statement| match statement {
            Stmt::Val {
                names,
                value: Some(Expr::Str(parts)),
                ..
            } if names[0] == "lock" => Some(parts.clone()),
            _ => None,
        });
        assert_eq!(lock.unwrap(), [StrPart::Text(" \u{1f512}".into())]);
    }

    #[test]
    fn reads_lambdas_calls_and_line_breaks() {
        let expr = parse_expr(
            "document.select(\".post-body table\").map { element: Element ->\n SManga.create().apply {\n setUrlWithoutDomain(element.select(\"a\").first()!!.absUrl(\"href\"))\n title = element.text()\n }\n }",
        )
        .unwrap();
        let Expr::Call {
            lambda: Some(lambda),
            ..
        } = expr
        else {
            panic!("a call with a lambda");
        };
        assert_eq!(lambda.params, [LambdaParam::Name("element".into())]);
        assert_eq!(lambda.body.len(), 1);
        let chained = parse_expr("a\n  .b()\n  ?.c ?: d").unwrap();
        assert!(matches!(chained, Expr::Binary { op, .. } if op == "?:"));
        let typed = parse_expr("x.parseAs<List<String>>()").unwrap();
        assert!(matches!(typed, Expr::Call { ref type_args, .. } if type_args.len() == 1));
        let compare = parse_expr("a < b && c > d").unwrap();
        assert!(matches!(compare, Expr::Binary { op, .. } if op == "&&"));
        let when = parse_expr("when (lang) {\n \"en\" -> \"a\"\n \"es\", \"pt\" -> \"b\"\n else -> throw Exception()\n}").unwrap();
        assert!(matches!(when, Expr::When { ref branches, .. } if branches.len() == 3));
        let pair = parse_expr("\"a\" to \"b\"").unwrap();
        assert!(matches!(pair, Expr::Binary { op, .. } if op == "to"));
        let template = parse_expr("\"$baseUrl/page/${page + 1}\"").unwrap();
        assert!(matches!(template, Expr::Str(ref parts) if parts.len() == 3));
    }
}
