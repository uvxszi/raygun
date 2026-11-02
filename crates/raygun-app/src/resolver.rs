//! Finds a stream for a track (ARCHITECTURE §6.1, M14): verified cache first, then candidates from
//! every `music.stream` provider, ranked by `raygun-resolver`, resolved lazily.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use raygun_db::streams;
use raygun_model::{MediaSource, TrackQuery};
use raygun_resolver::{Candidate, CandidateKey, History, Pick, ProviderRecord, rank, track_key};
use tauri::{AppHandle, Manager};

use crate::extensions::ExtensionService;
use crate::resolution::Exclusions;
use crate::startup::AppDb;

/// Searches are kept this long, so trying the next candidate doesn't search again.
const CANDIDATES_TTL: Duration = Duration::from_secs(10 * 60);
/// A failed candidate is skipped for this long.
const FAILURE_COOLDOWN_MS: i64 = 24 * 60 * 60 * 1000;
/// A cached URL this close to expiring is resolved again.
const EXPIRY_MARGIN_MS: i64 = 60 * 1000;

pub struct ResolverService {
    app: AppHandle,
    searches: Mutex<HashMap<String, (Instant, Vec<Candidate>)>>,
}

impl ResolverService {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            searches: Mutex::new(HashMap::new()),
        }
    }

    fn db(&self) -> &raygun_db::Db {
        &self.app.state::<AppDb>().inner().0
    }

    fn extensions(&self) -> Arc<ExtensionService> {
        self.app.state::<Arc<ExtensionService>>().inner().clone()
    }

    /// The best stream for `query` that `exclusions` don't rule out, or why there is none.
    pub async fn resolve(
        &self,
        query: &TrackQuery,
        exclusions: &Exclusions,
    ) -> Result<Pick, String> {
        let key = track_key(query);
        let now = unix_ms();
        let rows = streams::candidates(self.db(), &key)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "stream cache unreadable");
                Vec::new()
            });
        let excluded = |candidate_key: &CandidateKey, source: Option<&MediaSource>| {
            exclusions.tried.contains(candidate_key)
                || source.is_some_and(|source| exclusions.failed_sources.contains(source))
        };
        let recently_failed =
            |failed_at: Option<i64>| failed_at.is_some_and(|at| now - at < FAILURE_COOLDOWN_MS);

        // 1. A pick that played before and is still valid plays right away.
        for row in rows.iter().filter(|_| !exclusions.fresh_only) {
            let candidate_key = CandidateKey {
                provider_id: row.provider_id.clone(),
                candidate_id: row.candidate_id.clone(),
            };
            let source = row
                .source
                .as_deref()
                .and_then(|json| serde_json::from_str::<MediaSource>(json).ok());
            let fresh = row
                .expires_at
                .is_none_or(|expires| expires - EXPIRY_MARGIN_MS > now);
            if let Some(source) = source
                && row.verified_at.is_some()
                && fresh
                && !recently_failed(row.failed_at)
                && !excluded(&candidate_key, Some(&source))
            {
                tracing::info!(track = %key, provider = %row.provider_id, "playing cached stream");
                return Ok(Pick {
                    key: candidate_key,
                    track_key: key,
                    title: row.title.clone(),
                    artist: None,
                    provider_name: self.provider_name(&row.provider_id).await,
                    source,
                    expires_at: row.expires_at,
                    from_cache: true,
                });
            }
        }

        // 2. Rank fresh candidates against what earlier plays taught us.
        let mut history = History::default();
        for row in &rows {
            let candidate_key = CandidateKey {
                provider_id: row.provider_id.clone(),
                candidate_id: row.candidate_id.clone(),
            };
            if row.verified_at.is_some() {
                history.verified.insert(candidate_key.clone());
            }
            if recently_failed(row.failed_at) {
                history.failed.insert(candidate_key);
            }
        }
        for stats in streams::provider_stats(self.db()).await.unwrap_or_default() {
            history.providers.insert(
                stats.provider_id,
                ProviderRecord {
                    successes: u32::try_from(stats.successes).unwrap_or(u32::MAX),
                    failures: u32::try_from(stats.failures).unwrap_or(u32::MAX),
                },
            );
        }
        let candidates = self.candidates(&key, query).await;
        if candidates.is_empty() {
            return Err(format!(
                "No streaming source has \u{201c}{}\u{201d}",
                query.title
            ));
        }
        let ranked = rank(query, candidates, &history);
        tracing::info!(track = %key, ranked = ranked.len(), "stream candidates ranked");

        // 3. The best one that resolves.
        for scored in ranked {
            let candidate = scored.candidate;
            let candidate_key = candidate.key();
            if excluded(&candidate_key, candidate.source.as_ref()) {
                continue;
            }
            let resolved = match candidate.source.clone() {
                Some(source) => Ok((source, None)),
                None => self
                    .extensions()
                    .resolve_stream(&candidate.provider_id, &candidate.id)
                    .await
                    .map(|stream| (stream.source, stream.expires_at)),
            };
            match resolved {
                Ok((source, _)) if exclusions.failed_sources.contains(&source) => continue,
                Ok((source, expires_at)) => {
                    tracing::info!(
                        track = %key,
                        provider = %candidate.provider_id,
                        candidate = %candidate.id,
                        score = scored.score,
                        "stream picked"
                    );
                    return Ok(Pick {
                        key: candidate_key,
                        track_key: key,
                        title: candidate.title,
                        artist: candidate.artist,
                        provider_name: candidate.provider_name,
                        source,
                        expires_at,
                        from_cache: false,
                    });
                }
                Err(error) => {
                    tracing::warn!(provider = %candidate.provider_id, %error, "candidate didn't resolve");
                    let _ = streams::mark_failed(
                        self.db(),
                        &key,
                        &candidate.provider_id,
                        &candidate.id,
                        &candidate.title,
                        &error,
                        now,
                    )
                    .await;
                }
            }
        }
        Err(format!(
            "No playable source found for \u{201c}{}\u{201d}",
            query.title
        ))
    }

    /// Candidates from every provider, kept for a while per track.
    async fn candidates(&self, key: &str, query: &TrackQuery) -> Vec<Candidate> {
        if let Some((searched, candidates)) = self.searches.lock().unwrap().get(key)
            && searched.elapsed() < CANDIDATES_TTL
        {
            return candidates.clone();
        }
        let mut candidates = Vec::new();
        for answer in self.extensions().stream_candidates(query).await {
            match answer.result {
                Ok(found) => candidates.extend(found),
                Err(error) => {
                    tracing::warn!(provider = %answer.provider.id, %error, "stream provider failed");
                }
            }
        }
        if !candidates.is_empty() {
            self.searches
                .lock()
                .unwrap()
                .insert(key.to_owned(), (Instant::now(), candidates.clone()));
        }
        candidates
    }

    async fn provider_name(&self, provider_id: &str) -> String {
        self.extensions()
            .list()
            .await
            .into_iter()
            .flat_map(|extension| extension.providers)
            .find(|provider| provider.id == provider_id)
            .map_or_else(|| provider_id.to_owned(), |provider| provider.name)
    }

    /// The pick played: cache it and credit its provider.
    pub async fn mark_verified(&self, pick: &Pick) {
        let Ok(source) = serde_json::to_string(&pick.source) else {
            return;
        };
        if let Err(error) = streams::mark_verified(
            self.db(),
            &pick.track_key,
            &pick.key.provider_id,
            &pick.key.candidate_id,
            &pick.title,
            &source,
            pick.expires_at,
            unix_ms(),
        )
        .await
        {
            tracing::warn!(%error, "could not record a verified stream");
        }
    }

    /// The pick failed to play: skip it for a while and count it against its provider.
    pub async fn mark_failed(&self, pick: &Pick, reason: &str) {
        tracing::info!(provider = %pick.key.provider_id, candidate = %pick.key.candidate_id, %reason, "stream failed, trying the next");
        if let Err(error) = streams::mark_failed(
            self.db(),
            &pick.track_key,
            &pick.key.provider_id,
            &pick.key.candidate_id,
            &pick.title,
            reason,
            unix_ms(),
        )
        .await
        {
            tracing::warn!(%error, "could not record a failed stream");
        }
    }
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}
