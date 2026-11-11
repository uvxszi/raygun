//! Converting a Mihon extension's source code into a Raygun extension of runtime `mihon`.
//!
//! Most of Keiyoushi's extensions are a theme (`lib-multisrc`: Madara, MangaThemesia…) and a
//! handful of values: a path, a selector, a date format, the theme's variant. Raygun's runtime has
//! the themes written in JavaScript (`mihon/themes/*.js`, ported from Keiyoushi's Kotlin, Apache
//! 2.0), so converting such an extension is reading its values: the class it extends, what it
//! overrides with a value (`override val mangaSubString = "comic"`), the constants those use. An
//! override with code of its own (a parser, an interceptor, a WebView) isn't something Raygun
//! can read faithfully: the extension is then listed as one that can't be converted, with the
//! reason. Extensions built on no theme go to the transpiler (`crate::mihon::transpile`).

use std::collections::{BTreeSet, HashMap};

use serde_json::json;

use super::gradle;
use super::index::{ContentWarning, StoreExtension, source_hosts};
use super::kotlin::ast::{Arg, Body, ClassDecl, ClassKind, Expr, File, Member, Stmt};
use super::kotlin::{js, parse_file};
use super::source::Sources;
use crate::manga::MANGA_SOURCE;
use crate::manifest::API_VERSION;
use crate::package::{PackageError, VerifiedPackage, from_files};

/// Bumped when conversions change in a way installed sources should get: the catalogue then
/// offers them again.
pub const CONVERTER_VERSION: u32 = 5;
/// The converted script in the extension folder.
pub const ENTRY: &str = "index.js";

/// A theme Raygun runs: the Kotlin classes sources extend, and what they may set.
pub struct Theme {
    pub name: &'static str,
    /// Classes of the theme a source extends, with the variant each is.
    pub classes: &'static [(&'static str, &'static str)],
    /// Overrides whose value the JavaScript theme reads.
    pub options: &'static [&'static str],
    /// Overrides that change nothing Raygun does (related titles, settings screens, the
    /// WebView's address).
    pub ignored: &'static [&'static str],
    /// An older build's constructor parameters, in order (`Madara("Name", "https://…", "en")`).
    pub constructor: &'static [&'static str],
}

const IGNORED_EVERYWHERE: [&str; 11] = [
    "supportsRelatedMangas",
    "supportRelatedMangasBySearch",
    "fetchRelatedMangaList",
    "setupPreferenceScreen",
    "getMangaUrl",
    "getChapterUrl",
    "getHomeUrl",
    "versionId",
    "getMangaByUrl",
    "getMangasByUrl",
    "imageFetch",
];

