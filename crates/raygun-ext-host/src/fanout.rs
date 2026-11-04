//! Asking many sources at once. There may be dozens of them and many fail, so they're asked a
//! few at a time, the healthy ones first, each within its time, and their answers come as they
//! arrive. A source that keeps failing goes last and sits out a while (its last failure is the
//! answer meanwhile). Identical asks share one call, and an answer stays for a moment, so asking
//! again (a list refreshing to pick up late answers) costs nothing.

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use futures_util::future::{BoxFuture, FutureExt, Shared};
use futures_util::stream::{self, Stream, StreamExt};
use raygun_model::ProviderInfo;
use tokio::sync::oneshot;
// Tokio's clock, so tests can move it.
use tokio::time::Instant;

use crate::CallError;
use raygun_net::IssueKind;

/// Failures in a row before a source sits out.
const REST_AFTER: u32 = 3;
/// A browser check doesn't go away by asking again: the source sits out after this many.
const REST_AFTER_CHECK: u32 = 2;
/// The first rest; each failure after it doubles the rest, up to `LONGEST_REST`.
const FIRST_REST: Duration = Duration::from_secs(60);
const LONGEST_REST: Duration = Duration::from_secs(15 * 60);
/// A source this slow (smoothed) is asked after the quick ones.
const SLOW: Duration = Duration::from_secs(4);
/// Longest a call holds its place among the calls running at once: a source slower than this
/// goes on without one, so sources that hang can't hold up the rest for their whole timeout.
pub const HOLD: Duration = Duration::from_secs(2);
/// How long an answer is kept for asking again.
pub const MEMO_TTL: Duration = Duration::from_secs(90);
/// How long a failure is kept: long enough that asking again soon (a list picking up late
/// answers) gets it instead of asking the source anew, short enough for a retry to ask.
const MEMO_FAILURE_TTL: Duration = Duration::from_secs(20);
/// Longest a call nobody waits for is kept going for someone asking again.
const MEMO_IN_FLIGHT: Duration = Duration::from_secs(120);
/// Most answers kept.
const MEMO_ENTRIES: usize = 256;

/// Where a source stands, from what its calls did lately: the good first, then the slow, then
/// the failing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Standing {
    Good,
    Slow,
    Failing,
}

#[derive(Debug, Clone, Default)]
struct Record {
    /// Failures in a row (timeouts included).
    failures: u32,
    /// Until when the source isn't asked, after failing again and again.
    resting_until: Option<Instant>,
    last_error: Option<CallError>,
    /// Smoothed time to answer, in ms.
    latency_ms: Option<f64>,
}

/// What every source's calls did lately (this run only), shared by everything that asks them.
#[derive(Debug, Default)]
pub struct Health {
    records: Mutex<HashMap<String, Record>>,
}

impl Health {
    fn records(&self) -> std::sync::MutexGuard<'_, HashMap<String, Record>> {
        self.records.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn standing(&self, provider: &str) -> Standing {
        match self.records().get(provider) {
            Some(record) if record.failures > 0 => Standing::Failing,
            Some(record)
                if record
                    .latency_ms
                    .is_some_and(|ms| ms > SLOW.as_millis() as f64) =>
            {
                Standing::Slow
            }
            _ => Standing::Good,
        }
    }

    /// `providers` with the good first, then the slow, then the failing, each group in its order.
    pub fn order(&self, mut providers: Vec<ProviderInfo>) -> Vec<ProviderInfo> {
        providers.sort_by_cached_key(|provider| self.standing(&provider.id));
        providers
    }

    /// While a source sits out: its last failure, to answer with instead of asking it.
    pub fn resting(&self, provider: &str) -> Option<CallError> {
        let records = self.records();
        let record = records.get(provider)?;
        let until = record.resting_until?;
        (Instant::now() < until).then(|| CallError::Resting {
            last: Box::new(record.last_error.clone().unwrap_or(CallError::Cancelled)),
            failures: record.failures,
        })
    }

