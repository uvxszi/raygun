//! Extensions built on no theme: their Kotlin, turned into JavaScript for the Mihon runtime.
//!
//! A Keiyoushi source written on its own is a class extending `KeiSource` with `suspend`
//! functions (`getPopularManga`, `getLatestUpdates`, `getSearchMangaList`, `fetchMangaUpdate`,
//! `getPageList`) over Jsoup, OkHttp and Kotlin's standard library. Each function becomes an
//! async JavaScript function (`kotlin::emit`) over the runtime's tables of those libraries
//! (`mihon/kotlin.js`); `mihon.source` then answers Raygun's calls with them. Code the runtime has
//! no table for (WebViews, interceptors, crypto, image decoding) is refused with its name.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::convert::{Converted, sources_json};
use super::index::StoreExtension;
use super::kotlin::ast::{
    Body, ClassDecl, ClassKind, Expr, File, FunDecl, Member, Param, PropDecl,
};
use super::kotlin::emit::{Context, Gen};

/// What `KeiSource` (and `HttpSource` under it) gives a source, read through `self`.
const BASE_PROPERTIES: [&str; 11] = [
    "baseUrl",
    "lang",
    "name",
    "id",
    "client",
    "headers",
    "supportsLatest",
    "json",
    "network",
    "versionId",
    "preferences",
];
const BASE_FUNCTIONS: [(&str, &[&str]); 6] = [
    ("headersBuilder", &[]),
    ("getMangaUrl", &["manga"]),
    ("getChapterUrl", &["chapter"]),
    ("getImageRequest", &["page"]),
    ("imageRequest", &["page"]),
    ("getFilterList", &[]),
];

/// Overrides that change nothing Raygun asks of a source: filters read from the site first,
/// related titles, settings screens, deep links. (`getFilterList` is converted when it can be:
/// Raygun shows its filters; else searches have none.)
const SKIPPED: [&str; 9] = [
    "fetchFilterData",
    "setupPreferenceScreen",
    "fetchRelatedMangaList",
    "getMangaByUrl",
    "getMangasByUrl",
    "supportsRelatedMangas",
    "supportRelatedMangasBySearch",
    "supportsFilterFetching",
    "fetchBaseUrls",
];

/// The calls a source must have.
const NEEDED: [&str; 3] = ["getPopularManga", "fetchMangaUpdate", "getPageList"];

/// Themes whose `ImageInterceptor` reorders a page's tiles, and the recipe Raygun's image fetcher
/// knows for it (`raygun_net::images::Unscramble`).
const IMAGE_RECIPES: [(&str, &str); 2] = [("comiciviewer", "comici"), ("gigaviewer", "giga")];

/// What Raygun calls of a source, and what the runtime calls for it: where reachability starts.
const ENTRIES: [&str; 12] = [
    "getPopularManga",
    "getLatestUpdates",
    "getSearchMangaList",
    "fetchMangaUpdate",
    "getPageList",
    "getImageRequest",
    "getMangaUrl",
    "getChapterUrl",
    "configureHeaders",
    "configureClient",
    "headersBuilder",
    "supportsLatest",
];

/// Names a piece of code mentions (variables, members, calls), for following what reaches what.
fn mentions(body: &Body, found: &mut HashSet<String>) {
    match body {
        Body::Expr(expr) => expr_mentions(expr, found),
        Body::Block(block) => block_mentions(block, found),
    }
}

fn block_mentions(block: &[super::kotlin::ast::Stmt], found: &mut HashSet<String>) {
    use super::kotlin::ast::Stmt;
    for statement in block {
        match statement {
            Stmt::Val {
                value, delegate, ..
            } => {
                for expr in value.iter().chain(delegate.iter()) {
                    expr_mentions(expr, found);
                }
            }
            Stmt::Assign { target, value, .. } => {
                expr_mentions(target, found);
                expr_mentions(value, found);
            }
            Stmt::Expr(expr) => expr_mentions(expr, found),
            Stmt::For { iterable, body, .. } => {
                expr_mentions(iterable, found);
                block_mentions(body, found);
            }
            Stmt::While {
                condition, body, ..
            } => {
                expr_mentions(condition, found);
                block_mentions(body, found);
            }
            Stmt::Decl(member) => {
                if let Member::Fun(fun) = member.as_ref()
                    && let Some(body) = &fun.body
                {
                    mentions(body, found);
                }
            }
        }
    }
}

