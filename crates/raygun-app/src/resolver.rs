//! Finds a stream for a track (ARCHITECTURE §6.1, M14): verified cache first, then candidates from
//! every `music.stream` provider, ranked by `raygun-resolver`, resolved lazily. A deeper search
//! (a load looking for another source, `Exclusions::deep`) skips what was remembered, asks the
//! sources again, gives matches that failed lately a try and searches the other sources for the
//! song, as Find a source does; sources that can't answer now are left for a later look.
//!
//! What played is kept for good (M46.53): a stream's address expires, the pick (its source and
//! that source's id for the song) doesn't. The sources of the picks that played a song are asked
//! for a new stream, in turn, before any search; a pick that failed since it last played comes
//! after the others and loses its place in the ranking, without being forgotten.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use raygun_db::streams::{self, CandidateRow};
use raygun_ext_host::{CallError, ResolvedStream, StreamUse};
use raygun_model::{AudioQuality, MediaSource, TrackQuery};
use raygun_net::IssueKind;
use raygun_resolver::{
    Candidate, CandidateKey, History, Pick, ProviderRecord, QualityTarget, Scored, rank_for,
    track_key,
};
use tauri::{AppHandle, Manager};

use crate::extensions::ExtensionService;
use crate::resolution::{Exclusions, Notes};
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
/// A deeper search tries this many matches at most, and asks each source for this many streams
/// at most, so a song no source plays doesn't keep them busy.
const DEEP_TRIES: usize = 8;
const DEEP_PER_SOURCE: usize = 2;
/// Candidates each stream source is asked for in a deeper search.
const DEEP_CANDIDATES: u32 = 20;
/// Other sources' answers a deeper search looks through.
const OTHER_SOURCES: usize = 6;
/// A match this good (title, artist, no wrong length or version) is tried as soon as its source
/// answers, without waiting for the slower ones.
const CONFIDENT_SCORE: i32 = 60;

/// What a search is looking for, for each match it tries.
#[derive(Clone, Copy)]
struct Search<'a> {
    key: &'a str,
    query: &'a TrackQuery,
    exclusions: &'a Exclusions,
    using: StreamUse,
    now: i64,
}

/// Who a search asked so far.
#[derive(Default)]
struct Attempts {
    /// Streams asked of each source.
    asked: HashMap<String, usize>,
    /// Sources that limited requests: not asked again in this search.
    limited: HashSet<String>,
    /// Picks asked for a new stream before the search ([`remembered`]).
    asked_again: HashSet<CandidateKey>,
}

/// The stream sources' answers to a search, as they come.
enum Answers {
    /// A search kept from moments ago, as one answer.
    Kept(Option<Vec<Candidate>>),
    Coming(tokio::sync::mpsc::UnboundedReceiver<raygun_ext_host::ProviderCandidates>),
}

impl Answers {
    /// The next source's candidates, or its id and why it failed.
    async fn next(&mut self) -> Option<Result<Vec<Candidate>, (String, String)>> {
        match self {
            Self::Kept(kept) => kept.take().map(Ok),
            Self::Coming(receiver) => {
                let answer = receiver.recv().await?;
                Some(answer.result.map_err(|error| (answer.provider.id, error)))
            }
        }
    }
}

/// Picks that played a song whose sources are asked for a new stream before a search, at most.
const REMEMBERED_TRIES: usize = 3;
/// A song's known sources a download tries before a search, at most.
const KNOWN_TRIES: usize = 3;
/// The YouTube extension's stream provider (M46.51), when one is installed (Raygun ships none): its
/// `audio:<video>` candidates resolve to the video's own audio file, its plain `<video>` ones play
/// in the embed. Without it a YouTube item plays in the embed.
pub const YOUTUBE_STREAMS: &str = "dev.raygun.youtube/stream";
const YOUTUBE_AUDIO: &str = "audio:";
/// Nuclear's YouTube plugin's stream provider: its candidates are plain video ids, answered with
/// the video's own audio file by the host's YouTube API. It answers for a video's audio when the
/// YouTube extension isn't installed.
pub const NUCLEAR_YOUTUBE_STREAMS: &str = "nuclear.nuclear-plugin-youtube/youtube";

/// Which provider asks for a YouTube video's own audio file, and with which candidate: the
/// YouTube extension's `audio:<video>` when it's listed (or when neither is: its error says so),
/// else Nuclear's YouTube plugin's `<video>`.
fn youtube_audio_call(listed: &[String], video_id: &str) -> (&'static str, String) {
    let has = |id: &str| listed.iter().any(|listed| listed == id);
    if has(NUCLEAR_YOUTUBE_STREAMS) && !has(YOUTUBE_STREAMS) {
        (NUCLEAR_YOUTUBE_STREAMS, video_id.to_owned())
    } else {
        (YOUTUBE_STREAMS, format!("{YOUTUBE_AUDIO}{video_id}"))
    }
}

/// The installed YouTube sources through whose own fetch Raygun's YouTube module may ask YouTube,
/// in turn: the YouTube extension (any of its providers), then Nuclear's YouTube plugin. None
/// when neither is installed: Raygun ships no YouTube source.
pub fn youtube_extensions(listed: &[String]) -> Vec<String> {
    let extension_of = |provider: &str| provider.split('/').next().unwrap_or(provider).to_owned();
    let mut found: Vec<String> = Vec::new();
    for wanted in [YOUTUBE_STREAMS, NUCLEAR_YOUTUBE_STREAMS] {
        let extension = extension_of(wanted);
        if listed
            .iter()
            .any(|provider| extension_of(provider) == extension)
            && !found.contains(&extension)
        {
            found.push(extension);
        }
    }
    found
}

/// What "via …" says for a provider no installed extension lists: YouTube's embed (no YouTube
/// extension installed), or a remembered pick of an extension removed since. Its extension's own
/// name, from the last part of its id (`dev.raygun.audius/stream` → `Audius`).
fn unlisted_provider_name(provider_id: &str) -> String {
    if provider_id == YOUTUBE_STREAMS {
        return "YouTube".to_owned();
    }
    let extension = provider_id.split('/').next().unwrap_or(provider_id);
    let last = extension.rsplit('.').next().unwrap_or(extension);
    let mut chars = last.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => provider_id.to_owned(),
    }
}

