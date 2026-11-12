//! Nuclear plugin compatibility (ARCHITECTURE §7.4, M17): unpacks a plugin zip and runs the
//! converter (`packages/nuclear-compat`, built into `assets/nuclear-converter.js`) in a throwaway
//! QuickJS runtime. The result is an ordinary Raygun extension: a manifest and one script.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};

use serde::Deserialize;

use crate::package::{MAX_PACKAGE_BYTES, PackageError};

const CONVERTER: &str = include_str!("../assets/nuclear-converter.js");
/// The converter's `CONVERTER_VERSION`: installed plugins converted by an older one are offered
/// again as updates (M46.9.1). Conversions from before it was stamped count as 1.
pub const CONVERTER_VERSION: u32 = 7;
/// Sucrase's parser recurses deeply on large files; give it room.
const CONVERTER_STACK_BYTES: usize = 16 * 1024 * 1024;
const CONVERTER_MEMORY_BYTES: usize = 256 * 1024 * 1024;
/// Text files the converter may need; images, fixtures and the like are skipped.
const SOURCE_SUFFIXES: &[&str] = &[".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".json"];

/// What the converter reports about a plugin (see `CompatReport` in packages/nuclear-compat).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub supported: Vec<String>,
    pub unsupported_categories: Vec<String>,
    pub stubbed_apis: Vec<String>,
    pub ui_imports: Vec<String>,
    pub hosts: Vec<String>,
    pub modules: u32,
}

#[derive(Debug, Clone)]
pub struct Converted {
    /// `raygun-extension.json` contents.
    pub manifest: String,
    /// The extension's `dist/index.js`.
    pub bundle: String,
    pub report: Report,
}

/// What Nuclear plugins that give Raygun nothing to call do in Nuclear, by words in their name,
/// description or tags; the first that matches says it ([`purpose`]).
const PURPOSES: &[(&[&str], &str)] = &[
    (
        &["mini player", "miniplayer", "mini-player"],
        "Adds a mini player to Nuclear; Raygun has its own",
    ),
    (&["discord"], "Shows what Nuclear plays on Discord"),
    (
        &["equalizer", "equaliser"],
        "An equalizer for Nuclear; Raygun has its own in Settings → Sound",
    ),
    (
        &["visualizer", "visualiser", "visualization", "visualisation"],
        "Draws a visualizer in Nuclear's player",
    ),
    (
        &["sleep timer"],
        "Adds a sleep timer to Nuclear; Raygun's player has its own",
    ),
    (
        &["shortcut", "hotkey", "media key", "keybind"],
        "Adds shortcuts to Nuclear's player; Raygun has its own",
    ),
    (
        &["notification"],
        "Shows notifications from Nuclear; Raygun has its own",
    ),
    (
        &["theme", "skin"],
        "Changes how Nuclear looks; Raygun has its own look",
    ),
    (
        &["mpris", "remote control"],
        "Lets other apps control Nuclear; Raygun has its own remote",
    ),
];

/// Services a Nuclear scrobbler may send plays to, by how its name or description writes them.
const SCROBBLE_SERVICES: &[(&str, &str)] = &[
    ("last.fm", "Last.fm"),
    ("lastfm", "Last.fm"),
    ("listenbrainz", "ListenBrainz"),
    ("libre.fm", "Libre.fm"),
    ("librefm", "Libre.fm"),
    ("maloja", "Maloja"),
];

/// What a Nuclear plugin that gives Raygun nothing to call does in Nuclear, as the catalogue
/// says why it's not for Raygun: a scrobbler names its service, then words in its name,
/// description and tags pick one of [`PURPOSES`].
pub fn purpose(
    name: &str,
    description: Option<&str>,
    categories: &[String],
    tags: &[String],
) -> String {
    let text = [name, description.unwrap_or_default()]
        .into_iter()
        .chain(tags.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    if categories.iter().any(|category| category == "scrobbling") || text.contains("scrobbl") {
        return match SCROBBLE_SERVICES
            .iter()
            .find(|(written, _)| text.contains(written))
        {
            Some((_, service)) => {
                format!("Scrobbles what Nuclear plays to {service}; Raygun doesn't scrobble yet")
            }
            None => "Scrobbles what Nuclear plays; Raygun doesn't scrobble yet".to_owned(),
        };
    }
    PURPOSES
        .iter()
        .find(|(words, _)| words.iter().any(|word| text.contains(word)))
        .map_or(
            "Changes Nuclear's own player or screens; nothing Raygun can call",
            |(_, purpose)| purpose,
        )
        .to_owned()
}

/// How Nuclear plugins register providers in their code (as the converter finds them).
static REGISTERED: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(
        r#"\bkind\s*:\s*['"](metadata|streaming|lyrics|dashboard|discovery|playlists)['"]"#,
    )
    .expect("a valid pattern")
});

