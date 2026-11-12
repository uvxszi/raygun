//! Addons that list other addons (Stremio's addon collections: a manifest's `addonCatalogs`,
//! answered by its `addon_catalog` resource), read as a repository of addons: Sources → Add shows
//! what the collection lists, and each addon is added on its own from the catalogue (entries of
//! format `stremio`, as a Raygun index's `addons` are).

use std::future::Future;

use raygun_ext_host::manifest::{API_VERSION, is_extension_id};
use raygun_ext_host::repo::{INDEX_FORMAT, PackageFormat, RepoEntry, RepoIndex};
use serde_json::Value;

use super::protocol::{self, ListedAddon, Manifest};

/// Most addon lists read from one collection.
const MAX_LISTS: usize = 8;

/// The manifest `bytes` hold when they're an addon collection's (an addon offering only lists
/// of other addons).
pub fn collection(bytes: &[u8]) -> Option<Manifest> {
    // Most indexes aren't: only one naming the resource is read as JSON here.
    if !bytes.windows(13).any(|window| window == b"addon_catalog") {
        return None;
    }
    let value: Value = serde_json::from_slice(bytes).ok()?;
    if value.get("id").is_none() || value.get("resources").is_none() {
        return None;
    }
    protocol::parse_manifest(&value)
        .ok()
        .filter(Manifest::only_lists_addons)
}

/// The repository the collection at `manifest_url` stands for: the addons its addon catalogs
/// list, in their order, each once. `fetch` reads an address; a list that can't be read is
/// passed over unless none could be.
pub async fn read<F, Fut>(
    manifest_url: &str,
    manifest: &Manifest,
    fetch: F,
) -> Result<RepoIndex, String>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    let mut listed: Vec<ListedAddon> = Vec::new();
    let mut failure = None;
    for catalog in manifest.addon_catalogs.iter().take(MAX_LISTS) {
        let url = protocol::resource_url(
            manifest_url,
            "addon_catalog",
            &catalog.kind,
            &catalog.id,
            &[],
        );
        let answer = fetch(url).await.and_then(|bytes| {
            serde_json::from_slice::<Value>(&bytes)
                .map_err(|_| format!("{}'s list {} isn't JSON", manifest.name, catalog.name))
        });
        match answer {
            Ok(value) => {
                for addon in protocol::parse_addon_catalog(&value) {
                    if !listed
                        .iter()
                        .any(|known| known.transport_url == addon.transport_url)
                    {
                        listed.push(addon);
                    }
                }
            }
            Err(error) => failure = Some(error),
        }
    }
    if listed.is_empty()
        && let Some(error) = failure
    {
        return Err(error);
    }
    Ok(RepoIndex {
        format_version: INDEX_FORMAT,
        name: manifest.name.clone(),
        extensions: listed.iter().filter_map(entry_of).collect(),
        libraries: Vec::new(),
    })
}

/// A catalogue id for an addon's own: reverse-DNS labels as Raygun's ids are (`com.linvo.cinemeta`
/// stays, `Some Addon` becomes `stremio.some-addon`).
fn entry_id(addon_id: &str) -> Option<String> {
    let mut labels: Vec<String> = addon_id
        .to_ascii_lowercase()
        .split('.')
        .map(|part| {
            let label: String = part
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect();
            label.trim_matches('-').to_owned()
        })
        .filter(|label| !label.is_empty())
        .collect();
    if labels.len() < 2 {
        labels.insert(0, "stremio".to_owned());
    }
    let id = labels.join(".");
    is_extension_id(&id).then_some(id)
}

/// A version as semver, read leniently (`1.2` is `1.2.0`).
fn version_of(version: &str) -> String {
    let core = version.split(['-', '+']).next().unwrap_or_default();
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part.len() == 1 || !part.starts_with('0'))
        })
    {
        return version.to_owned();
    }
    let numbers: Vec<u64> = version
        .trim_start_matches(['v', 'V'])
        .split('.')
        .map_while(|part| part.parse().ok())
        .take(3)
        .collect();
    let number = |at: usize| numbers.get(at).copied().unwrap_or_default();
    format!("{}.{}.{}", number(0), number(1), number(2))
}

