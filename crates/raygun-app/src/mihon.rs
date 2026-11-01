//! Mihon backup import (M30). A `.tachibk` file is gzip-compressed protobuf; these structs decode
//! the fields Raygun uses, by the field numbers of Mihon's backup models
//! (`data/backup/models/*.kt` in mihon, Apache-2.0). Titles come in when Raygun has a source of
//! the same name.

use std::collections::{BTreeMap, HashMap};

use raygun_db::Db;
use raygun_db::titles::{self, TitleFields, UnitFields};
use raygun_model::{MangaSource, MihonImport, SkippedSource};

#[derive(Clone, PartialEq, prost::Message)]
pub struct MihonBackup {
    #[prost(message, repeated, tag = "1")]
    pub manga: Vec<MihonManga>,
    #[prost(message, repeated, tag = "101")]
    pub sources: Vec<MihonSource>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct MihonManga {
    #[prost(int64, tag = "1")]
    pub source: i64,
    #[prost(string, tag = "2")]
    pub url: String,
    #[prost(string, tag = "3")]
    pub title: String,
    #[prost(string, optional, tag = "4")]
    pub artist: Option<String>,
    #[prost(string, optional, tag = "5")]
    pub author: Option<String>,
    #[prost(string, optional, tag = "6")]
    pub description: Option<String>,
    #[prost(string, repeated, tag = "7")]
    pub genre: Vec<String>,
    #[prost(int32, tag = "8")]
    pub status: i32,
    #[prost(string, optional, tag = "9")]
    pub thumbnail_url: Option<String>,
    #[prost(int64, tag = "13")]
    pub date_added: i64,
    #[prost(message, repeated, tag = "16")]
    pub chapters: Vec<MihonChapter>,
    /// Mihon leaves it out when true (its default), so absent means a library title.
    #[prost(bool, optional, tag = "100")]
    pub favorite: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct MihonChapter {
    #[prost(string, tag = "1")]
    pub url: String,
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(string, optional, tag = "3")]
    pub scanlator: Option<String>,
    #[prost(bool, tag = "4")]
    pub read: bool,
    #[prost(int64, tag = "6")]
    pub last_page_read: i64,
    #[prost(int64, tag = "8")]
    pub date_upload: i64,
    #[prost(float, tag = "9")]
    pub chapter_number: f32,
    #[prost(int64, tag = "10")]
    pub source_order: i64,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct MihonSource {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(int64, tag = "2")]
    pub source_id: i64,
}

/// Decodes the protobuf inside a (decompressed) Mihon backup.
pub fn decode(bytes: &[u8]) -> Result<MihonBackup, String> {
    use prost::Message;
    let backup = MihonBackup::decode(bytes)
        .map_err(|error| format!("This isn't a Mihon backup ({error})"))?;
    if backup.manga.is_empty() && backup.sources.is_empty() {
        return Err("This backup has no manga".into());
    }
    Ok(backup)
}

/// Mihon's status numbers (SManga) as Raygun's status names.
fn status(code: i32) -> &'static str {
    match code {
        1 => "ongoing",
        2 => "completed",
        4 => "publishingFinished",
        5 => "cancelled",
        6 => "hiatus",
        _ => "unknown",
    }
}

/// The source's id for a title or chapter from its Mihon URL: the last path segment of a path
/// (`/manga/<uuid>` → `<uuid>`), else the URL itself.
fn remote_id(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    if (path.starts_with('/') || path.starts_with("http"))
        && let Some(segment) = path.rsplit('/').find(|segment| !segment.is_empty())
    {
        return segment.to_owned();
    }
    url.to_owned()
}