pub const THEMES: [Theme; 2] = [
    Theme {
        name: "madara",
        classes: &[("Madara", "ajax"), ("MadaraNoAjax", "noAjax")],
        options: &[
            "supportsPostId",
            "chapterMode",
            "mangaSubString",
            "genreDirectory",
            "filterNonMangaItems",
            "sendViewCount",
            "archiveSelector",
            "searchCardSelector",
            "archiveTitleSelector",
            "archiveUrlSelector",
            "mangaDetailsSelectorTitle",
            "mangaDetailsSelectorAuthor",
            "mangaDetailsSelectorArtist",
            "mangaDetailsSelectorStatus",
            "mangaDetailsSelectorDescription",
            "mangaDetailsSelectorThumbnail",
            "mangaDetailsSelectorGenre",
            "mangaDetailsSelectorTag",
            "seriesTypeSelector",
            "altNameSelector",
            "updatingRegex",
            "chapterListSelector",
            "chapterNameSelector",
            "chapterUrlSelector",
            "chapterDateSelector",
            "pageListParseSelector",
            "filterGenresSelector",
            "chapterDateFormat",
            "completedStatus",
            "ongoingStatus",
            "hiatusStatus",
            "cancelledStatus",
            "statusFilterOptions",
            "orderByFilterOptions",
            "genreConditionFilterOptions",
            "adultFilterOptions",
            "pageSize",
            "ajaxTemplate",
            "nextPageSelector",
            "orderQueryParameter",
            "searchQueryParameter",
            "supportsLatest",
            "supportsFilterFetching",
        ],
        ignored: &[],
        constructor: &["name", "baseUrl", "lang", "chapterDateFormat"],
    },
    Theme {
        name: "mangathemesia",
        // `MangaThemesiaAlt` sites put a number that changes before each slug.
        classes: &[("MangaThemesia", "default"), ("MangaThemesiaAlt", "alt")],
        options: &[
            "listUrl",
            "listSelector",
            "slugRegex",
            "mangaUrlDirectory",
            "datePattern",
            "dateFormat",
            "projectPageString",
            "searchMangaSelector",
            "searchMangaNextPageSelector",
            "seriesDetailsSelector",
            "seriesTitleSelector",
            "seriesArtistSelector",
            "seriesAuthorSelector",
            "seriesDescriptionSelector",
            "seriesAltNameSelector",
            "seriesGenreSelector",
            "seriesTypeSelector",
            "seriesStatusSelector",
            "seriesThumbnailSelector",
            "altNamePrefix",
            "chapterListSelector",
            "pageSelector",
            "sendViewCount",
            "statusOptions",
            "typeFilterOptions",
            "orderByFilterOptions",
            "projectFilterOptions",
            "hasProjectPage",
            "supportsFilterFetching",
            "supportsLatest",
        ],
        ignored: &[],
        constructor: &["name", "baseUrl", "lang", "mangaUrlDirectory", "dateFormat"],
    },
];

/// Keiyoushi's libraries the runtime has itself: `lib:i18n`'s messages come with the converted
/// code; `lib:textinterceptor` (text drawn as a page) and `lib:imagedecoder` (pictures decoded) are
/// the runtime's own (`mihon/kotlin.js`).
const PROVIDED_LIBRARIES: [&str; 3] = ["lib:i18n", "lib:textinterceptor", "lib:imagedecoder"];

/// The theme called `name`, when Raygun runs it.
pub fn theme(name: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|theme| theme.name == name)
}

/// A converted extension: its script and what it was made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converted {
    pub theme: Option<String>,
    pub code: String,
}

/// Converts an extension; the error is the reason it can't be, in a sentence.
pub fn convert(extension: &StoreExtension, sources: &Sources) -> Result<Converted, String> {
    let build = sources
        .build_file()
        .map(gradle::read)
        .ok_or("its folder has no build file")?;
    if let Some(library) = build.libraries.iter().find(|library| {
        !PROVIDED_LIBRARIES.contains(&library.as_str())
            && !sources.library_files.contains_key(library.as_str())
    }) {
        return Err(format!(
            "it builds on {library}, code Raygun doesn't convert"
        ));
    }
    let mut files = Vec::new();
    for (path, text) in sources.kotlin() {
        let file = parse_file(text)
            .map_err(|error| format!("Raygun couldn't read its code ({path}: {error})"))?;
        files.push((path.clone(), file));
    }
    // The libraries' code is the extension's own, as it's compiled into it.
    for (path, text) in sources.library_kotlin() {
        let file = parse_file(text)
            .map_err(|error| format!("Raygun couldn't read its code ({path}: {error})"))?;
        files.push((path, file));
    }
    if files.is_empty() {
        return Err("its folder has no Kotlin code".into());
    }
    let Some(name) = build.theme.as_deref() else {
        return super::transpile::convert(
            extension,
            &files,
            None,
            &sources.messages(),
            &sources.assets,
        );
    };
    // A theme Raygun runs by hand first; else the theme's own code, converted with the
    // extension's, when it was read.
    let by_hand = theme(name).map(|theme| convert_themed(extension, theme, &files));
    if let Some(Ok(converted)) = by_hand {
        return Ok(converted);
    }
    if sources.theme_files.is_empty() {
        return Err(match by_hand {
            Some(Err(reason)) => reason,
            _ => format!("it's built on the {name} theme, which Raygun doesn't convert yet"),
        });
    }
    if let Some(library) = sources
        .theme_build_file()
        .map(gradle::read)
        .and_then(|theme_build| {
            theme_build.libraries.into_iter().find(|library| {
                !PROVIDED_LIBRARIES.contains(&library.as_str())
                    && !sources.library_files.contains_key(library.as_str())
            })
        })
    {
        return Err(format!(
            "its theme, {name}, builds on {library}, code Raygun doesn't convert"
        ));
    }
    let mut theme_files = Vec::new();
    for (path, text) in sources.theme_kotlin() {
        let file = parse_file(text).map_err(|error| {
            format!("Raygun couldn't read the {name} theme's code ({path}: {error})")
        })?;
        theme_files.push((path.clone(), file));
    }
    let converted = super::transpile::convert(
        extension,
        &files,
        Some((name, &theme_files)),
        &sources.messages(),
        &sources.assets,
    )?;
    Ok(Converted {
        theme: Some(name.to_owned()),
        ..converted
    })
}

