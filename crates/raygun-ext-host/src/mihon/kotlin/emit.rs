//! JavaScript for whole Kotlin functions, for sources written without a theme
//! (`crate::mihon::transpile`). Everything the generated code calls is asynchronous and awaited,
//! as a source's `suspend` functions are; member calls on values go through the runtime's
//! Kotlin, Jsoup and OkHttp tables (`KT.m`, `mihon/kotlin.js`), which know what a value is when
//! the code runs. Calls the runtime doesn't have are refused here, with their name, so a source
//! whose code needs them isn't converted.

use std::collections::{HashMap, HashSet};

use super::ast::*;

/// The names a source's class knows besides its locals.
#[derive(Debug, Default)]
pub struct Context {
    /// Its functions (with the base class's), each overload's parameter names.
    pub functions: HashMap<String, Vec<Vec<String>>>,
    /// Its properties (with the base class's).
    pub properties: HashSet<String>,
    /// Extension functions and properties its files declare.
    pub extensions: HashSet<String>,
    pub extension_properties: HashSet<String>,
    /// Constants its files and companion objects declare, read as `C.name`.
    pub constants: HashSet<String>,
    /// Classes and objects its files declare (filters, DTOs), made as `D.Name(...)`.
    pub classes: HashSet<String>,
    /// The methods and properties of those classes, called on their instances.
    pub class_members: HashSet<String>,
    /// The values each of those classes' companion objects holds, read as `C["Class.name"]`.
    pub companions: HashMap<String, HashSet<String>>,
}

/// A companion value's key on `C`.
pub fn companion_key(class: &str, name: &str) -> String {
    format!("{class}.{name}")
}

/// Method names the runtime has, on some kind of value (`mihon/kotlin.js` keeps the same list).
#[rustfmt::skip]
pub const METHODS: &[&str] = &[
    // Any value
    "toString", "equals", "hashCode", "let", "also", "apply", "run", "takeIf", "takeUnless", "use",
    // Strings
    "substringAfter", "substringBefore", "substringAfterLast", "substringBeforeLast",
    "removePrefix", "removeSuffix", "removeSurrounding", "trim", "trimStart", "trimEnd", "split",
    "lowercase", "uppercase", "contains", "startsWith", "endsWith", "replace", "replaceFirst",
    "isEmpty", "isNotEmpty", "isBlank", "isNotBlank", "isNullOrBlank", "isNullOrEmpty", "ifBlank",
    "ifEmpty", "orEmpty", "toInt", "toIntOrNull", "toLong", "toLongOrNull", "toFloat",
    "toFloatOrNull", "toDouble", "toDoubleOrNull", "toBoolean", "substring", "take", "drop",
    "takeLast", "dropLast", "indexOf", "lastIndexOf", "padStart", "padEnd", "repeat", "lines",
    "matches", "toRegex", "format", "capitalize", "replaceFirstChar", "toHttpUrl",
    "toHttpUrlOrNull", "asJsoup", "parseAs", "trimIndent", "trimMargin", "titlecase", "toCharArray",
    "chunked", "count", "isDigit", "isLetter", "isLetterOrDigit", "isWhitespace", "isUpperCase",
    "isLowerCase", "digitToInt", "code", "toRequestBody", "toMediaType", "toMediaTypeOrNull",
    "encodeToByteArray", "decodeToString", "compareTo", "plus", "minus", "times", "div", "rem",
    "not", "and", "or", "coerceAtLeast", "coerceAtMost", "coerceIn", "toUInt", "roundToInt",
    "roundToLong", "toLocaleString", "reversed", "first", "last", "firstOrNull", "lastOrNull",
    "single", "singleOrNull", "get", "getOrNull", "getOrElse", "filter", "filterNot", "map",
    "mapNotNull", "any", "all", "none", "forEach", "forEachIndexed", "sumOf", "trimEnd",
    // Collections
    "mapIndexed", "mapIndexedNotNull", "filterNotNull", "filterIndexed", "filterIsInstance",
    "onEach", "find", "findLast", "joinToString", "takeWhile", "dropWhile", "distinct",
    "distinctBy", "sorted", "sortedBy", "sortedByDescending", "sortedDescending", "sortedWith",
    "asReversed", "flatten", "flatMap", "groupBy", "associate", "associateBy", "associateWith",
    "toList", "toMutableList", "toSet", "toMutableSet", "toTypedArray", "toMap", "toMutableMap",
    "sum", "maxOf", "minOf", "maxByOrNull", "minByOrNull", "maxOrNull", "minOrNull", "max", "min",
    "zip", "withIndex", "partition", "add", "addAll", "remove", "removeAt", "removeAll", "removeIf",
    "clear", "set", "subList", "firstNotNullOfOrNull", "firstNotNullOf", "indexOfFirst",
    "indexOfLast", "reduce", "fold", "average", "elementAt", "elementAtOrNull", "containsKey",
    "containsValue", "getOrDefault", "put", "putAll", "putIfAbsent", "getOrPut", "entries", "keys",
    "values", "component1", "component2", "component3", "plusElement", "shuffled", "random",
    "slice", "ifEmpty", "isNullOrEmpty", "orEmpty", "asSequence", "asIterable", "toSortedMap",
    "mapKeys", "mapValues", "filterKeys", "filterValues", "windowed", "lastIndexOf", "copy",
    "invoke",
    // Jsoup
    "select", "selectFirst", "selectLast", "attr", "absUrl", "hasAttr", "text", "ownText",
    "wholeText", "html", "outerHtml", "data", "hasClass", "className", "classNames", "id",
    "tagName", "nodeName", "parent", "parents", "children", "child", "nextElementSibling",
    "previousElementSibling", "nextElementSiblings", "siblingElements", "eachText", "eachAttr",
    "location", "body", "head", "title", "is", "not", "attrOrNull", "textOrNull", "ownTextOrNull",
    "hasText", "attributes", "selectFirstOrNull", "firstElementChild", "lastElementChild",
    "elementSiblingIndex", "baseUri",
    // OkHttp, Keiyoushi's helpers, Mihon's models
    "newBuilder", "build", "addQueryParameter", "addEncodedQueryParameter", "setQueryParameter",
    "setEncodedQueryParameter", "removeAllQueryParameters", "addPathSegment", "addPathSegments",
    "addEncodedPathSegment", "addEncodedPathSegments", "setPathSegment", "fragment",
    "queryParameter", "queryParameterValues", "resolve", "header", "headers", "addHeader",
    "removeHeader", "url", "method", "post", "put", "delete", "cacheControl", "fetch", "from",
    "newCall", "execute", "await", "awaitSuccess", "string", "bytes", "close", "body",
    "isSuccessful", "code", "request", "add", "setUrlWithoutDomain", "create", "headersBuilder",
    "getMangaUrl", "getChapterUrl", "tryParse", "parse", "time", "decodeFromString",
    "encodeToString", "decodeFromJsonElement", "jsonObject", "jsonArray", "jsonPrimitive",
    "content", "contentOrNull", "int", "intOrNull", "long", "longOrNull", "double", "doubleOrNull",
    "float", "boolean", "booleanOrNull", "jsonNull", "toJsonString", "toJsonElement",
    "parseToJsonElement", "find", "findAll", "groupValues", "value", "destructured", "range",
    "containsMatchIn", "matchEntire", "groups", "toPattern", "setTime", "getTime", "addHeaderLine",
    "getString", "getBoolean", "getInt", "edit", "putString", "putBoolean", "commit", "apply",
    "use", "head", "rateLimit", "rateLimitHost", "connectTimeout", "readTimeout", "writeTimeout",
    "callTimeout", "followRedirects", "append", "appendLine", "insert", "setLength", "getGenres",
    "toMultimap", "addEncoded", "putJsonObject", "putJsonArray", "getStringSet",
    "firstInstanceOrNull", "isSuccess", "isFailure", "getOrThrow", "getOrDefault",
    "exceptionOrNull", "contentType", "removeHeader", "enqueue", "encodedPath", "port", "host",
    "scheme", "query", "headersOf", "of", "forLanguageTag", "ofPattern", "currentTimeMillis",
    "values", "valueOf", "ordinal", "unaryMinus", "toChar", "tryParseDate", "tryParseDateTime",
    "firstInstance", "mapTo", "mapNotNullTo", "filterTo", "isIncluded", "isExcluded", "isIgnored",
    "toEpochMilliseconds", "toEpochMilli", "getLong", "putInt", "putLong", "putStringSet",
    "fromEpochMilliseconds", "ofEpochMilli", "ofEpochSecond", "now",
    // Coroutines, calendars and zones
    "awaitAll", "withLock", "withPermit", "lock", "unlock", "acquire", "release", "withZone",
    "withLocale", "setTimeZone", "getTimeZone", "systemDefault", "ofHours", "getDefault", "getID",
    "getInstance", "clone", "before", "after", "getTimeInMillis", "setTimeInMillis", "randomUUID",
    // java.time
    "atStartOfDay", "atZone", "toInstant", "toLocalDate", "toEpochSecond", "toEpochDay",
    "minusDays", "plusDays", "minusWeeks", "plusWeeks", "minusMonths", "plusMonths", "minusYears",
    "plusYears", "minusHours", "plusHours", "minusMinutes", "plusMinutes", "minusSeconds",
    "plusSeconds", "getYear", "getMonthValue", "getDayOfMonth", "isBefore", "isAfter", "ofEpochDay",
    "unescapeEntities", "getProperty", "getValue",
];

