//! Finds a stream for a track (ARCHITECTURE §6.1, M14): verified cache first, then candidates from
//! every `music.stream` provider, ranked by `raygun-resolver`, resolved lazily.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use raygun_db::streams::{self, CandidateRow};
use raygun_ext_host::{ResolvedStream, StreamUse};
use raygun_model::{AudioQuality, MediaSource, TrackQuery};
use raygun_resolver::{
    Candidate, CandidateKey, History, Pick, ProviderRecord, QualityTarget, Scored, rank_for,
    track_key,
};
use tauri::{AppHandle, Manager};

use crate::extensions::ExtensionService;
use crate::resolution::Exclusions;
use crate::startup::AppDb;

/// Searches are kept this long, so trying the next candidate doesn't search again.
const CANDIDATES_TTL: Duration = Duration::from_secs(10 * 60);
/// A failed candidate is skipped for this long.
const FAILURE_COOLDOWN_MS: i64 = 24 * 60 * 60 * 1000;
/// A YouTube video's own audio file that didn't resolve is asked for again after this long
/// when the video itself plays (M46.51).
const OWN_AUDIO_COOLDOWN_MS: i64 = 60 * 60 * 1000;
/// A cached URL this close to expiring is resolved again.
const EXPIRY_MARGIN_MS: i64 = 60 * 1000;
/// Matches tried again when all of them failed within the cooldown (M46.48).
const COOLED_RETRIES: usize = 1;
/// The YouTube extension's stream provider (M46.51), when one is installed (Raygun ships none): its
/// `audio:<video>` candidates resolve to the video's own audio file, its plain `<video>` ones play
/// in the embed. Without it a YouTube item plays in the embed.
pub const YOUTUBE_STREAMS: &str = "dev.raygun.youtube/stream";
const YOUTUBE_AUDIO: &str = "audio:";

/// The YouTube video a pick of YouTube's stream provider is, its audio file or its embed.
pub fn youtube_video(key: &CandidateKey) -> Option<&str> {
    (key.provider_id == YOUTUBE_STREAMS).then(|| {
        key.candidate_id
            .strip_prefix(YOUTUBE_AUDIO)
            .unwrap_or(&key.candidate_id)
    })
}