/// Keiyoushi's libraries converting `sources` would read, when they haven't been read yet: those
/// the extension's build, its theme's and the libraries read so far name, which the runtime
/// doesn't have itself.
pub fn libraries_to_read(sources: &Sources) -> Vec<String> {
    let builds = sources
        .build_file()
        .into_iter()
        .chain(sources.theme_build_file())
        .chain(sources.library_files.values().filter_map(|files| {
            files
                .get("build.gradle.kts")
                .or_else(|| files.get("build.gradle"))
                .map(String::as_str)
        }));
    let mut wanted: Vec<String> = Vec::new();
    for build in builds {
        for library in gradle::read(build).libraries {
            if library.starts_with("lib:")
                && !PROVIDED_LIBRARIES.contains(&library.as_str())
                && !sources.library_files.contains_key(&library)
                && !wanted.contains(&library)
            {
                wanted.push(library);
            }
        }
    }
    wanted
}

/// The theme whose code converting `sources` would read, when it hasn't been read yet: a
/// themed extension Raygun's own port of the theme refuses, or one on a theme it has no port of,
/// may still convert with it.
pub fn theme_to_read(sources: &Sources) -> Option<String> {
    if !sources.theme_files.is_empty() {
        return None;
    }
    sources.build_file().map(gradle::read)?.theme
}

/// The source class of a themed extension: the one marked `@Source`, else the one extending a
/// class of the theme.
fn source_class<'a>(
    theme: &Theme,
    files: &'a [(String, File)],
) -> Option<(&'a ClassDecl, &'a File)> {
    let classes = files.iter().flat_map(|(_, file)| {
        file.members.iter().filter_map(move |member| match member {
            Member::Class(class) => Some((class, file)),
            _ => None,
        })
    });
    let mut fallback = None;
    for (class, file) in classes {
        let extends_theme = class.supers.iter().any(|parent| {
            theme
                .classes
                .iter()
                .any(|(name, _)| *name == short(&parent.ty.name))
        });
        if class.annotations.iter().any(|name| name == "Source") && extends_theme {
            return Some((class, file));
        }
        if extends_theme && fallback.is_none() {
            fallback = Some((class, file));
        }
    }
    fallback
}