    /// Notes how a call went. Calls that didn't reach the source (missing calls, stopped ones,
    /// skipped ones) say nothing about it.
    pub fn record(&self, provider: &str, outcome: Result<(), &CallError>, elapsed: Duration) {
        let counts = match outcome {
            Ok(()) => true,
            Err(error) => matches!(
                error,
                CallError::Timeout { .. } | CallError::Failed(_) | CallError::Said { .. }
            ),
        };
        if !counts {
            return;
        }
        let mut records = self.records();
        let record = records.entry(provider.to_owned()).or_default();
        let ms = elapsed.as_secs_f64() * 1000.0;
        record.latency_ms = Some(record.latency_ms.map_or(ms, |old| old * 0.7 + ms * 0.3));
        match outcome {
            Ok(()) => {
                record.failures = 0;
                record.resting_until = None;
                record.last_error = None;
            }
            Err(error) => {
                record.failures += 1;
                record.last_error = Some(error.clone());
                let threshold = match rest_kind(error) {
                    Some(IssueKind::BotCheck) => Some(REST_AFTER_CHECK),
                    Some(_) => Some(REST_AFTER),
                    None => None,
                };
                if let Some(threshold) = threshold
                    && record.failures >= threshold
                {
                    let doublings = (record.failures - threshold).min(8);
                    let rest = (FIRST_REST * 2u32.pow(doublings)).min(LONGEST_REST);
                    record.resting_until = Some(Instant::now() + rest);
                }
            }
        }
    }

    /// Forgets a source's record (it was changed or reinstalled).
    pub fn forget(&self, provider: &str) {
        self.records().remove(provider);
    }
}

/// The kind of failure that makes a source sit out: the site's (a timeout, a refusal, a browser
/// check, limits, server trouble). The extension's own mistakes don't (they may only concern
/// some asks), and neither does this device being offline (every source would sit out after
/// the connection came back).
fn rest_kind(error: &CallError) -> Option<IssueKind> {
    let kind = match error {
        CallError::Timeout { .. } => IssueKind::Timeout,
        CallError::Failed(issue) => issue.kind,
        _ => return None,
    };
    matches!(
        kind,
        IssueKind::Timeout
            | IssueKind::Refused
            | IssueKind::RateLimited
            | IssueKind::ServerError
            | IssueKind::BotCheck
    )
    .then_some(kind)
}

/// How a fan-out runs.
#[derive(Debug, Clone, Copy)]
pub struct Plan {
    /// Calls running at once.
    pub concurrency: usize,
    /// Longest one source may take.
    pub timeout: Duration,
    /// Answer for sources sitting out (failing again and again) with their last failure.
    pub skip_resting: bool,
}

/// One source's answer to a fan-out.
#[derive(Debug)]
pub struct Asked<T> {
    /// The source's place in the fan-out's order (good sources first).
    pub order: usize,
    pub provider: ProviderInfo,
    pub answer: Result<T, CallError>,
}

/// Asks every one of `providers` with `ask`, `plan.concurrency` at a time, the good first and
/// the failing last; sources sitting out answer at once. A call slower than [`HOLD`] lets the
/// next one start. Answers come as they arrive (each in `plan.timeout`), and every call is
/// noted in `health`. Dropping the stream stops the calls still running.
pub fn fan_out<'a, T, F, Fut>(
    health: &'a Health,
    providers: Vec<ProviderInfo>,
    plan: Plan,
    ask: F,
) -> impl Stream<Item = Asked<T>> + 'a
where
    T: 'a,
    F: Fn(ProviderInfo) -> Fut + 'a,
    Fut: Future<Output = Result<T, CallError>> + 'a,
{
    let mut resting = Vec::new();
    let mut asking = Vec::new();
    for (order, provider) in health.order(providers).into_iter().enumerate() {
        match plan
            .skip_resting
            .then(|| health.resting(&provider.id))
            .flatten()
        {
            Some(error) => resting.push(Asked {
                order,
                provider,
                answer: Err(error),
            }),
            None => asking.push((order, provider)),
        }
    }
    let count = asking.len().max(1);
    // Places are handed out first come, first served: in the order above.
    let places = Arc::new(tokio::sync::Semaphore::new(plan.concurrency.max(1)));
    let ask = Arc::new(ask);
    let asks = stream::iter(asking)
        .map(move |(order, provider)| {
            let (places, ask) = (places.clone(), ask.clone());
            async move {
                let mut place = places.acquire_owned().await.ok();
                let started = Instant::now();
                let mut call = pin!(tokio::time::timeout(plan.timeout, ask(provider.clone())));
                let answer = tokio::select! {
                    answer = &mut call => answer,
                    () = tokio::time::sleep(HOLD) => {
                        place.take();
                        call.await
                    }
                };
                drop(place);
                let answer = answer.unwrap_or_else(|_| {
                    Err(CallError::Timeout {
                        name: provider.name.clone(),
                        after: plan.timeout,
                    })
                });
                health.record(&provider.id, answer.as_ref().map(|_| ()), started.elapsed());
                Asked {
                    order,
                    provider,
                    answer,
                }
            }
        })
        .buffer_unordered(count);
    stream::iter(resting).chain(asks)
}