fn expr_mentions(expr: &Expr, found: &mut HashSet<String>) {
    use super::kotlin::ast::{StrPart, WhenCondition};
    match expr {
        Expr::Name(name) => {
            found.insert(name.clone());
        }
        Expr::Str(parts) => {
            for part in parts {
                if let StrPart::Code(code) = part {
                    expr_mentions(code, found);
                }
            }
        }
        Expr::Member { target, name, .. } => {
            found.insert(name.clone());
            expr_mentions(target, found);
        }
        Expr::Call {
            callee,
            args,
            lambda,
            type_args,
        } => {
            // `parseAs<MangaDto>()` reaches the DTO.
            fn types(ty: &super::kotlin::ast::Type, found: &mut HashSet<String>) {
                found.insert(ty.name.rsplit('.').next().unwrap_or(&ty.name).to_owned());
                for arg in &ty.args {
                    types(arg, found);
                }
            }
            for ty in type_args {
                types(ty, found);
            }
            expr_mentions(callee, found);
            for arg in args {
                expr_mentions(&arg.value, found);
            }
            if let Some(lambda) = lambda {
                block_mentions(&lambda.body, found);
            }
        }
        Expr::Index { target, index } => {
            expr_mentions(target, found);
            for key in index {
                expr_mentions(key, found);
            }
        }
        Expr::NotNull(inner)
        | Expr::Unary { expr: inner, .. }
        | Expr::Postfix { expr: inner, .. }
        | Expr::Is { expr: inner, .. }
        | Expr::As { expr: inner, .. }
        | Expr::Throw(inner) => expr_mentions(inner, found),
        Expr::Binary { op, left, right } => {
            found.insert(op.clone());
            expr_mentions(left, found);
            expr_mentions(right, found);
        }
        Expr::If {
            condition,
            then,
            otherwise,
        } => {
            expr_mentions(condition, found);
            block_mentions(then, found);
            if let Some(otherwise) = otherwise {
                block_mentions(otherwise, found);
            }
        }
        Expr::When {
            subject, branches, ..
        } => {
            if let Some(subject) = subject {
                expr_mentions(subject, found);
            }
            for branch in branches {
                for condition in &branch.conditions {
                    match condition {
                        WhenCondition::Expr(value) => expr_mentions(value, found),
                        WhenCondition::In { range, .. } => expr_mentions(range, found),
                        WhenCondition::Is { .. } => {}
                    }
                }
                if let Some(guard) = &branch.guard {
                    expr_mentions(guard, found);
                }
                block_mentions(&branch.body, found);
            }
        }
        Expr::Try {
            body,
            catches,
            finally,
        } => {
            block_mentions(body, found);
            for catch in catches {
                block_mentions(&catch.body, found);
            }
            if let Some(finally) = finally {
                block_mentions(finally, found);
            }
        }
        Expr::Lambda(lambda) => block_mentions(&lambda.body, found),
        Expr::Reference { target, name } => {
            found.insert(name.clone());
            if let Some(target) = target {
                expr_mentions(target, found);
            }
        }
        Expr::Return {
            value: Some(value), ..
        } => expr_mentions(value, found),
        Expr::Object(class) => {
            for member in &class.members {
                if let Member::Fun(fun) = member
                    && let Some(body) = &fun.body
                {
                    mentions(body, found);
                }
            }
        }
        _ => {}
    }
}

/// The source's members (by name) the entry points reach, through what each mentions.
fn reachable(members: &[(String, HashSet<String>)]) -> HashSet<String> {
    let mut reached: HashSet<String> = ENTRIES.iter().map(|name| (*name).to_owned()).collect();
    let mut changed = true;
    while changed {
        changed = false;
        for (name, mentioned) in members {
            if !reached.contains(name) {
                continue;
            }
            for other in mentioned {
                if members.iter().any(|(member, _)| member == other)
                    && reached.insert(other.clone())
                {
                    changed = true;
                }
            }
        }
    }
    reached
}

fn quote(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_owned())
}

/// The source class: the one marked `@Source`, else one extending `KeiSource` or `HttpSource`.
fn source_class(files: &[(String, File)]) -> Option<&ClassDecl> {
    let classes = files.iter().flat_map(|(_, file)| {
        file.members.iter().filter_map(|member| match member {
            Member::Class(class) => Some(class),
            _ => None,
        })
    });
    let mut fallback = None;
    for class in classes {
        if class.annotations.iter().any(|name| name == "Source") {
            return Some(class);
        }
        let base = class.supers.iter().any(|parent| {
            matches!(
                parent.ty.name.rsplit('.').next(),
                Some("KeiSource" | "HttpSource" | "ParsedHttpSource")
            )
        });
        if base && fallback.is_none() {
            fallback = Some(class);
        }
    }
    fallback
}

/// A function's parameter names, as calls place their arguments; a vararg before other
/// parameters is `*name`: the call's positional arguments go to it, as a list. A function with a
/// receiver (`params: HttpUrl.Builder.() -> Unit`) is `^name`: its lambda reads the receiver.
fn param_names(params: &[Param]) -> Vec<String> {
    params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            if param.vararg && index + 1 < params.len() {
                format!("*{}", param.name)
            } else if param.ty.name == super::kotlin::ast::RECEIVER_FUNCTION {
                format!("^{}", param.name)
            } else {
                param.name.clone()
            }
        })
        .collect()
}

/// A property's value as a function body: its initializer, getter or delegate.
fn property_value(prop: &PropDecl) -> Option<Body> {
    if let Some(value) = &prop.value {
        return Some(Body::Expr(value.clone()));
    }
    if let Some(getter) = &prop.getter {
        return Some(getter.clone());
    }
    match &prop.delegate {
        // `by lazy { … }`: the lambda's value (its last expression), worked out once.
        Some(Expr::Call {
            callee,
            lambda: Some(lambda),
            args,
            type_args,
        }) if args.is_empty() && matches!(callee.as_ref(), Expr::Name(name) if name == "lazy") => {
            let mut lambda = lambda.clone();
            if lambda.label.is_none() {
                lambda.label = Some("lazy".to_owned());
            }
            Some(Body::Expr(Expr::Call {
                callee: Box::new(Expr::Name("run".to_owned())),
                type_args: type_args.clone(),
                args: Vec::new(),
                lambda: Some(lambda),
            }))
        }
        // `by getPreferencesLazy()`: a `Lazy`'s value.
        Some(other @ Expr::Call { callee, .. }) if matches!(callee.as_ref(), Expr::Name(name) if name.ends_with("Lazy")) => {
            Some(Body::Expr(Expr::Member {
                target: Box::new(other.clone()),
                name: "value".to_owned(),
                safe: false,
            }))
        }
        Some(other) => Some(Body::Expr(other.clone())),
        None => None,
    }
}