fn short(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

/// Constants a class and its file declare: `const val`, and `val`s with a value, by name.
fn constants(class: &ClassDecl, file: &File) -> HashMap<String, Expr> {
    let mut found = HashMap::new();
    let mut add = |members: &[Member]| {
        for member in members {
            if let Member::Prop(prop) = member
                && !prop.mutable
                && prop.receiver.is_none()
                && !prop.modifiers.iter().any(|m| m == "override")
                && let Some(value) = &prop.value
            {
                found.insert(prop.name.clone(), value.clone());
            }
        }
    };
    add(&file.members);
    for member in &class.members {
        if let Member::Class(inner) = member
            && inner.kind == ClassKind::Object
            && inner.modifiers.iter().any(|m| m == "companion")
        {
            add(&inner.members);
        }
    }
    add(&class.members);
    found
}

/// What an override's value is, as an expression: a property's value or getter, or a function
/// without parameters that returns one.
fn override_value(member: &Member) -> Option<&Expr> {
    let body = match member {
        Member::Prop(prop) => {
            if prop.receiver.is_some() {
                return None;
            }
            if let Some(value) = &prop.value {
                return Some(value);
            }
            if let Some(Expr::Call {
                callee,
                lambda: Some(lambda),
                ..
            }) = &prop.delegate
                && matches!(callee.as_ref(), Expr::Name(name) if name == "lazy")
                && let [Stmt::Expr(value)] = lambda.body.as_slice()
            {
                return Some(value);
            }
            prop.getter.as_ref()?
        }
        Member::Fun(fun) => {
            if fun.receiver.is_some() || !fun.params.is_empty() {
                return None;
            }
            fun.body.as_ref()?
        }
        _ => return None,
    };
    match body {
        Body::Expr(value) => Some(value),
        Body::Block(block) => match block.as_slice() {
            [
                Stmt::Expr(Expr::Return {
                    value: Some(value),
                    label: None,
                }),
            ] => Some(value),
            _ => None,
        },
    }
}

/// The calls of an expression chain or `apply { }` block: `rateLimit(2).rateLimitHost(…)`.
fn builder_calls<'a>(expr: &'a Expr, calls: &mut Vec<(&'a str, &'a [Arg])>) -> Result<(), String> {
    match expr {
        Expr::This(None) => Ok(()),
        Expr::Call {
            callee,
            args,
            lambda,
            ..
        } => {
            let (target, name) = match callee.as_ref() {
                Expr::Name(name) => (None, name.as_str()),
                Expr::Member { target, name, .. } => (Some(target.as_ref()), name.as_str()),
                _ => return Err("it builds its network client with code".into()),
            };
            if let Some(target) = target {
                builder_calls(target, calls)?;
            }
            if name == "apply" {
                let lambda = lambda
                    .as_ref()
                    .ok_or("it builds its network client with code")?;
                for statement in &lambda.body {
                    match statement {
                        Stmt::Expr(expr) => builder_calls(expr, calls)?,
                        _ => return Err("it builds its network client with code".into()),
                    }
                }
                return Ok(());
            }
            calls.push((name, args.as_slice()));
            Ok(())
        }
        _ => Err("it builds its network client with code".into()),
    }
}

/// A rate limit: (requests, milliseconds) from `rateLimit(2)`, `rateLimit(10, 1.seconds)`,
/// `rateLimit(permits = 3, period = 2)`.
fn rate_limit(args: &[Arg]) -> Option<(i64, i64)> {
    let value = |name: &str, position: usize| {
        args.iter()
            .find(|arg| arg.name.as_deref() == Some(name))
            .or_else(|| args.iter().filter(|arg| arg.name.is_none()).nth(position))
            .map(|arg| &arg.value)
    };
    let permits = match value("permits", 0)? {
        Expr::Int(permits) => *permits,
        _ => return None,
    };
    let period_ms = match value("period", 1) {
        None => 1000,
        Some(Expr::Int(seconds)) => seconds * 1000,
        // `1.seconds`, `500.milliseconds`
        Some(Expr::Member { target, name, .. }) => match (target.as_ref(), name.as_str()) {
            (Expr::Int(amount), "seconds") => amount * 1000,
            (Expr::Int(amount), "milliseconds") => *amount,
            (Expr::Int(amount), "minutes") => amount * 60_000,
            _ => return None,
        },
        _ => return None,
    };
    (permits > 0 && period_ms > 0).then_some((permits, period_ms))
}