/// Every answer of a fan-out, in its order.
pub async fn collect<T>(answers: impl Stream<Item = Asked<T>>) -> Vec<Asked<T>> {
    let mut all: Vec<Asked<T>> = answers.collect().await;
    all.sort_by_key(|asked| asked.order);
    all
}

/// How long to wait for a fan-out's answers when someone waits on them.
#[derive(Debug, Clone, Copy)]
pub struct Patience {
    /// Longest to wait for all of them.
    pub total: Duration,
    /// Once a useful answer is in, how much longer to wait for the rest.
    pub settle: Duration,
}

/// The answers of a fan-out that came in time, in its order, and whether every source answered.
/// Waits for all of them, but not past `patience`: never for the slowest when others answered
/// well before.
pub async fn gather<T>(
    answers: impl Stream<Item = Asked<T>>,
    patience: Patience,
    useful: impl Fn(&Result<T, CallError>) -> bool,
) -> (Vec<Asked<T>>, bool) {
    let mut answers = pin!(answers);
    let mut got = Vec::new();
    let deadline = Instant::now() + patience.total;
    let mut settle_by: Option<Instant> = None;
    let complete = loop {
        let until = settle_by.map_or(deadline, |settle| settle.min(deadline));
        match tokio::time::timeout_at(until, answers.next()).await {
            Ok(Some(asked)) => {
                if settle_by.is_none() && useful(&asked.answer) {
                    settle_by = Some(Instant::now() + patience.settle);
                }
                got.push(asked);
            }
            Ok(None) => break true,
            Err(_) => break false,
        }
    };
    got.sort_by_key(|asked| asked.order);
    (got, complete)
}

type SharedAnswer = Shared<BoxFuture<'static, Result<serde_json::Value, CallError>>>;

/// Calls shared by identical asks, and their answers for a moment after (failures for less).
/// A call stays going while it's kept, even when its first asker stopped waiting, so asking
/// again picks up its answer.
#[derive(Default)]
pub struct Memo {
    entries: Mutex<HashMap<String, (Instant, SharedAnswer)>>,
}

