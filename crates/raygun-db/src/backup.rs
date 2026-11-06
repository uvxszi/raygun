//! Backups (M30, M46.51): the rows worth keeping, as the same JSON objects device sync sends,
//! grouped into parts (comics, playlists, settings…) so a backup can hold some of them and a
//! restore can take some of what a backup holds. A restore merges rows in (last writer wins per
//! row). Rows for an item this database already has under another id (a title fetched on both
//! devices, a podcast followed on both) take our id, and their children follow.
//!
//! Device-local records (installed extensions, downloads) travel too, but are never merged here:
//! the app reads them back to install and download again.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{AssertSqlSafe, Row, Sqlite, Transaction};

use crate::sync::{SyncKind, TABLES, TableSpec, is_newer, read_row, text_field, write_row};
use crate::{Db, DbError, persist_hlc};

pub const FORMAT: &str = "raygun-backup";
/// 2 (M46.51): parts, more tables, and the app's own values.
pub const VERSION: u32 = 2;

const STAMP: [&str; 3] = ["updated_at_hlc", "origin_device", "deleted"];

/// The setting that holds quick access (its pins and when its pages were last played from): its
/// own part.
pub const PINS_KEY: &str = "rail";

/// One kind of data a backup holds, restores and a reset clears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Part {
    Comics,
    Books,
    Videos,
    Podcasts,
    Audiobooks,
    Websites,
    /// Playlists and favorite songs.
    Playlists,
    /// Quick access (the `rail` setting): its pins and when its pages were last played from.
    Pins,
    /// Songs played and videos watched.
    History,
    /// Every setting but quick access's.
    Settings,
    /// Repositories, installed extensions, their settings and Stremio addons.
    Sources,
    /// What was downloaded (records; files stay on their device).
    Downloads,
    /// Local Files' folders (records: what's in them is found again by a scan).
    LocalFiles,
}

impl Part {
    pub const ALL: [Part; 13] = [
        Part::Comics,
        Part::Books,
        Part::Videos,
        Part::Podcasts,
        Part::Audiobooks,
        Part::Websites,
        Part::Playlists,
        Part::Pins,
        Part::History,
        Part::Settings,
        Part::Sources,
        Part::Downloads,
        Part::LocalFiles,
    ];

    /// `title.kind` of a library part.
    pub fn title_kind(self) -> Option<&'static str> {
        match self {
            Part::Comics => Some("manga"),
            Part::Books => Some("novel"),
            Part::Videos => Some("video"),
            _ => None,
        }
    }

    /// `podcast.remote_kind` of a listening part.
    pub fn podcast_kind(self) -> Option<&'static str> {
        match self {
            Part::Podcasts => Some("podcast"),
            Part::Audiobooks => Some("audiobook"),
            _ => None,
        }
    }

    fn of_title_kind(kind: &str) -> Option<Part> {
        match kind {
            "manga" => Some(Part::Comics),
            "novel" => Some(Part::Books),
            "video" => Some(Part::Videos),
            _ => None,
        }
    }

    fn of_podcast_kind(kind: &str) -> Part {
        if kind == "audiobook" {
            Part::Audiobooks
        } else {
            Part::Podcasts
        }
    }
}

/// What a count counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Item {
    /// Library titles.
    Titles,
    /// Chapters of library titles (comics, books); downloaded chapters under Downloads.
    Chapters,
    /// Chapters read to the end.
    Read,
    /// Episodes of library titles or followed shows; downloaded episodes under Downloads.
    Episodes,
    /// Episodes watched to the end; videos watched under History.
    Watched,
    Categories,
    /// Followed podcasts.
    Shows,
    /// Audiobooks in the library.
    Books,
    Sites,
    Playlists,
    PlaylistSongs,
    Favorites,
    Pins,
    /// Pages quick access sorts by when they were last played from.
    Played,
    Settings,
    Repositories,
    Extensions,
    Addons,
    Plays,
    /// Downloaded songs.
    Songs,
    /// Downloaded videos.
    Videos,
    /// Downloads waiting or failed: records without a file.
    Waiting,
    /// Local Files' folders.
    Folders,
}

/// Counts by part and item, in a stable order.
pub type Counts = BTreeMap<Part, BTreeMap<Item, u32>>;