/// The network settings `configureClient` makes: a rate limit, or nothing that matters here.
fn client_settings(member: &Member) -> Result<Option<(i64, i64)>, String> {
    let Member::Fun(fun) = member else {
        return Err("it builds its network client with code".into());
    };
    let expr = match &fun.body {
        Some(Body::Expr(expr)) => expr,
        Some(Body::Block(block)) => match block.as_slice() {
            [
                Stmt::Expr(Expr::Return {
                    value: Some(value), ..
                }),
            ] => value,
            _ => return Err("it builds its network client with code".into()),
        },
        None => return Ok(None),
    };
    let mut calls = Vec::new();
    builder_calls(expr, &mut calls)?;
    let mut limit = None;
    for (name, args) in calls {
        match name {
            "rateLimit" | "rateLimitHost" => {
                let args = if name == "rateLimitHost" && !args.is_empty() {
                    &args[1..]
                } else {
                    args
                };
                let found = rate_limit(args).ok_or("its rate limit isn't a plain number")?;
                // The strictest limit holds for every request.
                limit = Some(match limit {
                    Some((permits, period)) if permits * found.1 < found.0 * period => {
                        (permits, period)
                    }
                    _ => found,
                });
            }
            "connectTimeout" | "readTimeout" | "writeTimeout" | "callTimeout" => {}
            "addInterceptor" | "addNetworkInterceptor" => {
                return Err("it adds its own network code (an interceptor)".into());
            }
            other => return Err(format!("its network client calls {other}")),
        }
    }
    Ok(limit)
}

/// The headers `configureHeaders` adds: `add("Referer", "$baseUrl/")`, `set(…)`.
fn header_settings(
    member: &Member,
    scope: &js::Scope<'_>,
) -> Result<Vec<(String, String)>, String> {
    let Member::Fun(fun) = member else {
        return Err("it builds its headers with code".into());
    };
    let Some(Body::Expr(expr)) = &fun.body else {
        return Err("it builds its headers with code".into());
    };
    let mut calls = Vec::new();
    builder_calls(expr, &mut calls).map_err(|_| "it builds its headers with code".to_owned())?;
    let mut headers = Vec::new();
    for (name, args) in calls {
        match (name, args) {
            ("add" | "set", [header, value]) if header.name.is_none() && value.name.is_none() => {
                let Expr::Str(_) = &header.value else {
                    return Err("it builds its headers with code".into());
                };
                headers.push((
                    js::value(&header.value, scope)?,
                    js::value(&value.value, scope)?,
                ));
            }
            ("removeAll", [_]) => {}
            _ => return Err("it builds its headers with code".into()),
        }
    }
    Ok(headers)
}

fn convert_themed(
    extension: &StoreExtension,
    theme: &Theme,
    files: &[(String, File)],
) -> Result<Converted, String> {
    let (class, file) = source_class(theme, files).ok_or_else(|| {
        format!(
            "Raygun couldn't find its source class among the {} theme's",
            theme.name
        )
    })?;
    let parent = class
        .supers
        .iter()
        .find(|parent| {
            theme
                .classes
                .iter()
                .any(|(name, _)| *name == short(&parent.ty.name))
        })
        .ok_or("its source class extends no class of its theme")?;
    let variant = theme
        .classes
        .iter()
        .find(|(name, _)| *name == short(&parent.ty.name))
        .map_or("default", |(_, variant)| *variant);
    let constants = constants(class, file);
    let scope = js::Scope {
        constants: &constants,
        options: theme.options,
    };
    let mut overrides: Vec<(String, String)> = Vec::new();
    // An older build passes the theme's values to its constructor.
    for (position, arg) in parent.args.iter().flatten().enumerate() {
        let name = match &arg.name {
            Some(name) => name.as_str(),
            None => theme
                .constructor
                .get(position)
                .copied()
                .ok_or("its theme constructor takes more values than Raygun knows")?,
        };
        if matches!(name, "name" | "baseUrl" | "lang") {
            // The store says these, per source.
            continue;
        }
        if !theme.options.contains(&name) {
            return Err(format!(
                "it passes {name} to its theme, which Raygun doesn't read"
            ));
        }
        overrides.push((name.to_owned(), js::value(&arg.value, &scope)?));
    }
    let mut rate = None;
    let mut headers = Vec::new();
    for member in &class.members {
        match member {
            Member::Init(_) => return Err("it runs code of its own when it starts".into()),
            Member::Class(_) | Member::TypeAlias(..) => continue,
            _ => {}
        }
        if !member.is_override() {
            // Its own helpers: only its overrides change what the theme does.
            continue;
        }
        let name = member.name();
        if IGNORED_EVERYWHERE.contains(&name) || theme.ignored.contains(&name) {
            continue;
        }
        match name {
            "configureClient" => {
                rate = client_settings(member)?;
                continue;
            }
            "configureHeaders" => {
                headers = header_settings(member, &scope)?;
                continue;
            }
            "baseUrl" | "lang" | "name" | "id" => {
                return Err(format!("it works out its {name} with code"));
            }
            _ => {}
        }
        if !theme.options.contains(&name) {
            return Err(format!("it has its own code for {name}"));
        }
        let value =
            override_value(member).ok_or_else(|| format!("it has its own code for {name}"))?;
        let value = js::value(value, &scope).map_err(|reason| format!("{name}: {reason}"))?;
        overrides.push((name.to_owned(), value));
    }
    let code = script(extension, theme.name, variant, &overrides, rate, &headers);
    Ok(Converted {
        theme: Some(theme.name.to_owned()),
        code,
    })
}