/// The YouTube video a pick of YouTube's stream provider is, its audio file or its embed.
pub fn youtube_video(key: &CandidateKey) -> Option<&str> {
    (key.provider_id == YOUTUBE_STREAMS).then(|| {
        key.candidate_id
            .strip_prefix(YOUTUBE_AUDIO)
            .unwrap_or(&key.candidate_id)
    })
}

/// The good matches ([`CONFIDENT_SCORE`]) among `ranked` not tried yet in this search, in
/// their order, marked as tried: they're tried while slower sources still answer.
fn confident_new(ranked: Vec<Scored>, attempted: &mut HashSet<CandidateKey>) -> Vec<Scored> {
    ranked
        .into_iter()
        .filter(|scored| {
            scored.score >= CONFIDENT_SCORE && attempted.insert(scored.candidate.key())
        })
        .collect()
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
        self.find(query, exclusions).await.0
    }

    /// [`Self::resolve`], with what the search met on its way: matches that failed for good,
    /// those whose source couldn't answer now, and whether a source limited requests.
    pub async fn find(
        &self,
        query: &TrackQuery,
        exclusions: &Exclusions,
    ) -> (Result<Pick, String>, Notes) {
        let mut notes = Notes::default();
        let found = self.search(query, exclusions, &mut notes).await;
        (found, notes)
    }

    async fn search(
        &self,
        query: &TrackQuery,
        exclusions: &Exclusions,
        notes: &mut Notes,
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
                || exclusions.unanswered.contains(candidate_key)
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

        // 1. A pick that played before and is still valid plays right away, those that failed
        // since they last played after the rest. One that played in YouTube's embed is found
        // again: the video's audio file may play now (M46.51). A deeper search is past them: what
        // was remembered didn't play.
        let mut cached: Vec<&CandidateRow> = rows
            .iter()
            .filter(|row| !exclusions.fresh_only && !exclusions.deep && !plays_in_embed(row))
            .collect();
        cached.sort_by_key(|row| row.failed_at.is_some());
        for row in cached {
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
                    artist: row.artist.clone(),
                    provider_name: self.provider_name(&row.provider_id).await,
                    source,
                    expires_at: row.expires_at,
                    from_cache: true,
                    bitrate_kbps,
                });
            }
        }

        // 1b. The sources that played the song before keep it: once a stream expired or stopped
        // working, its source is asked for a new one before any search, a few picks in turn (one
        // that failed since it last played after the rest). Only when they fail (or can't answer
        // now) are the sources searched again, without asking these again.
        let mut asked_again: HashSet<CandidateKey> = HashSet::new();
        if !exclusions.fresh_only && !exclusions.deep {
            let extensions = self.extensions();
            let providers = extensions.stream_provider_ids().await;
            let picks = remembered(&rows, exclusions, &providers, target, now);
            for row in picks.into_iter().take(REMEMBERED_TRIES) {
                if extensions.resting(&row.provider_id) {
                    continue;
                }
                asked_again.insert(CandidateKey {
                    provider_id: row.provider_id.clone(),
                    candidate_id: row.candidate_id.clone(),
                });
                match self.ask_again(&key, row, exclusions, using, notes).await {
                    Some(pick) => return Ok(pick),
                    None => {
                        tracing::info!(track = %key, provider = %row.provider_id, "a source that played the song before had no stream now")
                    }
                }
                // A source that limited requests is left alone for the rest of this search.
                if notes.limited {
                    break;
                }
            }
        }

        // 2. Rank fresh candidates against what earlier plays taught us.
        let mut history = History::default();
        for row in &rows {
            let candidate_key = CandidateKey {
                provider_id: row.provider_id.clone(),
                candidate_id: row.candidate_id.clone(),
            };
            // Having played in the embed doesn't put it ahead of the video's audio file, nor
            // does a pick that failed since it last played keep its place (M46.53).
            if row.verified_at.is_some() && row.failed_at.is_none() && !plays_in_embed(row) {
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
        // 3. The best one that resolves. Sources sitting out aren't asked, nor those that limited
        // requests in this search; a deeper search asks each source for a few streams at most.
        // Sources answer one by one: a good match ([`CONFIDENT_SCORE`]) is tried as soon as it's
        // in, while slower sources still look; the rest are ranked once every source answered.
        let search = Search {
            key: &key,
            query,
            exclusions,
            using,
            now,
        };
        let mut attempts = Attempts {
            asked_again,
            ..Attempts::default()
        };
        let mut attempted: HashSet<CandidateKey> = HashSet::new();
        let mut candidates = Vec::new();
        if exclusions.deep {
            candidates = self.candidates(&key, query, target, true, notes).await;
            candidates.extend(self.other_sources(&key, query, target, notes).await);
        } else {
            let mut answers = self.candidates_arriving(&key, query, target);
            while let Some(answer) = answers.next().await {
                match answer {
                    Ok(found) => candidates.extend(found),
                    // It said it needs a key: it isn't asked again until it's set up.
                    Err((_, error)) if raygun_ext_host::needs_setup_in(&error).is_some() => {
                        notes.needs_setup = true;
                        continue;
                    }
                    Err((provider, error)) => {
                        notes.troubled = true;
                        tracing::warn!(%provider, %error, "stream provider failed");
                        continue;
                    }
                }
                let ranked = to_try(query, candidates.clone(), &history, target, exclusions);
                for scored in confident_new(ranked, &mut attempted) {
                    if let Some(pick) = self.attempt(scored, &search, &mut attempts, notes).await {
                        return Ok(pick);
                    }
                }
            }
            // Stream sources waiting for their key weren't asked.
            if self.extensions().stream_sources_need_setup().await {
                notes.needs_setup = true;
            }
        }
        if candidates.is_empty() {
            return Err(format!(
                "No streaming source has \u{201c}{}\u{201d}",
                query.title
            ));
        }
        let ranked = to_try(query, candidates, &history, target, exclusions);
        tracing::info!(track = %key, ranked = ranked.len(), ?target, "stream candidates ranked");
        for scored in ranked {
            if attempted.contains(&scored.candidate.key()) {
                continue;
            }
            if let Some(pick) = self.attempt(scored, &search, &mut attempts, notes).await {
                return Ok(pick);
            }
        }
        Err(format!(
            "No playable source found for \u{201c}{}\u{201d}",
            query.title
        ))
    }

    /// Asks for a ranked match's stream (one that came with its stream plays without asking);
    /// `None` when it has none for this load, noting why in `notes`.
    async fn attempt(
        &self,
        scored: Scored,
        search: &Search<'_>,
        attempts: &mut Attempts,
        notes: &mut Notes,
    ) -> Option<Pick> {
        let Search {
            key,
            query,
            exclusions,
            using,
            now,
        } = *search;
        let extensions = self.extensions();
        let candidate = scored.candidate;
        let candidate_key = candidate.key();
        let bitrate_kbps = candidate.bitrate();
        let excluded = exclusions.tried.contains(&candidate_key)
            || exclusions.unanswered.contains(&candidate_key)
            || candidate
                .source
                .as_ref()
                .is_some_and(|source| exclusions.failed_sources.contains(source));
        // A pick asked for a stream just now isn't asked again (one the search gave a stream
        // with plays without asking).
        if excluded || (candidate.source.is_none() && attempts.asked_again.contains(&candidate_key))
        {
            return None;
        }
        let resolved = match candidate.source.clone() {
            Some(source) => Ok((source, None)),
            None => {
                let provider = candidate.provider_id.as_str();
                // A source waiting for its key isn't asked; that says nothing of the match.
                if extensions.needs_setup(provider) {
                    notes.needs_setup = true;
                    return None;
                }
                let calls = attempts.asked.entry(provider.to_owned()).or_default();
                if attempts.limited.contains(provider)
                    || extensions.resting(provider)
                    || (exclusions.deep && *calls >= DEEP_PER_SOURCE)
                {
                    notes.unanswered.push(candidate_key);
                    return None;
                }
                *calls += 1;
                extensions
                    .resolve_stream_call(provider, &candidate.id, using)
                    .await
                    .map(|stream| (stream.source, stream.expires_at))
            }
        };
        match resolved {
            // The stream that just failed, again: this match has nothing else for the load.
            Ok((source, _)) if exclusions.failed_sources.contains(&source) => {
                notes.failed.push(candidate_key);
                None
            }
            Ok((source, expires_at)) => {
                tracing::info!(
                    track = %key,
                    provider = %candidate.provider_id,
                    candidate = %candidate.id,
                    score = scored.score,
                    "stream picked"
                );
                // A song that came without a length has the one its source gives (M46.26).
                if query.duration_ms.is_none() {
                    self.learn_length(key, candidate.duration_ms);
                }
                Some(Pick {
                    key: candidate_key,
                    track_key: key.to_owned(),
                    title: candidate.title,
                    artist: candidate.artist,
                    provider_name: candidate.provider_name,
                    source,
                    expires_at,
                    from_cache: false,
                    bitrate_kbps,
                })
            }
            // The source's trouble now, not the match's: a later look asks again.
            Err(error) if passing(&error) => {
                tracing::info!(provider = %candidate.provider_id, %error, "a source couldn't answer now");
                if error.kind() == Some(IssueKind::RateLimited) {
                    notes.limited = true;
                    notes.retry_after = notes.retry_after.max(raygun_ext_host::asked_wait(&error));
                    attempts.limited.insert(candidate.provider_id.clone());
                }
                notes.unanswered.push(candidate_key);
                None
            }
            // The source just said it needs a key: not the match's failure.
            Err(error) if error.is_needs_setup() => {
                notes.needs_setup = true;
                None
            }
            Err(error) => {
                tracing::warn!(provider = %candidate.provider_id, %error, "candidate didn't resolve");
                let _ = streams::mark_failed(
                    self.db(),
                    key,
                    &candidate.provider_id,
                    &candidate.id,
                    &candidate.title,
                    &error.to_string(),
                    now,
                )
                .await;
                notes.failed.push(candidate_key);
                None
            }
        }
    }

    /// The stream sources' candidates for a song as they answer: a search kept from moments ago
    /// at once, else every source asked in the background, the whole answer kept for the next
    /// look once all of them are in (also when this one already played a match).
    fn candidates_arriving(&self, key: &str, query: &TrackQuery, target: QualityTarget) -> Answers {
        let kept = format!("{key}|{target:?}");
        if let Some((searched, candidates)) = self.searches.lock().unwrap().get(&kept)
            && searched.elapsed() < CANDIDATES_TTL
        {
            return Answers::Kept(Some(candidates.clone()));
        }
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let extensions = self.extensions();
        let app = self.app.clone();
        let query = query.clone();
        tauri::async_runtime::spawn(async move {
            let mut all = Vec::new();
            extensions
                .stream_candidates_each(&query, None, target, |answer| {
                    if let Ok(found) = &answer.result {
                        all.extend(found.iter().cloned());
                    }
                    let _ = sender.send(answer);
                })
                .await;
            if !all.is_empty()
                && let Some(resolver) = app.try_state::<Arc<ResolverService>>()
            {
                resolver
                    .searches
                    .lock()
                    .unwrap()
                    .insert(kept, (Instant::now(), all));
            }
        });
        Answers::Coming(receiver)
    }

    /// Asks the source of a pick that played `key` before for a new stream (see [`remembered`]).
    /// A stream that failed in this load, or none, counts against the pick as a search's would;
    /// a source that can't answer now is left for a later look. `None`: search instead.
    async fn ask_again(
        &self,
        key: &str,
        row: &CandidateRow,
        exclusions: &Exclusions,
        using: StreamUse,
        notes: &mut Notes,
    ) -> Option<Pick> {
        let candidate_key = CandidateKey {
            provider_id: row.provider_id.clone(),
            candidate_id: row.candidate_id.clone(),
        };
        // The source waits for its key or sign-in: the pick stays as it is, the sources are
        // searched instead.
        if self.extensions().needs_setup(&row.provider_id) {
            notes.needs_setup = true;
            return None;
        }
        let answer = self
            .extensions()
            .resolve_stream_call(&row.provider_id, &row.candidate_id, using)
            .await;
        match answer {
            Ok(stream) if exclusions.failed_sources.contains(&stream.source) => {
                notes.failed.push(candidate_key);
                None
            }
            Ok(stream) => {
                tracing::info!(track = %key, provider = %row.provider_id, candidate = %row.candidate_id, "the source that played the song before gave a new stream");
                Some(Pick {
                    key: candidate_key,
                    track_key: key.to_owned(),
                    title: row.title.clone(),
                    // Kept with the pick when it played (and kept when unknown now).
                    artist: row.artist.clone(),
                    provider_name: self.provider_name(&row.provider_id).await,
                    source: stream.source,
                    expires_at: stream.expires_at,
                    // A new stream: if it fails, the pick is skipped for a while and the
                    // sources are searched (a remembered one would be asked again).
                    from_cache: false,
                    bitrate_kbps: row.bitrate_kbps.and_then(|kbps| u32::try_from(kbps).ok()),
                })
            }
            Err(error) if passing(&error) => {
                tracing::info!(provider = %row.provider_id, %error, "a source couldn't answer now");
                if error.kind() == Some(IssueKind::RateLimited) {
                    notes.limited = true;
                    notes.retry_after = notes.retry_after.max(raygun_ext_host::asked_wait(&error));
                }
                notes.unanswered.push(candidate_key);
                None
            }
            Err(error) if error.is_needs_setup() => {
                notes.needs_setup = true;
                None
            }
            Err(error) => {
                tracing::warn!(provider = %row.provider_id, %error, "the source that played the song before has no stream for it now");
                let _ = streams::mark_failed(
                    self.db(),
                    key,
                    &row.provider_id,
                    &row.candidate_id,
                    &row.title,
                    &error.to_string(),
                    unix_ms(),
                )
                .await;
                notes.failed.push(candidate_key);
                None
            }
        }
    }

    /// Keeps a length a song's stream source gave, where none is known (M46.26).
    fn learn_length(&self, key: &str, ms: Option<u32>) {
        let Some(ms) = ms.filter(|ms| *ms > 0) else {
            return;
        };
        if let Some(durations) = self
            .app
            .try_state::<Arc<crate::durations::DurationService>>()
        {
            durations.learn_stream(key.to_owned(), i64::from(ms));
        }
    }

    /// The other sources' answers for a song (a deeper search): each source's best answer that
    /// names the song's artist, as Find a source lists them. An answer with a stream of its own
    /// is a candidate as it is; for the best one without, the stream sources are asked again
    /// under the names that source spells the song with.
    async fn other_sources(
        &self,
        key: &str,
        query: &TrackQuery,
        target: QualityTarget,
        notes: &mut Notes,
    ) -> Vec<Candidate> {
        let extensions = self.extensions();
        let answers = crate::import::find_songs(&extensions, query, OTHER_SOURCES).await;
        let mut found = Vec::new();
        let mut respelled: Option<TrackQuery> = None;
        for answer in answers {
            let track = answer.track;
            match track.source.clone().and_then(crate::collections::portable) {
                Some(source) => {
                    let id = match &track.external_id {
                        Some(id) => id.clone(),
                        None => serde_json::to_string(&source).unwrap_or_default(),
                    };
                    found.push(Candidate {
                        provider_id: answer.provider_id,
                        provider_name: answer.provider_name,
                        id,
                        title: track.title,
                        artist: track.artist,
                        duration_ms: track.duration_ms,
                        quality: None,
                        bitrate_kbps: None,
                        source: Some(source),
                    });
                }
                None if respelled.is_none() => {
                    let names = TrackQuery {
                        title: track.title,
                        artist: track.artist,
                        album: track.album,
                        duration_ms: track.duration_ms.or(query.duration_ms),
                    };
                    if track_key(&names) != key {
                        respelled = Some(names);
                    }
                }
                None => {}
            }
        }
        if let Some(names) = respelled {
            found.extend(
                self.candidates(&track_key(&names), &names, target, true, notes)
                    .await,
            );
        }
        found
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
            // A remembered file whose stream just failed is asked for again; a deeper search asks
            // even when it failed lately.
            match remembered {
                Some((source, expires_at))
                    if !failed_lately && !exclusions.failed_sources.contains(&source) =>
                {
                    return Some(pick(audio, source, expires_at, true));
                }
                _ if failed_lately && !exclusions.deep => {}
                _ => match self.resolve_youtube_audio(video_id, using).await {
                    // The same stream that failed just now: on to the embed.
                    Ok(stream) if exclusions.failed_sources.contains(&stream.source) => {}
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
        let video = MediaSource::YouTube {
            video_id: video_id.to_owned(),
        };
        if using.decodable
            || exclusions.tried.contains(&embed)
            || exclusions.failed_sources.contains(&video)
        {
            return None;
        }
        Some(pick(embed, video, None, false))
    }

    /// A YouTube video's own audio file (M46.51), for `using`: downloads and captions ask for it
    /// by the video.
    pub async fn youtube_audio(
        &self,
        video_id: &str,
        using: StreamUse,
    ) -> Result<ResolvedStream, String> {
        self.resolve_youtube_audio(video_id, using).await
    }

    /// Asks the installed YouTube source for a video's own audio file ([`youtube_audio_call`]).
    async fn resolve_youtube_audio(
        &self,
        video_id: &str,
        using: StreamUse,
    ) -> Result<ResolvedStream, String> {
        let listed: Vec<String> = self
            .extensions()
            .list()
            .await
            .into_iter()
            .flat_map(|extension| extension.providers)
            .map(|provider| provider.id)
            .collect();
        let (provider, candidate) = youtube_audio_call(&listed, video_id);
        self.extensions()
            .resolve_stream(provider, &candidate, using)
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
                // Data Saver caps it while it applies.
                let prefs = crate::settings::load_in_effect(self.db())
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

    /// Candidates from every provider, kept for a while per track and quality. `again` asks the
    /// providers anew, for more candidates (a deeper search: the kept ones didn't play). A
    /// provider that failed to answer is noted: then it's not known that no source has the song.
    async fn candidates(
        &self,
        key: &str,
        query: &TrackQuery,
        target: QualityTarget,
        again: bool,
        notes: &mut Notes,
    ) -> Vec<Candidate> {
        let key = format!("{key}|{target:?}");
        let key = key.as_str();
        if !again
            && let Some((searched, candidates)) = self.searches.lock().unwrap().get(key)
            && searched.elapsed() < CANDIDATES_TTL
        {
            return candidates.clone();
        }
        let extensions = self.extensions();
        let answers = if again {
            extensions
                .stream_candidates_up_to(query, DEEP_CANDIDATES, target)
                .await
        } else {
            extensions.stream_candidates(query, target).await
        };
        let mut candidates = Vec::new();
        for answer in answers {
            match answer.result {
                Ok(found) => candidates.extend(found),
                // It said it needs a key: it isn't asked again until it's set up.
                Err(error) if raygun_ext_host::needs_setup_in(&error).is_some() => {
                    notes.needs_setup = true;
                }
                Err(error) => {
                    notes.troubled = true;
                    tracing::warn!(provider = %answer.provider.id, %error, "stream provider failed");
                }
            }
        }
        // Stream sources waiting for their key weren't asked.
        if extensions.stream_sources_need_setup().await {
            notes.needs_setup = true;
        }
        if !candidates.is_empty() {
            self.searches
                .lock()
                .unwrap()
                .insert(key.to_owned(), (Instant::now(), candidates.clone()));
        }
        candidates
    }

    /// A stream source's name for people ("via …").
    pub(crate) async fn provider_name(&self, provider_id: &str) -> String {
        self.extensions()
            .list()
            .await
            .into_iter()
            .flat_map(|extension| extension.providers)
            .find(|provider| provider.id == provider_id)
            .map_or_else(
                || unlisted_provider_name(provider_id),
                |provider| provider.name,
            )
    }

    /// The sources the song remembered under `keys` is known to play from, best first (M46.53):
    /// the stream `playing` it now, then the picks that played it or were picked for it. See
    /// [`known_order`].
    pub async fn known_sources(&self, keys: &[String], playing: Option<&Pick>) -> Vec<Known> {
        let mut rows = Vec::new();
        for key in keys {
            match streams::candidates(self.db(), key).await {
                Ok(found) => rows.extend(found.into_iter().map(|row| (key.clone(), row))),
                Err(error) => tracing::warn!(%error, "stream cache unreadable"),
            }
        }
        known_order(playing, rows, unix_ms())
    }

    /// A known source's stream for `using` (a download): its source asked for a new one, which a
    /// failure demotes the pick for; when that source can't answer now, or for a stream picked
    /// for the song (no source to ask), the stream as kept. YouTube videos aren't asked here: a
    /// download saves their audio files itself ([`Known::video`]).
    pub async fn known_stream(&self, known: &Known, using: StreamUse) -> Result<Pick, String> {
        let extensions = self.extensions();
        let provider_id = known.key.provider_id.as_str();
        let pick = |source, expires_at, from_cache, provider_name| Pick {
            key: known.key.clone(),
            track_key: known.track_key.clone(),
            title: known.title.clone(),
            artist: known.artist.clone(),
            provider_name,
            source,
            expires_at,
            from_cache,
            bitrate_kbps: known.bitrate_kbps,
        };
        let now = unix_ms();
        let kept = known.source.clone().filter(|source| {
            matches!(source, MediaSource::Url { .. })
                && known
                    .expires_at
                    .is_none_or(|expires| expires - EXPIRY_MARGIN_MS > now)
        });
        if !extensions.stream_provider_ids().await.contains(provider_id) {
            // A stream picked for the song, or one whose source was taken away since.
            return kept
                .map(|source| pick(source, None, true, unlisted_provider_name(provider_id)))
                .ok_or_else(|| {
                    format!(
                        "{} has no stream for it now",
                        unlisted_provider_name(provider_id)
                    )
                });
        }
        let name = self.provider_name(provider_id).await;
        match extensions
            .resolve_stream_call(provider_id, &known.key.candidate_id, using)
            .await
        {
            Ok(stream) => Ok(pick(stream.source, stream.expires_at, false, name)),
            Err(error) if passing(&error) => match kept {
                Some(source) => Ok(pick(source, known.expires_at, true, name)),
                None => Err(error.to_string()),
            },
            Err(error) => {
                let message = error.to_string();
                let _ = streams::mark_failed(
                    self.db(),
                    &known.track_key,
                    provider_id,
                    &known.key.candidate_id,
                    &known.title,
                    &message,
                    unix_ms(),
                )
                .await;
                Err(message)
            }
        }
    }

    /// The pick a YouTube video's audio file is asked for under (M46.51): the installed YouTube
    /// source's, `None` without one.
    pub async fn youtube_audio_pick(&self, video_id: &str) -> Option<CandidateKey> {
        let listed: Vec<String> = self
            .extensions()
            .stream_provider_ids()
            .await
            .into_iter()
            .collect();
        let installed = listed
            .iter()
            .any(|id| id == YOUTUBE_STREAMS || id == NUCLEAR_YOUTUBE_STREAMS);
        installed.then(|| {
            let (provider, candidate) = youtube_audio_call(&listed, video_id);
            CandidateKey {
                provider_id: provider.to_owned(),
                candidate_id: candidate,
            }
        })
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
                artist: pick.artist.as_deref(),
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

    /// A remembered pick's stream stopped working: the stream is forgotten without counting
    /// against the pick or its provider, and the pick's source is asked for a new one.
    pub async fn forget_stream(&self, pick: &Pick) {
        tracing::info!(provider = %pick.key.provider_id, candidate = %pick.key.candidate_id, "a remembered stream stopped working; asking its source again");
        if let Err(error) = streams::forget_stream(
            self.db(),
            &pick.track_key,
            &pick.key.provider_id,
            &pick.key.candidate_id,
        )
        .await
        {
            tracing::warn!(%error, "could not forget a stream");
        }
    }

    /// A song's own stream (one picked for it) stopped working: picks remembered with it go too,
    /// under every key in `track_keys`.
    pub async fn forget_source(&self, track_keys: &[String], source: &MediaSource) {
        let Ok(json) = serde_json::to_string(source) else {
            return;
        };
        if let Err(error) = streams::forget_source(self.db(), track_keys, &json).await {
            tracing::warn!(%error, "could not forget a stream");
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

/// The picks whose sources are asked for a new stream before any search, in turn: the ones that
/// played the song (or were kept for it, M46.53), haven't failed lately and that this load hasn't
/// tried, from a stream source that's on now, at a quality that still fits; newest first, those
/// that failed since they last played after the rest (a failure demotes a pick, M46.53). Picks
/// with a stream that works are played from the cache before this; ones that played in YouTube's
/// embed are found again instead (M46.51).
fn remembered<'a>(
    rows: &'a [CandidateRow],
    exclusions: &Exclusions,
    providers: &HashSet<String>,
    target: QualityTarget,
    now: i64,
) -> Vec<&'a CandidateRow> {
    let mut picks: Vec<&CandidateRow> = rows
        .iter()
        .filter(|row| row.verified_at.is_some() && !plays_in_embed(row))
        .filter(|row| {
            let key = CandidateKey {
                provider_id: row.provider_id.clone(),
                candidate_id: row.candidate_id.clone(),
            };
            providers.contains(&row.provider_id)
                && row
                    .failed_at
                    .is_none_or(|at| now - at >= FAILURE_COOLDOWN_MS)
                && !exclusions.tried.contains(&key)
                && !exclusions.unanswered.contains(&key)
                && target.fits(row.bitrate_kbps.and_then(|kbps| u32::try_from(kbps).ok()))
        })
        .collect();
    // The rows come newest played first; the stable sort keeps that order in each group.
    picks.sort_by_key(|row| row.failed_at.is_some());
    picks
}

/// A source a song is known to play from (M46.53): the stream playing it now, a stream picked for
/// it, or a pick that played it before. Downloads start from these before any search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    /// The key it's remembered under.
    pub track_key: String,
    pub key: CandidateKey,
    pub title: String,
    pub artist: Option<String>,
    /// Its stream as last known: for a stream picked for the song, the stream itself.
    pub source: Option<MediaSource>,
    /// When that stream stops working (unix ms), if it does.
    pub expires_at: Option<i64>,
    pub bitrate_kbps: Option<u32>,
}

impl Known {
    /// The YouTube video it is, its audio file or its embed (M46.51).
    pub fn video(&self) -> Option<String> {
        youtube_video(&self.key)
            .map(str::to_owned)
            .or(match &self.source {
                Some(MediaSource::YouTube { video_id }) => Some(video_id.clone()),
                _ => None,
            })
    }
}

/// The sources a song is known to play from, best first, at most [`KNOWN_TRIES`]: the stream
/// `playing` it now, then the picks remembered under its keys (`rows`) that played it or were
/// picked for it, newest first, those that failed since they last played after the rest. None
/// that failed within the day; each once, under whichever key.
fn known_order(
    playing: Option<&Pick>,
    mut rows: Vec<(String, CandidateRow)>,
    now: i64,
) -> Vec<Known> {
    let mut known: Vec<Known> = playing
        .map(|pick| Known {
            track_key: pick.track_key.clone(),
            key: pick.key.clone(),
            title: pick.title.clone(),
            artist: pick.artist.clone(),
            source: Some(pick.source.clone()),
            expires_at: pick.expires_at,
            bitrate_kbps: pick.bitrate_kbps,
        })
        .into_iter()
        .collect();
    rows.retain(|(_, row)| {
        row.verified_at.is_some()
            && row
                .failed_at
                .is_none_or(|at| now - at >= FAILURE_COOLDOWN_MS)
    });
    rows.sort_by_key(|(_, row)| {
        (
            row.failed_at.is_some(),
            std::cmp::Reverse(row.verified_at.unwrap_or_default()),
        )
    });
    for (track_key, row) in rows {
        let key = CandidateKey {
            provider_id: row.provider_id,
            candidate_id: row.candidate_id,
        };
        if known.iter().any(|kept| kept.key == key) {
            continue;
        }
        known.push(Known {
            track_key,
            key,
            title: row.title,
            artist: row.artist,
            source: row
                .source
                .as_deref()
                .and_then(|json| serde_json::from_str::<MediaSource>(json).ok()),
            expires_at: row.expires_at,
            bitrate_kbps: row.bitrate_kbps.and_then(|kbps| u32::try_from(kbps).ok()),
        });
    }
    known.truncate(KNOWN_TRIES);
    known
}

/// Whether a failed resolve is the source's trouble now (it timed out, limited requests, had
/// server trouble, the connection dropped) rather than the match's: it's asked again later
/// instead of being kept out for a day.
fn passing(error: &CallError) -> bool {
    matches!(error, CallError::Cancelled | CallError::HostStopped)
        || error.kind().is_some_and(IssueKind::is_transient)
}

/// The matches to try for a track, best first, leaving out what this load tried. Matches that
/// failed lately are kept out for a day, but when that leaves nothing, the best of them gets
/// one real try per load (M46.48): a song played on purpose is never skipped untried. A deeper
/// search tries those too, after the rest, and a few matches at most.
fn to_try(
    query: &TrackQuery,
    candidates: Vec<Candidate>,
    history: &History,
    target: QualityTarget,
    exclusions: &Exclusions,
) -> Vec<Scored> {
    let untried = |scored: &Scored| {
        let key = scored.candidate.key();
        !exclusions.tried.contains(&key)
            && !exclusions.unanswered.contains(&key)
            && scored
                .candidate
                .source
                .as_ref()
                .is_none_or(|source| !exclusions.failed_sources.contains(source))
    };
    if exclusions.deep {
        let mut again = history.clone();
        again.failed.clear();
        let (fresh, failed): (Vec<Scored>, Vec<Scored>) =
            rank_for(query, candidates, &again, target)
                .into_iter()
                .filter(untried)
                .partition(|scored| !history.failed.contains(&scored.candidate.key()));
        return fresh.into_iter().chain(failed).take(DEEP_TRIES).collect();
    }
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
    #[test]
    fn a_videos_audio_comes_from_whichever_youtube_source_is_installed() {
        let listed = |ids: &[&str]| ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            youtube_audio_call(&listed(&[YOUTUBE_STREAMS]), "abc"),
            (YOUTUBE_STREAMS, "audio:abc".to_owned())
        );
        assert_eq!(
            youtube_audio_call(&listed(&[NUCLEAR_YOUTUBE_STREAMS]), "abc"),
            (NUCLEAR_YOUTUBE_STREAMS, "abc".to_owned())
        );
        // Both: the YouTube extension's, which also knows embeds.
        assert_eq!(
            youtube_audio_call(&listed(&[NUCLEAR_YOUTUBE_STREAMS, YOUTUBE_STREAMS]), "abc").0,
            YOUTUBE_STREAMS
        );
        // Neither: the YouTube extension's call, whose error says it isn't there.
        assert_eq!(youtube_audio_call(&[], "abc").0, YOUTUBE_STREAMS);
    }

    #[test]
    fn live_youtube_asks_through_whichever_youtube_source_is_installed() {
        let listed = |ids: &[&str]| ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            youtube_extensions(&listed(&[
                NUCLEAR_YOUTUBE_STREAMS,
                "dev.raygun.youtube/videos",
                "dev.example.films/films",
            ])),
            ["dev.raygun.youtube", "nuclear.nuclear-plugin-youtube"],
            "the YouTube extension by any of its providers, first"
        );
        assert!(youtube_extensions(&listed(&["dev.example.films/films"])).is_empty());
    }

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

    /// Three sources answer at 0.3 s (another song), 0.8 s (the song) and 5 s (the song too):
    /// the search tries the 0.8 s one as it comes, without waiting for the slowest.
    #[tokio::test(start_paused = true)]
    async fn a_good_match_is_tried_as_soon_as_its_source_answers() {
        let query = TrackQuery {
            title: "Swim".into(),
            artist: Some("Artist".into()),
            album: None,
            duration_ms: None,
        };
        let (sender, mut answers) = tokio::sync::mpsc::unbounded_channel();
        for (after, found) in [
            (300, candidate("other", "Something Else Entirely")),
            (800, candidate("good", "Swim")),
            (5_000, candidate("late", "Swim")),
        ] {
            let sender = sender.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(after)).await;
                let _ = sender.send(vec![found]);
            });
        }
        drop(sender);
        let started = tokio::time::Instant::now();
        let (history, exclusions) = (History::default(), Exclusions::default());
        let mut candidates = Vec::new();
        let mut attempted = HashSet::new();
        let mut tried = Vec::new();
        while let Some(found) = answers.recv().await {
            candidates.extend(found);
            let ranked = to_try(
                &query,
                candidates.clone(),
                &history,
                QualityTarget::Any,
                &exclusions,
            );
            for scored in confident_new(ranked, &mut attempted) {
                tried.push((scored.candidate.id.clone(), started.elapsed()));
            }
        }
        assert_eq!(
            tried,
            [
                ("good".to_owned(), Duration::from_millis(800)),
                // Tried too if the first gave no stream, once it answered; never twice.
                ("late".to_owned(), Duration::from_millis(5_000)),
            ]
        );
    }

    #[test]
    fn providers_no_extension_lists_get_readable_names() {
        assert_eq!(unlisted_provider_name(YOUTUBE_STREAMS), "YouTube");
        assert_eq!(unlisted_provider_name("dev.raygun.audius/stream"), "Audius");
        assert_eq!(
            unlisted_provider_name("community.paraline.openverse/search"),
            "Openverse"
        );
        assert_eq!(unlisted_provider_name("local"), "Local");
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
            artist: None,
        };
        assert!(plays_in_embed(&row(Some(
            r#"{"kind":"youTube","videoId":"dQw4w9WgXcQ"}"#
        ))));
        assert!(!plays_in_embed(&row(Some(
            r#"{"kind":"url","url":"https://rr1---sn-x.googlevideo.com/videoplayback","headers":{}}"#
        ))));
        assert!(!plays_in_embed(&row(None)));
    }

    fn row(provider: &str, id: &str, verified_at: Option<i64>) -> CandidateRow {
        CandidateRow {
            provider_id: provider.into(),
            candidate_id: id.into(),
            title: "Song".into(),
            source: None,
            expires_at: None,
            verified_at,
            failed_at: None,
            bitrate_kbps: None,
            artist: None,
        }
    }

    #[test]
    fn the_sources_that_played_a_song_are_asked_again_before_a_search() {
        let providers: HashSet<String> = ["dev.a/stream".to_owned(), "dev.b/stream".to_owned()]
            .into_iter()
            .collect();
        let now = 10 * FAILURE_COOLDOWN_MS;
        let none = Exclusions::default();
        let ids = |found: Vec<&CandidateRow>| -> Vec<String> {
            found.iter().map(|row| row.candidate_id.clone()).collect()
        };
        // Newest played first, in turn; candidates that never played aren't remembered.
        let rows = vec![
            row("dev.a/stream", "1", Some(now - 10)),
            row("dev.b/stream", "2", Some(now - 20)),
            row("dev.b/stream", "3", None),
        ];
        assert_eq!(
            ids(remembered(
                &rows,
                &none,
                &providers,
                QualityTarget::Any,
                now
            )),
            ["1", "2"]
        );
        // Failed lately, or tried in this load: the one before it.
        let mut failed = rows.clone();
        failed[0].failed_at = Some(now - 1);
        assert_eq!(
            ids(remembered(
                &failed,
                &none,
                &providers,
                QualityTarget::Any,
                now
            )),
            ["2"]
        );
        let tried = Exclusions {
            tried: vec![CandidateKey {
                provider_id: "dev.a/stream".into(),
                candidate_id: "1".into(),
            }],
            ..Exclusions::default()
        };
        assert_eq!(
            ids(remembered(
                &rows,
                &tried,
                &providers,
                QualityTarget::Any,
                now
            )),
            ["2"]
        );
        // A failure long ago demotes the pick: it's asked after the others, not dropped.
        failed[0].failed_at = Some(now - FAILURE_COOLDOWN_MS - 1);
        assert_eq!(
            ids(remembered(
                &failed,
                &none,
                &providers,
                QualityTarget::Any,
                now
            )),
            ["2", "1"]
        );
        // Played again since, it's first again (a play clears the failure).
        assert_eq!(
            ids(remembered(
                &rows,
                &none,
                &providers,
                QualityTarget::Any,
                now
            ))[0],
            "1"
        );
        // A source turned off since, or a pick the user made (kept under a source's name), and
        // one that played in YouTube's embed: nothing to ask.
        let only_c: HashSet<String> = ["dev.c/stream".to_owned()].into_iter().collect();
        assert!(remembered(&rows, &none, &only_c, QualityTarget::Any, now).is_empty());
        let picked = vec![row("Audius", "https://a.example/1", Some(now))];
        assert!(remembered(&picked, &none, &providers, QualityTarget::Any, now).is_empty());
        let mut embed = row("dev.a/stream", "4", Some(now));
        embed.source = Some(r#"{"kind":"youTube","videoId":"abc"}"#.into());
        assert!(remembered(&[embed], &none, &providers, QualityTarget::Any, now).is_empty());
        // A stream poorer than the quality wanted now is searched for instead.
        let mut low = row("dev.a/stream", "5", Some(now));
        low.bitrate_kbps = Some(96);
        assert!(remembered(&[low], &none, &providers, QualityTarget::Best, now).is_empty());
    }

    #[test]
    fn a_download_starts_from_what_plays_the_song_then_what_played_it() {
        let now = 10 * FAILURE_COOLDOWN_MS;
        let keyed = |key: &str, row: CandidateRow| (key.to_owned(), row);
        let mut picked = row("Audius", "https://a.example/9", Some(now - 5));
        picked.source = Some(r#"{"kind":"url","url":"https://a.example/9","headers":{}}"#.into());
        picked.artist = Some("Artist".into());
        let mut demoted = row("dev.b/stream", "2", Some(now - 1));
        demoted.failed_at = Some(now - FAILURE_COOLDOWN_MS - 1);
        let mut failed_lately = row("dev.c/stream", "3", Some(now - 1));
        failed_lately.failed_at = Some(now - 10);
        let rows = vec![
            // Under the song's own key and the key its names make: once each.
            keyed("artist|song", row("dev.a/stream", "1", Some(now - 30))),
            keyed("artist|song live", row("dev.a/stream", "1", Some(now - 30))),
            keyed("artist|song", picked),
            keyed("artist|song", demoted),
            keyed("artist|song", failed_lately),
            // Never played: not a known source.
            keyed("artist|song", row("dev.d/stream", "4", None)),
        ];
        let keys = |known: &[Known]| -> Vec<String> {
            known
                .iter()
                .map(|known| known.key.candidate_id.clone())
                .collect()
        };
        let known = known_order(None, rows.clone(), now);
        assert_eq!(keys(&known), ["https://a.example/9", "1", "2"]);
        assert_eq!(known[0].artist.as_deref(), Some("Artist"));
        assert!(matches!(known[0].source, Some(MediaSource::Url { .. })));

        // The stream playing it now comes first, and isn't asked for twice.
        let playing = Pick {
            key: CandidateKey {
                provider_id: "dev.a/stream".into(),
                candidate_id: "1".into(),
            },
            track_key: "artist|song".into(),
            title: "Song".into(),
            artist: None,
            provider_name: "A".into(),
            source: MediaSource::Url {
                url: "https://a.example/1.mp3".into(),
                headers: Default::default(),
            },
            expires_at: Some(now + 60_000),
            from_cache: false,
            bitrate_kbps: None,
        };
        let known = known_order(Some(&playing), rows, now);
        assert_eq!(keys(&known), ["1", "https://a.example/9", "2"]);
        assert_eq!(known[0].expires_at, Some(now + 60_000));

        // A YouTube video is saved as its audio file.
        let mut video = known[0].clone();
        video.key = CandidateKey {
            provider_id: YOUTUBE_STREAMS.into(),
            candidate_id: "audio:dQw4w9WgXcQ".into(),
        };
        assert_eq!(video.video().as_deref(), Some("dQw4w9WgXcQ"));
        video.key.provider_id = "picked".into();
        video.source = Some(MediaSource::YouTube {
            video_id: "aqz-KE-bpKQ".into(),
        });
        assert_eq!(video.video().as_deref(), Some("aqz-KE-bpKQ"));
        assert_eq!(known[1].video(), None);
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

    #[test]
    fn a_deeper_search_tries_every_match_those_that_failed_lately_last() {
        let query = TrackQuery {
            title: "Song".into(),
            artist: Some("Artist".into()),
            album: None,
            duration_ms: None,
        };
        let ids = |scored: &[Scored]| -> Vec<String> {
            scored
                .iter()
                .map(|scored| scored.candidate.id.clone())
                .collect()
        };
        let found = vec![
            candidate("a", "Song"),
            candidate("b", "Song"),
            candidate("c", "Song"),
        ];
        let mut history = History::default();
        history.failed.insert(found[0].key());
        let deep = Exclusions {
            deep: true,
            ..Exclusions::default()
        };
        assert_eq!(
            ids(&to_try(
                &query,
                found.clone(),
                &history,
                QualityTarget::Any,
                &deep
            )),
            ["b", "c", "a"]
        );
        // What this load tried waits, and so do matches whose source couldn't answer just now.
        let busy = Exclusions {
            tried: vec![found[1].key()],
            unanswered: vec![found[2].key()],
            ..deep.clone()
        };
        assert_eq!(
            ids(&to_try(&query, found, &history, QualityTarget::Any, &busy)),
            ["a"]
        );
        // A few at most.
        let many: Vec<Candidate> = (0..20).map(|n| candidate(&n.to_string(), "Song")).collect();
        assert_eq!(
            to_try(&query, many, &History::default(), QualityTarget::Any, &deep).len(),
            DEEP_TRIES
        );
    }

    #[test]
    fn a_source_busy_now_isnt_held_against_its_match() {
        let failed = |kind| CallError::Failed(raygun_net::Issue::new(kind, "Example", "HTTP"));
        assert!(passing(&failed(IssueKind::RateLimited)));
        assert!(passing(&failed(IssueKind::ServerError)));
        assert!(passing(&CallError::Timeout {
            name: "Example".into(),
            after: Duration::from_secs(12),
        }));
        assert!(!passing(&failed(IssueKind::NotFound)));
        assert!(!passing(&CallError::Said {
            name: "Example".into(),
            message: "This song isn't on Example".into(),
        }));
    }
}
