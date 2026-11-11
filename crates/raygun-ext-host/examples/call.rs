//! Loads one extension folder into Raygun's extension host and calls one of its providers, as the
//! app would, so an extension can be tried without the app (`raygun-ext test` runs this).
//!
//! ```text
//! cargo run -p raygun-ext-host --example call -- <extension folder>
//!     [<provider> <method> [<arguments as JSON>]]
//!     [--set <key>=<value>]... [--nsfw] [--data <folder>]
//! ```
//!
//! A site-scripts extension (runtime `site`) is tried on a page's address instead of a provider:
//! which of its groups run there, and with `--bundle <file>` the script Raygun's browser would
//! inject (for a desktop browser's console).
//!
//! Without a provider it lists what the extension registered. The answer goes to stdout as JSON;
//! everything else goes to stderr: the extension's log, what Raygun keeps of the answer (the
//! app's own parsers read it) and how long the call took against the time the app gives it.
//! `--set` gives a setting a value (JSON, or else text), `--nsfw` turns NSFW content on and
//! `--data` keeps the extension's storage and cookies in a folder between runs.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use raygun_ext_host::review::{self, time_limit};
use raygun_ext_host::{
    ContentFilter, ExtensionHost, ExtensionSource, HostConfig, MANIFEST_FILE, Manifest,
    UserSettings,
};
use raygun_model::ExtensionStatus;
use serde_json::{Value, json};

const USAGE: &str = "usage: call <extension folder> [<provider> <method> [<arguments as JSON>]] \
                     [--set <key>=<value>]... [--nsfw] [--data <folder>]\n       \
                     call <site-scripts folder> <page address> [--set <key>=<value>]... \
                     [--bundle <file>]";

/// Longer than any limit the app sets, so a slow call still answers and says how slow it was.
const CALL_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Default)]
struct Options {
    dir: PathBuf,
    provider: Option<String>,
    method: Option<String>,
    args: Option<String>,
    settings: BTreeMap<String, Value>,
    nsfw: bool,
    data: Option<PathBuf>,
    /// Where a site-scripts extension's composed script is written.
    bundle: Option<PathBuf>,
}

/// A page's address, which site scripts are tried on.
fn is_address(text: &str) -> bool {
    text.starts_with("https://") || text.starts_with("http://")
}

fn parse(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut options = Options::default();
    let mut positional = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--nsfw" => options.nsfw = true,
            "--data" => {
                options.data = Some(args.next().ok_or("--data needs a folder")?.into());
            }
            "--bundle" => {
                options.bundle = Some(args.next().ok_or("--bundle needs a file")?.into());
            }
            "--set" => {
                let pair = args.next().ok_or("--set needs <key>=<value>")?;
                let (key, value) = pair
                    .split_once('=')
                    .ok_or_else(|| format!("--set {pair}: use <key>=<value>"))?;
                // `--set language=English` is text; `--set fullSize=true` is JSON.
                let value = serde_json::from_str(value).unwrap_or_else(|_| json!(value));
                options.settings.insert(key.to_owned(), value);
            }
            "-h" | "--help" => return Err(USAGE.to_owned()),
            flag if flag.starts_with("--") => {
                return Err(format!("unknown option {flag}\n{USAGE}"));
            }
            _ => positional.push(arg),
        }
    }
    let mut positional = positional.into_iter();
    options.dir = positional.next().ok_or(USAGE)?.into();
    options.provider = positional.next();
    options.method = positional.next();
    options.args = positional.next();
    if options.method.is_none()
        && options
            .provider
            .as_deref()
            .is_some_and(|word| !is_address(word))
    {
        return Err(format!("a provider needs a method to call\n{USAGE}"));
    }
    if let Some(extra) = positional.next() {
        return Err(format!(
            "unexpected {extra:?}: quote the arguments as one JSON value\n{USAGE}"
        ));
    }
    Ok(options)
}

