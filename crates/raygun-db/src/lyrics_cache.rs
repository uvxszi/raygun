//! Lyrics kept on this device (M46.46): what a source gave for a song, by its track key, with when
//! it was fetched and last shown; and translations of them (M46.48). Device-local. Long lyrics
//! and translations are kept compressed ([`crate::packed`]).

use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::{Db, DbError, packed};

/// A song's kept row: its lyrics (JSON, or none found) and when they were fetched.
pub struct CachedLyrics {
    pub lyrics: Option<String>,
    pub fetched_at: i64,
}

/// A kept row as stored; one whose lyrics don't unpack counts as not kept (fetched again).
fn cached(row: &SqliteRow) -> Option<CachedLyrics> {
    let lyrics = match row.get::<Option<String>, _>("lyrics") {
        Some(stored) => Some(packed::unpack(&stored)?.into_owned()),
        None => None,
    };
    Some(CachedLyrics {
        lyrics,
        fetched_at: row.get("fetched_at"),
    })
}

/// The song's kept row, if any; reading it marks the song's lyrics as shown now.
pub async fn get(db: &Db, track_key: &str, now_ms: i64) -> Result<Option<CachedLyrics>, DbError> {
    let row = sqlx::query(
        "UPDATE local_lyrics_cache SET used_at = ?2 WHERE track_key = ?1
         RETURNING lyrics, fetched_at",
    )
    .bind(track_key)
    .bind(now_ms)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.as_ref().and_then(cached))
}

/// The song's kept row, if any, without marking its lyrics as shown (incognito, M47).
pub async fn peek(db: &Db, track_key: &str) -> Result<Option<CachedLyrics>, DbError> {
    let row = sqlx::query("SELECT lyrics, fetched_at FROM local_lyrics_cache WHERE track_key = ?1")
        .bind(track_key)
        .fetch_optional(db.pool())
        .await?;
    Ok(row.as_ref().and_then(cached))
}

/// Keeps what a source gave (or that none had any), shown now.
pub async fn put(
    db: &Db,
    track_key: &str,
    lyrics: Option<&str>,
    now_ms: i64,
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_lyrics_cache (track_key, lyrics, fetched_at, used_at)
         VALUES (?1, ?2, ?3, ?3)
         ON CONFLICT (track_key) DO UPDATE SET lyrics = excluded.lyrics,
             fetched_at = excluded.fetched_at, used_at = excluded.used_at",
    )
    .bind(track_key)
    .bind(lyrics.map(packed::pack))
    .bind(now_ms)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Marks a song's lyrics as shown now (its own lyrics show, so nothing is fetched).
pub async fn touch(db: &Db, track_key: &str, now_ms: i64) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_lyrics_cache (track_key, lyrics, fetched_at, used_at)
         VALUES (?1, NULL, 0, ?2)
         ON CONFLICT (track_key) DO UPDATE SET used_at = excluded.used_at",
    )
    .bind(track_key)
    .bind(now_ms)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Forgets these songs' rows and translations.
pub async fn remove(db: &Db, track_keys: &[String]) -> Result<(), DbError> {
    let mut tx = db.pool().begin().await?;
    for key in track_keys {
        sqlx::query("DELETE FROM local_lyrics_cache WHERE track_key = ?1")
            .bind(key)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM local_lyrics_translation WHERE track_key = ?1")
            .bind(key)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Keys of songs with kept lyrics.
pub async fn keys(db: &Db) -> Result<Vec<String>, DbError> {
    let rows = sqlx::query("SELECT track_key FROM local_lyrics_cache WHERE lyrics IS NOT NULL")
        .fetch_all(db.pool())
        .await?;
    Ok(rows.iter().map(|row| row.get("track_key")).collect())
}

/// Keys of songs whose lyrics weren't shown since `before_ms`, and of translations made before
/// then for songs without a kept row.
pub async fn stale(db: &Db, before_ms: i64) -> Result<Vec<String>, DbError> {
    let rows = sqlx::query(
        "SELECT track_key FROM local_lyrics_cache WHERE used_at < ?1
         UNION
         SELECT track_key FROM local_lyrics_translation
         WHERE made_at < ?1
           AND track_key NOT IN (SELECT track_key FROM local_lyrics_cache)",
    )
    .bind(before_ms)
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(|row| row.get("track_key")).collect())
}

/// What `local_lyrics_translation` keeps of one translation (M46.48).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationRow {
    /// `source:<name>`, `device` or `ai:<provider id>`.
    pub translator: String,
    pub language: String,
    /// `translation` or `romanization`.
    pub kind: String,
    /// Who made it, as shown.
    pub by_name: String,
    /// JSON lines: `[{t, s, r}]`.
    pub lines: String,
    pub made_at: i64,
}