fn names(text: &Option<String>) -> Vec<String> {
    text.iter()
        .flat_map(|names| names.split(','))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Stores the titles of `backup` whose source Raygun has, with their chapters, library state and
/// reading progress. Titles already stored keep what their source said; only library state and
/// progress come from the backup. New ones are fetched from the source when first viewed.
/// Chapters count as found when the title joins the library, so an import never fills the
/// updates feed.
pub async fn import(
    db: &Db,
    backup: &MihonBackup,
    sources: &[MangaSource],
    now_ms: i64,
) -> Result<MihonImport, raygun_db::DbError> {
    let source_names: HashMap<i64, &str> = backup
        .sources
        .iter()
        .map(|source| (source.source_id, source.name.as_str()))
        .collect();
    let mut summary = MihonImport::default();
    let mut skipped: BTreeMap<String, u32> = BTreeMap::new();
    for manga in &backup.manga {
        let name = source_names
            .get(&manga.source)
            .map_or_else(|| manga.source.to_string(), |name| (*name).to_owned());
        let Some(source) = sources
            .iter()
            .find(|source| source.name.eq_ignore_ascii_case(&name))
        else {
            *skipped.entry(name).or_default() += 1;
            continue;
        };
        let manga_id = remote_id(&manga.url);
        let known = titles::find_title(db, &source.id, &manga_id).await?;
        let mut chapters: Vec<&MihonChapter> = manga.chapters.iter().collect();
        chapters.sort_by_key(|chapter| chapter.source_order);
        let title_id = match known {
            Some(title) => title.id,
            None => store_title(db, source, &manga_id, manga, &chapters, now_ms).await?,
        };
        let stored = titles::title(db, &title_id).await?;
        if manga.favorite.unwrap_or(true) {
            if stored.is_some_and(|title| title.in_library_at.is_none()) {
                titles::set_in_library(db, &title_id, Some(now_ms)).await?;
            }
            summary.library_titles += 1;
        }

        let ids: HashMap<String, String> = titles::units(db, &title_id)
            .await?
            .into_iter()
            .map(|unit| (unit.fields.remote_id, unit.id))
            .collect();
        for chapter in &chapters {
            let Some(unit_id) = ids.get(&remote_id(&chapter.url)) else {
                continue;
            };
            if chapter.read {
                titles::set_completed(db, unit_id, true, now_ms).await?;
                summary.read_chapters += 1;
            } else if chapter.last_page_read > 0 {
                titles::set_progress(db, unit_id, chapter.last_page_read, None, now_ms).await?;
            }
        }
        summary.titles += 1;
        summary.chapters += chapters.len() as u32;
    }
    summary.skipped = skipped
        .into_iter()
        .map(|(name, titles)| SkippedSource { name, titles })
        .collect();
    Ok(summary)
}

/// Stores a title the backup brought, with its chapters, marked for a fetch from its source.
async fn store_title(
    db: &Db,
    source: &MangaSource,
    manga_id: &str,
    manga: &MihonManga,
    chapters: &[&MihonChapter],
    now_ms: i64,
) -> Result<String, raygun_db::DbError> {
    let title_id = titles::upsert_title(
        db,
        &TitleFields {
            kind: "manga".into(),
            source_id: source.id.clone(),
            remote_id: manga_id.to_owned(),
            title: manga.title.clone(),
            cover: manga.thumbnail_url.clone().filter(|url| !url.is_empty()),
            cover_headers: Default::default(),
            authors: names(&manga.author),
            artists: names(&manga.artist),
            description: manga.description.clone().filter(|text| !text.is_empty()),
            genres: manga.genre.clone(),
            status: status(manga.status).into(),
            url: None,
        },
        now_ms,
    )
    .await?;
    let units: Vec<UnitFields> = chapters
        .iter()
        .map(|chapter| UnitFields {
            remote_id: remote_id(&chapter.url),
            name: chapter.name.clone(),
            number: (chapter.chapter_number >= 0.0).then_some(f64::from(chapter.chapter_number)),
            volume: None,
            scanlator: chapter.scanlator.clone().filter(|name| !name.is_empty()),
            date: (chapter.date_upload > 0).then_some(chapter.date_upload),
        })
        .collect();
    titles::sync_units(db, &title_id, &units, "chapter", now_ms).await?;
    titles::mark_stale(db, &title_id).await?;
    Ok(title_id)
}

#[cfg(test)]
mod tests {
    use prost::Message;
    use tempfile::TempDir;

    use super::*;

    const MANGADEX: i64 = 2_499_283_573_021_220_255;

    fn chapter(id: &str, number: f32, order: i64, read: bool, page: i64) -> MihonChapter {
        MihonChapter {
            url: format!("/chapter/{id}"),
            name: format!("Chapter {number}"),
            scanlator: Some("Group".into()),
            read,
            last_page_read: page,
            date_upload: 1_700_000_000_000,
            chapter_number: number,
            source_order: order,
        }
    }

    fn backup() -> MihonBackup {
        MihonBackup {
            manga: vec![
                MihonManga {
                    source: MANGADEX,
                    url: "/manga/aaaa-1111".into(),
                    title: "Frieren".into(),
                    artist: Some("Abe".into()),
                    author: Some("Yamada, Someone".into()),
                    description: Some("An elf mage".into()),
                    genre: vec!["Fantasy".into()],
                    status: 2,
                    thumbnail_url: Some("https://uploads.example/cover.jpg".into()),
                    date_added: 1_600_000_000_000,
                    chapters: vec![
                        chapter("c1", 1.0, 2, true, 18),
                        chapter("c3", 3.0, 0, false, 0),
                        chapter("c2", 2.0, 1, false, 7),
                    ],
                    favorite: None,
                },
                MihonManga {
                    source: 42,
                    url: "/series/x".into(),
                    title: "Elsewhere".into(),
                    favorite: Some(true),
                    ..Default::default()
                },
                MihonManga {
                    source: 42,
                    url: "/series/y".into(),
                    title: "Elsewhere too".into(),
                    favorite: Some(false),
                    ..Default::default()
                },
            ],
            sources: vec![
                MihonSource {
                    name: "MangaDex".into(),
                    source_id: MANGADEX,
                },
                MihonSource {
                    name: "Some Scanlator".into(),
                    source_id: 42,
                },
            ],
        }
    }

    #[test]
    fn decodes_what_mihon_writes() {
        let bytes = backup().encode_to_vec();
        assert_eq!(decode(&bytes).unwrap(), backup());
        assert!(decode(b"\xff\xff\xff").is_err());
        assert!(decode(&[]).is_err(), "an empty message has no manga");
    }

    #[test]
    fn maps_urls_and_statuses() {
        assert_eq!(remote_id("/manga/aaaa-1111"), "aaaa-1111");
        assert_eq!(remote_id("/chapter/c1/"), "c1");
        assert_eq!(remote_id("https://site.example/title/9?lang=en"), "9");
        assert_eq!(remote_id("12345"), "12345");
        assert_eq!(status(5), "cancelled");
        assert_eq!(status(3), "unknown", "licensed has no Raygun status");
    }

    #[tokio::test]
    async fn imports_titles_with_progress_and_skips_unknown_sources() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let sources = [MangaSource {
            id: "dev.raygun.mangadex/mangadex".into(),
            name: "MangaDex".into(),
            extension_id: "dev.raygun.mangadex".into(),
        }];

        let summary = import(&db, &backup(), &sources, 1_000).await.unwrap();
        assert_eq!(
            summary,
            MihonImport {
                titles: 1,
                library_titles: 1,
                chapters: 3,
                read_chapters: 1,
                skipped: vec![SkippedSource {
                    name: "Some Scanlator".into(),
                    titles: 2
                }],
            }
        );

        let title = titles::find_title(&db, &sources[0].id, "aaaa-1111")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(title.fields.title, "Frieren");
        assert_eq!(title.fields.authors, ["Yamada", "Someone"]);
        assert_eq!(title.fields.status, "completed");
        assert_eq!(title.in_library_at, Some(1_000));
        let units = titles::units(&db, &title.id).await.unwrap();
        let order: Vec<_> = units
            .iter()
            .map(|unit| unit.fields.remote_id.as_str())
            .collect();
        assert_eq!(order, ["c3", "c2", "c1"], "Mihon's source order");
        assert!(units[2].completed_at.is_some());
        assert_eq!((units[1].position, units[1].completed_at), (Some(7), None));
        assert!(
            titles::updates(&db, "manga", 50).await.unwrap().is_empty(),
            "imported chapters aren't updates"
        );

        assert_eq!(
            title.refreshed_at, None,
            "fetched from the source when first viewed"
        );

        // Viewed since: the source's data replaces the backup's, and a second import keeps it.
        let mut fetched = title.fields.clone();
        fetched.title = "Frieren: Beyond Journey's End".into();
        titles::upsert_title(&db, &fetched, 1_500).await.unwrap();
        let again = import(&db, &backup(), &sources, 2_000).await.unwrap();
        assert_eq!(
            titles::title(&db, &title.id)
                .await
                .unwrap()
                .unwrap()
                .fields
                .title,
            "Frieren: Beyond Journey's End"
        );
        assert_eq!(again.titles, 1);
        assert_eq!(
            titles::units(&db, &title.id).await.unwrap().len(),
            3,
            "no duplicates"
        );
        let title = titles::title(&db, &title.id).await.unwrap().unwrap();
        assert_eq!(title.in_library_at, Some(1_000), "keeps when it joined");
    }
}