fn entry_of(addon: &ListedAddon) -> Option<RepoEntry> {
    let manifest = &addon.manifest;
    let provides: Vec<String> = ["catalog", "meta", "stream", "subtitles"]
        .iter()
        .filter(|resource| manifest.has_resource(resource))
        .map(|resource| format!("stremio.{resource}"))
        .collect();
    Some(RepoEntry {
        id: entry_id(&manifest.id)?,
        name: manifest.name.chars().take(80).collect(),
        version: version_of(&manifest.version),
        api_version: API_VERSION,
        description: manifest.description.clone(),
        author: None,
        provides,
        permissions: Default::default(),
        content_rating: if manifest.adult { "nsfw" } else { "safe" }.to_owned(),
        lang: None,
        package: addon.transport_url.clone(),
        icon: manifest.logo.clone(),
        sha256: String::new(),
        size: 0,
        format: PackageFormat::Stremio,
        sites: Vec::new(),
        not_for_raygun: manifest.not_for_raygun(&[]),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn collection_manifest() -> serde_json::Value {
        json!({
            "id": "org.example.collection", "name": "Open Addons", "version": "1.0.0",
            "resources": ["addon_catalog"], "types": ["movie", "series"],
            "addonCatalogs": [
                { "type": "all", "id": "official", "name": "Official" },
                { "type": "all", "id": "broken", "name": "Broken" },
            ],
        })
    }

    #[tokio::test]
    async fn a_collection_is_a_repository_of_its_addons() {
        let bytes = collection_manifest().to_string().into_bytes();
        let manifest = collection(&bytes).expect("an addon collection");
        let list = json!({ "addons": [
            { "transportName": "http", "transportUrl": "https://v3-cinemeta.strem.io/manifest.json",
              "manifest": { "id": "com.linvo.cinemeta", "name": "Cinemeta", "version": "3.0.13",
                "description": "The official addon for movie and series catalogs",
                "logo": "https://v3-cinemeta.strem.io/logo.png",
                "resources": ["catalog", "meta", "addon_catalog"], "types": ["movie", "series"],
                "catalogs": [{ "type": "movie", "id": "top", "name": "Popular" }] } },
            { "transportName": "http", "transportUrl": "https://opensubtitles-v3.strem.io/",
              "manifest": { "id": "org.stremio.opensubtitlesv3", "name": "OpenSubtitles v3",
                "version": "1.0", "resources": ["subtitles"], "types": ["movie", "series"] } },
            { "transportName": "http", "transportUrl": "https://addons.example/nested/manifest.json",
              "manifest": { "id": "Nested", "name": "Nested", "version": "1.0.0",
                "resources": ["addon_catalog"], "types": ["all"],
                "addonCatalogs": [{ "type": "all", "id": "more", "name": "More" }] } },
            { "transportName": "ipfs", "transportUrl": "https://addons.example/ipfs/manifest.json",
              "manifest": { "id": "ipfs", "name": "IPFS", "resources": ["stream"], "types": ["movie"] } },
            { "transportUrl": "https://v3-cinemeta.strem.io/manifest.json",
              "manifest": { "id": "com.linvo.cinemeta", "name": "Twice", "resources": ["meta"], "types": ["movie"] } },
            { "transportUrl": "javascript:alert(1)", "manifest": { "id": "bad", "resources": ["meta"] } },
        ] });
        let asked = std::sync::Mutex::new(Vec::new());
        let index = read(
            "https://addons.example/collection/manifest.json",
            &manifest,
            |url: String| {
                asked.lock().unwrap().push(url.clone());
                let list = list.clone();
                async move {
                    if url.ends_with("/official.json") {
                        Ok(list.to_string().into_bytes())
                    } else {
                        Err(format!("{url} answered HTTP 500"))
                    }
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(
            *asked.lock().unwrap(),
            [
                "https://addons.example/collection/addon_catalog/all/official.json",
                "https://addons.example/collection/addon_catalog/all/broken.json",
            ]
        );
        assert_eq!(index.name, "Open Addons");
        let ids: Vec<&str> = index
            .extensions
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "com.linvo.cinemeta",
                "org.stremio.opensubtitlesv3",
                "stremio.nested"
            ]
        );
        let cinemeta = &index.extensions[0];
        assert_eq!(cinemeta.format, PackageFormat::Stremio);
        assert_eq!(
            cinemeta.package,
            "https://v3-cinemeta.strem.io/manifest.json"
        );
        assert_eq!(cinemeta.provides, ["stremio.catalog", "stremio.meta"]);
        assert_eq!(cinemeta.not_for_raygun, None);
        let subtitles = &index.extensions[1];
        assert_eq!(
            subtitles.package, "https://opensubtitles-v3.strem.io/manifest.json",
            "listed by its folder, added by its manifest"
        );
        assert_eq!(subtitles.version, "1.0.0");
        assert!(
            index.extensions[2]
                .not_for_raygun
                .as_deref()
                .unwrap()
                .starts_with("Only lists other Stremio addons")
        );
        // An addon with details as well (Cinemeta) is an addon, not a collection.
        let mut cinemeta = collection_manifest();
        cinemeta["resources"] = json!(["catalog", "meta", "addon_catalog"]);
        assert!(collection(cinemeta.to_string().as_bytes()).is_none());
    }

    #[tokio::test]
    async fn a_collection_whose_lists_all_fail_says_why() {
        let manifest = collection(collection_manifest().to_string().as_bytes()).unwrap();
        let error = read(
            "https://addons.example/manifest.json",
            &manifest,
            |url| async move { Err::<Vec<u8>, _>(format!("{url} answered HTTP 503")) },
        )
        .await
        .unwrap_err();
        assert!(error.ends_with("answered HTTP 503"), "{error}");
    }
}