/// Tables merged on restore besides the synced ones, in restore order (parents first).
const EXTRA: &[TableSpec] = &[
    TableSpec {
        name: "extension_repo",
        key: "id",
        columns: &["id", "url", "name", STAMP[0], STAMP[1], STAMP[2]],
        kind: SyncKind::Library,
    },
    TableSpec {
        name: "stremio_addon",
        key: "id",
        columns: &[
            "id",
            "transport_url",
            "manifest",
            "fetched_at",
            "position",
            "enabled",
            STAMP[0],
            STAMP[1],
            STAMP[2],
        ],
        kind: SyncKind::Library,
    },
    TableSpec {
        name: "website",
        key: "id",
        columns: &[
            "id",
            "name",
            "url",
            "user_last_url",
            "user_block_ads",
            "position",
            STAMP[0],
            STAMP[1],
            STAMP[2],
        ],
        kind: SyncKind::Library,
    },
    TableSpec {
        name: "video_progress",
        key: "id",
        columns: &[
            "id",
            "key",
            "title",
            "subtitle",
            "poster",
            "request",
            "position_ms",
            "duration_ms",
            "user_watched_at",
            "updated_at",
            STAMP[0],
            STAMP[1],
            STAMP[2],
        ],
        kind: SyncKind::History,
    },
    TableSpec {
        name: "title",
        key: "id",
        columns: &[
            "id",
            "kind",
            "source_id",
            "remote_id",
            "remote_title",
            "remote_cover",
            "remote_cover_headers",
            "remote_authors",
            "remote_artists",
            "remote_description",
            "remote_genres",
            "remote_status",
            "remote_url",
            "remote_media_type",
            "remote_year",
            "user_in_library_at",
            "user_reader_mode",
            "user_list",
            "user_favorite_at",
            "user_pinned_at",
            "state_refreshed_at",
            STAMP[0],
            STAMP[1],
            STAMP[2],
        ],
        kind: SyncKind::Library,
    },
    TableSpec {
        name: "unit",
        key: "id",
        columns: &[
            "id",
            "title_id",
            "kind",
            "remote_id",
            "remote_name",
            "number",
            "volume",
            "remote_scanlator",
            "remote_date",
            "source_order",
            "state_found_at",
            STAMP[0],
            STAMP[1],
            STAMP[2],
        ],
        kind: SyncKind::Library,
    },
    TableSpec {
        name: "unit_progress",
        key: "unit_id",
        columns: &[
            "unit_id",
            "position",
            "total",
            "completed_at",
            STAMP[0],
            STAMP[1],
            STAMP[2],
        ],
        kind: SyncKind::Library,
    },
    TableSpec {
        name: "category",
        key: "id",
        columns: &[
            "id",
            "kind",
            "name",
            "position",
            "user_display",
            STAMP[0],
            STAMP[1],
            STAMP[2],
        ],
        kind: SyncKind::Library,
    },
    TableSpec {
        name: "podcast",
        key: "id",
        columns: &[
            "id",
            "feed_url",
            "remote_title",
            "remote_author",
            "remote_description",
            "remote_artwork",
            "remote_link",
            "remote_kind",
            "user_subscribed_at",
            "user_episode_order",
            "user_speed",
            "state_refreshed_at",
            STAMP[0],
            STAMP[1],
            STAMP[2],
        ],
        kind: SyncKind::Library,
    },
    TableSpec {
        name: "podcast_episode",
        key: "id",
        columns: &[
            "id",
            "podcast_id",
            "guid",
            "remote_title",
            "remote_description",
            "remote_audio_url",
            "remote_audio_type",
            "remote_duration_ms",
            "remote_published_at",
            "remote_artwork",
            "remote_number",
            "user_position_ms",
            "user_played_at",
            "user_listened_at",
            "state_found_at",
            STAMP[0],
            STAMP[1],
            STAMP[2],
        ],
        kind: SyncKind::Library,
    },
];

/// A table keyed by two columns, merged after the others (its parents are in by then).
struct Pair {
    name: &'static str,
    keys: [&'static str; 2],
    columns: &'static [&'static str],
}

const PAIRS: &[Pair] = &[
    Pair {
        name: "title_category",
        keys: ["title_id", "category_id"],
        columns: &["title_id", "category_id", STAMP[0], STAMP[1], STAMP[2]],
    },
    Pair {
        name: "extension_setting",
        keys: ["extension_id", "key"],
        columns: &["extension_id", "key", "value", STAMP[0], STAMP[1], STAMP[2]],
    },
];

