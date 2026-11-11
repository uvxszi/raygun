//! Captions for podcast episodes and audiobook chapters (M46.32): from the feed's transcript or
//! transcribed a window at a time, with the stretches already covered so nothing is heard twice.
//! Device-local.

use sqlx::Row;

use crate::{Db, DbError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptionRow {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaptionProgress {
    /// `feed`, `local` or `online`.
    pub source: String,
    /// Covered stretches, sorted and merged.
    pub ranges: Vec<(i64, i64)>,
    pub complete: bool,
}

impl CaptionProgress {
    /// The first moment at or after `from_ms` that has no captions yet.
    pub fn next_gap(&self, from_ms: i64) -> i64 {
        let mut at = from_ms;
        for (start, end) in &self.ranges {
            if *start <= at && at < *end {
                at = *end;
            }
        }
        at
    }
}

pub async fn cues(db: &Db, episode_id: &str) -> Result<Vec<CaptionRow>, DbError> {
    let rows = sqlx::query(
        "SELECT start_ms, end_ms, text FROM local_episode_caption WHERE episode_id = ?
         ORDER BY start_ms",
    )
    .bind(episode_id)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CaptionRow {
            start_ms: row.get("start_ms"),
            end_ms: row.get("end_ms"),
            text: row.get("text"),
        })
        .collect())
}

/// The last caption that starts before `at_ms`: a window starting there may hear its line again.
pub async fn cue_before(
    db: &Db,
    episode_id: &str,
    at_ms: i64,
) -> Result<Option<CaptionRow>, DbError> {
    let row = sqlx::query(
        "SELECT start_ms, end_ms, text FROM local_episode_caption
         WHERE episode_id = ? AND start_ms < ? ORDER BY start_ms DESC LIMIT 1",
    )
    .bind(episode_id)
    .bind(at_ms)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.map(|row| CaptionRow {
        start_ms: row.get("start_ms"),
        end_ms: row.get("end_ms"),
        text: row.get("text"),
    }))
}

pub async fn progress(db: &Db, episode_id: &str) -> Result<Option<CaptionProgress>, DbError> {
    let row = sqlx::query(
        "SELECT source, ranges, complete FROM local_caption_progress WHERE episode_id = ?",
    )
    .bind(episode_id)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.map(|row| CaptionProgress {
        source: row.get("source"),
        ranges: serde_json::from_str(&row.get::<String, _>("ranges")).unwrap_or_default(),
        complete: row.get::<i64, _>("complete") != 0,
    }))
}

/// Adds one window's captions and marks `covered` as done. `duration_ms`, when known, makes
/// the episode complete once everything up to it is covered.
pub async fn add(
    db: &Db,
    episode_id: &str,
    source: &str,
    cues: &[CaptionRow],
    covered: (i64, i64),
    duration_ms: Option<i64>,
) -> Result<CaptionProgress, DbError> {
    let mut tx = db.pool().begin().await?;
    // A later window may hear a cut sentence again: it replaces what overlapped it.
    sqlx::query(
        "DELETE FROM local_episode_caption WHERE episode_id = ? AND start_ms >= ? AND start_ms < ?",
    )
    .bind(episode_id)
    .bind(covered.0)
    .bind(covered.1)
    .execute(&mut *tx)
    .await?;
    for cue in cues {
        sqlx::query(
            "INSERT OR REPLACE INTO local_episode_caption (episode_id, start_ms, end_ms, text)
             VALUES (?, ?, ?, ?)",
        )
        .bind(episode_id)
        .bind(cue.start_ms)
        .bind(cue.end_ms)
        .bind(&cue.text)
        .execute(&mut *tx)
        .await?;
    }
    let stored: Vec<(i64, i64)> =
        sqlx::query("SELECT ranges FROM local_caption_progress WHERE episode_id = ?")
            .bind(episode_id)
            .fetch_optional(&mut *tx)
            .await?
            .and_then(|row| serde_json::from_str(&row.get::<String, _>("ranges")).ok())
            .unwrap_or_default();
    let ranges = merge(stored, covered);
    let complete = duration_ms.is_some_and(|duration| {
        ranges
            .first()
            .is_some_and(|(start, end)| *start <= 0 && *end >= duration)
    });
    sqlx::query(
        "INSERT INTO local_caption_progress (episode_id, source, ranges, complete)
         VALUES (?, ?, ?, ?)
         ON CONFLICT (episode_id) DO UPDATE SET source = excluded.source,
             ranges = excluded.ranges, complete = excluded.complete",
    )
    .bind(episode_id)
    .bind(source)
    .bind(serde_json::to_string(&ranges).unwrap_or_else(|_| "[]".into()))
    .bind(i64::from(complete))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(CaptionProgress {
        source: source.to_owned(),
        ranges,
        complete,
    })
}