/// What the lyrics sources gave is kept under translators with this prefix.
pub const SOURCE_PREFIX: &str = "source:";

/// A song's translations, the newest first.
pub async fn translations(db: &Db, track_key: &str) -> Result<Vec<TranslationRow>, DbError> {
    let rows = sqlx::query(
        "SELECT translator, language, kind, by_name, lines, made_at
         FROM local_lyrics_translation WHERE track_key = ?1
         ORDER BY made_at DESC, translator",
    )
    .bind(track_key)
    .fetch_all(db.pool())
    .await?;
    // One whose lines don't unpack is left out (made again when asked).
    Ok(rows
        .iter()
        .filter_map(|row| {
            let lines: String = row.get("lines");
            Some(TranslationRow {
                translator: row.get("translator"),
                language: row.get("language"),
                kind: row.get("kind"),
                by_name: row.get("by_name"),
                lines: packed::unpack(&lines)?.into_owned(),
                made_at: row.get("made_at"),
            })
        })
        .collect())
}

async fn insert_translation(
    executor: impl sqlx::SqliteExecutor<'_>,
    track_key: &str,
    row: &TranslationRow,
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_lyrics_translation
             (track_key, translator, language, kind, by_name, lines, made_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (track_key, translator, language, kind) DO UPDATE SET
             by_name = excluded.by_name, lines = excluded.lines, made_at = excluded.made_at",
    )
    .bind(track_key)
    .bind(&row.translator)
    .bind(&row.language)
    .bind(&row.kind)
    .bind(&row.by_name)
    .bind(packed::pack(&row.lines))
    .bind(row.made_at)
    .execute(executor)
    .await?;
    Ok(())
}

/// Keeps a translation made on this device or by AI, replacing the same translator's in the same
/// language.
pub async fn put_translation(
    db: &Db,
    track_key: &str,
    row: &TranslationRow,
) -> Result<(), DbError> {
    insert_translation(db.pool(), track_key, row).await
}