/// A device-local table backed up as it is and never merged.
struct Record {
    name: &'static str,
    order: &'static str,
    columns: &'static [&'static str],
}

const RECORDS: &[Record] = &[
    Record {
        name: "local_extension_install",
        order: "extension_id",
        columns: &["extension_id", "repo_url", "version", "installed_at"],
    },
    Record {
        name: "local_music_download",
        order: "id",
        columns: &[
            "id",
            "track_key",
            "title",
            "artist",
            "album",
            "duration_ms",
            "artwork_url",
            "source",
            "state",
            "queued_at",
        ],
    },
    Record {
        name: "local_manga_download",
        order: "unit_id",
        columns: &["unit_id", "title_id", "state", "queued_at"],
    },
    Record {
        name: "local_podcast_download",
        order: "episode_id",
        columns: &["episode_id", "state"],
    },
    Record {
        name: "local_video_download",
        order: "id",
        columns: &["id", "key", "title_id", "title", "subtitle", "state"],
    },
    // How each folder is read (`access`, migration 0039) comes too: a picked tree comes back only
    // while its grant is held, a path while it's a folder.
    Record {
        name: "local_folder",
        order: "added_at",
        columns: &["id", "location", "name", "path", "access", "added_at"],
    },
];

fn merged() -> impl Iterator<Item = &'static TableSpec> {
    TABLES.iter().chain(EXTRA.iter())
}

/// A backup's contents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupData {
    pub format: String,
    pub version: u32,
    /// Unix ms.
    pub created_at: i64,
    pub device_id: String,
    pub tables: BTreeMap<String, Vec<Value>>,
    /// The app's own values (recent searches, connected servers…), by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, Value>,
}

impl BackupData {
    /// How many live (not deleted) rows a table holds.
    pub fn live_rows(&self, table: &str) -> usize {
        self.rows(table).filter(|row| is_live(row)).count()
    }

    /// A table's rows (none when the backup doesn't have it).
    pub fn rows(&self, table: &str) -> impl Iterator<Item = &Value> {
        self.tables.get(table).into_iter().flatten()
    }
}

fn is_live(row: &Value) -> bool {
    row.get("deleted").and_then(Value::as_i64) != Some(1)
}

fn has(row: &Value, field: &str) -> bool {
    row.get(field).is_some_and(|value| !value.is_null())
}

/// Which part titles, units and podcasts belong to, by id, and which are in the library.
#[derive(Default)]
struct Owners {
    titles: HashMap<String, Part>,
    units: HashMap<String, Part>,
    podcasts: HashMap<String, Part>,
    library_titles: HashSet<String>,
    library_units: HashSet<String>,
    followed: HashSet<String>,
}

impl Owners {
    fn of(data: &BackupData) -> Self {
        let mut owners = Owners::default();
        for row in data.rows("title") {
            let (Some(id), Some(part)) = (
                text_field(row, "id"),
                text_field(row, "kind").and_then(|kind| Part::of_title_kind(&kind)),
            ) else {
                continue;
            };
            if is_live(row) && has(row, "user_in_library_at") {
                owners.library_titles.insert(id.clone());
            }
            owners.titles.insert(id, part);
        }
        for row in data.rows("unit") {
            let (Some(id), Some(title)) = (text_field(row, "id"), text_field(row, "title_id"))
            else {
                continue;
            };
            if let Some(part) = owners.titles.get(&title).copied() {
                if is_live(row) && owners.library_titles.contains(&title) {
                    owners.library_units.insert(id.clone());
                }
                owners.units.insert(id, part);
            }
        }
        for row in data.rows("podcast") {
            let Some(id) = text_field(row, "id") else {
                continue;
            };
            let kind = text_field(row, "remote_kind").unwrap_or_default();
            if is_live(row) && has(row, "user_subscribed_at") {
                owners.followed.insert(id.clone());
            }
            owners.podcasts.insert(id, Part::of_podcast_kind(&kind));
        }
        owners
    }
}