/// The sources a converted extension registers, as the runtime takes them.
pub fn sources_json(extension: &StoreExtension) -> serde_json::Value {
    let languages: BTreeSet<&str> = extension
        .sources
        .iter()
        .map(|source| source.lang.as_str())
        .collect();
    json!(
        extension
            .sources
            .iter()
            .map(|source| {
                let name = if languages.len() > 1 {
                    format!("{} ({})", source.name, source.lang)
                } else {
                    source.name.clone()
                };
                json!({
                    "id": source.id,
                    "name": name,
                    "lang": source.lang,
                    "baseUrl": source.base_url.trim_end_matches('/'),
                })
            })
            .collect::<Vec<_>>()
    )
}

fn script(
    extension: &StoreExtension,
    theme: &str,
    variant: &str,
    overrides: &[(String, String)],
    rate: Option<(i64, i64)>,
    headers: &[(String, String)],
) -> String {
    let mut code = format!(
        "// {name}, converted by Raygun from the Mihon extension {pkg} (Apache-2.0).\n'use strict';\nmihon.theme({theme}, {{\n  variant: {variant},\n  sources: {sources},\n",
        name = extension.name.replace(['\n', '\r'], " "),
        pkg = extension.pkg,
        theme = json!(theme),
        variant = json!(variant),
        sources = sources_json(extension),
    );
    code.push_str("  overrides: {\n");
    for (name, value) in overrides {
        code.push_str(&format!("    {name}: (t) => {value},\n"));
    }
    code.push_str("  },\n");
    if let Some((permits, period)) = rate {
        code.push_str(&format!(
            "  rateLimit: {{ permits: {permits}, periodMs: {period} }},\n"
        ));
    }
    if !headers.is_empty() {
        code.push_str("  headers: (t) => [\n");
        for (name, value) in headers {
            code.push_str(&format!("    [{name}, {value}],\n"));
        }
        code.push_str("  ],\n");
    }
    code.push_str("});\n");
    code
}

