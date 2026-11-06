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
use super::kotlin::ast::{Body, ClassDecl, ClassKind, Expr, File, Member, Param, PropDecl};
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

/// Overrides that change nothing Raygun asks of a source: filters it doesn't show yet, related
/// titles, settings screens, deep links.
const SKIPPED: [&str; 10] = [
    "getFilterList",
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

fn param_names(params: &[Param]) -> Vec<String> {
    params.iter().map(|param| param.name.clone()).collect()
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
        Some(Expr::Call {
            callee,
            lambda: Some(lambda),
            args,
            ..
        }) if args.is_empty() && matches!(callee.as_ref(), Expr::Name(name) if name == "lazy") => {
            Some(Body::Block(lambda.body.clone()))
        }
        Some(other) => Some(Body::Expr(other.clone())),
        None => None,
    }
}

/// What the class and its files declare, for the generator.
fn context(class: &ClassDecl, files: &[(String, File)]) -> Context {
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
    }
    add_members(&mut cx, &class.members, false);
    for member in &class.members {
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
    fn classes(members: &[Member], skip: &ClassDecl, cx: &mut Context) {
        for member in members {
            if let Member::Class(inner) = member {
                if std::ptr::eq(inner, skip) {
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
                }
                for nested in &inner.members {
                    if matches!(nested, Member::Fun(_) | Member::Prop(_)) {
                        cx.class_members.insert(nested.name().to_owned());
                    }
                }
                classes(&inner.members, skip, cx);
            }
        }
    }
    for (_, file) in files {
        classes(&file.members, class, &mut cx);
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
            Member::TypeAlias(_) => {}
            Member::Class(inner) if inner.modifiers.iter().any(|m| m == "companion") => {
                self::members(writer, &inner.members, out, "C", true, reached)?;
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
                if fun.receiver.is_some() {
                    out.push(format!("self.extension({}, {function});", quote(&fun.name)));
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

/// A class the source declares, with its companion's values on `C` (as `Class.name`): its code
/// reads them by their plain names.
fn class_code(writer: &mut Gen<'_>, class: &ClassDecl) -> Result<String, String> {
    let outer = writer.class_scope.replace(class.name.clone());
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
        code.push(class_definition(writer, class)?);
        Ok(code.join(" "))
    })();
    writer.class_scope = outer;
    result
}

/// A class the source declares (a filter, a DTO, an enum): `D.Name`, made by `KT.make`.
fn class_definition(writer: &mut Gen<'_>, class: &ClassDecl) -> Result<String, String> {
    let params: Vec<String> = class
        .params
        .iter()
        .map(|param| quote(&param.name))
        .collect();
    let mut defaults = Vec::new();
    for param in &class.params {
        if let Some(default) = &param.default {
            defaults.push(format!(
                "{}: async () => {}",
                quote(&param.name),
                writer.expr(default)?
            ));
        }
    }
    // Each field's type, for JSON decoded into the class (`List<ChapterDto>`).
    fn type_text(ty: &super::kotlin::ast::Type) -> String {
        let name = ty.name.rsplit('.').next().unwrap_or(&ty.name);
        if ty.args.is_empty() {
            name.to_owned()
        } else {
            format!(
                "{name}<{}>",
                ty.args.iter().map(type_text).collect::<Vec<_>>().join(",")
            )
        }
    }
    let types: Vec<String> = class
        .params
        .iter()
        .map(|param| format!("{}: {}", quote(&param.name), quote(&type_text(&param.ty))))
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
    for member in &class.members {
        match member {
            Member::Fun(fun) if fun.body.is_some() => {
                let mut method = fun.clone();
                // Members read the instance as their receiver.
                method.receiver = Some(super::kotlin::ast::Type {
                    name: class.name.clone(),
                    ..Default::default()
                });
                methods.push(format!(
                    "{}: {}",
                    quote(&fun.name),
                    writer
                        .function(&method)
                        .map_err(|reason| format!("{}.{}: {reason}", class.name, fun.name))?
                ));
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
                    methods.push(format!(
                        "{}: KT.getter({})",
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
    let kind = match class.kind {
        ClassKind::Enum => "enum",
        ClassKind::Object => "object",
        ClassKind::Interface => "interface",
        ClassKind::Class => "class",
    };
    Ok(format!(
        "D.{name} = KT.defineClass({{ name: {quoted}, kind: {kind:?}, typeParams: [{type_params}], params: [{params}], types: {{{types}}}, defaults: {{{defaults}}}, supers: [{supers}], superArgs: {super_args}, entries: [{entries}], members: {{{methods}}} }});",
        name = class.name,
        quoted = quote(&class.name),
        type_params = class
            .type_params
            .iter()
            .map(|name| quote(name))
            .collect::<Vec<_>>()
            .join(", "),
        params = params.join(", "),
        types = types.join(", "),
        defaults = defaults.join(", "),
        supers = supers.join(", "),
        entries = entries.join(", "),
        methods = methods.join(", "),
    ))
}

/// Converts a theme-less extension; the error says why it can't be.
pub fn convert(extension: &StoreExtension, files: &[(String, File)]) -> Result<Converted, String> {
    let class = source_class(files).ok_or("Raygun couldn't find its source class")?;
    let base = class
        .supers
        .iter()
        .find_map(|parent| parent.ty.name.rsplit('.').next().map(str::to_owned))
        .unwrap_or_default();
    if base != "KeiSource" {
        return Err(format!(
            "it's built on {base}, which Raygun's converter doesn't read yet"
        ));
    }
    let defined: HashSet<&str> = class.members.iter().map(Member::name).collect();
    if let Some(missing) = NEEDED.iter().find(|name| !defined.contains(**name)) {
        return Err(format!("it has no {missing} of its own"));
    }
    let cx = context(class, files);
    let reached = reachable(&member_mentions(class, files));
    let mut writer = Gen::new(&cx);
    let mut out = Vec::new();
    // Declarations of the source's other files and of the file around the class.
    for (_, file) in files {
        let others: Vec<Member> = file
            .members
            .iter()
            .filter(|member| !matches!(member, Member::Class(inner) if std::ptr::eq(inner, class)))
            .cloned()
            .collect();
        members(&mut writer, &others, &mut out, "C", true, &reached)?;
    }
    members(
        &mut writer,
        &class.members,
        &mut out,
        "self",
        false,
        &reached,
    )?;
    // A value read as `json` decodes with names the DTOs spell (`@SerialName`).
    let renames = serial_names(files);
    let code = format!(
        "// {name}, converted by Raygun from the Mihon extension {pkg} (Apache-2.0).\n'use strict';\nmihon.source({{\n  sources: {sources},\n  renames: {renames},\n  build: (self, C, D) => {{\n{body}\n  }},\n}});\n",
        name = extension.name.replace(['\n', '\r'], " "),
        pkg = extension.pkg,
        sources = sources_json(extension),
        renames = serde_json::to_string(&renames).unwrap_or_else(|_| "{}".into()),
        body = out.join("\n"),
    );
    Ok(Converted { theme: None, code })
}

/// Each member of the class, its companion, its files and their classes, with the names its code
/// mentions; a class stands for all its members.
fn member_mentions(class: &ClassDecl, files: &[(String, File)]) -> Vec<(String, HashSet<String>)> {
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
                Member::TypeAlias(_) => continue,
            }
            // A class's member reached reaches its class too.
            if let Some(owner) = owner {
                mentioned.insert(owner.to_owned());
            }
            found.push((member.name().to_owned(), mentioned));
        }
    }
    add(&class.members, None, &mut found);
    for (_, file) in files {
        let others: Vec<Member> = file
            .members
            .iter()
            .filter(|member| !matches!(member, Member::Class(inner) if std::ptr::eq(inner, class)))
            .cloned()
            .collect();
        add(&others, None, &mut found);
    }
    found
}

/// The JSON names DTO properties are read by (`@SerialName("manga_id") val id`), by property name.
fn serial_names(files: &[(String, File)]) -> HashMap<String, String> {
    let mut names = HashMap::new();
    let mut visit = |class: &ClassDecl| {
        for param in &class.params {
            for annotation in &param.annotations {
                if let Some(serial) = annotation.strip_prefix("SerialName=") {
                    names.insert(param.name.clone(), serial.to_owned());
                }
            }
        }
    };
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
    for class in classes {
        visit(class);
    }
    names
}