fn quote(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_owned())
}

/// A JavaScript name for a Kotlin local.
pub fn local(name: &str) -> String {
    let mut out = String::from("$");
    for char in name.chars() {
        if char.is_ascii_alphanumeric() || char == '_' {
            out.push(char);
        } else {
            out.push('_');
        }
    }
    out
}

/// How a block's last expression is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tail {
    /// Returned (a function's expression body, a lambda, a branch of an expression).
    Return,
    /// Dropped.
    Discard,
}

pub struct Gen<'a> {
    cx: &'a Context,
    /// Locals in scope, innermost last: each Kotlin name's JavaScript name.
    scopes: Vec<HashMap<String, String>>,
    /// Implicit receivers, innermost last (their JavaScript names).
    receivers: Vec<String>,
    /// The labels each receiver answers to in `this@label`: its function's name, its class's.
    receiver_labels: Vec<Vec<String>>,
    /// Labels of the lambdas being made, innermost last.
    labels: Vec<String>,
    /// How deep in lambdas and expression functions the code is: a plain `return` there leaves
    /// the Kotlin function, which the runtime does with an exception.
    depth: usize,
    /// A plain `return` was made into `KT.ret`: the function catches it.
    pub catches_return: bool,
    next: u32,
    /// The declared class whose code this is: its companion's values are in scope.
    pub class_scope: Option<String>,
    /// Local functions with a receiver (`fun Builder.addParam(…)` in a body), by JavaScript name:
    /// their receiver is passed first.
    local_extensions: HashSet<String>,
}

fn unsupported(what: impl std::fmt::Display) -> String {
    format!("its code uses {what}, which Raygun's converter doesn't read yet")
}

impl<'a> Gen<'a> {
    pub fn new(cx: &'a Context) -> Self {
        Self {
            cx,
            scopes: vec![HashMap::new()],
            receivers: Vec::new(),
            receiver_labels: Vec::new(),
            labels: Vec::new(),
            depth: 0,
            catches_return: false,
            next: 0,
            class_scope: None,
            local_extensions: HashSet::new(),
        }
    }

    /// A value of the current class's companion, by name.
    fn companion_value(&self, name: &str) -> Option<String> {
        let class = self.class_scope.as_deref()?;
        self.cx
            .companions
            .get(class)
            .filter(|names| names.contains(name))
            .map(|_| format!("(await C.get({}))", quote(&companion_key(class, name))))
    }

    fn temp(&mut self) -> String {
        self.next += 1;
        format!("$t{}", self.next)
    }

    /// Whether a member call by this name has somewhere to go: the runtime's tables, or a class
    /// the source declares.
    fn known(&self, name: &str) -> bool {
        METHODS.contains(&name) || self.cx.class_members.contains(name)
    }