/// Why a plugin, read from its unzipped sources before it's converted, isn't for Raygun: none of
/// the categories its `package.json` names, nor the provider kinds its code registers, is one
/// Raygun runs (as the converter decides). `None` when one is, or when it isn't a plugin the
/// converter would read at all (the converter says why).
pub fn not_for_raygun(files: &BTreeMap<String, String>) -> Option<String> {
    let package_path = files
        .keys()
        .filter(|path| *path == "package.json" || path.ends_with("/package.json"))
        .filter(|path| !path.contains("node_modules/"))
        .min_by_key(|path| path.matches('/').count())?;
    let root = &package_path[..package_path.len() - "package.json".len()];
    let package: serde_json::Value = serde_json::from_str(files.get(package_path)?).ok()?;
    let nuclear = package.get("nuclear");
    let text = |value: Option<&serde_json::Value>| {
        value.and_then(serde_json::Value::as_str).map(str::to_owned)
    };
    let texts = |value: Option<&serde_json::Value>| -> Vec<String> {
        value
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    let categories = match nuclear.and_then(|nuclear| nuclear.get("categories")) {
        Some(listed) => texts(Some(listed)),
        None => text(nuclear.and_then(|nuclear| nuclear.get("category")))
            .into_iter()
            .collect(),
    };
    let runs = |category: &str| crate::repo::nuclear_kind(category).is_some();
    if categories.iter().any(|category| runs(category)) {
        return None;
    }
    let registers = files.iter().any(|(path, source)| {
        path.starts_with(root)
            && !path.contains("node_modules/")
            && SOURCE_SUFFIXES
                .iter()
                .any(|suffix| *suffix != ".json" && path.ends_with(suffix))
            && REGISTERED
                .captures_iter(source)
                .any(|found| runs(&found[1]))
    });
    if registers {
        return None;
    }
    let name = text(nuclear.and_then(|nuclear| nuclear.get("displayName")))
        .or_else(|| text(package.get("name")))
        .unwrap_or_default();
    Some(purpose(
        &name,
        text(package.get("description")).as_deref(),
        &categories,
        &texts(package.get("keywords")),
    ))
}

/// Text sources of a plugin zip, by path. Paths are checked like Raygun packages (no absolute
/// paths, no `..`) and the total is capped.
pub fn unzip_sources(bytes: &[u8]) -> Result<BTreeMap<String, String>, PackageError> {
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(PackageError::TooLarge);
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| PackageError::Zip(error.to_string()))?;
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| PackageError::Zip(error.to_string()))?;
        if file.is_dir() || file.is_symlink() {
            continue;
        }
        let Some(path) = file.enclosed_name() else {
            let name = file
                .name()
                .map(|name| name.into_owned())
                .unwrap_or_default();
            return Err(PackageError::UnsafePath(name));
        };
        let path = path
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        if !SOURCE_SUFFIXES.iter().any(|suffix| path.ends_with(suffix)) {
            continue;
        }
        let mut contents = Vec::new();
        (&mut file)
            .take(MAX_PACKAGE_BYTES.saturating_sub(total) + 1)
            .read_to_end(&mut contents)
            .map_err(|error| PackageError::Zip(error.to_string()))?;
        total += contents.len() as u64;
        if total > MAX_PACKAGE_BYTES {
            return Err(PackageError::TooLarge);
        }
        files.insert(path, String::from_utf8_lossy(&contents).into_owned());
    }
    Ok(files)
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ConverterAnswer {
    Error {
        error: String,
    },
    Converted {
        manifest: serde_json::Value,
        bundle: String,
        report: Report,
    },
}

/// Converts plugin sources into a Raygun extension. Blocking: call it off the async runtime.
pub fn convert(files: &BTreeMap<String, String>) -> Result<Converted, String> {
    let input = serde_json::to_string(files).map_err(|error| error.to_string())?;
    let worker = std::thread::Builder::new()
        .name("raygun-nuclear-convert".to_owned())
        .stack_size(CONVERTER_STACK_BYTES)
        .spawn(move || run_converter(&input))
        .map_err(|error| error.to_string())?;
    let output = worker
        .join()
        .map_err(|_| "the Nuclear converter crashed".to_owned())??;
    match serde_json::from_str::<ConverterAnswer>(&output).map_err(|error| error.to_string())? {
        ConverterAnswer::Error { error } => Err(error),
        ConverterAnswer::Converted {
            manifest,
            bundle,
            report,
        } => Ok(Converted {
            manifest: manifest.to_string(),
            bundle,
            report,
        }),
    }
}

/// Most random bytes one ask gives (Web Crypto's `getRandomValues` limit).
const MAX_RANDOM_BYTES: usize = 65_536;

/// Settings a plugin may register (`Settings.register`), at most.
const MAX_SETTINGS: usize = 40;

