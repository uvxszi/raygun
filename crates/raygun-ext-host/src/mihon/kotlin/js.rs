//! JavaScript for the Kotlin values a themed source sets: a selector, a path, a date format, a
//! list of pairs, built from literals, string templates over the source's own fields (`baseUrl`,
//! `lang`, `name`), its constants and the theme's own value (`super.x`). What can't be read as a
//! value (a call into the network, code with effects) is refused with the reason, so the source
//! is listed as one that can't be converted.
//!
//! The JavaScript is an expression over `t`, the theme instance at run time, and `K`, the
//! runtime's Kotlin helpers (`mihon/prelude.js`).

use std::collections::HashMap;

use super::ast::*;

/// What names mean while making a value.
pub struct Scope<'a> {
    /// Constants of the class and its file (`const val`, `val` with a value), by name.
    pub constants: &'a HashMap<String, Expr>,
    /// The theme's own values, which another value may read (`archiveSelector()`).
    pub options: &'a [&'a str],
}

/// The source's own fields, read from the theme instance.
const SOURCE_FIELDS: [&str; 5] = ["baseUrl", "lang", "name", "id", "versionId"];

/// The JavaScript for `expr`.
pub fn value(expr: &Expr, scope: &Scope<'_>) -> Result<String, String> {
    Gen { scope, depth: 0 }.expr(expr)
}

struct Gen<'a> {
    scope: &'a Scope<'a>,
    /// Constants expanded within constants, against cycles.
    depth: usize,
}

fn quote(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_owned())
}

fn unsupported(what: &str) -> String {
    format!("its {what} isn't a value Raygun can read")
}

