//! Storage for the stream resolution cache (M14): which candidates played or failed for a track,
//! and how reliable each provider has been. Device-local; these rows never sync.

use sqlx::Row;

use crate::{Db, DbError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateRow {
    pub provider_id: String,
    pub candidate_id: String,
    pub title: String,
    /// JSON `MediaSource`, once resolved.
    pub source: Option<String>,
    pub expires_at: Option<i64>,
    pub verified_at: Option<i64>,
    pub failed_at: Option<i64>,
    /// The stream's bitrate when it played (M46.18).
    pub bitrate_kbps: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderStats {
    pub provider_id: String,
    pub successes: i64,
    pub failures: i64,
}

/// Everything remembered about a track's candidates.
pub async fn candidates(db: &Db, track_key: &str) -> Result<Vec<CandidateRow>, DbError> {
    let rows = sqlx::query(
        "SELECT provider_id, candidate_id, title, source, expires_at, verified_at, failed_at,
                bitrate_kbps
         FROM local_stream_candidate WHERE track_key = ?1
         ORDER BY verified_at DESC NULLS LAST",
    )
    .bind(track_key)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CandidateRow {
            provider_id: row.get("provider_id"),
            candidate_id: row.get("candidate_id"),
            title: row.get("title"),
            source: row.get("source"),
            expires_at: row.get("expires_at"),
            verified_at: row.get("verified_at"),
            failed_at: row.get("failed_at"),
            bitrate_kbps: row.get("bitrate_kbps"),
        })
        .collect())
}

/// The source Raygun found for a song and played: a stream source's candidate, with the names
/// that source gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundPick {
    pub track_key: String,
    pub provider_id: String,
    pub candidate_id: String,
    pub title: String,
    pub artist: Option<String>,
    pub verified_at: i64,
}

/// The newest pick that played one of `track_keys` and hasn't failed since, from a stream source
/// (picks the user made, kept under a source's name, are left out).
pub async fn found_pick(db: &Db, track_keys: &[String]) -> Result<Option<FoundPick>, DbError> {
    let mut found: Option<FoundPick> = None;
    for key in track_keys {
        let row = sqlx::query(
            "SELECT track_key, provider_id, candidate_id, title, artist, verified_at
             FROM local_stream_candidate
             WHERE track_key = ?1 AND verified_at IS NOT NULL AND failed_at IS NULL
               AND instr(provider_id, '/') > 0
             ORDER BY verified_at DESC LIMIT 1",
        )
        .bind(key)
        .fetch_optional(db.pool())
        .await?;
        let Some(row) = row else { continue };
        let pick = FoundPick {
            track_key: row.get("track_key"),
            provider_id: row.get("provider_id"),
            candidate_id: row.get("candidate_id"),
            title: row.get("title"),
            artist: row.get("artist"),
            verified_at: row.get("verified_at"),
        };
        if found
            .as_ref()
            .is_none_or(|kept| pick.verified_at > kept.verified_at)
        {
            found = Some(pick);
        }
    }
    Ok(found)
}

/// A stream that played, as [`mark_verified`] records it.
#[derive(Debug, Clone, Copy)]
pub struct Played<'a> {
    pub track_key: &'a str,
    pub provider_id: &'a str,
    pub candidate_id: &'a str,
    pub title: &'a str,
    /// The artist the source named, if it did.
    pub artist: Option<&'a str>,
    pub source_json: &'a str,
    pub expires_at: Option<i64>,
    pub bitrate_kbps: Option<i64>,
}

/// Records a candidate that played: its resolved source, and a success for its provider. Clears
/// any earlier failure.
pub async fn mark_verified(db: &Db, played: Played<'_>, now_ms: i64) -> Result<(), DbError> {
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "INSERT INTO local_stream_candidate
             (track_key, provider_id, candidate_id, title, source, expires_at, verified_at, failed_at, failure,
              bitrate_kbps, artist)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, ?8, ?9)
         ON CONFLICT (track_key, provider_id, candidate_id) DO UPDATE SET
             title = excluded.title,
             artist = COALESCE(excluded.artist, local_stream_candidate.artist),
             source = excluded.source,
             expires_at = excluded.expires_at,
             verified_at = excluded.verified_at,
             failed_at = NULL,
             failure = NULL,
             bitrate_kbps = excluded.bitrate_kbps",
    )
    .bind(played.track_key)
    .bind(played.provider_id)
    .bind(played.candidate_id)
    .bind(played.title)
    .bind(played.source_json)
    .bind(played.expires_at)
    .bind(now_ms)
    .bind(played.bitrate_kbps)
    .bind(played.artist)
    .execute(&mut *transaction)
    .await?;
    count(&mut transaction, played.provider_id, "successes").await?;
    transaction.commit().await?;
    Ok(())
}