/// The part a row belongs to, if any.
fn part_of(table: &str, row: &Value, owners: &Owners) -> Option<Part> {
    let owner = |field: &str, of: &HashMap<String, Part>| {
        text_field(row, field).and_then(|id| of.get(&id).copied())
    };
    match table {
        "setting" if text_field(row, "key").as_deref() == Some(PINS_KEY) => Some(Part::Pins),
        "setting" => Some(Part::Settings),
        "music_favorite" | "music_playlist" | "music_playlist_item" => Some(Part::Playlists),
        "music_play_history" | "video_progress" => Some(Part::History),
        "extension_repo" | "stremio_addon" | "extension_setting" | "local_extension_install" => {
            Some(Part::Sources)
        }
        "website" => Some(Part::Websites),
        "title" => text_field(row, "kind").and_then(|kind| Part::of_title_kind(&kind)),
        "unit" | "title_category" => owner("title_id", &owners.titles),
        "unit_progress" => owner("unit_id", &owners.units),
        "category" => text_field(row, "kind").and_then(|kind| Part::of_title_kind(&kind)),
        "podcast" => Some(Part::of_podcast_kind(
            &text_field(row, "remote_kind").unwrap_or_default(),
        )),
        "podcast_episode" => owner("podcast_id", &owners.podcasts),
        "local_music_download"
        | "local_manga_download"
        | "local_podcast_download"
        | "local_video_download" => Some(Part::Downloads),
        "local_folder" => Some(Part::LocalFiles),
        _ => None,
    }
}

/// Every row of `table` as JSON objects (tombstones included, so deletions win on restore too).
async fn read_table(
    db: &Db,
    table: &str,
    columns: &[&str],
    order: &str,
) -> Result<Vec<Value>, DbError> {
    let pairs = columns
        .iter()
        .map(|column| format!("'{column}', {column}"))
        .collect::<Vec<_>>()
        .join(", ");
    // Table and column names come from the constants above.
    let sql = format!("SELECT json_object({pairs}) AS row FROM {table} ORDER BY {order}");
    Ok(sqlx::query(AssertSqlSafe(sql))
        .fetch_all(db.pool())
        .await?
        .iter()
        .filter_map(|row| serde_json::from_str(&row.get::<String, _>("row")).ok())
        .collect())
}

/// The rows of `parts`, every one of them (tombstones included).
pub async fn export(db: &Db, now_ms: i64, parts: &[Part]) -> Result<BackupData, DbError> {
    let mut tables = BTreeMap::new();
    for spec in merged() {
        let rows = read_table(db, spec.name, spec.columns, spec.key).await?;
        tables.insert(spec.name.to_owned(), rows);
    }
    for pair in PAIRS {
        let order = pair.keys.join(", ");
        let rows = read_table(db, pair.name, pair.columns, &order).await?;
        tables.insert(pair.name.to_owned(), rows);
    }
    for record in RECORDS {
        let rows = read_table(db, record.name, record.columns, record.order).await?;
        tables.insert(record.name.to_owned(), rows);
    }
    let mut data = BackupData {
        format: FORMAT.into(),
        version: VERSION,
        created_at: now_ms,
        device_id: db.device_id().to_string(),
        tables,
        values: BTreeMap::new(),
    };
    keep_parts(&mut data, parts);
    Ok(data)
}

/// Leaves only the rows of `parts` (tables left empty go).
pub fn keep_parts(data: &mut BackupData, parts: &[Part]) {
    let owners = Owners::of(data);
    for (table, rows) in &mut data.tables {
        rows.retain(|row| part_of(table, row, &owners).is_some_and(|part| parts.contains(&part)));
    }
    data.tables.retain(|_, rows| !rows.is_empty());
}

/// The parts a backup has rows of.
pub fn parts_in(data: &BackupData) -> Vec<Part> {
    let owners = Owners::of(data);
    let mut parts: Vec<Part> = data
        .tables
        .iter()
        .flat_map(|(table, rows)| rows.iter().filter_map(|row| part_of(table, row, &owners)))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    parts.sort();
    parts
}