    /// Declares a local and gives its JavaScript name. Kotlin lets a local shadow a parameter or
    /// an outer local, even reading it in its own initializer (`val page = page + 1`), which
    /// JavaScript doesn't: such a local gets a name of its own.
    fn declare(&mut self, name: &str) -> String {
        let js = if self.is_local(name) {
            self.next += 1;
            format!("{}${}", local(name), self.next)
        } else {
            local(name)
        };
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_owned(), js.clone());
        }
        js
    }

    /// A local's JavaScript name, when the name is a local here.
    fn resolve(&self, name: &str) -> Option<String> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn is_local(&self, name: &str) -> bool {
        self.resolve(name).is_some()
    }

    fn scoped<T>(&mut self, work: impl FnOnce(&mut Self) -> T) -> T {
        self.scopes.push(HashMap::new());
        let result = work(self);
        self.scopes.pop();
        result
    }

    fn push_receiver<'l>(&mut self, name: &str, labels: impl IntoIterator<Item = Option<&'l str>>) {
        self.receivers.push(name.to_owned());
        self.receiver_labels
            .push(labels.into_iter().flatten().map(str::to_owned).collect());
    }

    fn pop_receiver(&mut self) {
        self.receivers.pop();
        self.receiver_labels.pop();
    }

    /// `this@label`: the receiver answering to it, else the source.
    fn labelled_this(&self, label: &str) -> String {
        self.receivers
            .iter()
            .zip(&self.receiver_labels)
            .rev()
            .find(|(_, labels)| labels.iter().any(|known| known == label))
            .map_or_else(|| "self".to_owned(), |(receiver, _)| receiver.clone())
    }

    /// The receivers in scope, innermost first, then the source: `[$r2, $r1, self]`.
    fn receiver_list(&self) -> String {
        let mut list: Vec<&str> = self.receivers.iter().rev().map(String::as_str).collect();
        list.push("self");
        format!("[{}]", list.join(", "))
    }

    // ---- Functions ----

    /// A function's JavaScript: `async ($a, $b = 2) => { … }`; a receiver comes first as `$this`.
    pub fn function(&mut self, fun: &FunDecl) -> Result<String, String> {
        let body = fun
            .body
            .as_ref()
            .ok_or_else(|| format!("{} has no body", fun.name))?;
        // The function's own state; the outer one comes back whatever happens inside.
        let saved = (
            self.depth,
            self.catches_return,
            std::mem::take(&mut self.labels),
            self.receivers.len(),
        );
        self.depth = 0;
        self.catches_return = false;
        let result = self.scoped(|writer| {
            let mut params = Vec::new();
            // Defaults are worked out in the body, in order: they may await, which a parameter
            // list can't.
            let mut defaults = Vec::new();
            if let Some(receiver) = &fun.receiver {
                // An outer function's receiver stays reachable (`this@Outer`).
                let name = if writer.receivers.is_empty() {
                    "$this".to_owned()
                } else {
                    writer.temp()
                };
                let class = receiver.name.rsplit('.').next().unwrap_or(&receiver.name);
                params.push(name.clone());
                writer.push_receiver(&name, [Some(fun.name.as_str()), Some(class)]);
            }
            for param in &fun.params {
                let name = writer.declare(&param.name);
                if let Some(default) = &param.default {
                    defaults.push(format!(
                        "if ({name} === undefined) {name} = {};",
                        writer.expr(default)?
                    ));
                }
                params.push(if param.vararg {
                    format!("...{name}")
                } else {
                    name
                });
            }
            let code = match body {
                Body::Expr(expr) => format!("return {};", writer.expr(expr)?),
                Body::Block(block) => writer.block(block, Tail::Discard)?,
            };
            let code = if defaults.is_empty() {
                code
            } else {
                format!("{} {code}", defaults.join(" "))
            };
            let code = if writer.catches_return {
                format!("try {{ {code} }} catch (e) {{ if (e instanceof KT.Return) return e.value; throw e; }}")
            } else {
                code
            };
            Ok(format!("async ({}) => {{ {code} }}", params.join(", ")))
        });
        let (depth, catches_return, labels, receivers) = saved;
        self.depth = depth;
        self.catches_return = catches_return;
        self.labels = labels;
        self.receivers.truncate(receivers);
        self.receiver_labels.truncate(receivers);
        result
    }

    // ---- Statements ----

    pub fn block(&mut self, block: &Block, tail: Tail) -> Result<String, String> {
        self.scoped(|writer| {
            let mut out = Vec::new();
            for (index, statement) in block.iter().enumerate() {
                let last = index + 1 == block.len();
                out.push(writer.statement(statement, last && tail == Tail::Return)?);
            }
            if tail == Tail::Return && block.is_empty() {
                out.push("return KT.unit;".to_owned());
            }
            Ok(out.join(" "))
        })
    }

    fn statement(&mut self, statement: &Stmt, returned: bool) -> Result<String, String> {
        Ok(match statement {
            Stmt::Val {
                names,
                destructured,
                mutable,
                value,
                delegate,
            } => {
                let keyword = if *mutable { "let" } else { "const" };
                let value = match (value, delegate) {
                    (Some(value), _) => self.expr(value)?,
                    (
                        None,
                        Some(Expr::Call {
                            callee,
                            lambda: Some(lambda),
                            ..
                        }),
                    ) if matches!(callee.as_ref(), Expr::Name(name) if name == "lazy") => {
                        self.lambda_call(lambda, "")?
                    }
                    (None, Some(_)) => return Err(unsupported("a delegated local")),
                    (None, None) => "undefined".to_owned(),
                };
                // Declared after the value is worked out: an initializer reads what the name
                // meant before.
                let code = if *destructured {
                    let temp = self.temp();
                    let mut parts = Vec::new();
                    for (index, name) in names.iter().enumerate() {
                        if name != "_" {
                            let js = self.declare(name);
                            parts.push(format!("{js} = KT.component({temp}, {index})"));
                        }
                    }
                    if parts.is_empty() {
                        format!("const {temp} = {value};")
                    } else {
                        format!("const {temp} = {value}; {keyword} {};", parts.join(", "))
                    }
                } else {
                    let js = self.declare(&names[0]);
                    format!("{keyword} {js} = {value};")
                };
                if returned {
                    format!("{code} return KT.unit;")
                } else {
                    code
                }
            }
            Stmt::Assign { target, op, value } => {
                let value = self.expr(value)?;
                let combined = |current: String| -> String {
                    match *op {
                        "=" => value.clone(),
                        "+=" => format!("KT.plusAssign({current}, {value})"),
                        "-=" => format!("KT.minusAssign({current}, {value})"),
                        "*=" => format!("({current} * {value})"),
                        "/=" => format!("KT.div({current}, {value})"),
                        _ => format!("({current} % {value})"),
                    }
                };
                let code = match target {
                    Expr::Name(name) if self.is_local(name) => {
                        let name = self.resolve(name).unwrap_or_else(|| local(name));
                        format!("{name} = {};", combined(name.clone()))
                    }
                    Expr::Name(name) => {
                        let receivers = self.receiver_list();
                        let current = format!("(await KT.rget({receivers}, {}))", quote(name));
                        format!(
                            "await KT.rset({receivers}, {}, {});",
                            quote(name),
                            combined(current)
                        )
                    }
                    Expr::Member { target, name, .. } => {
                        let object = self.expr(target)?;
                        let temp = self.temp();
                        let current = format!("(await KT.p({temp}, {}))", quote(name));
                        format!(
                            "{{ const {temp} = {object}; await KT.setp({temp}, {}, {}); }}",
                            quote(name),
                            combined(current)
                        )
                    }
                    Expr::Index { target, index } if index.len() == 1 => {
                        let object = self.expr(target)?;
                        let key = self.expr(&index[0])?;
                        let (temp, key_temp) = (self.temp(), self.temp());
                        let current = format!("(await KT.m({temp}, \"get\", [{key_temp}]))");
                        format!(
                            "{{ const {temp} = {object}; const {key_temp} = {key}; await KT.m({temp}, \"set\", [{key_temp}, {}]); }}",
                            combined(current)
                        )
                    }
                    _ => return Err(unsupported("an assignment to that")),
                };
                if returned {
                    format!("{code} return KT.unit;")
                } else {
                    code
                }
            }
            Stmt::Expr(expr) => self.expression_statement(expr, returned)?,
            Stmt::For {
                label,
                vars,
                iterable,
                body,
            } => {
                let iterable = self.expr(iterable)?;
                let code = self.scoped(|writer| -> Result<String, String> {
                    let head = match vars {
                        LambdaParam::Name(name) => {
                            let js = writer.declare(name);
                            format!("const {js} of KT.iter({iterable})")
                        }
                        LambdaParam::Destructured(names) => {
                            let temp = writer.temp();
                            let mut parts = Vec::new();
                            for (index, name) in names.iter().enumerate() {
                                if name != "_" {
                                    let js = writer.declare(name);
                                    parts.push(format!(
                                        "const {js} = KT.component({temp}, {index});"
                                    ));
                                }
                            }
                            return Ok(format!(
                                "{}for (const {temp} of KT.iter({iterable})) {{ {} {} }}",
                                label
                                    .as_ref()
                                    .map(|label| format!("{}: ", local(label)))
                                    .unwrap_or_default(),
                                parts.join(" "),
                                writer.block(body, Tail::Discard)?
                            ));
                        }
                    };
                    Ok(format!(
                        "{}for ({head}) {{ {} }}",
                        label
                            .as_ref()
                            .map(|label| format!("{}: ", local(label)))
                            .unwrap_or_default(),
                        writer.block(body, Tail::Discard)?
                    ))
                })?;
                if returned {
                    format!("{code} return KT.unit;")
                } else {
                    code
                }
            }
            Stmt::While {
                label,
                condition,
                body,
                after,
            } => {
                let condition = self.expr(condition)?;
                let body = self.block(body, Tail::Discard)?;
                let label = label
                    .as_ref()
                    .map(|label| format!("{}: ", local(label)))
                    .unwrap_or_default();
                let code = if *after {
                    format!("{label}do {{ {body} }} while (KT.truthy({condition}));")
                } else {
                    format!("{label}while (KT.truthy({condition})) {{ {body} }}")
                };
                if returned {
                    format!("{code} return KT.unit;")
                } else {
                    code
                }
            }
            Stmt::Decl(member) => match member.as_ref() {
                Member::Fun(fun) => {
                    let js = self.declare(&fun.name);
                    if fun.receiver.is_some() {
                        self.local_extensions.insert(js.clone());
                    }
                    let function = self.function(fun)?;
                    format!("const {js} = {function};")
                }
                _ => return Err(unsupported("a local class")),
            },
        })
    }

    /// An expression as a statement: control flow stays statements, the rest is evaluated.
    fn expression_statement(&mut self, expr: &Expr, returned: bool) -> Result<String, String> {
        match expr {
            Expr::Return { label, value } => {
                self.return_statement(label.as_deref(), value.as_deref())
            }
            Expr::Throw(value) => Ok(format!("throw KT.error({});", self.expr(value)?)),
            Expr::Break(label) => Ok(match label {
                Some(label) => format!("break {};", local(label)),
                None => "break;".to_owned(),
            }),
            Expr::Continue(label) => Ok(match label {
                Some(label) => format!("continue {};", local(label)),
                None => "continue;".to_owned(),
            }),
            Expr::If {
                condition,
                then,
                otherwise,
            } if !returned => {
                let condition = self.expr(condition)?;
                let then = self.block(then, Tail::Discard)?;
                Ok(match otherwise {
                    Some(otherwise) => format!(
                        "if (KT.truthy({condition})) {{ {then} }} else {{ {} }}",
                        self.block(otherwise, Tail::Discard)?
                    ),
                    None => format!("if (KT.truthy({condition})) {{ {then} }}"),
                })
            }
            Expr::When {
                subject,
                binding,
                branches,
            } if !returned => self.when_statement(
                subject.as_deref(),
                binding.as_deref(),
                branches,
                Tail::Discard,
            ),
            Expr::Try {
                body,
                catches,
                finally,
            } if !returned => self.try_statement(body, catches, finally.as_ref(), Tail::Discard),
            _ => {
                let value = self.expr(expr)?;
                Ok(if returned {
                    format!("return {value};")
                } else {
                    format!("{value};")
                })
            }
        }
    }

    fn return_statement(
        &mut self,
        label: Option<&str>,
        value: Option<&Expr>,
    ) -> Result<String, String> {
        let value = match value {
            Some(value) => self.expr(value)?,
            None => "KT.unit".to_owned(),
        };
        match label {
            // From the lambda being made: its own `return`.
            Some(label) if self.labels.last().map(String::as_str) == Some(label) => {
                Ok(format!("return {value};"))
            }
            Some(_) => Err(unsupported("a return from an outer lambda")),
            None if self.depth == 0 => Ok(format!("return {value};")),
            None => {
                self.catches_return = true;
                Ok(format!("throw new KT.Return({value});"))
            }
        }
    }

    fn when_statement(
        &mut self,
        subject: Option<&Expr>,
        binding: Option<&str>,
        branches: &[WhenBranch],
        tail: Tail,
    ) -> Result<String, String> {
        let subject_name = self.temp();
        let mut code = String::new();
        if let Some(subject) = subject {
            code.push_str(&format!("const {subject_name} = {};", self.expr(subject)?));
        }
        self.scoped(|writer| -> Result<(), String> {
            if let Some(binding) = binding {
                let js = writer.declare(binding);
                code.push_str(&format!(" const {js} = {subject_name};"));
            }
            let mut first = true;
            let mut has_else = false;
            for branch in branches {
                let test = if branch.conditions.is_empty() {
                    has_else = true;
                    None
                } else {
                    let mut tests = Vec::new();
                    for condition in &branch.conditions {
                        tests.push(
                            writer.condition(condition, subject.map(|_| subject_name.as_str()))?,
                        );
                    }
                    let mut test = tests.join(" || ");
                    if let Some(guard) = &branch.guard {
                        test = format!("({test}) && KT.truthy({})", writer.expr(guard)?);
                    }
                    Some(test)
                };
                let body = writer.block(&branch.body, tail)?;
                match test {
                    Some(test) => {
                        code.push_str(&format!(
                            "{}if ({test}) {{ {body} }}",
                            if first { " " } else { " else " }
                        ));
                    }
                    None if first => code.push_str(&format!(" {{ {body} }}")),
                    None => code.push_str(&format!(" else {{ {body} }}")),
                }
                first = false;
            }
            if tail == Tail::Return && !has_else {
                code.push_str(" return KT.unit;");
            }
            Ok(())
        })?;
        Ok(format!("{{ {code} }}"))
    }

    fn condition(
        &mut self,
        condition: &WhenCondition,
        subject: Option<&str>,
    ) -> Result<String, String> {
        Ok(match (condition, subject) {
            (WhenCondition::Expr(value), Some(subject)) => {
                format!("KT.eq({subject}, {})", self.expr(value)?)
            }
            (WhenCondition::Expr(value), None) => format!("KT.truthy({})", self.expr(value)?),
            (WhenCondition::Is { ty, negated }, Some(subject)) => {
                let test = format!("KT.is({subject}, {})", quote(&ty.name));
                if *negated { format!("!{test}") } else { test }
            }
            (WhenCondition::In { range, negated }, Some(subject)) => {
                let test = format!("KT.contains({}, {subject})", self.expr(range)?);
                if *negated { format!("!{test}") } else { test }
            }
            _ => return Err(unsupported("that when")),
        })
    }

    fn try_statement(
        &mut self,
        body: &Block,
        catches: &[Catch],
        finally: Option<&Block>,
        tail: Tail,
    ) -> Result<String, String> {
        let body = self.block(body, tail)?;
        let mut code = format!("try {{ {body} }}");
        if !catches.is_empty() {
            let error = self.temp();
            let mut handlers = String::new();
            for (index, catch) in catches.iter().enumerate() {
                let handler = self.scoped(|writer| -> Result<String, String> {
                    let js = writer.declare(&catch.name);
                    Ok(format!(
                        "const {js} = {error}; {}",
                        writer.block(&catch.body, tail)?
                    ))
                })?;
                // A Kotlin return passing through isn't a failure to catch.
                let test = format!(
                    "!(({error}) instanceof KT.Return) && KT.caught({error}, {})",
                    quote(&catch.ty.name)
                );
                handlers.push_str(&format!(
                    "{}if ({test}) {{ {handler} }}",
                    if index == 0 { "" } else { " else " }
                ));
            }
            code.push_str(&format!(
                " catch ({error}) {{ {handlers} else {{ throw {error}; }} }}"
            ));
        }
        if let Some(finally) = finally {
            code.push_str(&format!(
                " finally {{ {} }}",
                self.block(finally, Tail::Discard)?
            ));
        }
        Ok(code)
    }

    // ---- Expressions ----

    /// An async function of no arguments around a block, called at once: a block as a value.
    fn iife(
        &mut self,
        make: impl FnOnce(&mut Self) -> Result<String, String>,
    ) -> Result<String, String> {
        self.depth += 1;
        let body = make(self);
        self.depth -= 1;
        Ok(format!("(await (async () => {{ {} }})())", body?))
    }

    pub fn expr(&mut self, expr: &Expr) -> Result<String, String> {
        Ok(match expr {
            Expr::Int(value) => value.to_string(),
            Expr::Float(value) => {
                if value.is_finite() {
                    format!("{value:?}")
                } else {
                    return Err(unsupported("that number"));
                }
            }
            Expr::Bool(value) => value.to_string(),
            Expr::Null => "null".to_owned(),
            Expr::Char(value) => quote(&value.to_string()),
            Expr::Str(parts) => {
                let mut pieces = Vec::new();
                for part in parts {
                    match part {
                        StrPart::Text(text) => pieces.push(quote(text)),
                        StrPart::Code(code) => pieces.push(format!("KT.str({})", self.expr(code)?)),
                    }
                }
                match pieces.len() {
                    0 => "\"\"".to_owned(),
                    1 => pieces.remove(0),
                    _ => format!("({})", pieces.join(" + ")),
                }
            }
            Expr::Name(name) => self.name(name)?,
            Expr::This(label) => match (label, self.receivers.last()) {
                (None, Some(receiver)) => receiver.clone(),
                (Some(label), _) => self.labelled_this(label),
                (None, None) => "self".to_owned(),
            },
            Expr::Super => "self.base".to_owned(),
            Expr::Member { target, name, safe } => self.member(target, name, *safe)?,
            Expr::Call {
                callee,
                type_args,
                args,
                lambda,
            } => self.call(callee, type_args, args, lambda.as_deref())?,
            Expr::Index { target, index } => {
                let object = self.expr(target)?;
                let keys = index
                    .iter()
                    .map(|key| self.expr(key))
                    .collect::<Result<Vec<_>, _>>()?;
                format!("(await KT.m({object}, \"get\", [{}]))", keys.join(", "))
            }
            Expr::NotNull(inner) => format!("KT.nn({})", self.expr(inner)?),
            Expr::Unary { op, expr } => {
                let value = self.expr(expr)?;
                match *op {
                    "!" => format!("(!KT.truthy({value}))"),
                    "-" => format!("(-{value})"),
                    "+" => value,
                    _ => return Err(unsupported("++ and -- inside expressions")),
                }
            }
            Expr::Postfix { op, expr } => {
                // `i++` as a value: the value before, then the variable moves on.
                let Expr::Name(name) = expr.as_ref() else {
                    return Err(unsupported("++ on that"));
                };
                let Some(name) = self.resolve(name) else {
                    return Err(unsupported("++ on a property"));
                };
                format!("({name}{op})")
            }
            Expr::Binary { op, left, right } => self.binary(op, left, right)?,
            Expr::Is { expr, ty, negated } => {
                let test = format!("KT.is({}, {})", self.expr(expr)?, quote(&ty.name));
                if *negated { format!("(!{test})") } else { test }
            }
            Expr::As { expr, ty, safe } => {
                let value = self.expr(expr)?;
                if *safe {
                    format!("KT.asOrNull({value}, {})", quote(&ty.name))
                } else {
                    value
                }
            }
            Expr::If {
                condition,
                then,
                otherwise,
            } => {
                let simple = |block: &Block| matches!(block.as_slice(), [Stmt::Expr(expr)] if is_plain(expr));
                match otherwise {
                    Some(otherwise) if simple(then) && simple(otherwise) => {
                        let (Stmt::Expr(then), Stmt::Expr(otherwise)) = (&then[0], &otherwise[0])
                        else {
                            unreachable!()
                        };
                        format!(
                            "(KT.truthy({}) ? {} : {})",
                            self.expr(condition)?,
                            self.expr(then)?,
                            self.expr(otherwise)?
                        )
                    }
                    _ => {
                        let condition = self.expr(condition)?;
                        self.iife(|writer| {
                            let then = writer.block(then, Tail::Return)?;
                            let otherwise = match otherwise {
                                Some(otherwise) => writer.block(otherwise, Tail::Return)?,
                                None => "return KT.unit;".to_owned(),
                            };
                            Ok(format!(
                                "if (KT.truthy({condition})) {{ {then} }} else {{ {otherwise} }}"
                            ))
                        })?
                    }
                }
            }
            Expr::When {
                subject,
                binding,
                branches,
            } => {
                let (subject, binding) = (subject.clone(), binding.clone());
                self.iife(|writer| {
                    writer.when_statement(
                        subject.as_deref(),
                        binding.as_deref(),
                        branches,
                        Tail::Return,
                    )
                })?
            }
            Expr::Try {
                body,
                catches,
                finally,
            } => self.iife(|writer| {
                writer.try_statement(body, catches, finally.as_ref(), Tail::Return)
            })?,
            Expr::Lambda(lambda) => self.lambda(lambda, "")?,
            Expr::Reference { target, name } => self.reference(target.as_deref(), name)?,
            Expr::Throw(value) => format!("KT.raise({})", self.expr(value)?),
            Expr::Return { label, value } => {
                let value = match value {
                    Some(value) => self.expr(value)?,
                    None => "KT.unit".to_owned(),
                };
                match label {
                    Some(label)
                        if self.labels.last().map(String::as_str) == Some(label.as_str()) =>
                    {
                        // A labelled return from the lambda in an expression position.
                        format!("KT.ret({value}, {})", quote(label))
                    }
                    Some(_) => return Err(unsupported("a return from an outer lambda")),
                    None => {
                        self.catches_return = true;
                        format!("KT.ret({value})")
                    }
                }
            }
            Expr::Break(_) | Expr::Continue(_) => {
                return Err(unsupported("break or continue inside an expression"));
            }
            Expr::Object(_) => return Err(unsupported("an anonymous object")),
        })
    }

    fn name(&mut self, name: &str) -> Result<String, String> {
        if let Some(js) = self.resolve(name) {
            return Ok(js);
        }
        if name == "it" {
            return Err(unsupported("`it` outside a lambda"));
        }
        if let Some(value) = self.companion_value(name) {
            return Ok(value);
        }
        // Implicit receivers first, as Kotlin resolves them, then the source.
        if !self.receivers.is_empty() {
            return Ok(format!(
                "(await KT.rget({}, {}))",
                self.receiver_list(),
                quote(name)
            ));
        }
        if self.cx.properties.contains(name) {
            return Ok(format!("(await self.get({}))", quote(name)));
        }
        if self.cx.constants.contains(name) {
            return Ok(format!("(await C.get({}))", quote(name)));
        }
        if self.cx.classes.contains(name) {
            return Ok(format!("D.{name}"));
        }
        if self.cx.functions.contains_key(name) {
            // A function read as a value: a reference to it.
            return Ok(format!(
                "(async (...args) => self.call({}, args))",
                quote(name)
            ));
        }
        // Anything else is the runtime's, or nothing it could find when the code runs.
        if OBJECTS.contains(&name) || GLOBALS.contains(&name) {
            return Ok(format!("KT.global({})", quote(name)));
        }
        Err(unsupported(name))
    }

    /// An object the runtime has, as the owner of a member (`Calendar.getInstance()`).
    fn runtime_object(owner: &str, name: &str) -> Result<String, String> {
        if OBJECTS.contains(&owner) {
            Ok(format!("KT.global({})", quote(owner)))
        } else {
            Err(unsupported(format!("{owner}.{name}")))
        }
    }

    fn member(&mut self, target: &Expr, name: &str, safe: bool) -> Result<String, String> {
        if let Expr::Super = target {
            return Ok(format!("(await self.base.get({}))", quote(name)));
        }
        // A companion's or an object's member (`Companion.X`, `SManga.ONGOING`).
        if let Expr::Name(owner) = target
            && !self.is_local(owner)
            && owner.chars().next().is_some_and(char::is_uppercase)
            && self.receivers.is_empty()
            && !self.cx.properties.contains(owner)
            && !self.cx.constants.contains(owner)
        {
            if self
                .cx
                .companions
                .get(owner)
                .is_some_and(|names| names.contains(name))
            {
                return Ok(format!(
                    "(await C.get({}))",
                    quote(&companion_key(owner, name))
                ));
            }
            if self.cx.classes.contains(owner) {
                return Ok(format!("(await KT.p(D.{owner}, {}))", quote(name)));
            }
            let object = Self::runtime_object(owner, name)?;
            return Ok(format!("(await KT.p({object}, {}))", quote(name)));
        }
        let object = self.expr(target)?;
        if self.cx.extension_properties.contains(name) {
            let call = format!("(await self.ext({}, {object}, []))", quote(name));
            return Ok(if safe {
                format!(
                    "(await KT.safe({object}, async (v) => self.ext({}, v, [])))",
                    quote(name)
                )
            } else {
                call
            });
        }
        Ok(if safe {
            format!("(await KT.ps({object}, {}))", quote(name))
        } else {
            format!("(await KT.p({object}, {}))", quote(name))
        })
    }

    /// Arguments in order, named ones placed by `params` (a function of the source's); for others,
    /// named arguments go in an object after them.
    fn args(
        &mut self,
        args: &[Arg],
        params: Option<&[String]>,
    ) -> Result<(Vec<String>, Option<String>), String> {
        let mut positional = Vec::new();
        let mut named = Vec::new();
        for arg in args {
            let value = self.expr(&arg.value)?;
            let value = if arg.spread {
                format!("...KT.iter({value})")
            } else {
                value
            };
            match &arg.name {
                None => positional.push(value),
                Some(name) => named.push((name.clone(), value)),
            }
        }
        if named.is_empty() {
            return Ok((positional, None));
        }
        if let Some(params) = params {
            let mut placed: Vec<Option<String>> = positional.into_iter().map(Some).collect();
            for (name, value) in named {
                let index = params
                    .iter()
                    .position(|param| *param == name)
                    .ok_or_else(|| unsupported(format!("a named argument {name}")))?;
                if placed.len() <= index {
                    placed.resize(index + 1, None);
                }
                placed[index] = Some(value);
            }
            return Ok((
                placed
                    .into_iter()
                    .map(|value| value.unwrap_or_else(|| "undefined".to_owned()))
                    .collect(),
                None,
            ));
        }
        let object = named
            .iter()
            .map(|(name, value)| format!("{}: {value}", quote(name)))
            .collect::<Vec<_>>()
            .join(", ");
        Ok((positional, Some(format!("{{{object}}}"))))
    }

    /// The overload of one of the source's functions that takes these arguments.
    fn own_params(&self, name: &str, args: &[Arg]) -> Option<Vec<String>> {
        let overloads = self.cx.functions.get(name)?;
        let named: Vec<&str> = args.iter().filter_map(|arg| arg.name.as_deref()).collect();
        overloads
            .iter()
            .find(|params| {
                params.len() >= args.len()
                    && named
                        .iter()
                        .all(|name| params.iter().any(|param| param == name))
            })
            .or_else(|| overloads.first())
            .cloned()
    }

    fn call(
        &mut self,
        callee: &Expr,
        type_args: &[Type],
        args: &[Arg],
        lambda: Option<&Lambda>,
    ) -> Result<String, String> {
        let type_arg = type_args
            .first()
            .map(|ty| quote(&type_name(ty)))
            .unwrap_or_else(|| "null".to_owned());
        match callee {
            Expr::Name(name) => {
                // Scope functions without a receiver.
                match (name.as_str(), lambda) {
                    // `apply { … }` alone, in an extension or a scope: on `this`.
                    ("apply" | "also" | "let" | "takeIf" | "takeUnless", Some(_))
                        if args.is_empty() && !self.receivers.is_empty() =>
                    {
                        let on_this = Expr::Member {
                            target: Box::new(Expr::This(None)),
                            name: name.clone(),
                            safe: false,
                        };
                        return self.call(&on_this, type_args, args, lambda);
                    }
                    ("run", Some(lambda)) if args.is_empty() => {
                        return self.lambda_call(lambda, "");
                    }
                    ("with", Some(lambda)) if args.len() == 1 => {
                        let value = self.expr(&args[0].value)?;
                        return self.receiver_call(lambda, &value, "with");
                    }
                    ("buildList" | "buildSet" | "mutableListOf", Some(lambda))
                        if name != "mutableListOf" =>
                    {
                        return self.receiver_call_returning(lambda, "[]", name);
                    }
                    ("buildString", Some(lambda)) => {
                        let built =
                            self.receiver_call_returning(lambda, "KT.stringBuilder()", name)?;
                        return Ok(format!("KT.str({built})"));
                    }
                    ("buildJsonObject", Some(lambda)) => {
                        return self
                            .receiver_call_returning(lambda, "KT.jsonBuilder()", name)
                            .map(|built| format!("KT.built({built})"));
                    }
                    ("buildJsonArray", Some(lambda)) => {
                        return self
                            .receiver_call_returning(lambda, "KT.jsonArrayBuilder()", name)
                            .map(|built| format!("KT.built({built})"));
                    }
                    ("runCatching", Some(lambda)) => {
                        let call = self.lambda_call(lambda, name)?;
                        return Ok(format!("(await KT.runCatching(async () => {call}))"));
                    }
                    ("repeat", Some(lambda)) if args.len() == 1 => {
                        let times = self.expr(&args[0].value)?;
                        let function = self.lambda(lambda, name)?;
                        return Ok(format!("(await KT.repeat({times}, {function}))"));
                    }
                    (
                        "coroutineScope" | "supervisorScope" | "withContext" | "runBlocking",
                        Some(lambda),
                    ) => {
                        return self.lambda_call(lambda, name);
                    }
                    ("async" | "lazy", Some(lambda)) => {
                        let call = self.lambda_call(lambda, name)?;
                        return Ok(format!("KT.deferred(async () => {call})"));
                    }
                    _ => {}
                }
                if let Some(js) = self.resolve(name) {
                    let (args, _) = self.args(args, None)?;
                    let mut args = args;
                    if let Some(lambda) = lambda {
                        args.push(self.lambda(lambda, name)?);
                    }
                    // A local extension called on the implicit receiver.
                    if self.local_extensions.contains(&js) {
                        let receiver = self
                            .receivers
                            .last()
                            .cloned()
                            .unwrap_or_else(|| "self".to_owned());
                        args.insert(0, receiver);
                    }
                    return Ok(format!("(await {js}({}))", args.join(", ")));
                }
                // An implicit receiver's member (`setUrlWithoutDomain(…)` in `apply`).
                let lambda_code = lambda.map(|lambda| self.lambda(lambda, name)).transpose()?;
                if !self.receivers.is_empty() && !self.cx.functions.contains_key(name) {
                    if !self.known(name) && !self.cx.extensions.contains(name) {
                        return self.global_call(name, args, lambda_code, &type_arg);
                    }
                    let (args, named) = self.args(args, None)?;
                    return Ok(format!(
                        "(await KT.rcall({}, {}, [{}], {}, {}, {type_arg}))",
                        self.receiver_list(),
                        quote(name),
                        args.join(", "),
                        named.unwrap_or_else(|| "null".to_owned()),
                        lambda_code.unwrap_or_else(|| "null".to_owned())
                    ));
                }
                if let Some(params) = self.own_params(name, args) {
                    let (mut args, _) = self.args(args, Some(&params))?;
                    if let Some(lambda) = lambda_code {
                        args.push(lambda);
                    }
                    return Ok(format!(
                        "(await self.call({}, [{}]))",
                        quote(name),
                        args.join(", ")
                    ));
                }
                if self.cx.classes.contains(name) {
                    let (args, named) = self.args(args, None)?;
                    return Ok(format!(
                        "(await KT.make(D.{name}, [{}], {}))",
                        args.join(", "),
                        named.unwrap_or_else(|| "null".to_owned())
                    ));
                }
                self.global_call(name, args, lambda_code, &type_arg)
            }
            Expr::Member { target, name, safe } => {
                // Scope functions with a receiver.
                if let Some(lambda) = lambda
                    && args.is_empty()
                    && matches!(
                        name.as_str(),
                        "apply" | "also" | "let" | "run" | "takeIf" | "takeUnless" | "use"
                    )
                {
                    let value = self.expr(target)?;
                    let call = |writer: &mut Self, value: &str| -> Result<String, String> {
                        match name.as_str() {
                            "apply" => writer.receiver_call_returning(lambda, value, name),
                            "run" => writer.receiver_call(lambda, value, name),
                            "also" => {
                                let call = writer.it_call(lambda, value, name)?;
                                Ok(format!("(await KT.also({value}, async () => {call}))"))
                            }
                            "takeIf" => {
                                let call = writer.it_call(lambda, value, name)?;
                                Ok(format!("(KT.truthy({call}) ? {value} : null)"))
                            }
                            "takeUnless" => {
                                let call = writer.it_call(lambda, value, name)?;
                                Ok(format!("(KT.truthy({call}) ? null : {value})"))
                            }
                            _ => writer.it_call(lambda, value, name),
                        }
                    };
                    let temp = self.temp();
                    let inner = call(self, &temp)?;
                    return Ok(if *safe {
                        format!("(await KT.safe({value}, async ({temp}) => {inner}))")
                    } else {
                        format!("(await (async ({temp}) => {inner})({value}))")
                    });
                }
                if let Expr::Super = target.as_ref() {
                    let (args, _) = self.args(args, None)?;
                    return Ok(format!(
                        "(await self.base.call({}, [{}]))",
                        quote(name),
                        args.join(", ")
                    ));
                }
                // `Foo.create()`, `Jsoup.parse(…)`: an object's member.
                if let Expr::Name(owner) = target.as_ref()
                    && !self.is_local(owner)
                    && owner.chars().next().is_some_and(char::is_uppercase)
                    && !self.cx.properties.contains(owner)
                    && !self.cx.constants.contains(owner)
                {
                    let owner_code = if self.cx.classes.contains(owner) {
                        format!("D.{owner}")
                    } else {
                        Self::runtime_object(owner, name)?
                    };
                    let (args, named) = self.args(args, None)?;
                    let lambda = lambda.map(|lambda| self.lambda(lambda, name)).transpose()?;
                    return Ok(format!(
                        "(await KT.m({owner_code}, {}, [{}], {}, {}, {type_arg}))",
                        quote(name),
                        args.join(", "),
                        named.unwrap_or_else(|| "null".to_owned()),
                        lambda.unwrap_or_else(|| "null".to_owned())
                    ));
                }
                let object = self.expr(target)?;
                // `builder.addParam(…)`: a local extension, the value passed first.
                if let Some(js) = self
                    .resolve(name)
                    .filter(|js| self.local_extensions.contains(js))
                {
                    let (mut args, _) = self.args(args, None)?;
                    if let Some(lambda) = lambda {
                        args.push(self.lambda(lambda, name)?);
                    }
                    let temp = self.temp();
                    args.insert(0, temp.clone());
                    let call = format!("{js}({})", args.join(", "));
                    return Ok(if *safe {
                        format!("(await KT.safe({object}, async ({temp}) => {call}))")
                    } else {
                        format!("(await (async ({temp}) => {call})({object}))")
                    });
                }
                let own_extension = self.cx.extensions.contains(name);
                if !own_extension && !self.known(name) {
                    return Err(unsupported(format!("{name}()")));
                }
                let (args, named) = self.args(args, None)?;
                let lambda = lambda.map(|lambda| self.lambda(lambda, name)).transpose()?;
                let mut call_args = args;
                let lambda_arg = lambda.unwrap_or_else(|| "null".to_owned());
                let call = |receiver: &str| {
                    if own_extension {
                        let mut all = call_args.clone();
                        if lambda_arg != "null" {
                            all.push(lambda_arg.clone());
                        }
                        format!(
                            "self.ext({}, {receiver}, [{}])",
                            quote(name),
                            all.join(", ")
                        )
                    } else {
                        format!(
                            "KT.m({receiver}, {}, [{}], {}, {lambda_arg}, {type_arg})",
                            quote(name),
                            call_args.join(", "),
                            named.clone().unwrap_or_else(|| "null".to_owned())
                        )
                    }
                };
                let code = if *safe {
                    let inner = call("v");
                    format!("(await KT.safe({object}, async (v) => {inner}))")
                } else {
                    format!("(await {})", call(&object))
                };
                call_args.clear();
                Ok(code)
            }
            // `{ … }()`, `f(x)(y)`.
            _ => {
                let function = self.expr(callee)?;
                let (args, _) = self.args(args, None)?;
                Ok(format!(
                    "(await KT.invoke({function}, [{}]))",
                    args.join(", ")
                ))
            }
        }
    }

    fn global_call(
        &mut self,
        name: &str,
        args: &[Arg],
        lambda: Option<String>,
        type_arg: &str,
    ) -> Result<String, String> {
        if !GLOBALS.contains(&name) {
            return Err(unsupported(format!("{name}()")));
        }
        let (args, named) = self.args(args, None)?;
        Ok(format!(
            "(await KT.call({}, [{}], {}, {}, {type_arg}))",
            quote(name),
            args.join(", "),
            named.unwrap_or_else(|| "null".to_owned()),
            lambda.unwrap_or_else(|| "null".to_owned())
        ))
    }

    fn reference(&mut self, target: Option<&Expr>, name: &str) -> Result<String, String> {
        match target {
            // `::helper`: one of the source's functions.
            None if self.cx.functions.contains_key(name) => Ok(format!(
                "(async (...args) => self.call({}, args))",
                quote(name)
            )),
            None if self.cx.extensions.contains(name) => Ok(format!(
                "(async (receiver, ...args) => self.ext({}, receiver, args))",
                quote(name)
            )),
            // `::append` in `buildString { … }`: a member of an implicit receiver.
            None if !self.receivers.is_empty() && self.known(name) => Ok(format!(
                "(async (...args) => KT.rcall({}, {}, args, null, null, null))",
                self.receiver_list(),
                quote(name)
            )),
            None => Err(unsupported(format!("::{name}"))),
            // `String::isNotBlank`, `Element::text`: a member of whatever is passed.
            Some(Expr::Name(owner)) if owner.chars().next().is_some_and(char::is_uppercase) => {
                if name == "class" {
                    return Ok(quote(owner));
                }
                if self.cx.extensions.contains(name) {
                    return Ok(format!(
                        "(async (receiver, ...args) => self.ext({}, receiver, args))",
                        quote(name)
                    ));
                }
                if !self.known(name) {
                    return Err(unsupported(format!("{owner}::{name}")));
                }
                Ok(format!(
                    "(async (receiver, ...args) => KT.m(receiver, {}, args))",
                    quote(name)
                ))
            }
            // `list::add`: bound to a value.
            Some(target) => {
                let object = self.expr(target)?;
                if !self.known(name) {
                    return Err(unsupported(format!("::{name}")));
                }
                let temp = self.temp();
                Ok(format!(
                    "(({temp}) => async (...args) => KT.m({temp}, {}, args))({object})",
                    quote(name)
                ))
            }
        }
    }

    fn binary(&mut self, op: &str, left: &Expr, right: &Expr) -> Result<String, String> {
        // `x ?: return`, `x ?: throw …`, `x ?: continue`.
        if op == "?:" {
            let left = self.expr(left)?;
            let right = self.expr(right)?;
            return Ok(format!("({left} ?? {right})"));
        }
        let (left, right) = (self.expr(left)?, self.expr(right)?);
        Ok(match op {
            "+" => format!("KT.plus({left}, {right})"),
            "-" => format!("KT.minus({left}, {right})"),
            "*" => format!("({left} * {right})"),
            "/" => format!("KT.div({left}, {right})"),
            "%" => format!("({left} % {right})"),
            "==" => format!("KT.eq({left}, {right})"),
            "!=" => format!("(!KT.eq({left}, {right}))"),
            "===" => format!("({left} === {right})"),
            "!==" => format!("({left} !== {right})"),
            "<" | ">" | "<=" | ">=" => format!("(KT.cmp({left}, {right}) {op} 0)"),
            "&&" => format!("(KT.truthy({left}) && KT.truthy({right}))"),
            "||" => format!("(KT.truthy({left}) || KT.truthy({right}))"),
            "in" => format!("KT.contains({right}, {left})"),
            "!in" => format!("(!KT.contains({right}, {left}))"),
            ".." => format!("KT.range({left}, {right})"),
            "..<" | "until" => format!("KT.range({left}, ({right}) - 1)"),
            "downTo" => format!("KT.range({left}, {right}, -1)"),
            "step" => format!("KT.step({left}, {right})"),
            "to" => format!("KT.pair({left}, {right})"),
            "and" => format!("KT.and({left}, {right})"),
            "or" => format!("KT.or({left}, {right})"),
            "xor" => format!("({left} ^ {right})"),
            "shl" => format!("({left} << {right})"),
            "shr" => format!("({left} >> {right})"),
            "ushr" => format!("({left} >>> {right})"),
            infix if self.known(infix) || self.cx.extensions.contains(infix) => {
                if self.cx.extensions.contains(infix) {
                    format!("(await self.ext({}, {left}, [{right}]))", quote(infix))
                } else {
                    format!("(await KT.m({left}, {}, [{right}]))", quote(infix))
                }
            }
            other => return Err(unsupported(format!("the operator {other}"))),
        })
    }

    // ---- Lambdas ----

    /// A lambda as an async function. `label` is the function it's passed to (`map`), which a
    /// `return@map` leaves.
    fn lambda(&mut self, lambda: &Lambda, label: &str) -> Result<String, String> {
        let label = lambda.label.clone().unwrap_or_else(|| label.to_owned());
        self.scoped(|writer| {
            let mut params = Vec::new();
            let mut prologue = Vec::new();
            if lambda.implicit {
                params.push(writer.declare("it"));
            }
            for param in &lambda.params {
                match param {
                    // Unused parameters, each its own name (`{ _, _ -> }`).
                    LambdaParam::Name(name) if name == "_" => params.push(writer.temp()),
                    LambdaParam::Name(name) => params.push(writer.declare(name)),
                    LambdaParam::Destructured(names) => {
                        let temp = writer.temp();
                        params.push(temp.clone());
                        for (index, name) in names.iter().enumerate() {
                            if name != "_" {
                                let js = writer.declare(name);
                                prologue.push(format!("const {js} = KT.component({temp}, {index});"));
                            }
                        }
                    }
                }
            }
            writer.labels.push(label.clone());
            writer.depth += 1;
            let body = writer.block(&lambda.body, Tail::Return);
            writer.depth -= 1;
            writer.labels.pop();
            let body = body?;
            // A labelled return in an expression position comes as `KT.Return` with the label.
            let body = if label.is_empty() {
                body
            } else {
                format!(
                    "try {{ {body} }} catch (e) {{ if (e instanceof KT.Return && e.label === {}) return e.value; throw e; }}",
                    quote(&label)
                )
            };
            Ok(format!(
                "(async ({}) => {{ {} {body} }})",
                params.join(", "),
                prologue.join(" ")
            ))
        })
    }

    /// A lambda called at once with no argument.
    fn lambda_call(&mut self, lambda: &Lambda, label: &str) -> Result<String, String> {
        let function = self.lambda(lambda, label)?;
        Ok(format!("(await {function}())"))
    }

    /// A lambda called with `value` as `it` (or its parameter).
    fn it_call(&mut self, lambda: &Lambda, value: &str, label: &str) -> Result<String, String> {
        let function = self.lambda(lambda, label)?;
        Ok(format!("(await {function}({value}))"))
    }

    /// A lambda with `value` as its receiver (`run`, `with`), giving its own result.
    fn receiver_call(
        &mut self,
        lambda: &Lambda,
        value: &str,
        label: &str,
    ) -> Result<String, String> {
        let receiver = self.temp();
        self.push_receiver(&receiver, [Some(label), lambda.label.as_deref()]);
        let function = self.lambda(lambda, label);
        self.pop_receiver();
        Ok(format!(
            "(await (async ({receiver}) => {}())({value}))",
            function?
        ))
    }

    /// A lambda with `value` as its receiver, giving the receiver (`apply`, `buildList`).
    fn receiver_call_returning(
        &mut self,
        lambda: &Lambda,
        value: &str,
        label: &str,
    ) -> Result<String, String> {
        let receiver = self.temp();
        self.push_receiver(&receiver, [Some(label), lambda.label.as_deref()]);
        let function = self.lambda(lambda, label);
        self.pop_receiver();
        Ok(format!(
            "(await (async ({receiver}) => {{ await {}(); return {receiver}; }})({value}))",
            function?
        ))
    }
}