impl Gen<'_> {
    fn expr(&mut self, expr: &Expr) -> Result<String, String> {
        Ok(match expr {
            Expr::Int(value) => value.to_string(),
            Expr::Float(value) => {
                if value.is_finite() {
                    format!("{value}")
                } else {
                    return Err(unsupported("number"));
                }
            }
            Expr::Bool(value) => value.to_string(),
            Expr::Null => "null".to_owned(),
            Expr::Char(value) => quote(&value.to_string()),
            Expr::Str(parts) => self.template(parts)?,
            Expr::Name(name) => self.name(name)?,
            Expr::Member { target, name, safe } => self.member(target, name, *safe)?,
            Expr::Call {
                callee,
                args,
                lambda,
                ..
            } => {
                if lambda.is_some() {
                    return Err(unsupported("code"));
                }
                self.call(callee, args)?
            }
            Expr::Binary { op, left, right } => {
                let (left, right) = (self.expr(left)?, self.expr(right)?);
                match op.as_str() {
                    "+" => format!("K.plus({left}, {right})"),
                    "-" | "*" | "%" => format!("({left} {op} {right})"),
                    "/" => format!("K.div({left}, {right})"),
                    "to" => format!("[{left}, {right}]"),
                    "==" => format!("K.eq({left}, {right})"),
                    "!=" => format!("!K.eq({left}, {right})"),
                    "&&" | "||" => format!("({left} {op} {right})"),
                    "?:" => format!("({left} ?? {right})"),
                    "<" | ">" | "<=" | ">=" => format!("({left} {op} {right})"),
                    _ => return Err(unsupported(&format!("operator {op}"))),
                }
            }
            Expr::Unary { op, expr } => match *op {
                "-" | "!" | "+" => format!("({op}{})", self.expr(expr)?),
                _ => return Err(unsupported("operator")),
            },
            Expr::If {
                condition,
                then,
                otherwise,
            } => {
                let otherwise = otherwise.as_ref().ok_or_else(|| unsupported("if"))?;
                format!(
                    "({} ? {} : {})",
                    self.expr(condition)?,
                    self.single(then)?,
                    self.single(otherwise)?
                )
            }
            Expr::When {
                subject,
                binding: None,
                branches,
            } => self.when(subject.as_deref(), branches)?,
            Expr::Throw(_) => {
                // A source refusing a language it doesn't serve: an error when it's read.
                "K.raise(\"this source doesn't serve this language\")".to_owned()
            }
            // The theme's messages: `intl["alt_names_heading"]`.
            Expr::Index { target, index } if matches!(target.as_ref(), Expr::Name(name) if name == "intl") => {
                match index.as_slice() {
                    [key] => format!("t.intl({})", self.expr(key)?),
                    _ => return Err(unsupported("message")),
                }
            }
            _ => return Err(unsupported("code")),
        })
    }

    /// A branch that is one expression.
    fn single(&mut self, block: &Block) -> Result<String, String> {
        match block.as_slice() {
            [Stmt::Expr(expr)] => self.expr(expr),
            _ => Err(unsupported("code")),
        }
    }

    fn when(&mut self, subject: Option<&Expr>, branches: &[WhenBranch]) -> Result<String, String> {
        let subject = subject.map(|subject| self.expr(subject)).transpose()?;
        let mut out = String::from("K.raise(\"no branch fits\")");
        for branch in branches.iter().rev() {
            let body = self.single(&branch.body)?;
            if branch.conditions.is_empty() {
                out = body;
                continue;
            }
            let mut tests = Vec::new();
            for condition in &branch.conditions {
                match (condition, &subject) {
                    (WhenCondition::Expr(value), Some(subject)) => {
                        tests.push(format!("K.eq({subject}, {})", self.expr(value)?));
                    }
                    (WhenCondition::Expr(value), None) => tests.push(self.expr(value)?),
                    (WhenCondition::In { range, negated }, Some(subject)) => {
                        let test = format!("K.contains({}, {subject})", self.expr(range)?);
                        tests.push(if *negated { format!("!{test}") } else { test });
                    }
                    _ => return Err(unsupported("when")),
                }
            }
            out = format!("(({}) ? {body} : {out})", tests.join(" || "));
        }
        Ok(out)
    }

    fn template(&mut self, parts: &[StrPart]) -> Result<String, String> {
        let mut pieces = Vec::new();
        for part in parts {
            match part {
                StrPart::Text(text) => pieces.push(quote(text)),
                StrPart::Code(expr) => pieces.push(format!("K.str({})", self.expr(expr)?)),
            }
        }
        Ok(match pieces.len() {
            0 => "\"\"".to_owned(),
            1 => pieces.remove(0),
            _ => format!("({})", pieces.join(" + ")),
        })
    }

    fn name(&mut self, name: &str) -> Result<String, String> {
        if SOURCE_FIELDS.contains(&name) {
            return Ok(format!("t.{name}"));
        }
        if self.scope.options.contains(&name) {
            return Ok(format!("t.o.{name}"));
        }
        if let Some(constant) = self.scope.constants.get(name) {
            if self.depth > 8 {
                return Err(unsupported("constant"));
            }
            self.depth += 1;
            let value = self.expr(constant);
            self.depth -= 1;
            return value;
        }
        Err(format!("it reads {name}, which Raygun can't follow"))
    }

    /// A dotted constant: an enum entry, a locale, a regex option.
    fn member(&mut self, target: &Expr, name: &str, safe: bool) -> Result<String, String> {
        if let Expr::Super = target {
            return Ok(format!("t.base.{name}"));
        }
        if let Expr::Name(owner) = target {
            match owner.as_str() {
                "Locale" => return Ok(locale_constant(name)),
                "RegexOption" => {
                    return Ok(quote(match name {
                        "IGNORE_CASE" => "i",
                        "MULTILINE" => "m",
                        "DOT_MATCHES_ALL" => "s",
                        _ => "",
                    }));
                }
                "SManga" => {
                    return Ok(match name {
                        "ONGOING" => "1",
                        "COMPLETED" => "2",
                        "LICENSED" => "3",
                        "PUBLISHING_FINISHED" => "4",
                        "CANCELLED" => "5",
                        "ON_HIATUS" => "6",
                        _ => "0",
                    }
                    .to_owned());
                }
                // An enum entry the theme reads by name (`ChapterMode.MangaAjax`).
                first
                    if first.chars().next().is_some_and(char::is_uppercase)
                        && !self.scope.constants.contains_key(first) =>
                {
                    return Ok(quote(name));
                }
                _ => {}
            }
        }
        // A string's own properties.
        let target = self.expr(target)?;
        let access = match name {
            "length" => format!("K.str({target}).length"),
            _ => return Err(unsupported(&format!("property {name}"))),
        };
        Ok(if safe {
            format!("({target} == null ? null : {access})")
        } else {
            access
        })
    }

    fn args(&mut self, args: &[Arg]) -> Result<Vec<String>, String> {
        args.iter()
            .map(|arg| {
                if arg.spread {
                    Err(unsupported("arguments"))
                } else {
                    self.expr(&arg.value)
                }
            })
            .collect()
    }

    fn call(&mut self, callee: &Expr, args: &[Arg]) -> Result<String, String> {
        match callee {
            // Another of the theme's values: `archiveSelector()`.
            Expr::Name(name) if args.is_empty() && self.scope.options.contains(&name.as_str()) => {
                Ok(format!("t.o.{name}"))
            }
            Expr::Name(name) => {
                let values = self.args(args)?;
                Ok(match name.as_str() {
                    "listOf" | "arrayOf" | "setOf" | "mutableListOf" | "mutableSetOf"
                    | "listOfNotNull" | "arrayListOf" | "hashSetOf" | "linkedSetOf" => {
                        let list = format!("[{}]", values.join(", "));
                        if name == "listOfNotNull" {
                            format!("{list}.filter((x) => x != null)")
                        } else {
                            list
                        }
                    }
                    "emptyList" | "emptyArray" | "emptySet" => "[]".to_owned(),
                    "mapOf" | "mutableMapOf" | "hashMapOf" => {
                        format!("K.mapOf([{}])", values.join(", "))
                    }
                    "Pair" if values.len() == 2 => format!("[{}, {}]", values[0], values[1]),
                    "Locale" => format!("K.locale({})", values.join(", ")),
                    "SimpleDateFormat" => format!("K.dateFormat({})", values.join(", ")),
                    "Regex" => format!("K.regex({})", values.join(", ")),
                    "setOfNotNull" => format!("[{}].filter((x) => x != null)", values.join(", ")),
                    _ => return Err(format!("it calls {name}, which Raygun can't follow")),
                })
            }
            Expr::Member { target, name, safe } => {
                // `super.chapterListSelector()`: the theme's own value.
                if let Expr::Super = target.as_ref() {
                    return Ok(format!("t.base.{name}"));
                }
                if let Expr::Name(owner) = target.as_ref() {
                    let values = self.args(args)?;
                    match (owner.as_str(), name.as_str()) {
                        ("DateTimeFormatter", "ofPattern") => {
                            return Ok(format!("K.dateFormat({})", values.join(", ")));
                        }
                        ("Locale", "forLanguageTag") => {
                            return Ok(format!("K.locale({})", values.join(", ")));
                        }
                        ("Locale", "of") => return Ok(format!("K.locale({})", values.join(", "))),
                        ("TimeZone", "getTimeZone") | ("ZoneId", "of") => {
                            return Ok(format!("K.zone({})", values.join(", ")));
                        }
                        ("intl", "format") => return Ok(format!("t.intl({})", values.join(", "))),
                        _ => {}
                    }
                }
                let receiver = self.expr(target)?;
                let values = self.args(args)?;
                let call = match (name.as_str(), values.as_slice()) {
                    ("toRegex", _) => format!("K.regex({receiver}, {})", values.join(", ")),
                    ("trimIndent", []) => format!("K.trimIndent({receiver})"),
                    ("trimMargin", []) => format!("K.trimMargin({receiver})"),
                    ("trim", []) => format!("K.str({receiver}).trim()"),
                    ("lowercase", _) => format!("K.str({receiver}).toLowerCase()"),
                    ("uppercase", _) => format!("K.str({receiver}).toUpperCase()"),
                    ("replace", [from, to]) => format!("K.replace({receiver}, {from}, {to})"),
                    ("removePrefix", [prefix]) => format!("K.removePrefix({receiver}, {prefix})"),
                    ("removeSuffix", [suffix]) => format!("K.removeSuffix({receiver}, {suffix})"),
                    ("toInt" | "toLong", []) => format!("parseInt({receiver}, 10)"),
                    ("format", _) => format!("K.format({receiver}, {})", values.join(", ")),
                    ("toTypedArray" | "toList" | "toSet" | "toMutableList", []) => receiver.clone(),
                    ("joinToString", _) => {
                        format!("K.joinToString({receiver}, {})", values.join(", "))
                    }
                    ("plus", [other]) => format!("K.plus({receiver}, {other})"),
                    ("withZone", [zone]) => format!("K.withZone({receiver}, {zone})"),
                    _ => return Err(format!("it calls {name}, which Raygun can't follow")),
                };
                Ok(if *safe {
                    format!("({receiver} == null ? null : {call})")
                } else {
                    call
                })
            }
            _ => Err(unsupported("code")),
        }
    }
}

