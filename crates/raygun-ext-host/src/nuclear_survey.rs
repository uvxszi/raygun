//! Every plugin in Nuclear's registry, installed the way the app installs one and asked what it
//! provides: searches, stream lookups (the best candidate resolved), lyrics, charts and radios,
//! each timed and with its runtime's memory high-water mark. Then all of them at once, as a
//! search in the app asks them. It fails when a plugin runs out of memory, time or stack, fails
//! to start, or can't do what it can (`WORKING`):
//! `cargo test -p raygun-ext-host --lib -- --ignored nuclear_registry_survey --nocapture`
//! (`NUCLEAR_PLUGINS_DIR=<folder>` keeps the downloaded packages between runs).

use std::fmt::Write as _;
use std::io::Write as _;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use raygun_model::{ExtensionStatus, TrackQuery};
use serde_json::{Value, json};

use crate::heap::megabytes;
use crate::nuclear;
use crate::{CallError, ExtensionHost, ExtensionSource, HostConfig, INTERACTIVE_TIMEOUT};

const REGISTRY: &str =
    "https://cdn.jsdelivr.net/gh/NuclearPlayer/plugin-registry@master/plugins.json";
/// A public YouTube playlist, for playlists-by-address providers.
const PLAYLIST: &str = "https://www.youtube.com/playlist?list=PLSbDLF8wQ3oIkmgwFI3HmEYV-qhLwLVHx";