impl Memo {
    /// The answer for `key`: kept, joined while running, or from a call `start` makes now.
    pub fn answer(
        &self,
        key: String,
        start: impl FnOnce() -> BoxFuture<'static, Result<serde_json::Value, CallError>>,
    ) -> SharedAnswer {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let fresh = |at: &Instant, answer: &SharedAnswer| match answer.peek() {
            Some(Ok(_)) => at.elapsed() < MEMO_TTL,
            Some(Err(_)) => at.elapsed() < MEMO_FAILURE_TTL,
            None => at.elapsed() < MEMO_IN_FLIGHT,
        };
        if let Some((at, answer)) = entries.get(&key)
            && fresh(at, answer)
        {
            return answer.clone();
        }
        entries.retain(|_, (at, answer)| fresh(at, answer));
        if entries.len() >= MEMO_ENTRIES
            && let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(key, _)| key.clone())
        {
            entries.remove(&oldest);
        }
        let answer = start().shared();
        entries.insert(key, (Instant::now(), answer.clone()));
        answer
    }

    /// Forgets every kept answer (extensions were reloaded).
    pub fn clear(&self) {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

/// Turns for calls, a few at a time. Those waiting go by standing (good sources first), then in
/// the order they came, so a failing source never holds up a good one.
#[derive(Debug)]
pub struct Turns {
    slots: usize,
    state: Mutex<TurnState>,
}

#[derive(Debug, Default)]
struct TurnState {
    running: usize,
    next: u64,
    waiting: BTreeMap<(Standing, u64), oneshot::Sender<()>>,
}

/// A call's turn; the next one waiting goes when it's dropped.
#[derive(Debug)]
pub struct Turn(Arc<Turns>);

impl Turns {
    pub fn new(slots: usize) -> Arc<Self> {
        Arc::new(Self {
            slots: slots.max(1),
            state: Mutex::new(TurnState::default()),
        })
    }

    fn state(&self) -> std::sync::MutexGuard<'_, TurnState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Waits for a turn; `standing` says how soon.
    pub async fn wait(self: &Arc<Self>, standing: Standing) -> Turn {
        let (key, receiver) = {
            let mut state = self.state();
            if state.running < self.slots {
                state.running += 1;
                return Turn(self.clone());
            }
            let (sender, receiver) = oneshot::channel();
            let key = (standing, state.next);
            state.next += 1;
            state.waiting.insert(key, sender);
            (key, receiver)
        };
        let mut waiting = Waiting {
            turns: self.clone(),
            key,
            receiver: Some(receiver),
        };
        if let Some(receiver) = waiting.receiver.as_mut() {
            // The sender only goes away by handing over a turn.
            let _ = receiver.await;
        }
        waiting.receiver = None;
        Turn(self.clone())
    }

    /// Gives a finished turn to the best one waiting, or frees it.
    fn pass(&self) {
        let mut state = self.state();
        while let Some((_, sender)) = state.waiting.pop_first() {
            if sender.send(()).is_ok() {
                return;
            }
        }
        state.running = state.running.saturating_sub(1);
    }

    /// Calls running now, and calls waiting.
    pub fn load(&self) -> (usize, usize) {
        let state = self.state();
        (state.running, state.waiting.len())
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        self.0.pass();
    }
}

/// A wait for a turn; given up (the call was stopped), it leaves the line, or passes the turn on
/// if one was handed to it meanwhile.
struct Waiting {
    turns: Arc<Turns>,
    key: (Standing, u64),
    receiver: Option<oneshot::Receiver<()>>,
}

impl Drop for Waiting {
    fn drop(&mut self) {
        let Some(mut receiver) = self.receiver.take() else {
            return;
        };
        let left = self.turns.state().waiting.remove(&self.key).is_some();
        if !left && receiver.try_recv().is_ok() {
            self.turns.pass();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use raygun_net::Issue;

    fn provider(index: usize) -> ProviderInfo {
        ProviderInfo {
            id: format!("dev.test.p{index}/p"),
            extension_id: format!("dev.test.p{index}"),
            kind: "music.metadata".into(),
            name: format!("P{index}"),
            search_pill: None,
        }
    }

    fn failure(kind: IssueKind) -> CallError {
        CallError::Failed(Issue::new(kind, "P", "x"))
    }

    /// How a fake source behaves.
    #[derive(Debug, Clone, Copy)]
    enum Behaviour {
        Fast,
        Slow,
        Failing,
        Hanging,
    }

    fn behaviour(index: usize) -> Behaviour {
        match index % 4 {
            0 => Behaviour::Fast,
            1 => Behaviour::Slow,
            2 => Behaviour::Failing,
            _ => Behaviour::Hanging,
        }
    }

    async fn answer(index: usize) -> Result<usize, CallError> {
        match behaviour(index) {
            Behaviour::Fast => {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok(index)
            }
            Behaviour::Slow => {
                tokio::time::sleep(Duration::from_secs(3)).await;
                Ok(index)
            }
            Behaviour::Failing => {
                tokio::time::sleep(Duration::from_millis(20)).await;
                Err(failure(IssueKind::ServerError))
            }
            Behaviour::Hanging => std::future::pending().await,
        }
    }

    fn index_of(provider: &ProviderInfo) -> usize {
        provider.extension_id["dev.test.p".len()..].parse().unwrap()
    }

    fn plan() -> Plan {
        Plan {
            concurrency: 8,
            timeout: Duration::from_secs(10),
            skip_resting: true,
        }
    }

    /// Counts a call as running until it ends or is dropped.
    struct Counted(Arc<std::sync::atomic::AtomicUsize>);

    impl Drop for Counted {
        fn drop(&mut self) {
            self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn forty_sources_answer_within_one_timeout_a_few_at_a_time() {
        let health = Health::default();
        let providers: Vec<_> = (0..40).map(provider).collect();
        let running = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        // How many run at once before the slow ones give up their places.
        let sampler = {
            let running = running.clone();
            tokio::spawn(async move {
                tokio::time::sleep(HOLD / 2).await;
                running.load(std::sync::atomic::Ordering::SeqCst)
            })
        };
        let started = tokio::time::Instant::now();
        let answers = collect(fan_out(&health, providers, plan(), |provider| {
            let running = running.clone();
            async move {
                running.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let _counted = Counted(running);
                answer(index_of(&provider)).await
            }
        }))
        .await;
        assert_eq!(answers.len(), 40);
        assert!(sampler.await.unwrap() <= 8);
        let timeouts = answers
            .iter()
            .filter(|asked| matches!(asked.answer, Err(CallError::Timeout { .. })))
            .count();
        let answered = answers.iter().filter(|asked| asked.answer.is_ok()).count();
        assert_eq!((answered, timeouts), (20, 10));
        // Ten hanging sources, eight places, slow calls giving theirs up: about one timeout.
        let elapsed = started.elapsed();
        assert!(elapsed <= plan().timeout + HOLD * 3, "{elapsed:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn waiting_settles_soon_after_good_answers() {
        let health = Health::default();
        let providers: Vec<_> = (0..32).map(provider).collect();
        let started = tokio::time::Instant::now();
        let (answers, complete) = gather(
            fan_out(&health, providers, plan(), |provider| {
                answer(index_of(&provider))
            }),
            Patience {
                total: Duration::from_secs(12),
                settle: Duration::from_millis(1500),
            },
            Result::is_ok,
        )
        .await;
        assert!(!complete);
        // The fast ones and the failing ones are in; the slow and the hanging aren't awaited.
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        assert!(answers.iter().any(|asked| asked.answer.is_ok()));
        assert!(
            answers
                .iter()
                .all(|asked| !matches!(behaviour(index_of(&asked.provider)), Behaviour::Hanging))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn failing_sources_go_last_then_sit_out() {
        let health = Health::default();
        let providers: Vec<_> = (0..36).map(provider).collect();
        for _ in 0..REST_AFTER {
            collect(fan_out(&health, providers.clone(), plan(), |provider| {
                answer(index_of(&provider))
            }))
            .await;
        }
        let order: Vec<Standing> = health
            .order(providers.clone())
            .iter()
            .map(|provider| health.standing(&provider.id))
            .collect();
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(order, sorted, "good, then slow, then failing");
        assert_eq!(health.standing(&provider(0).id), Standing::Good);
        assert_eq!(health.standing(&provider(2).id), Standing::Failing);
        assert_eq!(health.standing(&provider(3).id), Standing::Failing);

        // Now the failing and hanging ones sit out: they answer at once with their last issue.
        let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let started = tokio::time::Instant::now();
        let answers = collect(fan_out(&health, providers.clone(), plan(), |provider| {
            asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            answer(index_of(&provider))
        }))
        .await;
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 18);
        // No timeouts to wait out: the slow ones' three seconds, twice at most.
        assert!(
            started.elapsed() < Duration::from_secs(7),
            "{:?}",
            started.elapsed()
        );
        let resting = answers
            .iter()
            .filter(|asked| matches!(&asked.answer, Err(CallError::Resting { .. })))
            .count();
        assert_eq!(resting, 18);
        let timeout = answers
            .iter()
            .find(|asked| index_of(&asked.provider) == 3)
            .unwrap();
        assert!(
            timeout
                .answer
                .as_ref()
                .unwrap_err()
                .to_string()
                .starts_with("P3 didn't answer in time"),
            "{:?}",
            timeout.answer
        );

        // After the rest, they're asked again; one that works is back.
        tokio::time::advance(FIRST_REST + Duration::from_secs(1)).await;
        assert!(health.resting(&provider(2).id).is_none());
        health.record(&provider(2).id, Ok(()), Duration::from_millis(10));
        assert_eq!(health.standing(&provider(2).id), Standing::Good);
    }

    #[tokio::test(start_paused = true)]
    async fn extension_mistakes_and_missing_calls_never_rest_a_source() {
        let health = Health::default();
        let id = provider(1).id;
        for _ in 0..5 {
            health.record(&id, Err(&failure(IssueKind::SiteChanged)), Duration::ZERO);
        }
        assert_eq!(health.standing(&id), Standing::Failing);
        assert!(health.resting(&id).is_none());
        let missing = CallError::MissingMethod {
            name: "P".into(),
            provider: id.clone(),
            method: "x".into(),
        };
        let other = provider(2).id;
        for _ in 0..5 {
            health.record(&other, Err(&missing), Duration::ZERO);
            health.record(&other, Err(&CallError::Cancelled), Duration::ZERO);
        }
        assert_eq!(health.standing(&other), Standing::Good);
        // A browser check rests a source sooner.
        let checked = provider(3).id;
        for _ in 0..REST_AFTER_CHECK {
            health.record(&checked, Err(&failure(IssueKind::BotCheck)), Duration::ZERO);
        }
        assert!(health.resting(&checked).is_some());
        // Being offline rests nobody: every source would sit out once the connection is back.
        let offline = provider(4).id;
        for _ in 0..5 {
            health.record(&offline, Err(&failure(IssueKind::Dns)), Duration::ZERO);
            health.record(&offline, Err(&failure(IssueKind::Offline)), Duration::ZERO);
        }
        assert!(health.resting(&offline).is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn identical_asks_share_one_call_and_a_late_answer_waits() {
        let memo = Memo::default();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let start = || {
            let calls = calls.clone();
            move || {
                async move {
                    calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    Ok(serde_json::json!(1))
                }
                .boxed()
            }
        };
        // The first asker stops waiting; the call goes on for the next one.
        let first = memo.answer("k".into(), start());
        assert!(
            tokio::time::timeout(Duration::from_secs(1), first)
                .await
                .is_err()
        );
        let second = memo.answer("k".into(), start());
        assert_eq!(second.await.unwrap(), serde_json::json!(1));
        let third = memo.answer("k".into(), start());
        assert_eq!(third.await.unwrap(), serde_json::json!(1));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        // Kept answers expire; failures sooner, so asking again soon gets them, a retry later
        // asks anew.
        tokio::time::advance(MEMO_TTL).await;
        let _ = memo.answer("k".into(), start()).await;
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        let failing = || || async { Err(CallError::Cancelled) }.boxed();
        let _ = memo.answer("f".into(), failing()).await;
        let working = || {
            let calls = calls.clone();
            move || {
                async move {
                    calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(serde_json::json!(2))
                }
                .boxed()
            }
        };
        assert_eq!(
            memo.answer("f".into(), working()).await,
            Err(CallError::Cancelled)
        );
        tokio::time::advance(MEMO_FAILURE_TTL).await;
        assert_eq!(
            memo.answer("f".into(), working()).await.unwrap(),
            serde_json::json!(2)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn turns_go_to_good_sources_first_and_survive_stopped_waits() {
        let turns = Turns::new(2);
        let first = turns.wait(Standing::Good).await;
        let second = turns.wait(Standing::Good).await;
        assert_eq!(turns.load(), (2, 0));
        let order = Arc::new(Mutex::new(Vec::new()));
        let waiter = |standing: Standing, label: &'static str| {
            let (turns, order) = (turns.clone(), order.clone());
            tokio::spawn(async move {
                let _turn = turns.wait(standing).await;
                order.lock().unwrap().push(label);
                tokio::time::sleep(Duration::from_millis(10)).await;
            })
        };
        let failing = waiter(Standing::Failing, "failing");
        tokio::task::yield_now().await;
        let good = waiter(Standing::Good, "good");
        tokio::task::yield_now().await;
        // A wait that's stopped leaves the line.
        let stopped = {
            let turns = turns.clone();
            tokio::spawn(async move {
                let _turn = turns.wait(Standing::Good).await;
                unreachable!("stopped before its turn");
            })
        };
        tokio::task::yield_now().await;
        stopped.abort();
        let _ = stopped.await;
        assert_eq!(turns.load(), (2, 2));
        // One turn frees up: the good source gets it though it came second.
        drop(first);
        good.await.unwrap();
        drop(second);
        failing.await.unwrap();
        assert_eq!(*order.lock().unwrap(), ["good", "failing"]);
        assert_eq!(turns.load(), (0, 0));
    }
}