/// What a call gets when no arguments are given: the first page of a list, else nothing.
fn default_args(method: &str) -> Value {
    match method {
        "popular" | "latest" => json!({ "page": 1 }),
        "charts" => json!({ "region": null }),
        "top" => json!({ "limit": 20 }),
        _ => json!({}),
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,raygun_ext_host::js=debug"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .without_time()
        .with_target(false)
        .init();
    match run(std::env::args().skip(1)).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    let options = parse(args)?;
    let manifest_path = options.dir.join(MANIFEST_FILE);
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|error| format!("can't read {}: {error}", manifest_path.display()))?;
    let manifest = Manifest::parse(&text).map_err(|error| format!("{MANIFEST_FILE}: {error}"))?;
    for (key, value) in &options.settings {
        manifest.check_setting(key, value)?;
    }

    let content = ContentFilter::default();
    content.set_show_nsfw(options.nsfw);
    let host = ExtensionHost::start(HostConfig {
        storage_dir: options.data.clone(),
        call_timeout: CALL_TIMEOUT,
        content,
        lanes: 1,
        ..HostConfig::default()
    })
    .map_err(|error| format!("the extension host didn't start: {error}"))?;
    let settings = UserSettings::from([(manifest.id.clone(), options.settings.clone())]);
    let started = Instant::now();
    let infos = host
        .load_with_settings(
            vec![ExtensionSource::installed(options.dir.clone())],
            settings,
        )
        .await;
    let info = infos
        .into_iter()
        .next()
        .ok_or("the extension host loaded nothing")?;
    if info.status != ExtensionStatus::Loaded {
        let error = info.error.unwrap_or_default();
        return Err(format!(
            "{} {} didn't load: {}",
            info.id,
            info.version,
            error.replace('\n', "\n  ")
        ));
    }
    eprintln!(
        "{} {} loaded in {} ms",
        info.id,
        info.version,
        started.elapsed().as_millis()
    );
    for provider in &info.providers {
        let local = provider
            .id
            .split_once('/')
            .map_or(provider.id.as_str(), |(_, local)| local);
        eprintln!("  {local}: {} \"{}\"", provider.kind, provider.name);
    }
    for setting in &info.settings {
        let value = setting
            .value
            .as_ref()
            .map_or_else(|| "null".to_owned(), |value| value.to_json().to_string());
        eprintln!("  setting {} = {value}", setting.key);
    }
    if let Some(scripts) = &manifest.site {
        return try_site(&host, &info.id, scripts, &options);
    }

    let (Some(provider), Some(method)) = (options.provider, options.method) else {
        return Ok(());
    };
    let full_id = if provider.contains('/') {
        provider
    } else {
        format!("{}/{provider}", info.id)
    };
    let kind = info
        .providers
        .iter()
        .find(|registered| registered.id == full_id)
        .map(|registered| registered.kind.clone())
        .ok_or_else(|| format!("{full_id} isn't one of the providers above"))?;
    let args = match &options.args {
        Some(text) => serde_json::from_str(text)
            .map_err(|error| format!("the arguments aren't JSON ({error}): {text}"))?,
        None => default_args(&method),
    };
    eprintln!("→ {method} {args}");

    let started = Instant::now();
    let answer = host.call(&full_id, &method, args).await;
    let elapsed = started.elapsed();
    let answer = answer.map_err(|error| {
        // What the app shows, then the details it keeps for "More info".
        let text = error.to_string();
        let (message, detail) = text.split_once('\n').unwrap_or((&text, ""));
        let mut said = format!(
            "{method} failed after {} ms: {message}",
            elapsed.as_millis()
        );
        if !detail.trim().is_empty() {
            said.push_str(&format!("\n  {}", detail.trim().replace('\n', "\n  ")));
        }
        said
    })?;
    println!(
        "{}",
        serde_json::to_string_pretty(&answer).unwrap_or_default()
    );

    match review::review(&kind, &method, &answer) {
        Some(review) => eprintln!("{}", review.summary()),
        None => eprintln!("Raygun never calls {method} on a {kind} provider"),
    }
    let limit = time_limit(&kind, &method);
    if elapsed > limit {
        eprintln!(
            "took {} ms: longer than the {} s Raygun waits for {method}, so the app would give up",
            elapsed.as_millis(),
            limit.as_secs()
        );
    } else {
        eprintln!(
            "took {} ms (Raygun waits up to {} s)",
            elapsed.as_millis(),
            limit.as_secs()
        );
    }
    Ok(())
}

/// What a site-scripts extension does on a page: which groups run there and with what, and the
/// script the browser injects when `--bundle` asks for it.
fn try_site(
    host: &ExtensionHost,
    id: &str,
    scripts: &raygun_ext_host::site::SiteScripts,
    options: &Options,
) -> Result<(), String> {
    eprintln!(
        "  runs on {} (origins {})",
        scripts.patterns().join(", "),
        scripts.origin_rules().join(", ")
    );
    if let Some(address) = options.provider.as_deref() {
        let page = url::Url::parse(address).map_err(|error| format!("{address}: {error}"))?;
        for (index, group) in scripts.groups.iter().enumerate() {
            let files = group
                .js
                .iter()
                .chain(&group.css)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            let frames = if group.all_frames {
                "every frame"
            } else {
                "the top page"
            };
            let verdict = if group.runs_on(&page) {
                format!("runs at {} in {frames}", group.run_at.as_str())
            } else {
                "doesn't run".to_owned()
            };
            eprintln!("  group {index} ({files}): {verdict}");
        }
        if !scripts.matches(&page) {
            eprintln!("nothing runs on {address}");
        }
    }
    if let Some(path) = &options.bundle {
        let site = host
            .site_extensions()
            .into_iter()
            .find(|site| site.id == id)
            .ok_or("the extension is turned off")?;
        let code = site.compose(raygun_ext_host::site::Platform::Desktop)?;
        std::fs::write(path, &code)
            .map_err(|error| format!("can't write {}: {error}", path.display()))?;
        eprintln!(
            "wrote the script Raygun injects ({} bytes) to {}",
            code.len(),
            path.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn reads_positionals_and_options() {
        let options = parse(args(&[
            "ext",
            "books",
            "search",
            r#"{"query":"dickens","page":1}"#,
            "--set",
            "language=English",
            "--set",
            "fullSize=true",
            "--nsfw",
        ]))
        .unwrap();
        assert_eq!(options.dir, PathBuf::from("ext"));
        assert_eq!(options.provider.as_deref(), Some("books"));
        assert_eq!(options.method.as_deref(), Some("search"));
        assert_eq!(options.settings["language"], json!("English"));
        assert_eq!(options.settings["fullSize"], json!(true));
        assert!(options.nsfw);
        assert!(
            parse(args(&["ext", "books"])).is_err(),
            "a provider needs a method"
        );
        assert!(parse(args(&["ext", "a", "b", "{}", "extra"])).is_err());
        assert!(parse(args(&[])).is_err());
    }

    #[test]
    fn lists_start_at_their_first_page() {
        assert_eq!(default_args("popular"), json!({ "page": 1 }));
        assert_eq!(default_args("details"), json!({}));
    }
}