/// Loads a converted package in a host of its own before it's installed: its script must run and
/// register every source. Returns why it didn't.
pub async fn validate(package: &VerifiedPackage, sources: usize) -> Result<(), String> {
    let files: Vec<(String, String)> = package
        .files()
        .iter()
        .map(|(name, bytes)| (name.clone(), String::from_utf8_lossy(bytes).into_owned()))
        .collect();
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(name, text)| (name.as_str(), text.as_str()))
        .collect();
    let host = crate::ExtensionHost::start(crate::HostConfig {
        lanes: 1,
        ..crate::HostConfig::default()
    })
    .map_err(|error| format!("the check couldn't start: {error}"))?;
    let infos = host
        .load(vec![crate::ExtensionSource::built_in(&borrowed)])
        .await;
    let info = infos
        .into_iter()
        .find(|info| info.id == package.manifest.id)
        .ok_or("the converted extension didn't load")?;
    if info.status != raygun_model::ExtensionStatus::Loaded {
        return Err(info
            .error
            .unwrap_or_else(|| "the converted extension didn't load".into()));
    }
    if info.providers.len() != sources {
        return Err(format!(
            "the converted extension registered {} of its {sources} sources",
            info.providers.len()
        ));
    }
    Ok(())
}

/// The extension folder for a converted extension.
pub fn package(
    extension: &StoreExtension,
    converted: &Converted,
    store: &str,
    folder: Option<&str>,
) -> Result<VerifiedPackage, PackageError> {
    let id = super::index::extension_id(&extension.pkg)
        .ok_or_else(|| PackageError::Manifest("its package name gives no extension id".into()))?;
    let website = extension
        .sources
        .iter()
        .map(|source| source.base_url.trim())
        .find(|url| url.starts_with("https://") || url.starts_with("http://"));
    let name: String = extension
        .name
        .trim_start_matches("Tachiyomi: ")
        .chars()
        .take(80)
        .collect();
    let manifest = json!({
        "id": id,
        "name": name,
        "version": super::index::semver_version(extension),
        "apiVersion": API_VERSION,
        "runtime": "mihon",
        "entry": ENTRY,
        "provides": [MANGA_SOURCE],
        "permissions": { "net": net_hosts(extension, &converted.code) },
        "contentRating": match extension.content {
            ContentWarning::Safe => "safe",
            ContentWarning::Mixed => "suggestive",
            ContentWarning::Nsfw => "nsfw",
        },
        "description": format!("{name}, converted from its Mihon extension."),
        "author": Some(store).filter(|store| !store.is_empty()),
        "website": website,
        "mihon": {
            "pkg": extension.pkg,
            "versionCode": extension.version_code,
            "converter": CONVERTER_VERSION,
            "theme": converted.theme,
            "source": folder,
        },
    });
    from_files(
        &manifest.to_string(),
        vec![(ENTRY.to_owned(), converted.code.clone().into_bytes())],
    )
}

/// The hosts a converted extension may reach: its sources' sites (and their subdomains), the
/// hosts its own code names in full addresses (an API beside the site, an image server), and,
/// as Mihon lets its extensions reach any address (a CDN the site's answers name, an API host
/// put together at run time), any public host (`*`), as Nuclear and Shosetsu conversions may.
fn net_hosts(extension: &StoreExtension, code: &str) -> Vec<String> {
    let mut hosts = named_hosts(extension, code);
    hosts.push("*".to_owned());
    hosts
}

fn named_hosts(extension: &StoreExtension, code: &str) -> Vec<String> {
    let mut hosts: std::collections::BTreeSet<String> =
        source_hosts(extension).into_iter().collect();
    for (at, _) in code.match_indices("://") {
        let scheme = code[..at]
            .rfind(|char: char| !char.is_ascii_alphabetic())
            .map_or(&code[..at], |index| &code[index + 1..at]);
        if scheme != "http" && scheme != "https" {
            continue;
        }
        let rest = &code[at + 3..];
        let end = rest
            .find(|char: char| !(char.is_ascii_alphanumeric() || char == '.' || char == '-'))
            .unwrap_or(rest.len());
        let host = rest[..end].trim_end_matches('.').to_ascii_lowercase();
        let covered = hosts.iter().any(|known| {
            known
                .strip_prefix("*.")
                .is_some_and(|parent| host.ends_with(&format!(".{parent}")))
        });
        if host.contains('.') && !covered && crate::permissions::HostPattern::parse(&host).is_some()
        {
            hosts.insert(host);
        }
    }
    hosts.into_iter().collect()
}