/// Stores a whole transcript (the feed's), replacing anything kept.
pub async fn replace(
    db: &Db,
    episode_id: &str,
    source: &str,
    cues: &[CaptionRow],
) -> Result<(), DbError> {
    clear(db, episode_id).await?;
    let end = cues.iter().map(|cue| cue.end_ms).max().unwrap_or(0).max(1);
    add(db, episode_id, source, cues, (0, end), Some(end)).await?;
    Ok(())
}

/// Moves an episode's captions and progress to another key (M46.45: a source the listener
/// switched away from is kept, to switch back without hearing it again). What `to` held goes.
pub async fn move_to(db: &Db, from: &str, to: &str) -> Result<(), DbError> {
    let mut tx = db.pool().begin().await?;
    for table in ["local_episode_caption", "local_caption_progress"] {
        // The table names are the constants above.
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM {table} WHERE episode_id = ?"
        )))
        .bind(to)
        .execute(&mut *tx)
        .await?;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE {table} SET episode_id = ?2 WHERE episode_id = ?1"
        )))
        .bind(from)
        .bind(to)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// The songs with a transcription (`song:<track key>`, M46.46).
pub async fn song_keys(db: &Db) -> Result<Vec<String>, DbError> {
    let rows =
        sqlx::query("SELECT episode_id FROM local_caption_progress WHERE episode_id LIKE 'song:%'")
            .fetch_all(db.pool())
            .await?;
    Ok(rows.iter().map(|row| row.get("episode_id")).collect())
}

/// The videos with AI subtitles (M46.33), by progress key: every caption that's neither a song's
/// nor an episode's (a podcast's or an audiobook's, shelved copies like `<id>#ai` included).
pub async fn video_keys(db: &Db) -> Result<Vec<String>, DbError> {
    let rows = sqlx::query(
        "SELECT episode_id FROM (
             SELECT episode_id FROM local_caption_progress
             UNION
             SELECT DISTINCT episode_id FROM local_episode_caption
         )
         WHERE episode_id NOT LIKE 'song:%'
           AND CASE WHEN instr(episode_id, '#') > 0
                    THEN substr(episode_id, 1, instr(episode_id, '#') - 1)
                    ELSE episode_id
               END NOT IN (SELECT id FROM podcast_episode)
         ORDER BY episode_id",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().map(|row| row.get("episode_id")).collect())
}

