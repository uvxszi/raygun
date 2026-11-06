//! Live checks against Keiyoushi's store (network; `cargo test -p raygun-ext-host -- --ignored
//! mihon_ --nocapture`). `MIHON_SOURCE_DIR` names a checkout of the store's source repository
//! (an unpacked `keiyoushi/extensions-source`) to read code from instead of GitHub.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use futures_util::StreamExt;
use reqwest::Url;

use super::convert::{self, Converted};
use super::index::{self, Store, StoreExtension};
use super::source::{self, Sources};
use crate::{ExtensionHost, ExtensionSource, HostConfig};

const KEIYOUSHI: &str =
    "https://raw.githubusercontent.com/keiyoushi/extensions/repo/index.min.json";

async fn download(client: &reqwest::Client, url: Url) -> Result<Vec<u8>, String> {
    let response = client
        .get(url.clone())
        .header("User-Agent", "Raygun")
        .send()
        .await
        .map_err(|error| format!("could not reach {url}: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "{url} answered HTTP {}",
            response.status().as_u16()
        ));
    }
    Ok(response
        .bytes()
        .await
        .map_err(|error| error.to_string())?
        .to_vec())
}

async fn keiyoushi(client: &reqwest::Client) -> (Url, Store) {
    let url: Url = KEIYOUSHI.parse().unwrap();
    let first = download(client, url.clone()).await.unwrap();
    assert!(
        index::detect(&first),
        "Keiyoushi's list reads as a Mihon store"
    );
    index::load(
        &url,
        first,
        |next| async move { download(client, next).await },
    )
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "network"]
async fn mihon_index_live() {
    let client = raygun_net::http_client().unwrap();
    let (from, store) = keiyoushi(&client).await;
    let repo = index::to_repo_index(&store).unwrap();
    let nsfw = repo
        .extensions
        .iter()
        .filter(|entry| entry.content_rating == "nsfw")
        .count();
    let sources: usize = store
        .extensions
        .iter()
        .map(|extension| extension.sources.len())
        .sum();
    println!(
        "{} from {from}: {} extensions ({nsfw} for adults), {sources} sources, signed {}",
        store.name,
        repo.extensions.len(),
        store.signing_key.as_deref().unwrap_or("?")
    );
    assert_eq!(store.name, "Keiyoushi");
    assert!(repo.extensions.len() > 1000);
    let located = store
        .extensions
        .iter()
        .filter(|extension| source::locate(extension, &from).is_some())
        .count();
    assert_eq!(
        located,
        store.extensions.len(),
        "every extension's code is found"
    );
}

/// Every extension's code, read from a checkout or, without one, from GitHub: one listing of the
/// whole repository, then raw files.
async fn all_sources(
    client: &reqwest::Client,
    store: &Store,
    from: &Url,
) -> HashMap<String, Result<Sources, String>> {
    let mut found = HashMap::new();
    if let Some(dir) = std::env::var_os("MIHON_SOURCE_DIR").map(PathBuf::from) {
        for extension in &store.extensions {
            let read = source::locate(extension, from)
                .ok_or_else(|| "no source folder".to_owned())
                .and_then(|folder| {
                    Sources::from_dir(&dir.join(&folder.path)).map_err(|error| error.to_string())
                });
            found.insert(extension.pkg.clone(), read);
        }
        return found;
    }
    let folders: Vec<(String, source::SourceFolder)> = store
        .extensions
        .iter()
        .filter_map(|extension| Some((extension.pkg.clone(), source::locate(extension, from)?)))
        .collect();
    let Some((_, first)) = folders.first() else {
        return found;
    };
    let tree_url: Url = format!(
        "https://api.github.com/repos/{}/{}/git/trees/{}?recursive=1",
        first.owner, first.repo, first.git_ref
    )
    .parse()
    .unwrap();
    let listing = download(client, tree_url).await.unwrap();
    let tree: serde_json::Value = serde_json::from_slice(&listing).unwrap();
    let mut by_folder: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in tree["tree"].as_array().unwrap() {
        let path = entry["path"].as_str().unwrap_or_default();
        let wanted = path.ends_with("/build.gradle.kts")
            || path.ends_with("/build.gradle")
            || (path.ends_with(".kt") && path.contains("/src/eu/"));
        if wanted && path.starts_with("src/") {
            let folder: String = path.split('/').take(3).collect::<Vec<_>>().join("/");
            by_folder.entry(folder).or_default().push(path.to_owned());
        }
    }
    let fetches = futures_util::stream::iter(folders.into_iter().map(|(pkg, folder)| {
        let paths = by_folder.get(&folder.path).cloned().unwrap_or_default();
        async move {
            let mut sources = Sources::default();
            for path in paths {
                let relative = path[folder.path.len() + 1..].to_owned();
                let Some(url) = folder.raw_url(&relative) else {
                    continue;
                };
                match download(client, url).await {
                    Ok(bytes) => {
                        sources
                            .files
                            .insert(relative, String::from_utf8_lossy(&bytes).into_owned());
                    }
                    Err(error) => return (pkg, Err(error)),
                }
            }
            (pkg, Ok(sources))
        }
    }))
    .buffer_unordered(24)
    .collect::<Vec<_>>()
    .await;
    found.extend(fetches);
    found
}