/// Log lines plugins write (their `console.warn` and `console.error`), kept to tell what a call
/// met on its way even when the plugin caught it.
#[derive(Clone, Default)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl LogBuffer {
    fn take(&self) -> String {
        let bytes = std::mem::take(&mut *self.0.lock().unwrap());
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl std::io::Write for LogBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogBuffer {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// One call's outcome.
struct Row {
    plugin: String,
    call: String,
    verdict: String,
    elapsed: Duration,
    peak: usize,
    detail: String,
}

/// What kind of failure a call met, as the survey sorts them.
fn verdict(error: &CallError, logs: &str) -> String {
    let text = format!("{error}\n{}", error.detail()).to_lowercase();
    let kind = if let CallError::Exhausted { what, .. } = error {
        match what {
            crate::Exhaustion::Memory => "OUT OF MEMORY",
            crate::Exhaustion::Time => "SCRIPT TOO LONG",
            crate::Exhaustion::Stack => "STACK OVERFLOW",
        }
    } else if matches!(
        error,
        CallError::Timeout { .. } | CallError::StillWorking { .. }
    ) {
        "TIMEOUT"
    } else if text.contains("isn't available to nuclear plugins")
        || text.contains("which raygun doesn't provide")
        || text.contains("is not defined")
        || text.contains("not a function")
    {
        "MISSING API"
    } else {
        match error.kind() {
            Some(kind) => kind.as_str(),
            None => "said",
        }
    };
    format!("{kind}{}", inner(logs))
}

/// What a plugin's own logs say it met on the way (failures it caught).
fn inner(logs: &str) -> String {
    let lower = logs.to_lowercase();
    let mut met = Vec::new();
    for (needle, name) in [
        ("out of memory", "OOM"),
        ("interrupted", "interrupted"),
        ("maximum call stack size exceeded", "stack overflow"),
    ] {
        if lower.contains(needle) {
            met.push(name);
        }
    }
    if met.is_empty() {
        String::new()
    } else {
        format!(" (inner {})", met.join(", "))
    }
}

/// The first line of a text, cut short.
fn short(text: &str, chars: usize) -> String {
    let line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    line.chars().take(chars).collect()
}

struct Survey {
    host: ExtensionHost,
    logs: LogBuffer,
    rows: Vec<Row>,
}

impl Survey {
    /// Calls `provider`'s `method` within `limit`, noting the outcome; `None` when it has no such
    /// method.
    async fn ask(
        &mut self,
        plugin: &str,
        provider: &str,
        method: &str,
        args: Value,
        limit: Duration,
        count: impl Fn(&Value) -> String,
    ) -> Option<Value> {
        let extension = provider.split_once('/').map_or(provider, |(id, _)| id);
        self.host.heap_use(extension, true);
        self.logs.take();
        let started = Instant::now();
        let answer = self.host.call_within(provider, method, args, limit).await;
        let elapsed = started.elapsed();
        let peak = self
            .host
            .heap_use(extension, false)
            .map_or(0, |heap| heap.peak);
        let logs = self.logs.take();
        let local = provider.split_once('/').map_or(provider, |(_, id)| id);
        let call = format!("{local}.{method}");
        let (verdict, detail, value) = match answer {
            Ok(value) => (format!("ok{}", inner(&logs)), count(&value), Some(value)),
            Err(error) if error.is_missing() => return None,
            Err(error) => (verdict(&error, &logs), short(&error.detail(), 110), None),
        };
        println!(
            "  {plugin:<28} {call:<34} {verdict:<22} {:>6.1}s {:>7.1} MB  {detail}",
            elapsed.as_secs_f64(),
            megabytes(peak)
        );
        let _ = std::io::stdout().flush();
        self.rows.push(Row {
            plugin: plugin.to_owned(),
            call,
            verdict,
            elapsed,
            peak,
            detail,
        });
        value
    }
}

fn length_of(key: &'static str) -> impl Fn(&Value) -> String {
    move |value: &Value| {
        let items = value.get(key).and_then(Value::as_array).map_or(0, Vec::len);
        format!("{items} {key}")
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs network access to jsDelivr, GitHub and every plugin's sites"]
async fn nuclear_registry_survey() {
    let logs = LogBuffer::default();
    let _ = tracing_subscriber::fmt()
        .with_env_filter("raygun_ext_host::js=warn")
        .with_ansi(false)
        .with_writer(logs.clone())
        .try_init();

    let client = raygun_net::http_client().unwrap();
    // GitHub's release downloads fail now and then (502): a few tries.
    let download = |url: reqwest::Url| {
        let client = client.clone();
        async move {
            let mut last = String::new();
            for attempt in 0..4 {
                if attempt > 0 {
                    tokio::time::sleep(Duration::from_secs(2 * attempt)).await;
                }
                match client.get(url.clone()).send().await {
                    Ok(response) if response.status().is_success() => {
                        return response
                            .bytes()
                            .await
                            .map(|bytes| bytes.to_vec())
                            .map_err(|error| error.to_string());
                    }
                    Ok(response) => last = format!("{url}: {}", response.status()),
                    Err(error) => last = error.to_string(),
                }
            }
            Err(last)
        }
    };
    // `NUCLEAR_PLUGINS_DIR` keeps the packages (`<id>-<version>.zip`) between runs.
    let kept = std::env::var_os("NUCLEAR_PLUGINS_DIR").map(std::path::PathBuf::from);
    let registry_url = reqwest::Url::parse(REGISTRY).unwrap();
    let mut index = crate::repo::parse_index(
        &String::from_utf8(download(registry_url.clone()).await.unwrap()).unwrap(),
    )
    .unwrap();
    // Plugins that only change Nuclear itself are listed as not for Raygun, never installed.
    for entry in index
        .extensions
        .iter()
        .filter(|entry| entry.not_for_raygun.is_some())
    {
        println!(
            "not for Raygun: {} ({})",
            entry.name,
            entry.not_for_raygun.as_deref().unwrap_or_default()
        );
    }
    index
        .extensions
        .retain(|entry| entry.not_for_raygun.is_none());
    println!("{} plugins Raygun can run", index.extensions.len());

    // Installed as the app installs them: downloaded, checked, converted.
    let mut sources = Vec::new();
    let mut install_failures = Vec::new();
    for entry in &index.extensions {
        let installed = async {
            let file = kept
                .as_ref()
                .map(|dir| dir.join(format!("{}-{}.zip", entry.id, entry.version)));
            let bytes = match file.as_ref().and_then(|file| std::fs::read(file).ok()) {
                Some(bytes) => bytes,
                None => {
                    let url = crate::repo::package_url(&registry_url, entry)
                        .map_err(|e| e.to_string())?;
                    let bytes = download(url).await?;
                    if let Some(file) = &file {
                        let _ = std::fs::write(file, &bytes);
                    }
                    bytes
                }
            };
            crate::package::check_nuclear_download(&bytes, entry).map_err(|e| e.to_string())?;
            let files = nuclear::unzip_sources(&bytes).map_err(|e| e.to_string())?;
            let converted = nuclear::convert(&files)?;
            crate::package::from_converted(&converted.manifest, &converted.bundle)
                .map_err(|e| e.to_string())?;
            Ok::<_, String>(converted)
        }
        .await;
        match installed {
            Ok(converted) => {
                println!(
                    "{} {}: stubbed {:?}, hosts {:?}",
                    entry.id, entry.version, converted.report.stubbed_apis, converted.report.hosts
                );
                sources.push(ExtensionSource::built_in(&[
                    (crate::MANIFEST_FILE, &converted.manifest),
                    ("dist/index.js", &converted.bundle),
                ]));
            }
            Err(error) => {
                println!("{}: not installed: {error}", entry.id);
                install_failures.push((entry.id.clone(), error));
            }
        }
    }

    let storage = tempfile::tempdir().unwrap();
    let host = ExtensionHost::start(HostConfig {
        storage_dir: Some(storage.path().to_owned()),
        ..HostConfig::default()
    })
    .unwrap();
    let infos = host.load(sources).await;
    let background = HostConfig::default().call_timeout;
    let mut survey = Survey {
        host: host.clone(),
        logs: logs.clone(),
        rows: Vec::new(),
    };
    println!(
        "\n  {:<28} {:<34} {:<22} {:>7} {:>10}  detail",
        "plugin", "call", "result", "time", "peak"
    );
    let query = TrackQuery {
        title: "One More Time".into(),
        artist: Some("Daft Punk".into()),
        album: Some("Discovery".into()),
        duration_ms: Some(320_000),
    };
    let song = json!({
        "title": query.title, "artist": query.artist, "album": query.album,
        "durationMs": query.duration_ms,
    });
    for info in &infos {
        let plugin = info.id.trim_start_matches("nuclear.").to_owned();
        if info.status != ExtensionStatus::Loaded {
            // The sentence is "… couldn't start"; what stopped it is on the lines after.
            let error = info.error.as_deref().unwrap_or("");
            let detail: String = error
                .lines()
                .skip(1)
                .collect::<Vec<_>>()
                .join(" / ")
                .chars()
                .take(160)
                .collect();
            println!(
                "  {plugin:<28} {:<34} {:<22}  {detail}",
                "load", "FAILED TO LOAD"
            );
            survey.rows.push(Row {
                plugin,
                call: "load".into(),
                verdict: "FAILED TO LOAD".into(),
                elapsed: Duration::ZERO,
                peak: 0,
                detail,
            });
            continue;
        }
        let loaded = host.heap_use(&info.id, true).unwrap_or_default();
        println!(
            "  {plugin:<28} {:<34} {:<22} {:>7} {:>7.1} MB  limit {:.0} MB",
            "load",
            "ok",
            "",
            megabytes(loaded.peak),
            megabytes(loaded.limit)
        );
        for provider in &info.providers {
            let id = provider.id.as_str();
            match provider.kind.as_str() {
                "music.metadata" => {
                    let args = json!({ "query": "Daft Punk One More Time", "limit": 10 });
                    survey
                        .ask(
                            &plugin,
                            id,
                            "search",
                            args,
                            INTERACTIVE_TIMEOUT,
                            length_of("tracks"),
                        )
                        .await;
                    let args = json!({ "query": PLAYLIST, "featured": false, "limit": 5 });
                    survey
                        .ask(
                            &plugin,
                            id,
                            "searchPlaylists",
                            args,
                            INTERACTIVE_TIMEOUT,
                            length_of("playlists"),
                        )
                        .await;
                }
                "music.stream" => {
                    let mut args = song.clone();
                    args["limit"] = json!(10);
                    let found = survey
                        .ask(
                            &plugin,
                            id,
                            "candidates",
                            args,
                            INTERACTIVE_TIMEOUT,
                            length_of("candidates"),
                        )
                        .await;
                    // The candidate the resolver would play: the best match.
                    let best = found.and_then(|value| {
                        let candidates = crate::host::parse_candidates(provider, value);
                        raygun_resolver::rank(
                            &query,
                            candidates,
                            &raygun_resolver::History::default(),
                        )
                        .into_iter()
                        .next()
                        .map(|ranked| ranked.candidate.id)
                    });
                    if let Some(candidate) = best {
                        let args = json!({ "id": candidate, "decodable": false });
                        survey
                            .ask(&plugin, id, "resolve", args, INTERACTIVE_TIMEOUT, |value| {
                                let url = value
                                    .pointer("/source/url")
                                    .and_then(Value::as_str)
                                    .unwrap_or("no url");
                                short(url, 70)
                            })
                            .await;
                    }
                }
                "lyrics" => {
                    survey
                        .ask(
                            &plugin,
                            id,
                            "lyrics",
                            song.clone(),
                            INTERACTIVE_TIMEOUT,
                            |value| {
                                let synced = value
                                    .get("synced")
                                    .and_then(Value::as_array)
                                    .map_or(0, Vec::len);
                                let plain = value
                                    .get("plain")
                                    .and_then(Value::as_str)
                                    .map_or(0, str::len);
                                format!("{synced} synced lines, {plain} chars plain")
                            },
                        )
                        .await;
                }
                "music.charts" => {
                    let charts = survey
                        .ask(
                            &plugin,
                            id,
                            "charts",
                            json!({ "region": null }),
                            background,
                            length_of("charts"),
                        )
                        .await;
                    let first = charts
                        .as_ref()
                        .and_then(|value| value.pointer("/charts/0/id"))
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    if let Some(chart) = first {
                        let args = json!({ "id": chart, "region": null, "limit": 20 });
                        survey
                            .ask(&plugin, id, "chart", args, background, length_of("tracks"))
                            .await;
                    }
                }
                "music.recommend" => {
                    let args =
                        json!({ "artist": "Daft Punk", "title": "One More Time", "limit": 10 });
                    survey
                        .ask(&plugin, id, "radio", args, background, length_of("tracks"))
                        .await;
                }
                other => println!("  {plugin:<28} {other}: not surveyed"),
            }
        }
    }

    // All at once, as Search and the resolver ask them.
    println!("\nAll plugins at once:");
    let mut problems = Vec::new();
    let started = Instant::now();
    let results = host.search_music("Daft Punk One More Time", 10).await;
    let searched = started.elapsed();
    let mut failed: Vec<(String, String)> = results
        .iter()
        .filter_map(|result| Some((result.provider.id.clone(), result.error.clone()?)))
        .collect();
    println!(
        "  search: {} sources in {searched:.1?}, {} failed",
        results.len(),
        failed.len()
    );
    let started = Instant::now();
    let answers = host
        .stream_candidates(&query, 10, raygun_resolver::QualityTarget::Any)
        .await;
    let looked = started.elapsed();
    let before = failed.len();
    failed.extend(answers.iter().filter_map(|answer| {
        Some((
            answer.provider.id.clone(),
            answer.result.as_ref().err()?.clone(),
        ))
    }));
    println!(
        "  candidates: {} sources in {looked:.1?}, {} failed",
        answers.len(),
        failed.len() - before
    );
    for (provider, error) in &failed {
        println!("    {provider}: {}", short(error, 120));
        if exhausted(error) {
            problems.push(format!("{provider} at once: {}", short(error, 120)));
        }
    }

    let mut table = String::new();
    writeln!(
        table,
        "\n| plugin | call | result | time | peak heap | detail |"
    )
    .unwrap();
    writeln!(table, "|---|---|---|---|---|---|").unwrap();
    for row in &survey.rows {
        writeln!(
            table,
            "| {} | {} | {} | {:.1} s | {:.1} MB | {} |",
            row.plugin,
            row.call,
            row.verdict,
            row.elapsed.as_secs_f64(),
            megabytes(row.peak),
            row.detail.replace('|', "/")
        )
        .unwrap();
    }
    for (id, error) in &install_failures {
        writeln!(
            table,
            "| {id} | install | FAILED | | | {} |",
            short(error, 110)
        )
        .unwrap();
    }
    println!("{table}");

    // No plugin runs out of memory, time or stack, or fails to start, even inside a call that
    // got by; and what the plugins can do (their sites answering, no account needed), they do.
    for row in &survey.rows {
        let verdict = row.verdict.as_str();
        if verdict.contains("inner")
            || [
                "OUT OF MEMORY",
                "SCRIPT TOO LONG",
                "STACK OVERFLOW",
                "FAILED TO LOAD",
            ]
            .iter()
            .any(|kind| verdict.starts_with(kind))
        {
            problems.push(format!(
                "{} {}: {verdict} ({})",
                row.plugin, row.call, row.detail
            ));
        }
    }
    for (plugin, call) in WORKING {
        match survey
            .rows
            .iter()
            .find(|row| row.plugin == *plugin && row.call == *call)
        {
            Some(row) if row.verdict == "ok" && !row.detail.starts_with("0 ") => {}
            // A site that's down or busy now isn't the plugin's limits.
            Some(row) if TRANSIENT.contains(&row.verdict.as_str()) => {
                println!("{plugin} {call}: {} now ({})", row.verdict, row.detail);
            }
            Some(row) => {
                problems.push(format!("{plugin} {call}: {} ({})", row.verdict, row.detail))
            }
            None => {
                let installed = !install_failures
                    .iter()
                    .any(|(id, _)| id.ends_with(&format!(".{plugin}")));
                if installed {
                    problems.push(format!("{plugin} {call}: not asked"));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Calls that work with the registry's plugins as they are (no account or server of one's own,
/// the site still answering, streams Raygun plays): `(plugin, provider.method)`.
const WORKING: &[(&str, &str)] = &[
    ("nuclear-plugin-bandcamp", "bandcamp.search"),
    ("nuclear-plugin-bandcamp", "bandcamp-stream.candidates"),
    ("nuclear-plugin-bandcamp", "bandcamp-stream.resolve"),
    ("nuclear-plugin-deezer-dashboard", "deezer-dashboard.chart"),
    ("nuclear-plugin-kugou", "kugou.lyrics"),
    ("nuclear-plugin-lastfm", "lastfm-discovery.radio"),
    (
        "nuclear-plugin-listenbrainz-dashboard",
        "listenbrainz-dashboard.chart",
    ),
    ("nuclear-plugin-lrclib", "lrclib.lyrics"),
    ("nuclear-plugin-musicbrainz", "musicbrainz-metadata.search"),
    ("nuclear-plugin-netease", "netease.search"),
    ("nucleartube", "nucleartube-streaming.candidates"),
    ("nucleartube", "nucleartube-streaming.resolve"),
    ("nucleartube", "nucleartube-metadata.search"),
    ("nucleartube", "nucleartube-playlist.searchPlaylists"),
    ("nuclear-plugin-omnisource", "omnisource-stream.candidates"),
    ("nuclear-plugin-omnisource", "omnisource-meta.search"),
    ("nuclear-plugin-soundcloud", "soundcloud.search"),
    ("nuclear-plugin-soundcloud", "soundcloud-stream.candidates"),
    (
        "nuclear-plugin-soundcloud-dashboard",
        "soundcloud-dashboard.chart",
    ),
    ("nuclear-plugin-something", "spotify.search"),
    ("nuclear-plugin-youtube", "youtube.candidates"),
    ("nuclear-plugin-youtube", "youtube.resolve"),
    (
        "nuclear-plugin-youtube-playlists",
        "youtube-playlists.searchPlaylists",
    ),
];

/// Verdicts that say a site is down, busy or slow right now.
const TRANSIENT: &[&str] = &[
    "TIMEOUT",
    "timeout",
    "rate-limited",
    "server-error",
    "unreachable",
    "dns",
    "offline",
    "refused",
];

/// Whether an error text says a script ran out of memory, time or stack.
fn exhausted(error: &str) -> bool {
    let lower = error.to_lowercase();
    [
        "out of memory",
        "kept the processor busy",
        "went too deep",
        "needed more memory",
    ]
    .iter()
    .any(|words| lower.contains(words))
}
