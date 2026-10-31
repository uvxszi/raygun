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
        "SELECT provider_id, candidate_id, title, source, expires_at, verified_at, failed_at
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
        })
        .collect())
}

/// Records a candidate that played: its resolved source, and a success for its provider. Clears
/// any earlier failure.
#[allow(clippy::too_many_arguments)]
pub async fn mark_verified(
    db: &Db,
    track_key: &str,
    provider_id: &str,
    candidate_id: &str,
    title: &str,
    source_json: &str,
    expires_at: Option<i64>,
    now_ms: i64,
) -> Result<(), DbError> {
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "INSERT INTO local_stream_candidate
             (track_key, provider_id, candidate_id, title, source, expires_at, verified_at, failed_at, failure)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL)
         ON CONFLICT (track_key, provider_id, candidate_id) DO UPDATE SET
             title = excluded.title,
             source = excluded.source,
             expires_at = excluded.expires_at,
             verified_at = excluded.verified_at,
             failed_at = NULL,
             failure = NULL",
    )
    .bind(track_key)
    .bind(provider_id)
    .bind(candidate_id)
    .bind(title)
    .bind(source_json)
    .bind(expires_at)
    .bind(now_ms)
    .execute(&mut *transaction)
    .await?;
    count(&mut transaction, provider_id, "successes").await?;
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