/// Replaces what the sources in `answered` (`source:<name>` translators) gave for a song with
/// `rows`; other sources' stay. A translation kept before keeps its time, which says when it was
/// last picked.
pub async fn replace_source_translations(
    db: &Db,
    track_key: &str,
    answered: &[String],
    rows: &[TranslationRow],
) -> Result<(), DbError> {
    let mut tx = db.pool().begin().await?;
    let before: std::collections::HashMap<(String, String, String), i64> = sqlx::query(
        "SELECT translator, language, kind, made_at FROM local_lyrics_translation
         WHERE track_key = ?1 AND substr(translator, 1, length(?2)) = ?2",
    )
    .bind(track_key)
    .bind(SOURCE_PREFIX)
    .fetch_all(&mut *tx)
    .await?
    .iter()
    .map(|row| {
        (
            (row.get("translator"), row.get("language"), row.get("kind")),
            row.get("made_at"),
        )
    })
    .collect();
    for translator in answered {
        sqlx::query(
            "DELETE FROM local_lyrics_translation WHERE track_key = ?1 AND translator = ?2",
        )
        .bind(track_key)
        .bind(translator)
        .execute(&mut *tx)
        .await?;
    }
    for row in rows {
        let key = (
            row.translator.clone(),
            row.language.clone(),
            row.kind.clone(),
        );
        let made_at = before.get(&key).copied().unwrap_or(row.made_at);
        let row = TranslationRow {
            made_at,
            ..row.clone()
        };
        insert_translation(&mut *tx, track_key, &row).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Marks a song's translation as picked now, so it comes first next time.
pub async fn pick_translation(
    db: &Db,
    track_key: &str,
    translator: &str,
    language: &str,
    now_ms: i64,
) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_lyrics_translation SET made_at = ?4
         WHERE track_key = ?1 AND translator = ?2 AND language = ?3",
    )
    .bind(track_key)
    .bind(translator)
    .bind(language)
    .bind(now_ms)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Keys of songs with translations made on this device or by AI.
pub async fn translated_keys(db: &Db) -> Result<Vec<String>, DbError> {
    let rows = sqlx::query(
        "SELECT DISTINCT track_key FROM local_lyrics_translation
         WHERE substr(translator, 1, length(?1)) <> ?1",
    )
    .bind(SOURCE_PREFIX)
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(|row| row.get("track_key")).collect())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn kept_lyrics_are_read_back_and_go_stale_when_not_shown() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        put(&db, "a|one", Some("{\"x\":1}"), 10).await.unwrap();
        touch(&db, "a|two", 20).await.unwrap();
        let read = get(&db, "a|one", 30).await.unwrap().unwrap();
        assert_eq!(read.lyrics.as_deref(), Some("{\"x\":1}"));
        assert_eq!(read.fetched_at, 10);
        assert_eq!(keys(&db).await.unwrap(), ["a|one"]);
        // "a|one" was shown at 30, "a|two" last at 20.
        assert_eq!(stale(&db, 25).await.unwrap(), ["a|two"]);
        remove(&db, &["a|one".into()]).await.unwrap();
        assert!(get(&db, "a|one", 40).await.unwrap().is_none());
    }

    fn row(translator: &str, language: &str, made_at: i64) -> TranslationRow {
        TranslationRow {
            translator: translator.into(),
            language: language.into(),
            kind: "translation".into(),
            by_name: translator.into(),
            lines: "[]".into(),
            made_at,
        }
    }

    #[tokio::test]
    async fn translations_are_kept_replaced_and_forgotten_with_the_lyrics() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        put(&db, "a|one", Some("{}"), 10).await.unwrap();
        put_translation(&db, "a|one", &row("device", "en", 20))
            .await
            .unwrap();
        let kugou = ["source:KuGou".to_owned()];
        replace_source_translations(&db, "a|one", &kugou, &[row("source:KuGou", "zh-Hans", 30)])
            .await
            .unwrap();
        // The answering sources' are replaced, a kept one keeping its time; machine translations
        // stay.
        replace_source_translations(
            &db,
            "a|one",
            &["source:KuGou".to_owned(), "source:Other".to_owned()],
            &[
                row("source:KuGou", "zh-Hans", 40),
                row("source:Other", "en", 40),
            ],
        )
        .await
        .unwrap();
        let names = |kept: Vec<TranslationRow>| -> Vec<(String, i64)> {
            kept.into_iter()
                .map(|row| (row.translator, row.made_at))
                .collect()
        };
        assert_eq!(
            names(translations(&db, "a|one").await.unwrap()),
            [
                ("source:Other".to_owned(), 40),
                ("source:KuGou".to_owned(), 30),
                ("device".to_owned(), 20)
            ]
        );
        // Picking one puts it first.
        pick_translation(&db, "a|one", "device", "en", 45)
            .await
            .unwrap();
        assert_eq!(
            names(translations(&db, "a|one").await.unwrap())[0],
            ("device".to_owned(), 45)
        );
        // Other answered without translations: its old one goes; KuGou didn't answer: its stays.
        replace_source_translations(&db, "a|one", &["source:Other".to_owned()], &[])
            .await
            .unwrap();
        // The same translator and language replaces its own.
        put_translation(&db, "a|one", &row("device", "en", 50))
            .await
            .unwrap();
        assert_eq!(translations(&db, "a|one").await.unwrap().len(), 2);
        assert_eq!(translated_keys(&db).await.unwrap(), ["a|one"]);

        // A translation of a song without a kept row goes stale by its own age.
        put_translation(&db, "b|two", &row("ai:x", "de", 5))
            .await
            .unwrap();
        assert_eq!(stale(&db, 8).await.unwrap(), ["b|two"]);
        remove(&db, &["a|one".into(), "b|two".into()])
            .await
            .unwrap();
        assert!(translations(&db, "a|one").await.unwrap().is_empty());
        assert!(translated_keys(&db).await.unwrap().is_empty());
    }

    /// Lyrics as `LyricsStore` keeps them: synced lines and the plain text (John Newton's 1779
    /// hymn, in the public domain).
    fn hymn() -> String {
        let verses = [
            "Amazing grace! How sweet the sound",
            "That saved a wretch like me!",
            "I once was lost, but now am found;",
            "Was blind, but now I see.",
            "'Twas grace that taught my heart to fear,",
            "And grace my fears relieved;",
            "How precious did that grace appear",
            "The hour I first believed.",
            "Through many dangers, toils and snares,",
            "I have already come;",
            "'Tis grace hath brought me safe thus far,",
            "And grace will lead me home.",
            "The Lord has promised good to me,",
            "His Word my hope secures;",
            "He will my Shield and Portion be,",
            "As long as life endures.",
        ];
        let lines: Vec<&str> = verses
            .iter()
            .chain(verses.iter().take(8))
            .copied()
            .collect();
        let synced: Vec<String> = lines
            .iter()
            .enumerate()
            .map(|(index, text)| {
                format!(r#"{{"timeMs":{},"text":"{text}"}}"#, 12_480 + index * 4_350)
            })
            .collect();
        format!(
            r#"{{"providerName":"LRCLIB","synced":[{}],"plain":"{}","offsetMs":0,"edited":false,"translations":[]}}"#,
            synced.join(","),
            lines.join("\\n")
        )
    }

    async fn stored(db: &Db, sql: &'static str) -> String {
        sqlx::query_scalar(sql).fetch_one(db.pool()).await.unwrap()
    }

    #[tokio::test]
    async fn long_lyrics_and_translations_are_kept_compressed() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let lyrics = hymn();
        put(&db, "a|one", Some(&lyrics), 10).await.unwrap();
        let kept = stored(&db, "SELECT lyrics FROM local_lyrics_cache").await;
        assert!(
            kept.len() * 2 < lyrics.len(),
            "{} of {}",
            kept.len(),
            lyrics.len()
        );
        let read = get(&db, "a|one", 20).await.unwrap().unwrap();
        assert_eq!(read.lyrics, Some(lyrics.clone()));
        assert_eq!(
            peek(&db, "a|one").await.unwrap().unwrap().lyrics,
            Some(lyrics.clone())
        );

        let translation = TranslationRow {
            lines: lyrics.clone(),
            ..row("device", "de", 30)
        };
        put_translation(&db, "a|one", &translation).await.unwrap();
        let kept = stored(&db, "SELECT lines FROM local_lyrics_translation").await;
        assert!(kept.len() < lyrics.len());
        assert_eq!(translations(&db, "a|one").await.unwrap(), [translation]);
    }

    #[tokio::test]
    async fn rows_kept_before_read_and_damaged_ones_are_fetched_again() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        // As earlier versions wrote it: plain text.
        let lyrics = hymn();
        sqlx::query(
            "INSERT INTO local_lyrics_cache (track_key, lyrics, fetched_at, used_at)
             VALUES ('a|one', ?1, 10, 10), ('a|two', ?2, 10, 10)",
        )
        .bind(&lyrics)
        .bind("\u{1}z1:broken")
        .execute(db.pool())
        .await
        .unwrap();
        assert_eq!(
            get(&db, "a|one", 20).await.unwrap().unwrap().lyrics,
            Some(lyrics)
        );
        assert!(get(&db, "a|two", 20).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn peeking_leaves_when_lyrics_were_shown() {
        let temp = TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        put(&db, "a|one", Some("{\"x\":1}"), 10).await.unwrap();
        let read = peek(&db, "a|one").await.unwrap().unwrap();
        assert_eq!(read.lyrics.as_deref(), Some("{\"x\":1}"));
        assert_eq!(read.fetched_at, 10);
        assert_eq!(
            stale(&db, 11).await.unwrap(),
            ["a|one"],
            "still shown last at 10"
        );
        assert!(peek(&db, "a|two").await.unwrap().is_none());
    }
}