/// What the source's classes (its own and the theme's beneath) and the files declare, for the
/// generator.
fn context(chain: &[&ClassDecl], files: &[(String, File)]) -> Context {
    let mut cx = Context::default();
    for name in BASE_PROPERTIES {
        cx.properties.insert(name.to_owned());
    }
    for (name, params) in BASE_FUNCTIONS {
        cx.functions
            .entry(name.to_owned())
            .or_default()
            .push(params.iter().map(|param| (*param).to_owned()).collect());
    }
    let add_members = |cx: &mut Context, members: &[Member], top: bool| {
        for member in members {
            match member {
                Member::Fun(fun) if fun.receiver.is_some() => {
                    cx.extensions.insert(fun.name.clone());
                }
                Member::Fun(fun) => {
                    cx.functions
                        .entry(fun.name.clone())
                        .or_default()
                        .push(param_names(&fun.params));
                }
                Member::Prop(prop) if prop.receiver.is_some() => {
                    cx.extension_properties.insert(prop.name.clone());
                }
                Member::Prop(prop) if top => {
                    cx.constants.insert(prop.name.clone());
                }
                Member::Prop(prop) => {
                    cx.properties.insert(prop.name.clone());
                }
                Member::Class(inner) if inner.modifiers.iter().any(|m| m == "companion") => {}
                Member::Class(inner) => {
                    cx.classes.insert(inner.name.clone());
                }
                _ => {}
            }
        }
    };
    for (_, file) in files {
        add_members(&mut cx, &file.members, true);
        for member in &file.members {
            if let Member::TypeAlias(name, target) = member {
                cx.type_aliases.insert(name.clone(), target.clone());
            }
        }
    }
    for class in chain {
        add_members(&mut cx, &class.members, false);
    }
    for member in chain.iter().flat_map(|class| &class.members) {
        if let Member::Class(inner) = member
            && inner.modifiers.iter().any(|m| m == "companion")
        {
            for member in &inner.members {
                match member {
                    Member::Prop(prop) if prop.receiver.is_none() => {
                        cx.constants.insert(prop.name.clone());
                    }
                    Member::Fun(fun) if fun.receiver.is_some() => {
                        cx.extensions.insert(fun.name.clone());
                    }
                    Member::Fun(fun) => {
                        cx.functions
                            .entry(fun.name.clone())
                            .or_default()
                            .push(param_names(&fun.params));
                    }
                    _ => {}
                }
            }
        }
    }
    // Classes declared in the source's files (filters, DTOs), nested ones too, and what their
    // instances have.
    fn classes(members: &[Member], skip: &[&ClassDecl], cx: &mut Context) {
        for member in members {
            if let Member::Class(inner) = member {
                if skip.iter().any(|class| std::ptr::eq(inner, *class)) {
                    classes(&inner.members, skip, cx);
                    continue;
                }
                if !inner.modifiers.iter().any(|m| m == "companion") {
                    cx.classes.insert(inner.name.clone());
                    let values: HashSet<String> = companion_values(inner)
                        .map(|prop| prop.name.clone())
                        .collect();
                    if !values.is_empty() {
                        cx.companions.insert(inner.name.clone(), values);
                    }
                    let functions: HashSet<String> = companion_functions(inner)
                        .map(|fun| fun.name.clone())
                        .collect();
                    if !functions.is_empty() {
                        cx.companion_functions.insert(inner.name.clone(), functions);
                    }
                }
                for nested in &inner.members {
                    if matches!(nested, Member::Fun(_) | Member::Prop(_)) {
                        cx.class_members.insert(nested.name().to_owned());
                    }
                    // A class's own extensions (`private fun Float.formatNumber()`) are the
                    // source's (`transpile::class_definition`).
                    if let Member::Fun(fun) = nested
                        && fun.receiver.is_some()
                        && fun.body.is_some()
                    {
                        cx.extensions.insert(fun.name.clone());
                    }
                }
                // Its constructor's properties too (`GenreRoute::name`).
                for param in &inner.params {
                    cx.class_members.insert(param.name.clone());
                }
                classes(&inner.members, skip, cx);
            }
        }
    }
    for (_, file) in files {
        classes(&file.members, chain, &mut cx);
    }
    cx
}