/// Whether an expression can sit in a ternary as it is (no statements inside).
fn is_plain(expr: &Expr) -> bool {
    !matches!(
        expr,
        Expr::Return { .. } | Expr::Throw(_) | Expr::Break(_) | Expr::Continue(_)
    )
}

fn type_name(ty: &Type) -> String {
    if ty.args.is_empty() {
        ty.name.clone()
    } else {
        format!(
            "{}<{}>",
            ty.name,
            ty.args.iter().map(type_name).collect::<Vec<_>>().join(",")
        )
    }
}

/// Functions and objects the runtime has without a receiver (`mihon/kotlin.js`'s `GLOBALS`).
#[rustfmt::skip]
pub const GLOBALS: &[&str] = &[
    "listOf", "mutableListOf", "arrayListOf", "arrayOf", "listOfNotNull", "emptyList", "emptyArray",
    "setOf", "mutableSetOf", "hashSetOf", "linkedSetOf", "setOfNotNull", "emptySet", "mapOf",
    "mutableMapOf", "hashMapOf", "linkedMapOf", "emptyMap", "Pair", "Triple", "error", "require",
    "requireNotNull", "check", "checkNotNull", "TODO", "println", "print", "maxOf", "minOf",
    "Regex", "Exception", "RuntimeException", "IllegalStateException", "IllegalArgumentException",
    "UnsupportedOperationException", "NoSuchElementException", "IOException", "Error",
    "HttpException", "GET", "POST", "PUT", "DELETE", "MangasPage", "Page", "SMangaUpdate",
    "FilterList", "SimpleDateFormat", "StringBuilder", "Date", "JsonPrimitive", "JsonObject",
    "JsonArray", "Locale", "abs", "round", "floor", "ceil", "getPreferences", "getPreferencesLazy",
    "Selection", "Json", "Mutex", "Semaphore", "awaitAll", "Char", "String",
];

/// Objects the runtime has (`mihon/kotlin.js`'s `OBJECTS`): `Name.member`, or `Name` as a value.
#[rustfmt::skip]
pub const OBJECTS: &[&str] = &[
    "SManga", "SChapter", "Jsoup", "Fetch", "Locale", "TimeZone", "ZoneId", "ZoneOffset",
    "Calendar", "TimeUnit", "CacheControl", "UUID", "Json", "MediaType", "String", "Character",
    "Unit", "LocalDate", "LocalDateTime", "ZonedDateTime", "Parser", "DateTimeFormatter", "System",
    "FormBody", "Headers", "Request", "RegexOption", "Filter", "Math", "Integer", "Long", "Int",
    "URLEncoder", "URLDecoder", "Charsets", "Base64", "JsonNull", "Instant", "Clock",
];