/// `Locale.ENGLISH`, `Locale.ROOT`… as the runtime's locale.
fn locale_constant(name: &str) -> String {
    let tag = match name {
        "ENGLISH" | "US" | "UK" | "CANADA" => "en",
        "FRENCH" | "FRANCE" | "CANADA_FRENCH" => "fr",
        "GERMAN" | "GERMANY" => "de",
        "ITALIAN" | "ITALY" => "it",
        "JAPANESE" | "JAPAN" => "ja",
        "KOREAN" | "KOREA" => "ko",
        "CHINESE" | "CHINA" | "SIMPLIFIED_CHINESE" | "PRC" => "zh",
        "TRADITIONAL_CHINESE" | "TAIWAN" => "zh-TW",
        _ => "root",
    };
    format!("K.locale({})", quote(tag))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mihon::kotlin::parse_expr;

    fn js(source: &str) -> Result<String, String> {
        let constants = HashMap::from([("PREFIX".to_owned(), parse_expr("\"/series\"").unwrap())]);
        value(
            &parse_expr(source).unwrap(),
            &Scope {
                constants: &constants,
                options: &["archiveSelector"],
            },
        )
    }

    #[test]
    fn values_become_javascript() {
        assert_eq!(js("\"comic\"").unwrap(), "\"comic\"");
        assert_eq!(js("ChapterMode.MangaAjax").unwrap(), "\"MangaAjax\"");
        assert_eq!(
            js("DateTimeFormatter.ofPattern(\"dd/MM/yyyy\", Locale.ROOT)").unwrap(),
            "K.dateFormat(\"dd/MM/yyyy\", K.locale(\"root\"))"
        );
        assert_eq!(
            js("SimpleDateFormat(\"MMMM d, yyyy\", Locale(\"pt\", \"BR\"))").unwrap(),
            "K.dateFormat(\"MMMM d, yyyy\", K.locale(\"pt\", \"BR\"))"
        );
        assert_eq!(
            js("super.seriesGenreSelector + \", .x\"").unwrap(),
            "K.plus(t.base.seriesGenreSelector, \", .x\")"
        );
        assert_eq!(
            js("\"$baseUrl$PREFIX\"").unwrap(),
            "(K.str(t.baseUrl) + K.str(\"/series\"))"
        );
        assert_eq!(
            js("listOf(\"a\" to \"b\", Pair(\"c\", \"d\"))").unwrap(),
            "[[\"a\", \"b\"], [\"c\", \"d\"]]"
        );
        assert!(
            js("when (lang) { \"en\" -> \"x\" else -> \"y\" }")
                .unwrap()
                .contains("K.eq(t.lang, \"en\")")
        );
        assert_eq!(js("archiveSelector()").unwrap(), "t.o.archiveSelector");
        assert_eq!(
            js("listOf(intl[\"order_by_filter_az\"] to \"title\")").unwrap(),
            "[[t.intl(\"order_by_filter_az\"), \"title\"]]"
        );
        assert!(js("client.newCall(GET(baseUrl)).execute()").is_err());
        assert!(js("preferences.getString(\"x\", \"y\")").is_err());
    }
}