/// Code defining the class's members on `self` (functions, properties, extensions) and its
/// constants on `C`.
fn members(
    writer: &mut Gen<'_>,
    members: &[Member],
    out: &mut Vec<String>,
    target: &str,
    companion: bool,
    reached: &HashSet<String>,
) -> Result<(), String> {
    // Code the source never reaches from what Raygun calls is left out when it can't convert:
    // it says so if it's called after all.
    let unreached = |name: &str, reason: String| -> Result<String, String> {
        if reached.contains(name) {
            Err(format!("{name}: {reason}"))
        } else {
            Ok(format!(
                "async () => KT.raise({})",
                quote(&format!("Raygun couldn't convert {name}: {reason}"))
            ))
        }
    };
    let mut overloads: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for member in members {
        match member {
            Member::Init(_) => return Err("it runs code of its own when it starts".into()),
            Member::TypeAlias(..) => {}
            Member::Class(inner) if inner.modifiers.iter().any(|m| m == "companion") => {
                self::members(writer, &inner.members, out, "C", true, reached)?;
            }
            Member::Class(inner) if writer.replaced.contains_key(&inner.name) => {
                out.push(writer.replaced[&inner.name].clone());
            }
            Member::Class(inner) => match class_code(writer, inner) {
                Ok(code) => out.push(code),
                Err(reason) if !reached.contains(&inner.name) => out.push(format!(
                    "D.{} = KT.unconverted({}, {});",
                    inner.name,
                    quote(&inner.name),
                    quote(&reason)
                )),
                Err(reason) => return Err(reason),
            },
            Member::Fun(fun) if writer.replaced.contains_key(&fun.name) => {
                out.push(writer.replaced[&fun.name].clone());
            }
            Member::Fun(fun) => {
                if SKIPPED.contains(&fun.name.as_str()) || fun.body.is_none() {
                    continue;
                }
                if fun.name == "configureClient" || fun.name == "configureHeaders" {
                    let function = writer.function(fun)?;
                    out.push(format!("self.extension({}, {function});", quote(&fun.name)));
                    continue;
                }
                let function = match writer.function(fun) {
                    Ok(function) => function,
                    Err(reason) => unreached(&fun.name, reason)?,
                };
                if let Some(receiver) = &fun.receiver {
                    // Overloads by receiver (`Elements.imgAttr()` beside `Element.imgAttr()`) are
                    // told apart by the receiver's type when called, then by their arguments.
                    out.push(format!(
                        "self.extension({}, {function}, {}, {});",
                        quote(&fun.name),
                        quote(receiver.name.rsplit('.').next().unwrap_or(&receiver.name)),
                        shape(fun, &[])
                    ));
                } else {
                    let required = fun
                        .params
                        .iter()
                        .filter(|param| param.default.is_none() && !param.vararg)
                        .count();
                    let total = if fun.params.iter().any(|param| param.vararg) {
                        usize::MAX / 2
                    } else {
                        fun.params.len()
                    };
                    overloads
                        .entry(fun.name.clone())
                        .or_default()
                        .push(format!("[{required}, {total}, {function}]"));
                }
            }
            Member::Prop(prop) => {
                if SKIPPED.contains(&prop.name.as_str()) {
                    continue;
                }
                let Some(body) = property_value(prop) else {
                    // `lateinit var`, a property set later.
                    out.push(format!(
                        "{target}.prop({}, async () => null);",
                        quote(&prop.name)
                    ));
                    continue;
                };
                let getter = super::kotlin::ast::FunDecl {
                    name: prop.name.clone(),
                    modifiers: Vec::new(),
                    annotations: Vec::new(),
                    receiver: prop.receiver.clone(),
                    params: Vec::new(),
                    ret: None,
                    body: Some(body),
                    line: prop.line,
                };
                let function = match writer.function(&getter) {
                    Ok(function) => function,
                    Err(reason) => unreached(&prop.name, reason)?,
                };
                if prop.receiver.is_some() {
                    out.push(format!(
                        "self.extension({}, {function});",
                        quote(&prop.name)
                    ));
                } else if companion || target == "C" {
                    out.push(format!("C.prop({}, {function});", quote(&prop.name)));
                } else {
                    // A getter runs each time; a value once.
                    let once = prop.getter.is_none();
                    out.push(format!(
                        "{target}.prop({}, {function}, {once});",
                        quote(&prop.name)
                    ));
                }
            }
        }
    }
    for (name, functions) in overloads {
        out.push(format!(
            "self.define({}, [{}]);",
            quote(&name),
            functions.join(", ")
        ));
    }
    Ok(())
}

/// What tells a function's overloads apart: `[required, total, [types]]`, the arguments it needs,
/// how many it takes, and each parameter's type (`null` for a type parameter, which takes
/// anything).
fn shape(fun: &super::kotlin::ast::FunDecl, type_params: &[String]) -> String {
    let required = fun
        .params
        .iter()
        .filter(|param| param.default.is_none() && !param.vararg)
        .count();
    let total = if fun.params.iter().any(|param| param.vararg) {
        usize::MAX / 2
    } else {
        fun.params.len()
    };
    let types: Vec<String> = fun
        .params
        .iter()
        .map(|param| {
            let name = param.ty.name.rsplit('.').next().unwrap_or(&param.ty.name);
            let known = type_params.iter().all(|own| own != name)
                && !(name.len() == 1 && name.chars().all(|c| c.is_ascii_uppercase()))
                && !name.is_empty()
                && !name.starts_with('(');
            if known {
                quote(name)
            } else {
                "null".to_owned()
            }
        })
        .collect();
    format!("[{required}, {total}, [{}]]", types.join(", "))
}

/// The values a class's companion object holds (`companion object { val OPTIONS = … }`).
fn companion_values(class: &ClassDecl) -> impl Iterator<Item = &PropDecl> {
    class
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Class(inner) if inner.modifiers.iter().any(|m| m == "companion") => {
                Some(inner.members.iter())
            }
            _ => None,
        })
        .flatten()
        .filter_map(|member| match member {
            Member::Prop(prop) if prop.receiver.is_none() => Some(prop),
            _ => None,
        })
}

/// The functions a class's companion object has (`companion object { fun nextJsFilter(…) }`).
fn companion_functions(class: &ClassDecl) -> impl Iterator<Item = &FunDecl> {
    class
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Class(inner) if inner.modifiers.iter().any(|m| m == "companion") => {
                Some(inner.members.iter())
            }
            _ => None,
        })
        .flatten()
        .filter_map(|member| match member {
            Member::Fun(fun) if fun.receiver.is_none() && fun.body.is_some() => Some(fun),
            _ => None,
        })
}

/// A class the source declares, with its companion's values on `C` (as `Class.name`): its code
/// reads them by their plain names. The classes declared inside it come after it.
fn class_code(writer: &mut Gen<'_>, class: &ClassDecl) -> Result<String, String> {
    class_code_in(writer, class, &[])
}

/// The classes a nested class is declared in, outermost first.
type Enclosing<'a> = [&'a ClassDecl];

fn is_companion(class: &ClassDecl) -> bool {
    class.modifiers.iter().any(|m| m == "companion")
}

fn class_code_in(
    writer: &mut Gen<'_>,
    class: &ClassDecl,
    outer: &Enclosing<'_>,
) -> Result<String, String> {
    let code = own_class_code(writer, class, outer)?;
    let mut all = vec![code];
    let mut scope = outer.to_vec();
    scope.push(class);
    for member in &class.members {
        if let Member::Class(nested) = member
            && !is_companion(nested)
            && !writer.replaced.contains_key(&nested.name)
        {
            all.push(
                class_code_in(writer, nested, &scope).unwrap_or_else(|reason| {
                    format!(
                        "D.{} = KT.unconverted({}, {});",
                        nested.name,
                        quote(&nested.name),
                        quote(&reason)
                    )
                }),
            );
        }
    }
    Ok(all.join(" "))
}