#[tokio::test]
#[ignore = "network"]
async fn mihon_convert_survey() {
    let client = raygun_net::http_client().unwrap();
    let (from, store) = keiyoushi(&client).await;
    let sources = all_sources(&client, &store, &from).await;
    let mut converted = 0usize;
    let mut by_theme: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    // `MIHON_SURVEY_DUMP=<dir>` keeps the code of each conversion that doesn't load;
    // `MIHON_SURVEY_ONLY=<pkg>,…` surveys only those packages (or names ending so).
    let dump = std::env::var_os("MIHON_SURVEY_DUMP").map(PathBuf::from);
    let only: Vec<String> = std::env::var("MIHON_SURVEY_ONLY")
        .map(|list| list.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    let comics: Vec<&StoreExtension> = store
        .extensions
        .iter()
        .filter(|extension| !extension.pkg.contains(".animeextension."))
        .filter(|extension| {
            only.is_empty()
                || only
                    .iter()
                    .any(|name| extension.pkg.ends_with(name.as_str()))
        })
        .collect();
    for extension in &comics {
        let code = match sources.get(&extension.pkg) {
            Some(Ok(code)) => code,
            Some(Err(error)) => {
                *reasons.entry(format!("unreadable: {error}")).or_default() += 1;
                continue;
            }
            None => {
                *reasons.entry("no source folder".to_owned()).or_default() += 1;
                continue;
            }
        };
        let theme = code
            .build_file()
            .and_then(|build| super::gradle::read(build).theme)
            .unwrap_or_else(|| "(none)".to_owned());
        let slot = by_theme.entry(theme).or_default();
        slot.0 += 1;
        // Each conversion is loaded, as installing does: one that doesn't load isn't converted.
        let converted_one = match convert::convert(extension, code) {
            Ok(result) => {
                if let Some(dir) = dump
                    .as_ref()
                    .filter(|_| std::env::var_os("MIHON_SURVEY_DUMP_ALL").is_some())
                {
                    let _ = std::fs::create_dir_all(dir.join("all"));
                    let _ = std::fs::write(
                        dir.join("all").join(format!("{}.js", extension.pkg)),
                        &result.code,
                    );
                }
                let package = convert::package(extension, &result, &store.name, None).unwrap();
                convert::validate(&package, extension.sources.len())
                    .await
                    .map_err(|error| {
                        let line = error.lines().nth(1).unwrap_or(&error).to_owned();
                        println!("  {}: doesn't load: {line}", extension.pkg);
                        if let Some(dir) = &dump {
                            let _ = std::fs::create_dir_all(dir);
                            let _ = std::fs::write(
                                dir.join(format!("{}.js", extension.pkg)),
                                &result.code,
                            );
                        }
                        format!("its conversion doesn't load: {line}")
                    })
            }
            Err(reason) => Err(reason),
        };
        match converted_one {
            Ok(()) => {
                converted += 1;
                slot.1 += 1;
            }
            Err(reason) => {
                // `MIHON_SURVEY_DETAIL=<theme>` lists that theme's refusals one by one; code
                // Raygun couldn't parse is always listed.
                let detail = std::env::var("MIHON_SURVEY_DETAIL").ok();
                let theme = code
                    .build_file()
                    .and_then(|build| super::gradle::read(build).theme)
                    .unwrap_or_else(|| "(none)".to_owned());
                if detail.as_deref() == Some(theme.as_str())
                    || reason.contains("couldn't read its code")
                {
                    println!("  {}: {reason}", extension.pkg);
                }
                // Reasons grouped by their first words, so lists of names don't split them, and
                // without the function they were met in.
                let reason = match reason.find("its code uses") {
                    Some(at) => reason[at..].to_owned(),
                    None => reason,
                };
                let key = reason
                    .split(" which ")
                    .next()
                    .unwrap_or(&reason)
                    .chars()
                    .take(70)
                    .collect::<String>();
                *reasons.entry(key).or_default() += 1;
            }
        }
    }
    let total = comics.len();
    println!(
        "converted {converted} of {total} extensions ({:.1}%); unsupported {}",
        converted as f64 * 100.0 / total as f64,
        total - converted
    );
    let mut themes: Vec<_> = by_theme.into_iter().collect();
    themes.sort_by_key(|(_, (all, _))| std::cmp::Reverse(*all));
    for (theme, (all, done)) in themes.iter().take(20) {
        println!("  {theme:16} {done:4} of {all:4}");
    }
    let mut reasons: Vec<_> = reasons.into_iter().collect();
    reasons.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    println!("top reasons:");
    for (reason, count) in reasons.iter().take(25) {
        println!("  {count:4}  {reason}");
    }
    assert!(converted > 0);
}

/// A converted source with its real address, loaded in a host of its own.
async fn loaded(extension: &StoreExtension, converted: &Converted) -> Option<ExtensionHost> {
    let package = convert::package(extension, converted, "Keiyoushi", None).ok()?;
    let files: Vec<(String, String)> = package
        .files()
        .iter()
        .map(|(name, bytes)| (name.clone(), String::from_utf8_lossy(bytes).into_owned()))
        .collect();
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(name, text)| (name.as_str(), text.as_str()))
        .collect();
    let host = ExtensionHost::start(HostConfig {
        lanes: 1,
        ..HostConfig::default()
    })
    .ok()?;
    host.load(vec![ExtensionSource::built_in(&borrowed)]).await;
    Some(host)
}