/// Whether a remembered pick played in YouTube's embed: it's found again, since the video's audio
/// file may play now (M46.51).
fn plays_in_embed(row: &CandidateRow) -> bool {
    row.source
        .as_deref()
        .and_then(|json| serde_json::from_str::<MediaSource>(json).ok())
        .is_some_and(|source| matches!(source, MediaSource::YouTube { .. }))
}

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
        let target = self.quality_target(exclusions.quality).await;
        let using = StreamUse {
            quality: target,
            decodable: exclusions.decodable,
        };
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

        // 0. The item's own YouTube video: its audio file, else its embed (M46.51).
        if let Some(video_id) = &exclusions.youtube
            && let Some(pick) = self
                .own_video(&key, query, video_id, &rows, exclusions, using, now)
                .await
        {
            return Ok(pick);
        }

        // 1. A pick that played before and is still valid plays right away. One that played in
        // YouTube's embed is found again: the video's audio file may play now (M46.51).
        for row in rows
            .iter()
            .filter(|row| !exclusions.fresh_only && !plays_in_embed(row))
        {
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
            let bitrate_kbps = row.bitrate_kbps.and_then(|kbps| u32::try_from(kbps).ok());
            if let Some(source) = source
                && row.verified_at.is_some()
                && fresh
                && !recently_failed(row.failed_at)
                && !excluded(&candidate_key, Some(&source))
                && target.fits(bitrate_kbps)
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
                    bitrate_kbps,
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
            // Having played in the embed doesn't put it ahead of the video's audio file.
            if row.verified_at.is_some() && !plays_in_embed(row) {
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
        let candidates = self.candidates(&key, query, target).await;
        if candidates.is_empty() {
            return Err(format!(
                "No streaming source has \u{201c}{}\u{201d}",
                query.title
            ));
        }
        let ranked = to_try(query, candidates, &history, target, exclusions);
        tracing::info!(track = %key, ranked = ranked.len(), ?target, "stream candidates ranked");

        // 3. The best one that resolves.
        for scored in ranked {
            let candidate = scored.candidate;
            let candidate_key = candidate.key();
            let bitrate_kbps = candidate.bitrate();
            if excluded(&candidate_key, candidate.source.as_ref()) {
                continue;
            }
            let resolved = match candidate.source.clone() {
                Some(source) => Ok((source, None)),
                None => self
                    .extensions()
                    .resolve_stream(&candidate.provider_id, &candidate.id, using)
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
                        bitrate_kbps,
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

    /// The item's own YouTube video (M46.51): its audio file, remembered or asked for now, else its
    /// embed (only to play: downloads and captions read the file); `None` once both were tried.
    #[allow(clippy::too_many_arguments)]
    async fn own_video(
        &self,
        key: &str,
        query: &TrackQuery,
        video_id: &str,
        rows: &[CandidateRow],
        exclusions: &Exclusions,
        using: StreamUse,
        now: i64,
    ) -> Option<Pick> {
        let candidate = |id: String| CandidateKey {
            provider_id: YOUTUBE_STREAMS.into(),
            candidate_id: id,
        };
        let audio = candidate(format!("{YOUTUBE_AUDIO}{video_id}"));
        let embed = candidate(video_id.to_owned());
        let provider_name = self.provider_name(YOUTUBE_STREAMS).await;
        let pick = |key_of: CandidateKey, source, expires_at, from_cache| Pick {
            key: key_of,
            track_key: key.to_owned(),
            title: query.title.clone(),
            artist: query.artist.clone(),
            provider_name: provider_name.clone(),
            source,
            expires_at,
            from_cache,
            bitrate_kbps: None,
        };
        if !exclusions.tried.contains(&audio) {
            let row = rows.iter().find(|row| {
                row.provider_id == audio.provider_id && row.candidate_id == audio.candidate_id
            });
            let failed_lately = row
                .and_then(|row| row.failed_at)
                .is_some_and(|at| now - at < OWN_AUDIO_COOLDOWN_MS);
            let remembered = row
                .filter(|row| {
                    !exclusions.fresh_only
                        && row.verified_at.is_some()
                        && row
                            .expires_at
                            .is_some_and(|expires| expires - EXPIRY_MARGIN_MS > now)
                })
                .and_then(|row| {
                    let source =
                        serde_json::from_str::<MediaSource>(row.source.as_deref()?).ok()?;
                    Some((source, row.expires_at))
                });
            match remembered {
                Some((source, expires_at)) if !failed_lately => {
                    return Some(pick(audio, source, expires_at, true));
                }
                _ if failed_lately => {}
                _ => match self
                    .extensions()
                    .resolve_stream(YOUTUBE_STREAMS, &audio.candidate_id, using)
                    .await
                {
                    Ok(stream) => {
                        return Some(pick(audio, stream.source, stream.expires_at, false));
                    }
                    Err(error) => {
                        tracing::info!(video = %video_id, %error, "no audio file from YouTube; its embed plays");
                        let _ = streams::mark_failed(
                            self.db(),
                            key,
                            YOUTUBE_STREAMS,
                            &audio.candidate_id,
                            &query.title,
                            &error,
                            now,
                        )
                        .await;
                    }
                },
            }
        }
        if using.decodable || exclusions.tried.contains(&embed) {
            return None;
        }
        Some(pick(
            embed,
            MediaSource::YouTube {
                video_id: video_id.to_owned(),
            },
            None,
            false,
        ))
    }

    /// A YouTube video's own audio file (M46.51), for `using`: downloads and captions ask for it
    /// by the video.
    pub async fn youtube_audio(
        &self,
        video_id: &str,
        using: StreamUse,
    ) -> Result<ResolvedStream, String> {
        self.extensions()
            .resolve_stream(
                YOUTUBE_STREAMS,
                &format!("{YOUTUBE_AUDIO}{video_id}"),
                using,
            )
            .await
    }

    /// What a stream is for (M46.51): the quality `asked` (else the streaming setting's), read by
    /// Raygun itself when `decodable`.
    pub async fn stream_use(&self, asked: Option<AudioQuality>, decodable: bool) -> StreamUse {
        StreamUse {
            quality: self.quality_target(asked).await,
            decodable,
        }
    }

    /// The quality a resolution aims for (M46.18): what was asked (downloads), else the
    /// streaming setting, or the mobile data one while this device is on mobile data.
    async fn quality_target(&self, asked: Option<AudioQuality>) -> QualityTarget {
        let quality = match asked {
            Some(quality) => quality,
            None => {
                let prefs = crate::settings::load(self.db())
                    .await
                    .map(|settings| settings.audio_quality)
                    .unwrap_or_default();
                if crate::network::on_mobile_data(&self.app) {
                    prefs.cellular
                } else {
                    prefs.streaming
                }
            }
        };
        quality
            .target_kbps()
            .map_or(QualityTarget::Best, QualityTarget::Kbps)
    }

    /// Candidates from every provider, kept for a while per track and quality.
    async fn candidates(
        &self,
        key: &str,
        query: &TrackQuery,
        target: QualityTarget,
    ) -> Vec<Candidate> {
        let key = format!("{key}|{target:?}");
        let key = key.as_str();
        if let Some((searched, candidates)) = self.searches.lock().unwrap().get(key)
            && searched.elapsed() < CANDIDATES_TTL
        {
            return candidates.clone();
        }
        let mut candidates = Vec::new();
        for answer in self.extensions().stream_candidates(query, target).await {
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
            streams::Played {
                track_key: &pick.track_key,
                provider_id: &pick.key.provider_id,
                candidate_id: &pick.key.candidate_id,
                title: &pick.title,
                source_json: &source,
                expires_at: pick.expires_at,
                bitrate_kbps: pick.bitrate_kbps.map(i64::from),
            },
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

/// The matches to try for a track, best first, leaving out what this load tried. Matches that
/// failed lately are kept out for a day, but when that leaves nothing, the best of them gets
/// one real try per load (M46.48): a song played on purpose is never skipped untried.
fn to_try(
    query: &TrackQuery,
    candidates: Vec<Candidate>,
    history: &History,
    target: QualityTarget,
    exclusions: &Exclusions,
) -> Vec<Scored> {
    let untried = |scored: &Scored| {
        !exclusions.tried.contains(&scored.candidate.key())
            && scored
                .candidate
                .source
                .as_ref()
                .is_none_or(|source| !exclusions.failed_sources.contains(source))
    };
    let ranked = rank_for(query, candidates.clone(), history, target);
    let retried = exclusions
        .tried
        .iter()
        .any(|tried| history.failed.contains(tried));
    if ranked.iter().any(untried) || history.failed.is_empty() || retried {
        return ranked;
    }
    let mut again = history.clone();
    again.failed.clear();
    rank_for(query, candidates, &again, target)
        .into_iter()
        .filter(untried)
        .take(COOLED_RETRIES)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, title: &str) -> Candidate {
        Candidate {
            provider_id: "dev.example/stream".into(),
            provider_name: "Example".into(),
            id: id.into(),
            title: title.into(),
            artist: Some("Artist".into()),
            duration_ms: None,
            quality: None,
            bitrate_kbps: None,
            source: None,
        }
    }

    #[test]
    fn youtube_picks_name_their_video_and_embeds_are_found_again() {
        let key = |provider: &str, id: &str| CandidateKey {
            provider_id: provider.into(),
            candidate_id: id.into(),
        };
        assert_eq!(
            youtube_video(&key(YOUTUBE_STREAMS, "audio:dQw4w9WgXcQ")),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(
            youtube_video(&key(YOUTUBE_STREAMS, "dQw4w9WgXcQ")),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(youtube_video(&key("dev.example/stream", "audio:x")), None);

        let row = |source: Option<&str>| CandidateRow {
            provider_id: YOUTUBE_STREAMS.into(),
            candidate_id: "dQw4w9WgXcQ".into(),
            title: "Song".into(),
            source: source.map(str::to_owned),
            expires_at: None,
            verified_at: Some(1),
            failed_at: None,
            bitrate_kbps: None,
        };
        assert!(plays_in_embed(&row(Some(
            r#"{"kind":"youTube","videoId":"dQw4w9WgXcQ"}"#
        ))));
        assert!(!plays_in_embed(&row(Some(
            r#"{"kind":"url","url":"https://rr1---sn-x.googlevideo.com/videoplayback","headers":{}}"#
        ))));
        assert!(!plays_in_embed(&row(None)));
    }

    #[test]
    fn matches_that_failed_lately_still_get_one_try() {
        let query = TrackQuery {
            title: "Song".into(),
            artist: Some("Artist".into()),
            album: None,
            duration_ms: None,
        };
        let found = vec![candidate("a", "Song"), candidate("b", "Song")];
        let mut history = History::default();
        let titles = |scored: &[Scored]| -> Vec<String> {
            scored
                .iter()
                .map(|scored| scored.candidate.id.clone())
                .collect()
        };
        let none = Exclusions::default();
        // Nothing failed: both, best first.
        assert_eq!(
            titles(&to_try(
                &query,
                found.clone(),
                &history,
                QualityTarget::Any,
                &none
            ))
            .len(),
            2
        );
        // Both failed within the day: one of them is tried anyway.
        history.failed.insert(found[0].key());
        history.failed.insert(found[1].key());
        let once = to_try(&query, found.clone(), &history, QualityTarget::Any, &none);
        assert_eq!(once.len(), 1);
        // It failed again in this load: the song is skipped, saying why.
        let tried = Exclusions {
            tried: vec![once[0].candidate.key()],
            ..Exclusions::default()
        };
        assert!(to_try(&query, found.clone(), &history, QualityTarget::Any, &tried).is_empty());
        // One still fresh: only it, the failed one stays out.
        history.failed.remove(&found[1].key());
        assert_eq!(
            titles(&to_try(&query, found, &history, QualityTarget::Any, &none)),
            ["b"]
        );
    }
}