/// A Nuclear setting definition as the shim passes it on (`shim/api.ts`): Raygun's setting
/// shape, with `required` for what the plugin can't work without.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisteredSetting {
    key: String,
    #[serde(rename = "type")]
    kind: crate::manifest::SettingKind,
    label: Option<String>,
    #[serde(default)]
    options: Vec<String>,
    default: Option<serde_json::Value>,
    #[serde(default)]
    required: bool,
}

/// Adds a setting the plugin registered as it loaded, so people can fill it in on its page
/// (Nuclear sets them in its own settings screen). Invalid ones are refused.
fn register_setting(
    registered: &std::cell::RefCell<Vec<crate::manifest::SettingDef>>,
    json: &str,
) -> Result<(), String> {
    use crate::manifest::{SettingDef, SettingKind};

    let raw: RegisteredSetting = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let key = raw.key.trim();
    if key.is_empty() || key.len() > 64 {
        return Err(format!("not a setting key: {key:?}"));
    }
    let setting = SettingDef {
        key: key.to_owned(),
        required: raw.required && raw.kind != SettingKind::Boolean,
        kind: raw.kind,
        label: raw
            .label
            .map(|label| label.trim().chars().take(80).collect::<String>())
            .filter(|label| !label.is_empty()),
        options: raw.options,
        default: None,
    };
    let default = raw.default.filter(|value| setting.accepts(value));
    if setting.kind == SettingKind::Select && setting.options.is_empty() {
        return Err(format!("the setting {key} has no options"));
    }
    let mut settings = registered.borrow_mut();
    if settings.len() >= MAX_SETTINGS {
        return Err("too many settings".to_owned());
    }
    if !settings.iter().any(|known| known.key == setting.key) {
        settings.push(SettingDef { default, ..setting });
    }
    Ok(())
}

/// What the shim gets beyond the `raygun` API, as `__rgNuclear` (taken and removed before the
/// plugin runs): documents parsed and queried here (`DOMParser`), digests and HMACs, random
/// bytes (`crypto`), and the plugin's settings, shown on its page (`registerSetting`).
pub(crate) fn install_natives(
    ctx: &rquickjs::Ctx<'_>,
    registered: std::rc::Rc<std::cell::RefCell<Vec<crate::manifest::SettingDef>>>,
) -> rquickjs::Result<()> {
    use rquickjs::{Function, Object};

    use crate::js::{err, ok};

    let envelope = |result: Result<serde_json::Value, String>| match result {
        Ok(value) => ok(value),
        Err(message) => err(message),
    };
    let natives = Object::new(ctx.clone())?;
    natives.set(
        "registerSetting",
        Function::new(ctx.clone(), move |json: String| {
            envelope(register_setting(&registered, &json).map(|()| serde_json::Value::Null))
        })?,
    )?;
    let documents = std::rc::Rc::new(crate::lnreader::dom::Documents::default());
    natives.set(
        "dom",
        Function::new(ctx.clone(), move |op: String, args: String| {
            envelope(documents.run(&op, &args))
        })?,
    )?;
    natives.set(
        "crypto",
        Function::new(ctx.clone(), move |args: String| {
            envelope(crate::mihon::runtime::crypto(&args).map(serde_json::Value::String))
        })?,
    )?;
    natives.set(
        "random",
        Function::new(ctx.clone(), move |count: f64| {
            use base64::Engine as _;
            let count = if count.is_finite() {
                (count.max(0.0) as usize).min(MAX_RANDOM_BYTES)
            } else {
                0
            };
            let mut bytes = vec![0; count];
            envelope(
                getrandom::fill(&mut bytes)
                    .map(|()| {
                        serde_json::Value::String(
                            base64::engine::general_purpose::STANDARD.encode(&bytes),
                        )
                    })
                    .map_err(|error| format!("no random bytes: {error}")),
            )
        })?,
    )?;
    ctx.globals().set("__rgNuclear", natives)
}

fn run_converter(input: &str) -> Result<String, String> {
    use rquickjs::{CatchResultExt, Context, Function, Runtime};
    let runtime = Runtime::new().map_err(|error| error.to_string())?;
    runtime.set_memory_limit(CONVERTER_MEMORY_BYTES);
    runtime.set_max_stack_size(CONVERTER_STACK_BYTES - 1024 * 1024);
    let context = Context::full(&runtime).map_err(|error| error.to_string())?;
    context.with(|ctx| {
        ctx.eval::<(), _>(CONVERTER)
            .catch(&ctx)
            .map_err(|error| format!("Nuclear converter failed to start: {error}"))?;
        let convert: Function = ctx
            .globals()
            .get("__convert")
            .map_err(|error| error.to_string())?;
        convert
            .call::<_, String>((input,))
            .catch(&ctx)
            .map_err(|error| error.to_string())
    })
}