/// What a backup holds, counted as [`crate::data::counts`] counts a database.
pub fn index(data: &BackupData) -> Counts {
    let owners = Owners::of(data);
    let mut counts = Counts::new();
    let mut add = |part: Part, item: Item, by: u32| {
        *counts.entry(part).or_default().entry(item).or_default() += by;
    };
    let live = |table: &'static str| data.rows(table).filter(|row| is_live(row));
    let done = |row: &Value| text_field(row, "state").as_deref() == Some("done");
    for row in live("title") {
        if let (Some(id), Some(part)) = (
            text_field(row, "id"),
            text_field(row, "kind").and_then(|kind| Part::of_title_kind(&kind)),
        ) && owners.library_titles.contains(&id)
        {
            add(part, Item::Titles, 1);
        }
    }
    for row in live("unit") {
        let Some(id) = text_field(row, "id") else {
            continue;
        };
        if let (true, Some(part)) = (
            owners.library_units.contains(&id),
            owners.units.get(&id).copied(),
        ) {
            add(part, units_item(part), 1);
        }
    }
    for row in live("unit_progress") {
        let Some(unit) = text_field(row, "unit_id") else {
            continue;
        };
        if let (true, true, Some(part)) = (
            has(row, "completed_at"),
            owners.library_units.contains(&unit),
            owners.units.get(&unit).copied(),
        ) {
            add(part, finished_item(part), 1);
        }
    }
    for row in live("category") {
        if let Some(part) = text_field(row, "kind").and_then(|kind| Part::of_title_kind(&kind)) {
            add(part, Item::Categories, 1);
        }
    }
    for row in live("podcast") {
        let Some(id) = text_field(row, "id") else {
            continue;
        };
        if owners.followed.contains(&id) {
            let part = owners.podcasts[&id];
            add(part, followed_item(part), 1);
        }
    }
    for row in live("podcast_episode") {
        if let Some(podcast) = text_field(row, "podcast_id")
            && owners.followed.contains(&podcast)
        {
            add(owners.podcasts[&podcast], Item::Episodes, 1);
        }
    }
    let mut add_rows = |table: &'static str, part: Part, item: Item| {
        let count = data.rows(table).filter(|row| is_live(row)).count() as u32;
        if count > 0 {
            add(part, item, count);
        }
    };
    add_rows("website", Part::Websites, Item::Sites);
    add_rows("music_playlist", Part::Playlists, Item::Playlists);
    add_rows("music_favorite", Part::Playlists, Item::Favorites);
    add_rows("music_play_history", Part::History, Item::Plays);
    add_rows("video_progress", Part::History, Item::Watched);
    add_rows("extension_repo", Part::Sources, Item::Repositories);
    add_rows("stremio_addon", Part::Sources, Item::Addons);
    add_rows("local_extension_install", Part::Sources, Item::Extensions);
    add_rows("local_folder", Part::LocalFiles, Item::Folders);
    let live_playlists: HashSet<String> = live("music_playlist")
        .filter_map(|row| text_field(row, "id"))
        .collect();
    let songs = live("music_playlist_item")
        .filter(|row| text_field(row, "playlist_id").is_some_and(|id| live_playlists.contains(&id)))
        .count() as u32;
    if songs > 0 {
        add(Part::Playlists, Item::PlaylistSongs, songs);
    }
    for row in live("setting") {
        if text_field(row, "key").as_deref() == Some(PINS_KEY) {
            add(Part::Pins, Item::Pins, list_count(row.get("value"), "pins"));
            let played = list_count(row.get("value"), "played");
            if played > 0 {
                add(Part::Pins, Item::Played, played);
            }
        } else {
            add(Part::Settings, Item::Settings, 1);
        }
    }
    for (table, item) in [
        ("local_music_download", Item::Songs),
        ("local_manga_download", Item::Chapters),
        ("local_podcast_download", Item::Episodes),
        ("local_video_download", Item::Videos),
    ] {
        for row in data.rows(table) {
            add(
                Part::Downloads,
                if done(row) { item } else { Item::Waiting },
                1,
            );
        }
    }
    counts
}

/// How many entries a list of the `rail` setting's JSON holds (a string in a row, an object once
/// parsed): its `pins`, or its `played` stamps.
fn list_count(value: Option<&Value>, list: &str) -> u32 {
    let parsed = match value {
        Some(Value::String(text)) => serde_json::from_str(text).unwrap_or(Value::Null),
        Some(other) => other.clone(),
        None => Value::Null,
    };
    parsed
        .get(list)
        .and_then(Value::as_array)
        .map_or(0, |entries| entries.len() as u32)
}

pub(crate) fn units_item(part: Part) -> Item {
    if part == Part::Videos {
        Item::Episodes
    } else {
        Item::Chapters
    }
}

pub(crate) fn finished_item(part: Part) -> Item {
    if part == Part::Videos {
        Item::Watched
    } else {
        Item::Read
    }
}

