//! Recent searches: what was searched for lately on this device, newest first, for Search to
//! offer again. Kept in `local_meta` (never synced); none are kept while search history is off,
//! and none are added while incognito is on (`search_recent_add`).

use raygun_db::Db;
use raygun_model::{RecentSearch, Settings};
use tokio::sync::Mutex;

const KEY: &str = "search.recent";
/// Searches kept; the oldest go first.
pub const KEPT: usize = 30;
/// Longest query kept, in characters.
const MAX_CHARS: usize = 200;
/// Search's kinds; anything else is kept as `all`.
const KINDS: [&str; 6] = ["all", "music", "video", "manga", "novel", "library"];

/// One change at a time, so two searches remembered together both stay.
static WRITING: Mutex<()> = Mutex::const_new(());

/// Whether searches are kept now. Every rule that stops Search remembering lives here.
pub fn remembers(settings: &Settings) -> bool {
    settings.privacy.search_history
}

/// The searches kept, newest first; none while search history is off.
pub async fn list(db: &Db) -> Result<Vec<RecentSearch>, String> {
    if !remembers(&settings(db).await) {
        return Ok(Vec::new());
    }
    stored(db).await
}

/// Remembers a search: first in the list, once per query and kind (case aside).
pub async fn add(db: &Db, query: &str, kind: &str, at: f64) -> Result<Vec<RecentSearch>, String> {
    let query = tidy(query);
    if query.is_empty() || !remembers(&settings(db).await) {
        return list(db).await;
    }
    let kind = if KINDS.contains(&kind) { kind } else { "all" };
    let _writing = WRITING.lock().await;
    let mut searches = stored(db).await?;
    searches.retain(|other| !same(other, &query, kind));
    searches.insert(
        0,
        RecentSearch {
            query,
            kind: kind.to_owned(),
            searched_at: at,
        },
    );
    searches.truncate(KEPT);
    store(db, &searches).await?;
    Ok(searches)
}

/// Forgets one search.
pub async fn remove(db: &Db, query: &str, kind: &str) -> Result<Vec<RecentSearch>, String> {
    let _writing = WRITING.lock().await;
    let mut searches = stored(db).await?;
    let query = tidy(query);
    searches.retain(|other| !same(other, &query, kind));
    store(db, &searches).await?;
    Ok(searches)
}

/// Forgets every search.
pub async fn clear(db: &Db) -> Result<(), String> {
    let _writing = WRITING.lock().await;
    store(db, &[]).await
}

/// Every search kept, newest first, whether search history is on or not (backups, M46.51).
pub async fn all(db: &Db) -> Result<Vec<RecentSearch>, String> {
    stored(db).await
}

/// Adds searches from a backup to the ones kept: once per query and kind, newest first.
pub async fn merge(db: &Db, more: Vec<RecentSearch>) -> Result<(), String> {
    let _writing = WRITING.lock().await;
    let mut searches = stored(db).await?;
    for search in more {
        let query = tidy(&search.query);
        if query.is_empty() || searches.iter().any(|kept| same(kept, &query, &search.kind)) {
            continue;
        }
        searches.push(RecentSearch { query, ..search });
    }
    searches.sort_by(|a, b| b.searched_at.total_cmp(&a.searched_at));
    searches.truncate(KEPT);
    store(db, &searches).await
}

async fn settings(db: &Db) -> Settings {
    crate::settings::load(db).await.unwrap_or_default()
}

async fn stored(db: &Db) -> Result<Vec<RecentSearch>, String> {
    let json = raygun_db::local_value(db, KEY)
        .await
        .map_err(|error| error.to_string())?;
    Ok(json
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default())
}

async fn store(db: &Db, searches: &[RecentSearch]) -> Result<(), String> {
    let json = serde_json::to_string(searches).map_err(|error| error.to_string())?;
    raygun_db::set_local_value(db, KEY, &json)
        .await
        .map_err(|error| error.to_string())
}

/// The query as it's kept: trimmed, inner runs of spaces made one, at most [`MAX_CHARS`].
fn tidy(query: &str) -> String {
    query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_CHARS)
        .collect()
}

fn same(search: &RecentSearch, query: &str, kind: &str) -> bool {
    search.kind == kind && search.query.to_lowercase() == query.to_lowercase()
}

#[cfg(test)]
mod tests {
    use raygun_model::{PrivacyPrefs, SettingChange};
    use tempfile::TempDir;

    use super::*;

    async fn open(temp: &TempDir) -> Db {
        Db::open(&temp.path().join("raygun.db")).await.unwrap()
    }

    #[tokio::test]
    async fn keeps_the_newest_first_once_each() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        add(&db, "daft punk", "music", 1.0).await.unwrap();
        add(&db, "  Frieren ", "manga", 2.0).await.unwrap();
        let searches = add(&db, "Daft  Punk", "music", 3.0).await.unwrap();
        let shown: Vec<_> = searches
            .iter()
            .map(|search| (search.query.as_str(), search.kind.as_str()))
            .collect();
        assert_eq!(shown, [("Daft Punk", "music"), ("Frieren", "manga")]);

        // The same words in another kind are another search; unknown kinds count as everything.
        add(&db, "frieren", "nowhere", 4.0).await.unwrap();
        assert_eq!(list(&db).await.unwrap()[0].kind, "all");
        assert_eq!(list(&db).await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn keeps_a_bounded_list() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        for index in 0..(KEPT + 5) {
            add(&db, &format!("query {index}"), "all", index as f64)
                .await
                .unwrap();
        }
        let searches = list(&db).await.unwrap();
        assert_eq!(searches.len(), KEPT);
        assert_eq!(searches[0].query, format!("query {}", KEPT + 4));
        assert!(add(&db, "   ", "all", 0.0).await.unwrap().len() == KEPT);
    }

    #[tokio::test]
    async fn forgets_one_or_all() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        add(&db, "one", "all", 1.0).await.unwrap();
        add(&db, "two", "music", 2.0).await.unwrap();
        let left = remove(&db, "ONE", "all").await.unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].query, "two");
        clear(&db).await.unwrap();
        assert!(list(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn keeps_nothing_while_search_history_is_off() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        add(&db, "kept", "all", 1.0).await.unwrap();
        let mut privacy = PrivacyPrefs {
            search_history: false,
            ..PrivacyPrefs::default()
        };
        crate::settings::update(&db, SettingChange::Privacy(privacy))
            .await
            .unwrap();
        assert!(add(&db, "secret", "all", 2.0).await.unwrap().is_empty());
        assert!(list(&db).await.unwrap().is_empty());

        privacy.search_history = true;
        crate::settings::update(&db, SettingChange::Privacy(privacy))
            .await
            .unwrap();
        let searches = list(&db).await.unwrap();
        assert_eq!(searches.len(), 1);
        assert_eq!(searches[0].query, "kept");
    }
}