/// Records a candidate that failed to resolve or play, and a failure for its provider.
pub async fn mark_failed(
    db: &Db,
    track_key: &str,
    provider_id: &str,
    candidate_id: &str,
    title: &str,
    failure: &str,
    now_ms: i64,
) -> Result<(), DbError> {
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "INSERT INTO local_stream_candidate
             (track_key, provider_id, candidate_id, title, failed_at, failure)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (track_key, provider_id, candidate_id) DO UPDATE SET
             failed_at = excluded.failed_at,
             failure = excluded.failure",
    )
    .bind(track_key)
    .bind(provider_id)
    .bind(candidate_id)
    .bind(title)
    .bind(now_ms)
    .bind(failure)
    .execute(&mut *transaction)
    .await?;
    count(&mut transaction, provider_id, "failures").await?;
    transaction.commit().await?;
    Ok(())
}

/// Remembers a stream the user picked for a track as one that plays (M46.48): it plays at once
/// the next time, before any search. It counts for no provider.
pub async fn remember_pick(db: &Db, played: Played<'_>, now_ms: i64) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO local_stream_candidate
             (track_key, provider_id, candidate_id, title, source, expires_at, verified_at,
              failed_at, failure, bitrate_kbps, artist)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, ?8, ?9)
         ON CONFLICT (track_key, provider_id, candidate_id) DO UPDATE SET
             title = excluded.title,
             artist = COALESCE(excluded.artist, local_stream_candidate.artist),
             source = excluded.source,
             expires_at = excluded.expires_at,
             verified_at = excluded.verified_at,
             failed_at = NULL,
             failure = NULL,
             bitrate_kbps = excluded.bitrate_kbps",
    )
    .bind(played.track_key)
    .bind(played.provider_id)
    .bind(played.candidate_id)
    .bind(played.title)
    .bind(played.source_json)
    .bind(played.expires_at)
    .bind(now_ms)
    .bind(played.bitrate_kbps)
    .bind(played.artist)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Gives track `to` the streams that played for track `from` and haven't failed since, as the
/// newest picks (M46.48: a song plays what was found under the names of the source the user
/// picked for it).
pub async fn copy_picks(db: &Db, from: &str, to: &str, now_ms: i64) -> Result<(), DbError> {
    if from == to {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO local_stream_candidate
             (track_key, provider_id, candidate_id, title, source, expires_at, verified_at,
              failed_at, failure, bitrate_kbps, artist)
         SELECT ?2, provider_id, candidate_id, title, source, expires_at, ?3, NULL, NULL,
                bitrate_kbps, artist
         FROM local_stream_candidate
         WHERE track_key = ?1 AND verified_at IS NOT NULL AND failed_at IS NULL
           AND source IS NOT NULL
         ON CONFLICT (track_key, provider_id, candidate_id) DO UPDATE SET
             title = excluded.title,
             artist = COALESCE(excluded.artist, local_stream_candidate.artist),
             source = excluded.source,
             expires_at = excluded.expires_at,
             verified_at = excluded.verified_at,
             failed_at = NULL,
             failure = NULL,
             bitrate_kbps = excluded.bitrate_kbps",
    )
    .bind(from)
    .bind(to)
    .bind(now_ms)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Forgets the stream a remembered pick played from, which stopped working: the candidate isn't
/// played from it again, but keeps having played the track (it's asked for a new stream). Not a
/// failure of the candidate or its provider.
pub async fn forget_stream(
    db: &Db,
    track_key: &str,
    provider_id: &str,
    candidate_id: &str,
) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE local_stream_candidate SET source = NULL, expires_at = NULL
         WHERE track_key = ?1 AND provider_id = ?2 AND candidate_id = ?3",
    )
    .bind(track_key)
    .bind(provider_id)
    .bind(candidate_id)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// [`forget_stream`] for every pick of the tracks that plays from `source_json` (a stream picked
/// for a song, which the song carried itself and which stopped working).
pub async fn forget_source(
    db: &Db,
    track_keys: &[String],
    source_json: &str,
) -> Result<(), DbError> {
    for key in track_keys {
        sqlx::query(
            "UPDATE local_stream_candidate SET source = NULL, expires_at = NULL
             WHERE track_key = ?1 AND source = ?2",
        )
        .bind(key)
        .bind(source_json)
        .execute(db.pool())
        .await?;
    }
    Ok(())
}

/// Forgets tracks' failed candidates (M46.48): after the user picks a source, what failed before
/// is tried again rather than kept out for a day.
pub async fn clear_failures(db: &Db, track_keys: &[String]) -> Result<(), DbError> {
    for key in track_keys {
        sqlx::query(
            "UPDATE local_stream_candidate SET failed_at = NULL, failure = NULL
             WHERE track_key = ?1 AND failed_at IS NOT NULL",
        )
        .bind(key)
        .execute(db.pool())
        .await?;
    }
    Ok(())
}

async fn count(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    provider_id: &str,
    column: &'static str,
) -> Result<(), DbError> {
    let sql = format!(
        "INSERT INTO local_stream_provider_stats (provider_id, {column}) VALUES (?1, 1)
         ON CONFLICT (provider_id) DO UPDATE SET {column} = {column} + 1"
    );
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(provider_id)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

pub async fn provider_stats(db: &Db) -> Result<Vec<ProviderStats>, DbError> {
    let rows = sqlx::query(
        "SELECT provider_id, successes, failures FROM local_stream_provider_stats ORDER BY provider_id",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| ProviderStats {
            provider_id: row.get("provider_id"),
            successes: row.get("successes"),
            failures: row.get("failures"),
        })
        .collect())
}