pub(crate) fn followed_item(part: Part) -> Item {
    if part == Part::Audiobooks {
        Item::Books
    } else {
        Item::Shows
    }
}

/// What a restore did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Restored {
    pub received: usize,
    pub changed: usize,
    /// Backup ids that took ours (titles, chapters, podcasts…), for the app's own references.
    pub ids: HashMap<String, String>,
}

impl Restored {
    /// Our id for a backup's id.
    pub fn ours(&self, id: &str) -> String {
        self.ids.get(id).cloned().unwrap_or_else(|| id.to_owned())
    }
}

/// Columns pointing at a parent row, remapped when the parent took our id.
fn references(table: &str) -> &'static [&'static str] {
    match table {
        "unit" => &["title_id"],
        "title_category" => &["title_id", "category_id"],
        "unit_progress" => &["unit_id"],
        "podcast_episode" => &["podcast_id"],
        "music_playlist_item" => &["playlist_id"],
        _ => &[],
    }
}

/// The columns that name the same item under another id, and whether a deleted row of ours
/// counts (a table whose natural key is unique even among tombstones).
fn natural_key(table: &str, row: &Value) -> Option<(&'static [&'static str], bool)> {
    match table {
        "extension_repo" => Some((&["url"], true)),
        "stremio_addon" => Some((&["transport_url"], false)),
        "website" => Some((&["url"], false)),
        "video_progress" => Some((&["key"], false)),
        "title" => Some((&["source_id", "remote_id"], false)),
        "unit" => Some((&["title_id", "remote_id"], false)),
        "category" => Some((&["kind", "name"], false)),
        "podcast" => Some((&["feed_url"], false)),
        "podcast_episode" => Some((&["podcast_id", "guid"], false)),
        // One live favorite per song (M46.48): a song favorited here too keeps our row.
        "music_favorite" if is_live(row) => Some((&["track_key"], false)),
        _ => None,
    }
}

/// Merges the rows of `parts` in. Unknown tables are ignored, so newer backups restore what this
/// version knows.
pub async fn restore(db: &Db, backup: &BackupData, parts: &[Part]) -> Result<Restored, DbError> {
    let owners = Owners::of(backup);
    let wanted = |table: &str, row: &Value| {
        part_of(table, row, &owners).is_some_and(|part| parts.contains(&part))
    };
    let mut restored = Restored::default();
    let mut transaction = db.pool().begin().await?;
    let mut newest = 0u64;
    for spec in merged() {
        for incoming in backup.rows(spec.name).filter(|row| wanted(spec.name, row)) {
            let mut row = incoming.clone();
            for field in references(spec.name) {
                remap(&mut row, field, &restored.ids);
            }
            if let Some((natural, any_state)) = natural_key(spec.name, &row)
                && let Some((from, to)) = adopt_id(
                    &mut transaction,
                    &mut row,
                    spec.key,
                    spec.name,
                    natural,
                    any_state,
                )
                .await?
            {
                restored.ids.insert(from, to);
            }
            let Some(key) = text_field(&row, spec.key) else {
                continue;
            };
            restored.received += 1;
            newest = newest.max(stamp_of(&row));
            let existing = read_row(&mut *transaction, spec, &key).await?;
            if let Some(existing) = existing
                .as_ref()
                .filter(|existing| !is_newer(&row, existing))
            {
                if spec.name == "title"
                    && fill_library_gap(db, &mut transaction, &row, existing).await?
                {
                    restored.changed += 1;
                }
                continue;
            }
            write_row(&mut transaction, spec, &row).await?;
            restored.changed += 1;
        }
    }
    for pair in PAIRS {
        for incoming in backup.rows(pair.name).filter(|row| wanted(pair.name, row)) {
            let mut row = incoming.clone();
            for field in references(pair.name) {
                remap(&mut row, field, &restored.ids);
            }
            if pair.keys.iter().any(|key| text_field(&row, key).is_none()) {
                continue;
            }
            restored.received += 1;
            newest = newest.max(stamp_of(&row));
            if write_pair(&mut transaction, pair, &row).await? {
                restored.changed += 1;
            }
        }
    }
    if newest > 0 {
        let timestamp = db.observe(newest);
        persist_hlc(&mut *transaction, timestamp).await?;
    }
    transaction.commit().await?;
    Ok(restored)
}

