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
pub const CONVERTER_VERSION: u32 = 5;
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

/// What the shim gets beyond the `raygun` API, as `__rgNuclear` (taken and removed before the
/// plugin runs): documents parsed and queried here (`DOMParser`), digests and HMACs, and random
/// bytes (`crypto`).
pub(crate) fn install_natives(ctx: &rquickjs::Ctx<'_>) -> rquickjs::Result<()> {
    use rquickjs::{Function, Object};

    use crate::js::{err, ok};

    let envelope = |result: Result<serde_json::Value, String>| match result {
        Ok(value) => ok(value),
        Err(message) => err(message),
    };
    let natives = Object::new(ctx.clone())?;
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
