//! The extension catalogue's caches. Repositories can list thousands of extensions (Mihon's
//! stores, Shosetsu's index): an index read once is shared rather than copied for every look,
//! and each repository's listing, built from its index and what this device has, is kept while
//! neither changes.

use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use raygun_ext_host::repo::RepoIndex;
use raygun_model::{CatalogEntry, ExtensionRepo};

/// Repository indexes as read, by address, with when each was read.
#[derive(Default)]
pub(super) struct Indexes(HashMap<String, (Instant, Arc<RepoIndex>)>);

impl Indexes {
    /// The index read from `url` less than `max_age` ago.
    pub(super) fn get(&self, url: &str, max_age: Duration) -> Option<Arc<RepoIndex>> {
        self.0
            .get(url)
            .filter(|(read, _)| read.elapsed() < max_age)
            .map(|(_, index)| index.clone())
    }

    /// Keeps the index just read from `url`.
    pub(super) fn put(&mut self, url: &str, index: RepoIndex) -> Arc<RepoIndex> {
        let index = Arc::new(index);
        self.0
            .insert(url.to_owned(), (Instant::now(), index.clone()));
        index
    }
}

/// What a listing depends on besides its index (the extensions on this device and their
/// versions, conversions tried, addons added, the 18+ switch and marks), as one fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Inputs(u64);

impl Inputs {
    pub(super) fn of(parts: &impl Hash) -> Self {
        let mut hasher = DefaultHasher::new();
        parts.hash(&mut hasher);
        Self(hasher.finish())
    }
}

/// `items` in a fixed order, so the same set always gives the same fingerprint.
pub(super) fn sorted<T: Ord>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut items: Vec<T> = items.into_iter().collect();
    items.sort();
    items
}

/// One repository's listing as built.
struct Listing {
    url: String,
    index: Arc<RepoIndex>,
    inputs: Inputs,
    built: Instant,
    entries: Vec<CatalogEntry>,
}

/// Each repository's listing as last built, by the repository's id.
#[derive(Default)]
pub(super) struct Listings(HashMap<String, Listing>);

impl Listings {
    /// The listing built for `repo` from this very `index` with these `inputs`, less than
    /// `max_age` ago. Built listings go stale with time too: their icons' proxy links are handed
    /// out again when they're built (the proxy forgets links it hasn't seen for long).
    pub(super) fn get(
        &self,
        repo: &ExtensionRepo,
        index: &Arc<RepoIndex>,
        inputs: Inputs,
        max_age: Duration,
    ) -> Option<Vec<CatalogEntry>> {
        let listing = self.0.get(&repo.id)?;
        (listing.url == repo.url
            && Arc::ptr_eq(&listing.index, index)
            && listing.inputs == inputs
            && listing.built.elapsed() < max_age)
            .then(|| listing.entries.clone())
    }

    pub(super) fn put(
        &mut self,
        repo: &ExtensionRepo,
        index: Arc<RepoIndex>,
        inputs: Inputs,
        entries: Vec<CatalogEntry>,
    ) {
        self.0.insert(
            repo.id.clone(),
            Listing {
                url: repo.url.clone(),
                index,
                inputs,
                built: Instant::now(),
                entries,
            },
        );
    }