pub async fn clear(db: &Db, episode_id: &str) -> Result<(), DbError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("DELETE FROM local_episode_caption WHERE episode_id = ?")
        .bind(episode_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM local_caption_progress WHERE episode_id = ?")
        .bind(episode_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Forgets a key's captions and the copies kept aside for it (`<key>#ai`, `<key>#feed`).
pub async fn clear_with_shelves(db: &Db, episode_id: &str) -> Result<(), DbError> {
    let mut tx = db.pool().begin().await?;
    for table in ["local_episode_caption", "local_caption_progress"] {
        // The table names are the constants above; `#` shelves are matched by prefix.
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM {table} WHERE episode_id = ?1
                 OR substr(episode_id, 1, length(?1) + 1) = ?1 || '#'"
        )))
        .bind(episode_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

fn merge(mut ranges: Vec<(i64, i64)>, added: (i64, i64)) -> Vec<(i64, i64)> {
    ranges.push(added);
    ranges.sort_unstable();
    let mut merged: Vec<(i64, i64)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(start_ms: i64, end_ms: i64, text: &str) -> CaptionRow {
        CaptionRow {
            start_ms,
            end_ms,
            text: text.into(),
        }
    }

    #[test]
    fn stretches_merge_and_gaps_are_found() {
        let ranges = merge(merge(vec![(0, 30_000)], (60_000, 90_000)), (28_000, 60_000));
        assert_eq!(ranges, vec![(0, 90_000)]);
        let progress = CaptionProgress {
            ranges: vec![(0, 30_000), (60_000, 90_000)],
            ..Default::default()
        };
        assert_eq!(progress.next_gap(10_000), 30_000);
        assert_eq!(progress.next_gap(45_000), 45_000);
        assert_eq!(progress.next_gap(60_000), 90_000);
    }

    #[tokio::test]
    async fn windows_add_up_to_a_complete_episode() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let first = [
            cue(0, 4_000, "Chapter one."),
            cue(26_000, 30_000, "It was a"),
        ];
        add(&db, "e1", "local", &first, (0, 26_000), Some(50_000))
            .await
            .unwrap();
        // The next window starts where the cut sentence began and replaces it.
        let second = [
            cue(26_000, 31_000, "It was a dark night."),
            cue(31_000, 50_000, "The end."),
        ];
        let progress = add(&db, "e1", "local", &second, (26_000, 50_000), Some(50_000))
            .await
            .unwrap();
        assert!(progress.complete);
        let texts: Vec<String> = cues(&db, "e1")
            .await
            .unwrap()
            .into_iter()
            .map(|cue| cue.text)
            .collect();
        assert_eq!(texts, ["Chapter one.", "It was a dark night.", "The end."]);
        let before = cue_before(&db, "e1", 31_000).await.unwrap().unwrap();
        assert_eq!(before.text, "It was a dark night.");
        assert!(cue_before(&db, "e1", 0).await.unwrap().is_none());
        clear(&db, "e1").await.unwrap();
        assert!(super::progress(&db, "e1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn a_song_is_forgotten_with_its_shelves_and_nothing_else() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let line = [cue(0, 2_000, "Hello.")];
        for key in [
            "song:a|b",
            "song:a|b#ai",
            "song:a|b#feed",
            "song:a|bc",
            "song:x|y",
        ] {
            replace(&db, key, "local", &line).await.unwrap();
        }
        clear_with_shelves(&db, "song:a|b").await.unwrap();
        for gone in ["song:a|b", "song:a|b#ai", "song:a|b#feed"] {
            assert!(super::progress(&db, gone).await.unwrap().is_none());
            assert!(cues(&db, gone).await.unwrap().is_empty());
        }
        assert_eq!(song_keys(&db).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn videos_are_the_captions_of_neither_songs_nor_episodes() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        sqlx::query(
            "INSERT INTO podcast_episode (id, podcast_id, guid, remote_title, remote_audio_url,
                 state_found_at, updated_at_hlc, origin_device)
             VALUES ('e1', 'p1', 'g1', 'Episode', 'https://feed.example/e1.mp3', 1, 1, 'd')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let line = [cue(0, 2_000, "Hello.")];
        for key in [
            "e1",
            "e1#ai",
            "song:a|b",
            "stremio:movie:tt0032138",
            "source:dev.example/films:42",
        ] {
            replace(&db, key, "local", &line).await.unwrap();
        }
        assert_eq!(
            video_keys(&db).await.unwrap(),
            ["source:dev.example/films:42", "stremio:movie:tt0032138"]
        );
    }
}
