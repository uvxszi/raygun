//! Artists' pages kept on this device: popular songs, what open databases say, the discography
//! and coming events, each part in its own file under `cache/artists/`. A page opens from the kept
//! parts at once. A part older than a day still shows while it's asked again in the background,
//! and [`crate::events::ArtistProfileEvent`] says when a changed one is in.
//!
//! An empty answer (nobody knew, or the sources were down) never replaces a full one; it counts as
//! fresh for an hour only, so a missing part is asked again soon.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// How long a part with something in it is fresh.
const FRESH_MS: u64 = 24 * 60 * 60 * 1000;
/// How long an empty part is fresh.
const EMPTY_FRESH_MS: u64 = 60 * 60 * 1000;
/// After a background ask brought nothing, the next waits this long.
const RETRY_AFTER: Duration = Duration::from_secs(30 * 60);
/// Files kept at most (four parts an artist); the least recently written go first.
const MAX_FILES: usize = 2000;
/// The folder is trimmed after this many writes.
const TRIM_EVERY: usize = 50;

/// The parts of an artist's page, kept and asked again on their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Popular,
    Info,
    Releases,
    Events,
}

impl Part {
    fn name(self) -> &'static str {
        match self {
            Self::Popular => "popular",
            Self::Info => "info",
            Self::Releases => "releases",
            Self::Events => "events",
        }
    }
}

/// An answer that may have nothing in it.
pub trait Answer {
    fn is_empty_answer(&self) -> bool;
}

impl<T> Answer for Vec<T> {
    fn is_empty_answer(&self) -> bool {
        self.is_empty()
    }
}

impl<T> Answer for Option<T> {
    fn is_empty_answer(&self) -> bool {
        self.is_none()
    }
}

#[derive(Serialize, Deserialize)]
struct Kept<T> {
    /// The artist as asked, so two names never share a file.
    name: String,
    saved_ms: u64,
    value: T,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

/// The name parts are kept by: trimmed and in lower case.
fn folded(name: &str) -> String {
    name.trim().to_lowercase()
}

type Changed = Box<dyn Fn(&str) + Send + Sync>;

pub struct ArtistProfiles {
    dir: PathBuf,
    /// Told the artist's name when a background ask changed a part.
    changed: Changed,
    /// Parts being asked again now.
    refreshing: Mutex<HashSet<String>>,
    /// When a background ask last brought nothing, by part.
    empty_at: Mutex<HashMap<String, Instant>>,
    writes: AtomicUsize,
}

impl ArtistProfiles {
    pub fn new(dir: PathBuf, changed: impl Fn(&str) + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            dir,
            changed: Box::new(changed),
            refreshing: Mutex::new(HashSet::new()),
            empty_at: Mutex::new(HashMap::new()),
            writes: AtomicUsize::new(0),
        })
    }

    fn file(&self, name: &str, part: Part) -> PathBuf {
        let digest = Sha256::digest(folded(name).as_bytes());
        let hex: String = digest[..16]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self.dir.join(format!("{hex}.{}.json", part.name()))
    }

    /// The part of `name`'s page: the kept one when it's fresh; a stale one at once while
    /// `fetch` asks again in the background; without one, `fetch`'s answer.
    pub async fn get<T, F, Fut>(self: &Arc<Self>, name: &str, part: Part, fetch: F) -> T
    where
        T: Answer + Serialize + DeserializeOwned + Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = T> + Send + 'static,
    {
        let file = self.file(name, part);
        match read::<T>(&file, name).await {
            Some(kept) if fresh(&kept, now_ms()) => kept.value,
            Some(kept) => {
                let shown = serde_json::to_value(&kept.value).unwrap_or_default();
                self.refresh_later(name, file, kept.value.is_empty_answer(), shown, fetch());
                kept.value
            }
            None => {
                let value = fetch().await;
                self.write(&file, name, &value).await;
                value
            }
        }
    }

    /// Asks for a stale part again in the background, once at a time; the kept one stays when
    /// the new answer is empty and it isn't.
    fn refresh_later<T, Fut>(
        self: &Arc<Self>,
        name: &str,
        file: PathBuf,
        kept_empty: bool,
        shown: serde_json::Value,
        fetching: Fut,
    ) where
        T: Answer + Serialize + Send + Sync + 'static,
        Fut: Future<Output = T> + Send + 'static,
    {
        let key = file.to_string_lossy().into_owned();
        let tried_lately = self
            .empty_at
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&key)
            .is_some_and(|at| at.elapsed() < RETRY_AFTER);
        if tried_lately
            || !self
                .refreshing
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .insert(key.clone())
        {
            return;
        }
        let profiles = Arc::clone(self);
        let name = name.to_owned();
        tokio::spawn(async move {
            let value = fetching.await;
            if value.is_empty_answer() && !kept_empty {
                profiles
                    .empty_at
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .insert(key.clone(), Instant::now());
            } else {
                profiles.write(&file, &name, &value).await;
                if serde_json::to_value(&value).unwrap_or_default() != shown {
                    (profiles.changed)(&name);
                }
            }
            profiles
                .refreshing
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&key);
        });
    }

    async fn write<T: Serialize>(&self, file: &Path, name: &str, value: &T) {
        let kept = Kept {
            name: folded(name),
            saved_ms: now_ms(),
            value,
        };
        let Ok(json) = serde_json::to_vec(&kept) else {
            return;
        };
        // Written whole, then put in place: a reader never sees half a file.
        let partial = file.with_extension("part");
        let stored = async {
            tokio::fs::create_dir_all(&self.dir).await?;
            tokio::fs::write(&partial, &json).await?;
            tokio::fs::rename(&partial, file).await
        };
        if let Err(error) = stored.await {
            tracing::debug!(%error, "couldn't keep an artist's page");
            return;
        }
        if self.writes.fetch_add(1, Ordering::Relaxed) % TRIM_EVERY == TRIM_EVERY - 1 {
            self.trim_later();
        }
    }

    /// Keeps the folder at most [`MAX_FILES`] files, dropping the least recently written.
    pub fn trim_later(&self) {
        let dir = self.dir.clone();
        tauri::async_runtime::spawn_blocking(move || trim(&dir, MAX_FILES));
    }
}