fn stamp_of(row: &Value) -> u64 {
    row.get("updated_at_hlc")
        .and_then(Value::as_i64)
        .unwrap_or_default() as u64
}

/// Writes a two-key row unless ours is newer. Returns whether it was written.
async fn write_pair(
    transaction: &mut Transaction<'_, Sqlite>,
    pair: &Pair,
    row: &Value,
) -> Result<bool, DbError> {
    let name = pair.name;
    let columns = pair.columns.join(", ");
    let values = pair
        .columns
        .iter()
        .map(|column| format!("json_extract(?1, '$.{column}')"))
        .collect::<Vec<_>>()
        .join(", ");
    let updates = pair
        .columns
        .iter()
        .filter(|column| !pair.keys.contains(column))
        .map(|column| format!("{column} = excluded.{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    let keys = pair.keys.join(", ");
    // Names come from the PAIRS constant.
    let sql = format!(
        "INSERT INTO {name} ({columns}) VALUES ({values})
         ON CONFLICT ({keys}) DO UPDATE SET {updates}
         WHERE (excluded.updated_at_hlc, excluded.origin_device)
             > ({name}.updated_at_hlc, {name}.origin_device)"
    );
    let done = sqlx::query(AssertSqlSafe(sql))
        .bind(row.to_string())
        .execute(&mut **transaction)
        .await?;
    Ok(done.rows_affected() > 0)
}

/// Our title row is newer (say, refreshed here) but isn't in the library while the backup's is:
/// the user's library choice comes back anyway, as a new local write.
async fn fill_library_gap(
    db: &Db,
    transaction: &mut Transaction<'_, Sqlite>,
    incoming: &Value,
    existing: &Value,
) -> Result<bool, DbError> {
    let added_at = incoming.get("user_in_library_at").and_then(Value::as_i64);
    let live = |row: &Value| row.get("deleted").and_then(Value::as_i64) == Some(0);
    let missing = existing
        .get("user_in_library_at")
        .is_none_or(Value::is_null);
    let (Some(added_at), true, true, true) = (added_at, missing, live(incoming), live(existing))
    else {
        return Ok(false);
    };
    let stamp = db.tick();
    sqlx::query(
        "UPDATE title SET user_in_library_at = ?2, updated_at_hlc = ?3, origin_device = ?4
         WHERE id = ?1",
    )
    .bind(text_field(existing, "id"))
    .bind(added_at)
    .bind(stamp as i64)
    .bind(db.device_id().to_string())
    .execute(&mut **transaction)
    .await?;
    persist_hlc(&mut **transaction, stamp).await?;
    Ok(true)
}

/// When a row of `table` (a live one, unless `any_state`) matches `row` on `natural` columns
/// under another id, gives `row` that id. Returns `(backup id, our id)` when it did.
async fn adopt_id(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &mut Value,
    key: &str,
    table: &str,
    natural: &[&str],
    any_state: bool,
) -> Result<Option<(String, String)>, DbError> {
    let Some(incoming_id) = text_field(row, key) else {
        return Ok(None);
    };
    let conditions = natural
        .iter()
        .enumerate()
        .map(|(index, column)| format!("{column} = ?{}", index + 1))
        .collect::<Vec<_>>()
        .join(" AND ");
    let live = if any_state { "" } else { " AND deleted = 0" };
    // Table and column names are constants from `natural_key`.
    let sql = format!(
        "SELECT {key} AS id FROM {table} WHERE {conditions}{live} ORDER BY deleted LIMIT 1"
    );
    let mut query = sqlx::query(AssertSqlSafe(sql));
    for column in natural {
        query = query.bind(text_field(row, column).unwrap_or_default());
    }
    let ours: Option<String> = query
        .fetch_optional(&mut **transaction)
        .await?
        .map(|found| found.get("id"));
    match ours {
        Some(ours) if ours != incoming_id => {
            row[key] = Value::from(ours.clone());
            Ok(Some((incoming_id, ours)))
        }
        _ => Ok(None),
    }
}

/// Points a reference at our id when its parent was adopted.
fn remap(row: &mut Value, field: &str, ids: &HashMap<String, String>) {
    if let Some(ours) = text_field(row, field).and_then(|current| ids.get(&current)) {
        row[field] = Value::from(ours.clone());
    }
}

#[cfg(test)]
#[path = "backup_tests.rs"]
pub(crate) mod tests;