/// A type's name as the runtime knows the class (`ChapterListDto.ChapterDto` for a `ChapterDto`
/// declared in a class around it), with its arguments.
fn qualified_type(ty: &super::kotlin::ast::Type, scope: &Enclosing<'_>) -> String {
    let short = ty.name.rsplit('.').next().unwrap_or(&ty.name);
    let name = if ty.name.contains('.') {
        ty.name.clone()
    } else {
        (0..scope.len())
            .rev()
            .find(|at| {
                scope[*at].members.iter().any(|member| {
                    matches!(member, Member::Class(nested) if nested.name == short && !is_companion(nested))
                })
            })
            .map_or_else(
                || short.to_owned(),
                |at| {
                    let mut path: Vec<&str> =
                        scope[..=at].iter().map(|class| class.name.as_str()).collect();
                    path.push(short);
                    path.join(".")
                },
            )
    };
    if ty.args.is_empty() {
        name
    } else {
        format!(
            "{name}<{}>",
            ty.args
                .iter()
                .map(|arg| qualified_type(arg, scope))
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

fn own_class_code(
    writer: &mut Gen<'_>,
    class: &ClassDecl,
    outer: &Enclosing<'_>,
) -> Result<String, String> {
    let outer_scope = writer.class_scope.replace(class.name.clone());
    let result = (|| {
        let mut code = Vec::new();
        for prop in companion_values(class) {
            let Some(body) = property_value(prop) else {
                continue;
            };
            let getter = super::kotlin::ast::FunDecl {
                name: prop.name.clone(),
                modifiers: Vec::new(),
                annotations: Vec::new(),
                receiver: None,
                params: Vec::new(),
                ret: None,
                body: Some(body),
                line: prop.line,
            };
            let function = writer
                .function(&getter)
                .map_err(|reason| format!("{}.{}: {reason}", class.name, prop.name))?;
            code.push(format!(
                "C.prop({}, {function});",
                quote(&super::kotlin::emit::companion_key(&class.name, &prop.name))
            ));
        }
        code.push(class_definition(writer, class, outer)?);
        Ok(code.join(" "))
    })();
    writer.class_scope = outer_scope;
    result
}

/// A class the source declares (a filter, a DTO, an enum): `D.Name`, made by `KT.make`.
fn class_definition(
    writer: &mut Gen<'_>,
    class: &ClassDecl,
    outer: &Enclosing<'_>,
) -> Result<String, String> {
    let mut scope = outer.to_vec();
    scope.push(class);
    let qualified_name = scope
        .iter()
        .map(|class| class.name.as_str())
        .collect::<Vec<_>>()
        .join(".");
    let params: Vec<String> = class
        .params
        .iter()
        .map(|param| quote(&param.name))
        .collect();
    // Each default sees the parameters before it (`val coinPrice: Int = if (isLocked) 1 else 0`):
    // it's a function of them all, called with the values set so far.
    let mut defaults = Vec::new();
    for param in &class.params {
        if let Some(default) = &param.default {
            let function = super::kotlin::ast::FunDecl {
                name: param.name.clone(),
                modifiers: Vec::new(),
                annotations: Vec::new(),
                receiver: None,
                params: class
                    .params
                    .iter()
                    .map(|param| Param {
                        default: None,
                        vararg: false,
                        ..param.clone()
                    })
                    .collect(),
                ret: None,
                body: Some(Body::Expr(default.clone())),
                line: class.line,
            };
            defaults.push(format!(
                "{}: {}",
                quote(&param.name),
                writer.function(&function)?
            ));
        }
    }
    // Each field's protobuf number and how its number is written (`@ProtoNumber`, `@ProtoType`).
    let proto: Vec<String> = class
        .params
        .iter()
        .filter_map(|param| {
            let number = param
                .annotations
                .iter()
                .find_map(|annotation| annotation.strip_prefix("ProtoNumber="))?;
            let kind = param
                .annotations
                .iter()
                .find_map(|annotation| annotation.strip_prefix("ProtoType="))
                .map_or_else(|| "null".to_owned(), quote);
            Some(format!("{}: [{number}, {kind}]", quote(&param.name)))
        })
        .collect();
    // Each field's type, for JSON decoded into the class (`List<ChapterDto>`), nested classes by
    // the names that tell them apart.
    let types: Vec<String> = class
        .params
        .iter()
        .map(|param| {
            format!(
                "{}: {}",
                quote(&param.name),
                quote(&qualified_type(&writer.unaliased(&param.ty), &scope))
            )
        })
        .collect();
    let supers: Vec<String> = class
        .supers
        .iter()
        .map(|parent| quote(&parent.ty.name))
        .collect();
    // The first parent's constructor arguments, over the class's parameters (`Filter.Select(…)`).
    let super_args = match class.supers.first().and_then(|parent| parent.args.as_ref()) {
        Some(args) if !args.is_empty() => {
            let fake = super::kotlin::ast::FunDecl {
                name: "super".into(),
                modifiers: Vec::new(),
                annotations: Vec::new(),
                receiver: None,
                params: class.params.clone(),
                ret: None,
                body: Some(Body::Expr(Expr::Call {
                    callee: Box::new(Expr::Name("listOf".into())),
                    type_args: Vec::new(),
                    args: args.clone(),
                    lambda: None,
                })),
                line: class.line,
            };
            writer.function(&fake)?
        }
        _ => "null".to_owned(),
    };
    let entries: Vec<String> = class
        .entries
        .iter()
        .map(|entry| quote(&entry.name))
        .collect();
    let mut methods = Vec::new();
    // Its functions by name: each overload as `[required, total, [types], code]`, and its code.
    let mut overloaded: Vec<(String, Vec<(String, String)>)> = Vec::new();
    // Extensions the class declares for its own code (`private fun Float.formatNumber()`): the
    // source's, as they read nothing of the instance but their receiver.
    let mut extensions = Vec::new();
    for member in &class.members {
        match member {
            Member::Fun(fun) if fun.body.is_some() && fun.receiver.is_some() => {
                let receiver = fun
                    .receiver
                    .as_ref()
                    .map(|ty| ty.name.as_str())
                    .unwrap_or("");
                let function = writer
                    .function(fun)
                    .map_err(|reason| format!("{}.{}: {reason}", class.name, fun.name))?;
                extensions.push(format!(
                    "self.extension({}, {function}, {}, {});",
                    quote(&fun.name),
                    quote(receiver.rsplit('.').next().unwrap_or(receiver)),
                    shape(fun, &class.type_params)
                ));
            }
            Member::Fun(fun) if fun.body.is_some() => {
                let mut method = fun.clone();
                // Members read the instance as their receiver.
                method.receiver = Some(super::kotlin::ast::Type {
                    name: class.name.clone(),
                    ..Default::default()
                });
                let code = writer
                    .function(&method)
                    .map_err(|reason| format!("{}.{}: {reason}", class.name, fun.name))?;
                let shape = shape(fun, &class.type_params);
                let overload = format!("[{}, {code}]", &shape[1..shape.len() - 1]);
                match overloaded.iter_mut().find(|(name, _)| *name == fun.name) {
                    Some((_, list)) => list.push((overload, code)),
                    None => overloaded.push((fun.name.clone(), vec![(overload, code)])),
                }
            }
            Member::Prop(prop) => {
                if let Some(body) = property_value(prop) {
                    let getter = super::kotlin::ast::FunDecl {
                        name: prop.name.clone(),
                        modifiers: Vec::new(),
                        annotations: Vec::new(),
                        receiver: Some(super::kotlin::ast::Type {
                            name: class.name.clone(),
                            ..Default::default()
                        }),
                        params: Vec::new(),
                        ret: None,
                        body: Some(body),
                        line: prop.line,
                    };
                    // A `var` with a value is a field, set when the instance is made; other
                    // properties are worked out when read.
                    let wrapper = if prop.mutable && prop.value.is_some() && prop.getter.is_none() {
                        "KT.field"
                    } else {
                        "KT.getter"
                    };
                    methods.push(format!(
                        "{}: {wrapper}({})",
                        quote(&prop.name),
                        writer
                            .function(&getter)
                            .map_err(|reason| format!("{}.{}: {reason}", class.name, prop.name))?
                    ));
                }
            }
            Member::Init(_) => return Err(format!("{} runs code when it's made", class.name)),
            _ => {}
        }
    }
    for (name, list) in overloaded {
        methods.push(if let [(_, code)] = list.as_slice() {
            format!("{}: {code}", quote(&name))
        } else {
            let overloads: Vec<&str> = list.iter().map(|(overload, _)| overload.as_str()).collect();
            format!(
                "{}: KT.overloaded([{}])",
                quote(&name),
                overloads.join(", ")
            )
        });
    }
    let kind = match class.kind {
        ClassKind::Enum => "enum",
        ClassKind::Object => "object",
        ClassKind::Interface => "interface",
        ClassKind::Class => "class",
    };
    // Its companion's functions, called on the class (`ChapterDto.nextJsFilter(…)`).
    let mut statics = Vec::new();
    for fun in companion_functions(class) {
        let function = writer.function(fun).unwrap_or_else(|reason| {
            format!(
                "async () => KT.raise({})",
                quote(&format!(
                    "Raygun couldn't convert {}.{}: {reason}",
                    class.name, fun.name
                ))
            )
        });
        statics.push(format!("{}: {function}", quote(&fun.name)));
    }
    Ok(format!(
        "{extensions}D.{name} = KT.defineClass({{ name: {quoted}, qualified: {qualified}, serialName: {serial}, kind: {kind:?}, typeParams: [{type_params}], params: [{params}], types: {{{types}}}, proto: {{{proto}}}, defaults: {{{defaults}}}, required: {required}, supers: [{supers}], superArgs: {super_args}, entries: [{entries}], members: {{{methods}}}, statics: {{{statics}}} }});",
        extensions = extensions
            .iter()
            .map(|code| format!("{code} "))
            .collect::<String>(),
        qualified = quote(&qualified_name),
        // The name a polymorphic answer gives it (`"type": "selfhosted"`).
        serial = quote(
            class
                .annotations
                .iter()
                .find_map(|annotation| annotation.strip_prefix("SerialName="))
                .unwrap_or(&class.name)
        ),
        statics = statics.join(", "),
        name = class.name,
        quoted = quote(&class.name),
        required = serde_json::to_string(&required_fields(class)).unwrap_or_else(|_| "[]".into()),
        type_params = class
            .type_params
            .iter()
            .map(|name| quote(name))
            .collect::<Vec<_>>()
            .join(", "),
        params = params.join(", "),
        types = types.join(", "),
        proto = proto.join(", "),
        defaults = defaults.join(", "),
        supers = supers.join(", "),
        entries = entries.join(", "),
        methods = methods.join(", "),
    ))
}

/// The class named `name` among the files' top-level declarations.
fn find_class<'a>(files: &'a [(String, File)], name: &str) -> Option<&'a ClassDecl> {
    files.iter().find_map(|(_, file)| {
        file.members.iter().find_map(|member| match member {
            Member::Class(class) if class.name == name => Some(class),
            _ => None,
        })
    })
}

/// Converts an extension written on `KeiSource`, or on a theme whose code is in `theme_files`
/// (Keiyoushi's `lib-multisrc/<theme>`): the theme's classes become layers beneath the source's
/// own, as Kotlin inherits them. `messages` are the `lib:i18n` message files by language. The
/// error says why it can't be converted.
pub fn convert(
    extension: &StoreExtension,
    files: &[(String, File)],
    theme: Option<(&str, &[(String, File)])>,
    messages: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<Converted, String> {
    let (theme_name, theme_files) = theme.unwrap_or(("", &[]));
    let all_files: Vec<(String, File)> = files.iter().chain(theme_files).cloned().collect();
    let (own_files, theme_part) = all_files.split_at(files.len());
    let class = source_class(own_files).ok_or("Raygun couldn't find its source class")?;
    // The classes it builds on, nearest first, down to `KeiSource`.
    let mut chain = vec![class];
    loop {
        let current = chain[chain.len() - 1];
        let parent = current
            .supers
            .iter()
            .find_map(|parent| parent.ty.name.rsplit('.').next())
            .unwrap_or_default();
        if parent == "KeiSource" {
            break;
        }
        let found = find_class(theme_part, parent).or_else(|| find_class(own_files, parent));
        match found {
            Some(next)
                if chain.len() < 6 && !chain.iter().any(|known| std::ptr::eq(*known, next)) =>
            {
                chain.push(next);
            }
            _ => {
                return Err(format!(
                    "it's built on {parent}, which Raygun's converter doesn't read yet"
                ));
            }
        }
    }
    let defined: HashSet<&str> = chain
        .iter()
        .flat_map(|class| class.members.iter().map(Member::name))
        .collect();
    if let Some(missing) = NEEDED.iter().find(|name| !defined.contains(**name)) {
        return Err(format!("it has no {missing} of its own"));
    }
    // The theme's other source classes (`MadaraNoAjax` beside `Madara`) aren't this source's: they
    // are left out with the source's own.
    let mut source_like: HashSet<&str> = HashSet::from(["KeiSource"]);
    loop {
        let before = source_like.len();
        for (_, file) in &all_files {
            for member in &file.members {
                if let Member::Class(class) = member
                    && class.supers.iter().any(|parent| {
                        source_like.contains(parent.ty.name.rsplit('.').next().unwrap_or_default())
                    })
                {
                    source_like.insert(&class.name);
                }
            }
        }
        if source_like.len() == before {
            break;
        }
    }
    let left_out: Vec<&ClassDecl> = all_files
        .iter()
        .flat_map(|(_, file)| &file.members)
        .filter_map(|member| match member {
            Member::Class(class) if source_like.contains(class.name.as_str()) => Some(class),
            _ => None,
        })
        .collect();
    let cx = context(&chain, &all_files);
    let reached = reachable(&member_mentions(&left_out, &chain, &all_files));
    let mut writer = Gen::new(&cx);
    // A theme whose interceptor puts shuffled pictures back together: Raygun's image fetcher
    // does that instead, told by the pages' addresses (`raygun_net::images::Unscramble`).
    let image_recipe = IMAGE_RECIPES
        .iter()
        .find(|(theme, _)| *theme == theme_name)
        .map(|(_, recipe)| *recipe);
    if image_recipe.is_some() {
        writer.replaced.insert(
            "ImageInterceptor".to_owned(),
            "D.ImageInterceptor = KT.passThrough(\"ImageInterceptor\");".to_owned(),
        );
    }
    // Madara's chapter protector: OpenSSL's key derivation and AES-CBC, which the runtime does in
    // Rust.
    if theme_name == "madara" {
        writer.replaced.insert(
            "decryptChapterData".to_owned(),
            "self.define(\"decryptChapterData\", [[2, 2, async ($data, $password) => K.decrypt({ data: await KT.p($data, \"ciphertext\"), password: KT.str($password), salt: await KT.p($data, \"salt\") })]]);".to_owned(),
        );
    }
    let in_chain = |member: &Member| matches!(member, Member::Class(inner) if left_out.iter().any(|class| std::ptr::eq(inner, *class)));
    // The base-most layer first, with the theme's files; the source's own on top, with its files.
    let mut layers = Vec::new();
    for (index, layer) in chain.iter().rev().enumerate() {
        let mut out = Vec::new();
        let layer_files: &[(String, File)] = if index + 1 == chain.len() {
            own_files
        } else if index == 0 {
            theme_part
        } else {
            &[]
        };
        for (_, file) in layer_files {
            let others: Vec<Member> = file
                .members
                .iter()
                .filter(|member| !in_chain(member))
                .cloned()
                .collect();
            // A file's imports say which classes its names mean (Mihon's `Page` or its own).
            writer.imports.clone_from(&file.imports);
            members(&mut writer, &others, &mut out, "C", true, &reached)?;
        }
        writer.imports = all_files
            .iter()
            .find(|(_, file)| {
                file.members.iter().any(
                    |member| matches!(member, Member::Class(class) if std::ptr::eq(class, *layer)),
                )
            })
            .map(|(_, file)| file.imports.clone())
            .unwrap_or_default();
        members(
            &mut writer,
            &layer.members,
            &mut out,
            "self",
            false,
            &reached,
        )?;
        layers.push(format!(
            "    (self, C, D) => {{\n{}\n    }},",
            out.join("\n")
        ));
    }
    // A value read as `json` decodes with names the DTOs spell (`@SerialName`).
    let renames = serial_names(&all_files);
    let aliases = json_aliases(&all_files);
    let layers = layers.join("\n");
    let from_source = images_from_source(&layers);
    let code = format!(
        "// {name}, converted by Raygun from the Mihon extension {pkg} (Apache-2.0).\n'use strict';\nmihon.source({{\n  sources: {sources},\n  renames: {renames},\n  aliases: {aliases},\n  messages: {messages},\n  imageRecipe: {recipe},\n  imagesFromSource: {from_source},\n  layers: [\n{layers}\n  ],\n}});\n",
        name = extension.name.replace(['\n', '\r'], " "),
        pkg = extension.pkg,
        sources = sources_json(extension),
        renames = serde_json::to_string(&renames).unwrap_or_else(|_| "{}".into()),
        aliases = serde_json::to_string(&aliases).unwrap_or_else(|_| "{}".into()),
        messages = serde_json::to_string(messages).unwrap_or_else(|_| "{}".into()),
        recipe = image_recipe.map_or_else(|| "null".to_owned(), quote),
    );
    Ok(Converted { theme: None, code })
}

/// Calls in converted code that mean its client draws, decodes or rewrites pictures: Android's
/// graphics, `lib:textinterceptor` and `lib:imagedecoder`, or an answer's bytes read and made
/// into a new body. Such a source's pages are fetched through it (`image`), as Mihon fetches
/// them through the source's client.
const PICTURE_CALLS: [&str; 7] = [
    "KT.global(\"BitmapFactory\")",
    "KT.global(\"Bitmap\")",
    "KT.call(\"Canvas\"",
    "KT.call(\"TextInterceptor\"",
    "KT.global(\"ImageDecoder\")",
    "\"asResponseBody\"",
    "\"byteStream\"",
];

/// Whether the source's client changes the pictures it fetches: it draws them, or an interceptor
/// decrypts answers (`response.aesDecrypt(cipher)`).
fn images_from_source(code: &str) -> bool {
    PICTURE_CALLS.iter().any(|call| code.contains(call))
        || ((code.contains("\"aesDecrypt") || code.contains("\"cipherSource\""))
            && code.contains("\"intercept\""))
}

/// Each member of the classes, their companions, the files and their classes (but the source
/// classes `left_out`), with the names their code mentions; a class stands for all its members.
fn member_mentions(
    left_out: &[&ClassDecl],
    chain: &[&ClassDecl],
    files: &[(String, File)],
) -> Vec<(String, HashSet<String>)> {
    let mut found = Vec::new();
    fn add(members: &[Member], owner: Option<&str>, found: &mut Vec<(String, HashSet<String>)>) {
        for member in members {
            let mut mentioned = HashSet::new();
            match member {
                Member::Fun(fun) => {
                    if let Some(body) = &fun.body {
                        mentions(body, &mut mentioned);
                    }
                    for param in &fun.params {
                        if let Some(default) = &param.default {
                            expr_mentions(default, &mut mentioned);
                        }
                    }
                }
                Member::Prop(prop) => {
                    if let Some(body) = property_value(prop) {
                        mentions(&body, &mut mentioned);
                    }
                }
                Member::Class(inner) => {
                    add(&inner.members, Some(&inner.name), found);
                    let mut all = HashSet::new();
                    for parent in &inner.supers {
                        for arg in parent.args.iter().flatten() {
                            expr_mentions(&arg.value, &mut all);
                        }
                    }
                    for nested in &inner.members {
                        all.insert(nested.name().to_owned());
                    }
                    found.push((inner.name.clone(), all));
                    continue;
                }
                Member::Init(block) => block_mentions(block, &mut mentioned),
                Member::TypeAlias(..) => continue,
            }
            // A class's member reached reaches its class too.
            if let Some(owner) = owner {
                mentioned.insert(owner.to_owned());
            }
            found.push((member.name().to_owned(), mentioned));
        }
    }
    for class in chain {
        add(&class.members, None, &mut found);
    }
    for (_, file) in files {
        let others: Vec<Member> = file
            .members
            .iter()
            .filter(|member| {
                !matches!(member, Member::Class(inner) if left_out.iter().any(|class| std::ptr::eq(inner, *class)))
            })
            .cloned()
            .collect();
        add(&others, None, &mut found);
    }
    found
}

/// Every class the files declare, nested ones too.
fn all_classes(files: &[(String, File)]) -> Vec<&ClassDecl> {
    fn walk<'a>(members: &'a [Member], found: &mut Vec<&'a ClassDecl>) {
        for member in members {
            if let Member::Class(class) = member {
                found.push(class);
                walk(&class.members, found);
            }
        }
    }
    let mut classes = Vec::new();
    for (_, file) in files {
        walk(&file.members, &mut classes);
    }
    classes
}

/// The JSON names DTO properties are read by (`@SerialName("manga_id") val id`), by property name.
fn serial_names(files: &[(String, File)]) -> HashMap<String, String> {
    let mut names = HashMap::new();
    for class in all_classes(files) {
        for param in &class.params {
            for annotation in &param.annotations {
                if let Some(serial) = annotation.strip_prefix("SerialName=") {
                    names.insert(param.name.clone(), serial.to_owned());
                }
            }
        }
    }
    names
}

/// A property's other JSON names (`@JsonNames("results", "manga_list")`).
fn other_names(param: &super::kotlin::ast::Param) -> impl Iterator<Item = &str> {
    param
        .annotations
        .iter()
        .filter_map(|annotation| annotation.strip_prefix("JsonNames="))
        .flat_map(|names| names.split('\u{1f}'))
        .filter(|name| !name.is_empty())
}

/// The other JSON names DTO properties are read by, by property name.
fn json_aliases(files: &[(String, File)]) -> HashMap<String, Vec<String>> {
    let mut names: HashMap<String, Vec<String>> = HashMap::new();
    for class in all_classes(files) {
        for param in &class.params {
            for other in other_names(param) {
                let known = names.entry(param.name.clone()).or_default();
                if !known.iter().any(|name| name == other) {
                    known.push(other.to_owned());
                }
            }
        }
    }
    names
}

/// A DTO's required JSON fields, each with its other names: those neither optional (with a
/// default) nor nullable, as kotlinx.serialization's descriptor marks them.
fn required_fields(class: &ClassDecl) -> Vec<Vec<String>> {
    class
        .params
        .iter()
        .filter(|param| param.default.is_none() && !param.ty.nullable && param.property)
        .map(|param| {
            let serial = param
                .annotations
                .iter()
                .find_map(|annotation| annotation.strip_prefix("SerialName="))
                .unwrap_or(&param.name);
            std::iter::once(serial)
                .chain(other_names(param))
                .map(str::to_owned)
                .collect()
        })
        .collect()
}