fn fresh<T: Answer>(kept: &Kept<T>, now: u64) -> bool {
    let lasts = if kept.value.is_empty_answer() {
        EMPTY_FRESH_MS
    } else {
        FRESH_MS
    };
    now.saturating_sub(kept.saved_ms) < lasts
}

async fn read<T: DeserializeOwned>(file: &Path, name: &str) -> Option<Kept<T>> {
    let bytes = tokio::fs::read(file).await.ok()?;
    let kept: Kept<T> = serde_json::from_slice(&bytes).ok()?;
    (kept.name == folded(name)).then_some(kept)
}

fn trim(dir: &Path, max: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect();
    if files.len() <= max {
        return;
    }
    files.sort_by_key(|(modified, _)| *modified);
    let extra = files.len() - max;
    for (_, path) in files.into_iter().take(extra) {
        let _ = std::fs::remove_file(path);
    }
}

/// Events from `today` (`YYYY-MM-DD`) on; a date the source knows only to the month or the year
/// counts while that month or year lasts.
pub fn upcoming(events: &mut Vec<raygun_model::ArtistEvent>, today: &str) {
    events.retain(|event| {
        let date = event.date.trim();
        let known = date.len().min(today.len());
        date.is_empty() || date >= today.get(..known).unwrap_or(today)
    });
}