    /// Forgets the listings of repositories that are no longer added.
    pub(super) fn keep_only(&mut self, repos: &[ExtensionRepo]) {
        let added: HashSet<&str> = repos.iter().map(|repo| repo.id.as_str()).collect();
        self.0.retain(|id, _| added.contains(id.as_str()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(name: &str) -> RepoIndex {
        RepoIndex {
            format_version: 1,
            name: name.into(),
            extensions: Vec::new(),
            libraries: Vec::new(),
        }
    }

    fn repo(id: &str, url: &str) -> ExtensionRepo {
        ExtensionRepo {
            id: id.into(),
            url: url.into(),
            name: id.into(),
        }
    }

    fn entry(id: &str) -> CatalogEntry {
        CatalogEntry {
            repo_id: "r".into(),
            id: id.into(),
            name: id.into(),
            version: "1.0.0".into(),
            description: None,
            author: None,
            provides: Vec::new(),
            net: Vec::new(),
            content_rating: "safe".into(),
            adult: false,
            lang: None,
            size: 0,
            state: raygun_model::CatalogState::Available,
            nuclear: false,
            installed_version: None,
            icon: None,
            unsupported: None,
            unsupported_kind: None,
            sites: Vec::new(),
        }
    }

    const LONG: Duration = Duration::from_secs(600);

    #[test]
    fn an_index_is_shared_until_it_is_too_old() {
        let mut indexes = Indexes::default();
        assert!(indexes.get("https://a.example/index.json", LONG).is_none());
        let kept = indexes.put("https://a.example/index.json", index("A"));
        let again = indexes.get("https://a.example/index.json", LONG).unwrap();
        assert!(Arc::ptr_eq(&kept, &again), "the same index, not a copy");
        assert!(
            indexes
                .get("https://a.example/index.json", Duration::ZERO)
                .is_none()
        );
        assert!(indexes.get("https://b.example/index.json", LONG).is_none());
    }

    #[test]
    fn a_listing_is_kept_while_its_index_and_inputs_stay() {
        let mut listings = Listings::default();
        let first = Arc::new(index("A"));
        let repo_a = repo("a", "https://a.example/index.json");
        let inputs = Inputs::of(&(sorted(["x", "y"]), true));
        assert!(listings.get(&repo_a, &first, inputs, LONG).is_none());
        listings.put(&repo_a, first.clone(), inputs, vec![entry("one")]);

        let kept = listings.get(&repo_a, &first, inputs, LONG).unwrap();
        assert_eq!(kept, vec![entry("one")]);
        // The same set in another order is the same fingerprint.
        assert_eq!(Inputs::of(&(sorted(["y", "x"]), true)), inputs);

        // Something on this device changed: built again.
        let changed = Inputs::of(&(sorted(["x", "y", "z"]), true));
        assert!(listings.get(&repo_a, &first, changed, LONG).is_none());
        // The index was read again, even unchanged: built again.
        let reread = Arc::new(index("A"));
        assert!(listings.get(&repo_a, &reread, inputs, LONG).is_none());
        // The repository's address changed under the same id.
        let moved = repo("a", "https://a.example/v2/index.json");
        assert!(listings.get(&moved, &first, inputs, LONG).is_none());
        // Too old.
        assert!(
            listings
                .get(&repo_a, &first, inputs, Duration::ZERO)
                .is_none()
        );
    }

    #[test]
    fn removed_repositories_lose_their_listings() {
        let mut listings = Listings::default();
        let shared = Arc::new(index("A"));
        let inputs = Inputs::of(&0);
        let (repo_a, repo_b) = (
            repo("a", "https://a.example/index.json"),
            repo("b", "https://b.example/index.json"),
        );
        listings.put(&repo_a, shared.clone(), inputs, vec![entry("one")]);
        listings.put(&repo_b, shared.clone(), inputs, vec![entry("two")]);
        listings.keep_only(std::slice::from_ref(&repo_b));
        assert!(listings.get(&repo_a, &shared, inputs, LONG).is_none());
        assert!(listings.get(&repo_b, &shared, inputs, LONG).is_some());
    }
}

/// Timings of the catalogue's work with many entries, before and after its caches.
#[cfg(test)]
mod timings {
    use std::time::Instant;

    use raygun_ext_host::repo;
    use raygun_model::{CatalogEntry, ExtensionRepo, RepoCatalog};

    use super::super::catalog_entry;
    use super::*;

    /// A Raygun index listing `count` extensions, each with an icon.
    fn index_json(count: usize) -> String {
        let entries: Vec<serde_json::Value> = (0..count)
            .map(|index| {
                serde_json::json!({
                    "id": format!("dev.example.source{index}"),
                    "name": format!("Source {index}"),
                    "version": "1.2.3",
                    "apiVersion": raygun_ext_host::manifest::API_VERSION,
                    "description": format!("Reads comics from example{index}.org, with search, latest and filters."),
                    "author": "Someone",
                    "provides": ["manga.source"],
                    "permissions": { "net": [format!("example{index}.org")] },
                    "contentRating": "safe",
                    "lang": "en",
                    "package": format!("packages/source{index}.zip"),
                    "icon": format!("icons/source{index}.png"),
                    "sha256": "0".repeat(64),
                    "size": 12000,
                })
            })
            .collect();
        serde_json::json!({ "formatVersion": 1, "name": "Many", "extensions": entries }).to_string()
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "timings: cargo test -p raygun-app -- --ignored catalog_timings --nocapture"]
    async fn catalog_timings() {
        const COUNT: usize = 5000;
        let json = index_json(COUNT);
        let started = Instant::now();
        let index = repo::parse_index(&json).unwrap();
        println!(
            "parse {} entries ({} KB): {:?}",
            index.extensions.len(),
            json.len() / 1024,
            started.elapsed()
        );
        let started = Instant::now();
        let copy = index.clone();
        println!("copy the index (each look before): {:?}", started.elapsed());
        drop(copy);
        let mut indexes = Indexes::default();
        let shared = indexes.put("https://example.org/repo/index.json", index);
        let started = Instant::now();
        let again = indexes
            .get(
                "https://example.org/repo/index.json",
                Duration::from_secs(60),
            )
            .unwrap();
        println!("share the index (each look now): {:?}", started.elapsed());

        let proxy = raygun_net::StreamProxy::start(raygun_net::http_client().unwrap(), None)
            .await
            .unwrap();
        let images = proxy.images();
        let base: reqwest::Url = "https://example.org/repo/index.json".parse().unwrap();
        let started = Instant::now();
        let entries: Vec<CatalogEntry> = again
            .extensions
            .iter()
            .map(|entry| CatalogEntry {
                icon: repo::icon_url(&base, entry)
                    .map(|url| images.cover(url.as_str(), &Default::default())),
                ..catalog_entry("repo", entry, None)
            })
            .collect();
        println!(
            "build the listing (each look before): {:?}",
            started.elapsed()
        );
        let repo = ExtensionRepo {
            id: "repo".into(),
            url: base.to_string(),
            name: "Many".into(),
        };
        let inputs = Inputs::of(&(sorted(["dev.example.source1"]), false));
        let mut listings = Listings::default();
        listings.put(&repo, shared.clone(), inputs, entries);
        let started = Instant::now();
        let kept = listings
            .get(&repo, &shared, inputs, Duration::from_secs(60))
            .unwrap();
        println!(
            "take the kept listing (each look now): {:?}",
            started.elapsed()
        );
        let listings = vec![RepoCatalog {
            repo,
            entries: kept,
            error: None,
        }];
        let started = Instant::now();
        let text = serde_json::to_string(&listings).unwrap();
        println!(
            "serialize the listing for the UI ({} KB): {:?}",
            text.len() / 1024,
            started.elapsed()
        );

        let dir = tempfile::tempdir().unwrap();
        let db = raygun_db::Db::open(&dir.path().join("raygun.db"))
            .await
            .unwrap();
        for index in 0..400 {
            raygun_db::set_local_value(
                &db,
                &format!("extension.icon.dev.example.source{index}"),
                &format!("https://example.org/icons/{index}.png"),
            )
            .await
            .unwrap();
        }
        let started = Instant::now();
        for index in 0..400 {
            raygun_db::local_value(&db, &format!("extension.icon.dev.example.source{index}"))
                .await
                .unwrap();
        }
        println!(
            "400 icons, one query each (before): {:?}",
            started.elapsed()
        );
        let started = Instant::now();
        let icons = raygun_db::local_values_with_prefix(&db, "extension.icon.")
            .await
            .unwrap();
        println!(
            "{} icons in one query (now): {:?}",
            icons.len(),
            started.elapsed()
        );
    }
}