/// The first popular page of a sample of converted sources (`MIHON_SAMPLE`, default 24): how many
/// answer with titles.
#[tokio::test]
#[ignore = "network"]
async fn mihon_popular_live() {
    let client = raygun_net::http_client().unwrap();
    let (from, store) = keiyoushi(&client).await;
    let sources = all_sources(&client, &store, &from).await;
    let sample: usize = std::env::var("MIHON_SAMPLE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(24);
    let candidates: Vec<(StoreExtension, Converted)> = store
        .extensions
        .iter()
        .filter(|extension| extension.content != index::ContentWarning::Nsfw)
        .filter_map(|extension| {
            let code = sources.get(&extension.pkg)?.as_ref().ok()?;
            Some((extension.clone(), convert::convert(extension, code).ok()?))
        })
        .collect();
    let step = (candidates.len() / sample.max(1)).max(1);
    let picked: Vec<_> = candidates.into_iter().step_by(step).take(sample).collect();
    let answered = Arc::new(std::sync::Mutex::new(Vec::new()));
    for (extension, converted) in &picked {
        let Some(host) = loaded(extension, converted).await else {
            continue;
        };
        let Some(id) = index::extension_id(&extension.pkg) else {
            continue;
        };
        let provider = format!("{id}/{}", extension.sources[0].id);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(40),
            host.manga_list(&provider, crate::manga::MangaList::Popular, 1),
        )
        .await;
        let line = match result {
            Ok(Ok(listing)) if !listing.items.is_empty() => {
                format!(
                    "ok    {} ({}): {} titles, first {:?}",
                    extension.name,
                    converted.theme.as_deref().unwrap_or("-"),
                    listing.items.len(),
                    listing.items[0].title
                )
            }
            Ok(Ok(_)) => format!(
                "empty {} ({})",
                extension.name,
                converted.theme.as_deref().unwrap_or("-")
            ),
            Ok(Err(error)) => format!(
                "fail  {}: {}",
                extension.name,
                error.lines().next().unwrap_or_default()
            ),
            Err(_) => format!("slow  {}", extension.name),
        };
        println!("{line}");
        answered.lock().unwrap().push(line);
    }
    let lines = answered.lock().unwrap();
    let ok = lines.iter().filter(|line| line.starts_with("ok")).count();
    println!("{ok} of {} sampled sources listed titles", lines.len());
    assert!(ok > 0);
}