/// Today's date in UTC, as `YYYY-MM-DD`.
pub fn today() -> String {
    let seconds = (now_ms() / 1000) as i64;
    chrono::DateTime::from_timestamp(seconds, 0)
        .map(|moment| moment.date_naive().format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU32;

    use super::*;

    fn profiles(dir: &Path) -> (Arc<ArtistProfiles>, Arc<Mutex<Vec<String>>>) {
        let told = Arc::new(Mutex::new(Vec::new()));
        let heard = told.clone();
        let profiles = ArtistProfiles::new(dir.to_path_buf(), move |name| {
            heard.lock().unwrap().push(name.to_owned());
        });
        (profiles, told)
    }

    /// Makes the kept part look `age_ms` old.
    async fn age(profiles: &ArtistProfiles, name: &str, part: Part, age_ms: u64) {
        let file = profiles.file(name, part);
        let mut kept: serde_json::Value =
            serde_json::from_slice(&tokio::fs::read(&file).await.unwrap()).unwrap();
        kept["saved_ms"] = (now_ms() - age_ms).into();
        tokio::fs::write(&file, serde_json::to_vec(&kept).unwrap())
            .await
            .unwrap();
    }

    async fn settled(profiles: &ArtistProfiles) {
        for _ in 0..200 {
            if profiles.refreshing.lock().unwrap().is_empty() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    #[tokio::test]
    async fn a_kept_part_answers_without_asking_until_a_day_has_passed() {
        let temp = tempfile::tempdir().unwrap();
        let (profiles, told) = profiles(temp.path());
        let asked = Arc::new(AtomicU32::new(0));
        let ask = |answer: Vec<String>| {
            let asked = asked.clone();
            move || async move {
                asked.fetch_add(1, Ordering::SeqCst);
                answer
            }
        };

        let first = profiles
            .get("Daft Punk", Part::Releases, ask(vec!["Discovery".into()]))
            .await;
        assert_eq!(first, vec!["Discovery".to_owned()]);
        // Kept by name in any case, and fresh: not asked again.
        let again = profiles
            .get(" daft punk ", Part::Releases, ask(vec!["Other".into()]))
            .await;
        assert_eq!(again, vec!["Discovery".to_owned()]);
        assert_eq!(asked.load(Ordering::SeqCst), 1);

        // A day later the kept one shows while it's asked again, and the change is told.
        age(&profiles, "Daft Punk", Part::Releases, FRESH_MS + 1).await;
        let stale = profiles
            .get(
                "Daft Punk",
                Part::Releases,
                ask(vec!["Discovery".into(), "Homework".into()]),
            )
            .await;
        assert_eq!(stale, vec!["Discovery".to_owned()]);
        settled(&profiles).await;
        assert_eq!(asked.load(Ordering::SeqCst), 2);
        assert_eq!(told.lock().unwrap().as_slice(), ["Daft Punk".to_owned()]);
        let fresh = profiles
            .get("Daft Punk", Part::Releases, ask(Vec::new()))
            .await;
        assert_eq!(fresh, vec!["Discovery".to_owned(), "Homework".to_owned()]);
    }

    #[tokio::test]
    async fn an_empty_answer_never_replaces_a_full_one() {
        let temp = tempfile::tempdir().unwrap();
        let (profiles, told) = profiles(temp.path());
        profiles
            .get("Air", Part::Info, || async {
                Some("French duo".to_owned())
            })
            .await;
        age(&profiles, "Air", Part::Info, FRESH_MS + 1).await;
        let shown = profiles
            .get("Air", Part::Info, || async { None::<String> })
            .await;
        assert_eq!(shown.as_deref(), Some("French duo"));
        settled(&profiles).await;
        let kept = profiles
            .get("Air", Part::Info, || async { None::<String> })
            .await;
        assert_eq!(kept.as_deref(), Some("French duo"));
        assert!(told.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_empty_part_is_asked_again_after_an_hour() {
        let temp = tempfile::tempdir().unwrap();
        let (profiles, _) = profiles(temp.path());
        let none = profiles
            .get("Nobody", Part::Events, || async { Vec::<String>::new() })
            .await;
        assert!(none.is_empty());
        let still = profiles
            .get("Nobody", Part::Events, || async { vec!["Show".to_owned()] })
            .await;
        assert!(still.is_empty(), "fresh for an hour");
        age(&profiles, "Nobody", Part::Events, EMPTY_FRESH_MS + 1).await;
        profiles
            .get("Nobody", Part::Events, || async { vec!["Show".to_owned()] })
            .await;
        settled(&profiles).await;
        let found = profiles
            .get("Nobody", Part::Events, || async { Vec::<String>::new() })
            .await;
        assert_eq!(found, vec!["Show".to_owned()]);
    }

    #[test]
    fn the_folder_keeps_the_newest_files() {
        let temp = tempfile::tempdir().unwrap();
        for index in 0..5 {
            let path = temp.path().join(format!("{index}.json"));
            std::fs::write(&path, b"{}").unwrap();
            let when = SystemTime::now() - Duration::from_secs(100 - index);
            std::fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(when)
                .unwrap();
        }
        trim(temp.path(), 3);
        let mut left: Vec<String> = std::fs::read_dir(temp.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["2.json", "3.json", "4.json"]);
    }

    #[test]
    fn past_events_are_left_out() {
        let event = |date: &str| raygun_model::ArtistEvent {
            name: date.to_owned(),
            date: date.to_owned(),
            place: None,
            url: None,
            source: "MusicBrainz".into(),
        };
        let mut events = vec![
            event("2026-11-08"),
            event("2026-11-09"),
            event("2026-11"),
            event("2026-10"),
            event("2027"),
            event(""),
        ];
        upcoming(&mut events, "2026-11-09");
        let left: Vec<&str> = events.iter().map(|event| event.date.as_str()).collect();
        assert_eq!(left, ["2026-11-09", "2026-11", "2027", ""]);
    }
}
